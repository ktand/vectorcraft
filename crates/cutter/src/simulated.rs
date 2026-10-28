//! A machine that isn't there (C4.8): a transport that takes what is sent, stays busy a while for
//! each part of the job as a machine moving would, and answers the status query from that; and a
//! provider that lists such machines next to real ones. With it the app's Send, progress, pause
//! and cancel can be tried without a cutter. The answers it gives are the driver's to choose.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::device::{DeviceInfo, DeviceProvider, Transport, TransportError};

pub use crate::SIMULATED_ID_PREFIX;

/// Longest a simulated machine stays busy for one write.
const MAX_BUSY: Duration = Duration::from_secs(30);

/// How a simulated machine answers and how long it works.
#[derive(Clone, Debug)]
pub struct Behaviour {
    /// The status query (a write of exactly this asks; it isn't work).
    pub query: Vec<u8>,
    /// The answer while it isn't moving.
    pub ready: Vec<u8>,
    /// The answer while it is.
    pub moving: Vec<u8>,
    /// Busy this long for any part of the job…
    pub busy_per_write: Duration,
    /// …and this much more for each byte of it.
    pub busy_per_byte: Duration,
    /// After a write holding the first, the machine answers the second unasked once it has
    /// worked it through (C6.2: the registration marks found).
    pub answers: Vec<(Vec<u8>, Vec<u8>)>,
}

/// What a simulated machine was sent, shared with whoever made it.
#[derive(Clone, Debug, Default)]
pub struct Sent(pub Arc<Mutex<Vec<u8>>>);

impl Sent {
    pub fn bytes(&self) -> Vec<u8> {
        self.0.lock().map(|b| b.clone()).unwrap_or_default()
    }
}

/// The transport to one simulated machine.
#[derive(Debug)]
pub struct PacedTransport {
    behaviour: Behaviour,
    busy_until: Instant,
    asked: bool,
    /// An answer to give unasked, once the machine isn't busy.
    pending: Option<Vec<u8>>,
    sent: Sent,
}

impl PacedTransport {
    pub fn new(behaviour: Behaviour, sent: Sent) -> Self {
        PacedTransport { behaviour, busy_until: Instant::now(), asked: false, pending: None, sent }
    }
}

impl Transport for PacedTransport {
    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if bytes == self.behaviour.query.as_slice() {
            self.asked = true;
            return Ok(());
        }
        let per_byte = self.behaviour.busy_per_byte.saturating_mul(u32::try_from(bytes.len()).unwrap_or(u32::MAX));
        let busy = self.behaviour.busy_per_write.saturating_add(per_byte).min(MAX_BUSY);
        self.busy_until = self.busy_until.max(Instant::now()) + busy;
        if let Some((_, answer)) =
            self.behaviour.answers.iter().find(|(when, _)| !when.is_empty() && bytes.windows(when.len()).any(|w| w == when.as_slice()))
        {
            self.pending = Some(answer.clone());
        }
        if let Ok(mut s) = self.sent.0.lock() {
            s.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], _timeout_ms: u32) -> Result<usize, TransportError> {
        let busy = Instant::now() < self.busy_until;
        let unasked = if busy { None } else { self.pending.take() };
        let reply = match unasked {
            Some(answer) => answer,
            None if std::mem::take(&mut self.asked) => (if busy { &self.behaviour.moving } else { &self.behaviour.ready }).clone(),
            None => return Ok(0),
        };
        let n = reply.len().min(buf.len());
        if let (Some(dst), Some(src)) = (buf.get_mut(..n), reply.get(..n)) {
            dst.copy_from_slice(src);
        }
        Ok(n)
    }
}

/// One simulated machine: what it is and how it behaves.
#[derive(Clone, Debug)]
pub struct SimulatedMachine {
    /// Its id should start with [`SIMULATED_ID_PREFIX`].
    pub info: DeviceInfo,
    pub behaviour: Behaviour,
    pub sent: Sent,
}

impl DeviceProvider for SimulatedMachine {
    fn devices(&self) -> Vec<DeviceInfo> {
        vec![self.info.clone()]
    }

    fn open(&self, id: &str) -> Result<Box<dyn Transport>, TransportError> {
        if id == self.info.id { Ok(Box::new(PacedTransport::new(self.behaviour.clone(), self.sent.clone()))) } else { Err(TransportError::Gone) }
    }
}

/// Several providers as one: their machines together, each opened by the one that lists it.
pub struct Providers(pub Vec<Arc<dyn DeviceProvider>>);

impl DeviceProvider for Providers {
    fn devices(&self) -> Vec<DeviceInfo> {
        self.0.iter().flat_map(|p| p.devices()).collect()
    }

    fn open(&self, id: &str) -> Result<Box<dyn Transport>, TransportError> {
        let owner = self.0.iter().find(|p| p.devices().iter().any(|d| d.id == id)).ok_or(TransportError::Gone)?;
        owner.open(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine() -> SimulatedMachine {
        SimulatedMachine {
            info: DeviceInfo { id: format!("{SIMULATED_ID_PREFIX}test"), vendor_id: 1, product_id: 2, name: "Test".into() },
            behaviour: Behaviour {
                query: b"?".to_vec(),
                ready: b"R".to_vec(),
                moving: b"M".to_vec(),
                busy_per_write: Duration::from_millis(80),
                busy_per_byte: Duration::ZERO,
                answers: vec![(b"FIND".to_vec(), b"FOUND".to_vec())],
            },
            sent: Sent::default(),
        }
    }

    fn ask(t: &mut dyn Transport) -> Vec<u8> {
        t.write(b"?").unwrap();
        let mut buf = [0u8; 8];
        let n = t.read(&mut buf, 0).unwrap();
        buf[..n].to_vec()
    }

    #[test]
    fn it_is_busy_for_a_while_after_work_then_ready() {
        let m = machine();
        let mut t = m.open(&m.info.id).unwrap();
        assert_eq!(ask(t.as_mut()), b"R");
        t.write(b"job").unwrap();
        assert_eq!(ask(t.as_mut()), b"M");
        std::thread::sleep(Duration::from_millis(120));
        assert_eq!(ask(t.as_mut()), b"R");
        assert_eq!(m.sent.bytes(), b"job", "the queries aren't kept as work");
        let mut buf = [0u8; 8];
        assert_eq!(t.read(&mut buf, 0).unwrap(), 0, "nothing to say unasked");
    }

    /// C6.2: a write that asks for an answer gets it unasked, once the machine has worked it.
    #[test]
    fn it_answers_what_it_is_asked_once_done() {
        let m = machine();
        let mut t = m.open(&m.info.id).unwrap();
        t.write(b"FIND").unwrap();
        let mut buf = [0u8; 8];
        assert_eq!(t.read(&mut buf, 0).unwrap(), 0, "still busy");
        std::thread::sleep(Duration::from_millis(120));
        let n = t.read(&mut buf, 0).unwrap();
        assert_eq!(&buf[..n], b"FOUND");
        assert_eq!(t.read(&mut buf, 0).unwrap(), 0, "once");
    }

    #[test]
    fn providers_list_together_and_open_their_own() {
        let a: Arc<dyn DeviceProvider> = Arc::new(machine());
        let mut other = machine();
        other.info.id = format!("{SIMULATED_ID_PREFIX}other");
        let both = Providers(vec![a, Arc::new(other)]);
        assert_eq!(both.devices().len(), 2);
        assert!(both.open("simulated:other").is_ok());
        assert!(matches!(both.open("nope"), Err(TransportError::Gone)));
    }
}
