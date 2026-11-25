use super::config::HostParams;
use super::file::Retry;
use super::prompt::Prompter;
use super::session::{connect, Conn};
use crate::blockcache::{BlockReader, RangeRead, BLOCK_SIZE, MAX_BLOCKS};
use crate::source::{DirEntry, ReadSeek, Source};
use russh_sftp::client::fs::File;
use std::io::{self, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::runtime::Handle;

fn io_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

struct Shared {
    params: HostParams,
    home: PathBuf,
    prompter: Arc<dyn Prompter>,
    rt: Handle,
    conn: Mutex<Option<Arc<Conn>>>,
}

impl Shared {
    /// The lock is held while connecting so parallel readers share one login (and one set of prompts).
    fn conn(&self) -> io::Result<Arc<Conn>> {
        let mut slot = self.conn.lock().unwrap();
        if let Some(c) = slot.as_ref() {
            return Ok(c.clone());
        }
        let c = Arc::new(
            self.rt
                .block_on(connect(&self.params, &self.home, self.prompter.clone()))
                .map_err(|e| io::Error::new(io::ErrorKind::NotConnected, e))?,
        );
        *slot = Some(c.clone());
        Ok(c)
    }

    /// Replaces `stale` with a fresh connection, unless another reader already did.
    fn reconnect(&self, stale: &Arc<Conn>) -> io::Result<Arc<Conn>> {
        {
            let mut slot = self.conn.lock().unwrap();
            if slot.as_ref().is_some_and(|c| Arc::ptr_eq(c, stale)) {
                slot.take();
            }
        }
        self.conn()
    }
}

/// Library rooted at an absolute folder on an SSH host. Calls block; use from blocking threads.
pub struct SftpSource {
    shared: Arc<Shared>,
    root: String,
}

impl SftpSource {
    pub fn new(params: HostParams, home: PathBuf, root: String, prompter: Arc<dyn Prompter>, rt: Handle) -> Self {
        let shared = Shared { params, home, prompter, rt, conn: Mutex::new(None) };
        Self { shared: Arc::new(shared), root: root.trim_end_matches('/').to_string() }
    }

    /// Same connection, different library root.
    pub fn with_root(&self, root: &str) -> SftpSource {
        SftpSource { shared: self.shared.clone(), root: root.trim_end_matches('/').to_string() }
    }

    /// Connects now (so authentication prompts happen up front).
    pub fn ensure_connected(&self) -> io::Result<()> {
        self.shared.conn().map(|_| ())
    }

    pub fn path(&self, rel: &str) -> String {
        match (self.root.is_empty(), rel.is_empty()) {
            (_, true) if self.root.is_empty() => "/".into(),
            (_, true) => self.root.clone(),
            (true, false) => format!("/{rel}"),
            (false, false) => format!("{}/{rel}", self.root),
        }
    }

    /// Absolute form of a remote path ("." gives the login folder).
    pub fn canonicalize(&self, path: &str) -> io::Result<String> {
        let c = self.shared.conn()?;
        self.shared.rt.block_on(c.sftp.canonicalize(path)).map_err(io_err)
    }

    fn list_once(&self, conn: &Conn, path: &str) -> io::Result<Vec<DirEntry>> {
        let entries = self.shared.rt.block_on(conn.sftp.read_dir(path)).map_err(io_err)?;
        Ok(entries
            .map(|e| {
                let m = e.metadata();
                DirEntry {
                    name: e.file_name(),
                    is_dir: m.is_dir(),
                    size: m.len(),
                    mtime: m.mtime.unwrap_or(0) as i64,
                }
            })
            .filter(|e| e.name != "." && e.name != "..")
            .collect())
    }
}

struct SftpRange {
    file: File,
    len: u64,
    rt: Handle,
}

impl SftpRange {
    fn open(shared: &Shared, conn: &Conn, path: &str) -> io::Result<SftpRange> {
        let (file, len) = shared.rt.block_on(async {
            let f = conn.sftp.open(path).await.map_err(io_err)?;
            let len = f.metadata().await.map_err(io_err)?.len();
            Ok::<_, io::Error>((f, len))
        })?;
        Ok(SftpRange { file, len, rt: shared.rt.clone() })
    }
}

impl RangeRead for SftpRange {
    fn len(&self) -> u64 {
        self.len
    }
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let file = &mut self.file;
        self.rt.block_on(async {
            file.seek(SeekFrom::Start(offset)).await?;
            file.read(buf).await
        })
    }
}

impl Source for SftpSource {
    fn list_dir(&self, rel: &str) -> io::Result<Vec<DirEntry>> {
        let path = self.path(rel);
        let conn = self.shared.conn()?;
        match self.list_once(&conn, &path) {
            Ok(v) => Ok(v),
            Err(_) => self.list_once(&*self.shared.reconnect(&conn)?, &path),
        }
    }

    fn open(&self, rel: &str) -> io::Result<Box<dyn ReadSeek>> {
        let path = self.path(rel);
        let shared = self.shared.clone();
        let conn = shared.conn()?;
        let first = match SftpRange::open(&shared, &conn, &path) {
            Ok(r) => r,
            Err(_) => SftpRange::open(&shared, &*shared.reconnect(&conn)?, &path)?,
        };
        let reopen_shared = shared.clone();
        let reopen = move || {
            let current = reopen_shared.conn()?;
            let c = reopen_shared.reconnect(&current)?;
            SftpRange::open(&reopen_shared, &c, &path)
        };
        Ok(Box::new(BlockReader::new(Retry::new(first, reopen), BLOCK_SIZE, MAX_BLOCKS)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::prompt::{BoxFut, SecretKind};
    use super::*;
    use crate::archive::{read_entry, ZipIndex};

    struct Deny;
    impl Prompter for Deny {
        fn confirm_host_key(&self, _: String, _: String) -> BoxFut<bool> {
            Box::pin(async { false })
        }
        fn ask_secret(&self, _: SecretKind, _: String) -> BoxFut<Option<(String, bool)>> {
            Box::pin(async { None })
        }
    }

    /// COMICS_SFTP_HOST=<~/.ssh/config alias>: connect with agent/keys only and list the login folder.
    #[test]
    #[ignore]
    fn sftp_connects() {
        let Ok(host) = std::env::var("COMICS_SFTP_HOST") else { return };
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let cfg = std::fs::read_to_string(home.join(".ssh/config")).unwrap_or_default();
        let params = super::super::config::resolve(&host, &cfg, &home, &std::env::var("USER").unwrap());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let src = SftpSource::new(params, home, String::new(), Arc::new(Deny), rt.handle().clone());
        let t = std::time::Instant::now();
        let login = src.canonicalize(".").unwrap();
        let n = src.list_dir(login.trim_start_matches('/')).unwrap().len();
        eprintln!("{host}: login {login}, {n} entries, {:?}", t.elapsed());
    }

    /// COMICS_SFTP_HOST=<~/.ssh/config alias> COMICS_SFTP_ZIP=<absolute remote zip path>
    #[test]
    #[ignore]
    fn sftp_reads_partial() {
        let (Ok(host), Ok(zip)) = (std::env::var("COMICS_SFTP_HOST"), std::env::var("COMICS_SFTP_ZIP")) else { return };
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let cfg = std::fs::read_to_string(home.join(".ssh/config")).unwrap_or_default();
        let params = super::super::config::resolve(&host, &cfg, &home, &std::env::var("USER").unwrap());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (dir, file) = zip.rsplit_once('/').unwrap();
        let src = SftpSource::new(params, home, dir.to_string(), Arc::new(Deny), rt.handle().clone());
        let t = std::time::Instant::now();
        let listed = src.list_dir("").unwrap();
        assert!(listed.iter().any(|e| e.name == file));
        let size = listed.iter().find(|e| e.name == file).unwrap().size;
        let shared = src.shared.clone();
        let conn = shared.conn().unwrap();
        let range = SftpRange::open(&shared, &conn, &src.path(file)).unwrap();
        let mut zipr = zip::ZipArchive::new(BlockReader::new(range, BLOCK_SIZE, MAX_BLOCKS)).unwrap();
        let first = ZipIndex::from_archive(&mut zipr).unwrap().volumes.first().map_or(0, |v| v.pages[0].entry_index);
        let page = read_entry(&mut zipr, first).unwrap();
        let fetched = zipr.into_inner().bytes_fetched();
        eprintln!("size {size}, fetched {fetched}, page {} bytes, {:?}", page.len(), t.elapsed());
        assert!(fetched * 20 < size, "fetched {fetched} of {size}");
    }
}
