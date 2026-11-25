use std::io::{self, Read, Seek};

pub mod local;

pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

#[derive(Debug, Clone, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: i64,
}

/// A library root. `rel` paths are '/'-separated and relative to the root; the root itself is "".
pub trait Source: Send + Sync {
    fn list_dir(&self, rel: &str) -> io::Result<Vec<DirEntry>>;
    fn open(&self, rel: &str) -> io::Result<Box<dyn ReadSeek>>;
}
