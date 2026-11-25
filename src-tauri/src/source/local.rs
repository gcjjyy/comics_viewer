use super::{DirEntry, ReadSeek, Source};
use std::io;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

pub struct LocalSource {
    root: PathBuf,
}

impl LocalSource {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl LocalSource {
    fn path(&self, rel: &str) -> PathBuf {
        rel.split('/').filter(|p| !p.is_empty()).fold(self.root.clone(), |acc, p| acc.join(p))
    }
}

impl Source for LocalSource {
    fn list_dir(&self, rel: &str) -> io::Result<Vec<DirEntry>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.path(rel))? {
            let entry = entry?;
            let meta = entry.metadata()?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            out.push(DirEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: meta.is_dir(),
                size: meta.len(),
                mtime,
            });
        }
        Ok(out)
    }
    fn open(&self, rel: &str) -> io::Result<Box<dyn ReadSeek>> {
        Ok(Box::new(std::fs::File::open(self.path(rel))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn lists_and_opens_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("a")).unwrap();
        std::fs::write(dir.path().join("a/b.zip"), b"hello").unwrap();
        std::fs::write(dir.path().join("c.txt"), b"x").unwrap();
        let src = LocalSource::new(dir.path().to_path_buf());

        let mut root = src.list_dir("").unwrap();
        root.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(root.iter().map(|e| (e.name.as_str(), e.is_dir)).collect::<Vec<_>>(), [("a", true), ("c.txt", false)]);

        let entries = src.list_dir("a").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!((entries[0].name.as_str(), entries[0].is_dir, entries[0].size), ("b.zip", false, 5));
        assert!(entries[0].mtime > 0);

        let mut s = String::new();
        src.open("a/b.zip").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "hello");
    }
}
