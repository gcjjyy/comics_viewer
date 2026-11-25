use crate::blockcache::RangeRead;
use std::io;

/// Retries a failed read once after reopening (which reconnects the session).
pub struct Retry<R: RangeRead, F: FnMut() -> io::Result<R> + Send> {
    inner: R,
    reopen: F,
}

impl<R: RangeRead, F: FnMut() -> io::Result<R> + Send> Retry<R, F> {
    pub fn new(inner: R, reopen: F) -> Self {
        Self { inner, reopen }
    }
}

impl<R: RangeRead, F: FnMut() -> io::Result<R> + Send> RangeRead for Retry<R, F> {
    fn len(&self) -> u64 {
        self.inner.len()
    }
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.read_at(offset, buf) {
            Ok(n) => Ok(n),
            Err(_) => {
                self.inner = (self.reopen)()?;
                self.inner.read_at(offset, buf)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockcache::tests::MemRange;

    #[test]
    fn retries_once_after_reopen() {
        let mut flaky = MemRange::new(vec![1, 2, 3, 4]);
        flaky.fail_on_call = Some(1);
        let mut reopened = 0;
        let mut r = Retry::new(flaky, || {
            reopened += 1;
            Ok(MemRange::new(vec![1, 2, 3, 4]))
        });
        let mut buf = [0u8; 2];
        assert_eq!(r.read_at(1, &mut buf).unwrap(), 2);
        assert_eq!(buf, [2, 3]);
        drop(r);
        assert_eq!(reopened, 1);
    }

    #[test]
    fn gives_up_when_retry_fails() {
        let mut flaky = MemRange::new(vec![1, 2]);
        flaky.fail_on_call = Some(1);
        let mut r = Retry::new(flaky, || {
            let mut again = MemRange::new(vec![1, 2]);
            again.fail_on_call = Some(1);
            Ok(again)
        });
        assert_eq!(r.read_at(0, &mut [0u8; 1]).unwrap_err().kind(), io::ErrorKind::ConnectionReset);
    }

    #[test]
    fn gives_up_when_reopen_fails() {
        let mut flaky = MemRange::new(vec![1, 2]);
        flaky.fail_on_call = Some(1);
        let mut r = Retry::new(flaky, || Err(io::Error::new(io::ErrorKind::NotConnected, "down")));
        assert_eq!(r.read_at(0, &mut [0u8; 1]).unwrap_err().kind(), io::ErrorKind::NotConnected);
    }
}
