use lru::LruCache;
use std::io::{self, Read, Seek, SeekFrom};
use std::num::NonZeroUsize;

pub const BLOCK_SIZE: usize = 256 * 1024;
pub const MAX_BLOCKS: usize = 64;

/// Random-access byte source (e.g. a remote file).
pub trait RangeRead: Send {
    fn len(&self) -> u64;
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize>;
}

/// Read + Seek over a RangeRead, fetching fixed-size blocks and keeping the most recent ones.
pub struct BlockReader<R: RangeRead> {
    inner: R,
    block_size: usize,
    blocks: LruCache<u64, Vec<u8>>,
    pos: u64,
    len: u64,
    fetched: u64,
}

impl<R: RangeRead> BlockReader<R> {
    pub fn new(inner: R, block_size: usize, max_blocks: usize) -> Self {
        let len = inner.len();
        let cap = NonZeroUsize::new(max_blocks.max(1)).unwrap();
        Self { inner, block_size, blocks: LruCache::new(cap), pos: 0, len, fetched: 0 }
    }

    pub fn bytes_fetched(&self) -> u64 {
        self.fetched
    }

    fn block(&mut self, index: u64) -> io::Result<&Vec<u8>> {
        if !self.blocks.contains(&index) {
            let start = index * self.block_size as u64;
            let want = (self.len - start).min(self.block_size as u64) as usize;
            let mut buf = vec![0u8; want];
            let mut filled = 0;
            while filled < want {
                let n = self.inner.read_at(start + filled as u64, &mut buf[filled..])?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
            buf.truncate(filled);
            self.fetched += filled as u64;
            self.blocks.put(index, buf);
        }
        Ok(self.blocks.get(&index).unwrap())
    }
}

impl<R: RangeRead> Read for BlockReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let bs = self.block_size as u64;
        let (index, offset) = (self.pos / bs, (self.pos % bs) as usize);
        let block = self.block(index)?;
        if offset >= block.len() {
            return Ok(0);
        }
        let n = buf.len().min(block.len() - offset);
        buf[..n].copy_from_slice(&block[offset..offset + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl<R: RangeRead> Seek for BlockReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::End(d) => self.len as i128 + d as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
        };
        if target < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
        }
        self.pos = target as u64;
        Ok(self.pos)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// In-memory RangeRead that counts calls and can fail on a chosen call.
    pub struct MemRange {
        pub data: Vec<u8>,
        pub calls: usize,
        pub fail_on_call: Option<usize>,
    }

    impl MemRange {
        pub fn new(data: Vec<u8>) -> Self {
            Self { data, calls: 0, fail_on_call: None }
        }
    }

    impl RangeRead for MemRange {
        fn len(&self) -> u64 {
            self.data.len() as u64
        }
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.fail_on_call == Some(self.calls) {
                return Err(io::Error::new(io::ErrorKind::ConnectionReset, "reset"));
            }
            let start = (offset as usize).min(self.data.len());
            let n = buf.len().min(self.data.len() - start);
            buf[..n].copy_from_slice(&self.data[start..start + n]);
            Ok(n)
        }
    }

    fn data(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 31 % 251) as u8).collect()
    }

    #[test]
    fn reads_only_needed_blocks() {
        let d = data(10_000_000);
        let mut r = BlockReader::new(MemRange::new(d.clone()), BLOCK_SIZE, MAX_BLOCKS);
        let mut tail = [0u8; 100];
        r.seek(SeekFrom::End(-100)).unwrap();
        r.read_exact(&mut tail).unwrap();
        assert_eq!(&tail[..], &d[d.len() - 100..]);
        let mut mid = [0u8; 1024];
        r.seek(SeekFrom::Start(5_000_000)).unwrap();
        r.read_exact(&mut mid).unwrap();
        assert_eq!(&mid[..], &d[5_000_000..5_001_024]);
        assert!(r.bytes_fetched() <= 3 * BLOCK_SIZE as u64);
    }

    #[test]
    fn read_across_block_boundary() {
        let d = data(3 * 1000);
        let mut r = BlockReader::new(MemRange::new(d.clone()), 1000, 8);
        let mut buf = vec![0u8; 1500];
        r.seek(SeekFrom::Start(700)).unwrap();
        r.read_exact(&mut buf).unwrap();
        assert_eq!(buf, d[700..2200]);
    }

    #[test]
    fn rereading_cached_range_does_not_fetch() {
        let mut r = BlockReader::new(MemRange::new(data(5000)), 1000, 8);
        let mut buf = [0u8; 10];
        r.read_exact(&mut buf).unwrap();
        let fetched = r.bytes_fetched();
        r.seek(SeekFrom::Start(0)).unwrap();
        r.read_exact(&mut buf).unwrap();
        assert_eq!(r.bytes_fetched(), fetched);
    }

    #[test]
    fn evicts_oldest_block_beyond_capacity() {
        let mut r = BlockReader::new(MemRange::new(data(5000)), 1000, 2);
        let mut b = [0u8; 1];
        for off in [0u64, 1000, 2000] {
            r.seek(SeekFrom::Start(off)).unwrap();
            r.read_exact(&mut b).unwrap();
        }
        let fetched = r.bytes_fetched();
        r.seek(SeekFrom::Start(0)).unwrap();
        r.read_exact(&mut b).unwrap();
        assert_eq!(r.bytes_fetched(), fetched + 1000);
    }

    #[test]
    fn read_past_end_returns_zero() {
        let mut r = BlockReader::new(MemRange::new(data(10)), 4, 2);
        r.seek(SeekFrom::Start(10)).unwrap();
        assert_eq!(r.read(&mut [0u8; 4]).unwrap(), 0);
    }

    #[test]
    fn propagates_inner_error() {
        let mut m = MemRange::new(data(5000));
        m.fail_on_call = Some(1);
        let mut r = BlockReader::new(m, 1000, 2);
        let err = r.read(&mut [0u8; 4]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::ConnectionReset);
    }
}
