use std::io;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

/// Use a vector of bytes behind a Arc<Mutex> as writer in order to inspect the tracing output
/// for testing purposes.
/// Stolen directly from the test suite of tracing-subscriber.
pub struct MockWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl MockWriter {
    pub fn new(buf: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { buf }
    }

    pub fn map_error<Guard>(err: TryLockError<Guard>) -> io::Error {
        match err {
            TryLockError::WouldBlock => io::Error::from(io::ErrorKind::WouldBlock),
            TryLockError::Poisoned(_) => io::Error::from(io::ErrorKind::Other),
        }
    }

    pub fn buf(&self) -> io::Result<MutexGuard<'_, Vec<u8>>> {
        self.buf.try_lock().map_err(Self::map_error)
    }
}

impl io::Write for MockWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf()?.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf()?.flush()
    }
}
