//! A transport that keeps what is written and answers from a script: the machine in tests.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::device::{Transport, TransportError};

/// What a [`MemoryTransport`] keeps and answers with, shared so a test can look at it while a
/// sender owns the transport.
#[derive(Debug, Default)]
pub struct Recorded {
    /// Every write, in order.
    pub writes: Vec<Vec<u8>>,
    /// The answers to give, one per read, in order; once they run out, [`Recorded::idle`].
    pub replies: VecDeque<Vec<u8>>,
    /// What a read answers with once `replies` is empty (empty: nothing, as a timeout).
    pub idle: Vec<u8>,
    /// Fail every write after this many with this error.
    pub fail_after: Option<(usize, TransportError)>,
}

/// A transport in memory.
#[derive(Clone, Debug, Default)]
pub struct MemoryTransport {
    pub state: Arc<Mutex<Recorded>>,
}

impl MemoryTransport {
    /// A transport that answers every read with `idle` (a machine's "ready", say).
    pub fn answering(idle: &[u8]) -> Self {
        let t = MemoryTransport::default();
        if let Ok(mut s) = t.state.lock() {
            s.idle = idle.to_vec();
        }
        t
    }

    /// Everything written, joined.
    pub fn written(&self) -> Vec<u8> {
        self.state.lock().map(|s| s.writes.concat()).unwrap_or_default()
    }
}

impl Transport for MemoryTransport {
    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let mut s = self.state.lock().map_err(|_| TransportError::Io("poisoned".into()))?;
        if let Some((n, e)) = &s.fail_after
            && s.writes.len() >= *n
        {
            return Err(e.clone());
        }
        s.writes.push(bytes.to_vec());
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], _timeout_ms: u32) -> Result<usize, TransportError> {
        let mut s = self.state.lock().map_err(|_| TransportError::Io("poisoned".into()))?;
        let reply = match s.replies.pop_front() {
            Some(r) => r,
            None => s.idle.clone(),
        };
        let n = reply.len().min(buf.len());
        if let (Some(dst), Some(src)) = (buf.get_mut(..n), reply.get(..n)) {
            dst.copy_from_slice(src);
        }
        Ok(n)
    }
}
