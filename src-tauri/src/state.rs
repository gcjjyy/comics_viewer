use crate::archive::{read_entry, ZipIndex};
use crate::db::{Db, LibraryKind};
use crate::keys::split_key;
use crate::library::{find_zips, group_works, FoundZip, Work};
use crate::source::local::LocalSource;
use crate::source::{ReadSeek, Source};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use zip::ZipArchive;

type Archive = Arc<Mutex<ZipArchive<Box<dyn ReadSeek>>>>;

/// Counting semaphore limiting concurrent thumbnail generation.
pub struct Gate {
    free: Mutex<usize>,
    cv: Condvar,
}

impl Gate {
    pub fn new(n: usize) -> Self {
        Self { free: Mutex::new(n), cv: Condvar::new() }
    }
    pub fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        let mut free = self.cv.wait_while(self.free.lock().unwrap(), |n| *n == 0).unwrap();
        *free -= 1;
        drop(free);
        let out = f();
        *self.free.lock().unwrap() += 1;
        self.cv.notify_one();
        out
    }
}

pub struct AppState {
    pub db: Mutex<Db>,
    sources: Mutex<HashMap<i64, Arc<dyn Source>>>,
    archives: Mutex<LruCache<(i64, String), Archive>>,
    indexes: Mutex<HashMap<(i64, String), Arc<ZipIndex>>>,
    pub thumb_dir: PathBuf,
    pub thumb_gate: Gate,
}

pub fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl AppState {
    pub fn new(db: Db, thumb_dir: PathBuf) -> Self {
        Self {
            db: Mutex::new(db),
            sources: Mutex::new(HashMap::new()),
            archives: Mutex::new(LruCache::new(NonZeroUsize::new(4).unwrap())),
            indexes: Mutex::new(HashMap::new()),
            thumb_dir,
            thumb_gate: Gate::new(3),
        }
    }

    pub fn source(&self, lib: i64) -> Result<Arc<dyn Source>, String> {
        if let Some(s) = self.sources.lock().unwrap().get(&lib) {
            return Ok(s.clone());
        }
        let library = self
            .db
            .lock()
            .unwrap()
            .libraries()
            .map_err(err)?
            .into_iter()
            .find(|l| l.id == lib)
            .ok_or("서재를 찾을 수 없습니다")?;
        let src: Arc<dyn Source> = match library.kind {
            LibraryKind::Local => Arc::new(LocalSource::new(PathBuf::from(&library.root))),
            LibraryKind::Sftp => return Err("SSH 서재는 아직 지원하지 않습니다".into()),
        };
        self.sources.lock().unwrap().insert(lib, src.clone());
        Ok(src)
    }

    /// Drops cached connections and open archives for a library (after errors or removal).
    pub fn forget(&self, lib: i64) {
        self.sources.lock().unwrap().remove(&lib);
        let mut a = self.archives.lock().unwrap();
        let stale: Vec<_> = a.iter().filter(|((l, _), _)| *l == lib).map(|(k, _)| k.clone()).collect();
        for k in stale {
            a.pop(&k);
        }
    }

    fn index_for(&self, lib: i64, src: &dyn Source, z: &FoundZip) -> Option<ZipIndex> {
        if let Ok(Some(idx)) = self.db.lock().unwrap().zip_index_cached(lib, &z.rel_path, z.size, z.mtime) {
            return Some(idx);
        }
        let idx = ZipIndex::read(src.open(&z.rel_path).ok()?).ok()?;
        let _ = self.db.lock().unwrap().put_zip_index(lib, &z.rel_path, z.size, z.mtime, &idx);
        self.indexes.lock().unwrap().remove(&(lib, z.rel_path.clone()));
        Some(idx)
    }

    /// Full scan; stores and returns the grouped works.
    pub fn scan(&self, lib: i64) -> Result<Vec<Work>, String> {
        let src = self.source(lib)?;
        let zips = find_zips(src.as_ref()).map_err(err)?;
        let works = group_works(&zips, &|z| self.index_for(lib, src.as_ref(), z));
        self.db.lock().unwrap().put_works(lib, &works).map_err(err)?;
        Ok(works)
    }

    pub fn works(&self, lib: i64) -> Result<Vec<Work>, String> {
        if let Some(w) = self.db.lock().unwrap().works_cached(lib).map_err(err)? {
            return Ok(w);
        }
        self.scan(lib)
    }

    pub fn index(&self, lib: i64, zip_path: &str) -> Result<Arc<ZipIndex>, String> {
        let k = (lib, zip_path.to_string());
        if let Some(i) = self.indexes.lock().unwrap().get(&k) {
            return Ok(i.clone());
        }
        let cached = self.db.lock().unwrap().zip_cached_latest(lib, zip_path).map_err(err)?;
        let idx = match cached {
            Some((_, _, idx)) => idx,
            None => ZipIndex::from_archive(&mut self.archive(lib, zip_path)?.lock().unwrap()).map_err(err)?,
        };
        let idx = Arc::new(idx);
        self.indexes.lock().unwrap().insert(k, idx.clone());
        Ok(idx)
    }

    fn archive(&self, lib: i64, zip_path: &str) -> Result<Archive, String> {
        let k = (lib, zip_path.to_string());
        if let Some(a) = self.archives.lock().unwrap().get(&k) {
            return Ok(a.clone());
        }
        let reader = self.source(lib)?.open(zip_path).map_err(err)?;
        let a: Archive = Arc::new(Mutex::new(ZipArchive::new(reader).map_err(err)?));
        self.archives.lock().unwrap().put(k, a.clone());
        Ok(a)
    }

    /// Page bytes and MIME type for page `index` of volume `key`.
    pub fn read_page(&self, lib: i64, key: &str, index: usize) -> Result<(Vec<u8>, &'static str), String> {
        let (zip_path, inner) = split_key(key);
        let idx = self.index(lib, zip_path)?;
        let vol = idx
            .volumes
            .iter()
            .find(|v| v.inner_dir.as_deref() == inner)
            .ok_or("권을 찾을 수 없습니다")?;
        let page = vol.pages.get(index).ok_or("페이지 번호가 범위를 벗어났습니다")?;
        let archive = self.archive(lib, zip_path)?;
        let result = read_entry(&mut archive.lock().unwrap(), page.entry_index);
        match result {
            Ok(bytes) => Ok((bytes, mime_for(&page.name))),
            Err(e) => {
                self.archives.lock().unwrap().pop(&(lib, zip_path.to_string()));
                Err(err(e))
            }
        }
    }
}

pub fn mime_for(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "image/jpeg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn sample_scan_read_and_thumb() {
        let Ok(dir) = std::env::var("COMICS_SAMPLE_DIR") else { return };
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::open(&tmp.path().join("t.db")).unwrap();
        let lib = db.add_library(LibraryKind::Local, "s", None, &dir).unwrap();
        std::fs::create_dir(tmp.path().join("th")).unwrap();
        let s = AppState::new(db, tmp.path().join("th"));
        let t = std::time::Instant::now();
        let works = s.scan(lib).unwrap();
        eprintln!("scan: {} works in {:?}", works.len(), t.elapsed());
        let sd = works.iter().find(|w| w.title == "슬램덩크 완전판").unwrap();
        let key = &sd.volumes[2].key;
        let (bytes, mime) = s.read_page(lib, key, 5).unwrap();
        assert_eq!(mime, "image/jpeg");
        assert_eq!(&bytes[..2], b"\xFF\xD8");
        let t = std::time::Instant::now();
        let p = crate::thumbs::ensure(&s, lib, key).unwrap();
        eprintln!("thumb in {:?}: {} bytes", t.elapsed(), std::fs::metadata(&p).unwrap().len());
        let img = image::open(&p).unwrap();
        assert_eq!(img.width(), crate::thumbs::THUMB_WIDTH);
        // second scan uses the cache
        let t = std::time::Instant::now();
        assert_eq!(s.scan(lib).unwrap(), works);
        eprintln!("rescan: {:?}", t.elapsed());
    }
}
