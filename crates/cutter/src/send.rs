//! Sending an encoded job to a machine: each chunk once the machine says it is ready, with
//! progress, and cancelled between chunks. [`send`] does it where it is called; [`spawn`] on a
//! thread of its own, for the app (native builds; the web build has no threads yet).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::device::{DeviceStatus, Driver, Encoded, Transport, TransportError};

/// How long one status read waits, ms.
const READ_TIMEOUT_MS: u32 = 1_000;

/// How patient the sender is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patience {
    /// Status asked this many times before a chunk at most (the machine moving between them).
    pub polls: u32,
    /// The pause between two asks, ms (0 in tests).
    pub poll_ms: u64,
}

impl Default for Patience {
    /// Ten minutes of asking every half second: a long cut finishes in that.
    fn default() -> Self {
        Patience { polls: 1_200, poll_ms: 500 }
    }
}

/// How a send is going.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub chunks_sent: usize,
    pub chunks: usize,
    /// Set once it stops: `Ok` when every chunk went, else why not.
    pub outcome: Option<Result<(), SendError>>,
}

/// Why a send stopped before the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SendError {
    Cancelled,
    /// The machine has no mat or material loaded.
    Unloaded,
    /// The machine stayed busy past the sender's patience.
    StillMoving,
    Transport(TransportError),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Cancelled => write!(f, "the job was cancelled"),
            SendError::Unloaded => write!(f, "load the mat into the cutter first"),
            SendError::StillMoving => write!(f, "the cutter stayed busy too long"),
            SendError::Transport(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SendError {}

/// Asks the machine how it is until it is ready.
fn wait_ready(t: &mut dyn Transport, driver: &dyn Driver, patience: Patience, cancel: &AtomicBool) -> Result<(), SendError> {
    let mut buf = [0u8; 64];
    for attempt in 0..patience.polls.max(1) {
        if cancel.load(Ordering::Relaxed) {
            return Err(SendError::Cancelled);
        }
        t.write(driver.status_query()).map_err(SendError::Transport)?;
        let n = t.read(&mut buf, READ_TIMEOUT_MS).map_err(SendError::Transport)?;
        match driver.parse_status(buf.get(..n).unwrap_or_default()) {
            DeviceStatus::Ready => return Ok(()),
            DeviceStatus::Unloaded => return Err(SendError::Unloaded),
            // Moving, nothing yet, or an answer not read: ask again after a pause.
            DeviceStatus::Moving | DeviceStatus::Unknown => {
                if attempt + 1 < patience.polls && patience.poll_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(patience.poll_ms));
                }
            }
        }
    }
    Err(SendError::StillMoving)
}

/// Sends `job` to `t`: each chunk once the machine is ready. `progress` follows it; `cancel`
/// stops it before the next chunk.
pub fn send(
    t: &mut dyn Transport,
    driver: &dyn Driver,
    job: &Encoded,
    patience: Patience,
    cancel: &AtomicBool,
    progress: &Mutex<Progress>,
) -> Result<(), SendError> {
    let set = |f: &dyn Fn(&mut Progress)| {
        if let Ok(mut p) = progress.lock() {
            f(&mut p);
        }
    };
    set(&|p| *p = Progress { chunks: job.chunks.len(), ..Progress::default() });
    let result = (|| {
        for (i, chunk) in job.chunks.iter().enumerate() {
            wait_ready(t, driver, patience, cancel)?;
            if cancel.load(Ordering::Relaxed) {
                return Err(SendError::Cancelled);
            }
            t.write(chunk).map_err(SendError::Transport)?;
            set(&|p| p.chunks_sent = i + 1);
        }
        Ok(())
    })();
    set(&|p| p.outcome = Some(result.clone()));
    result
}

/// A send running on its own thread.
#[derive(Clone, Debug, Default)]
pub struct Running {
    pub progress: Arc<Mutex<Progress>>,
    pub cancel: Arc<AtomicBool>,
}

impl Running {
    /// How it is going.
    pub fn progress(&self) -> Progress {
        self.progress.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Asks it to stop before the next chunk.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Whether it has stopped (done, failed or cancelled).
    pub fn finished(&self) -> bool {
        self.progress().outcome.is_some()
    }
}

/// [`send`] on a thread of its own.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn(mut t: Box<dyn Transport>, driver: Arc<dyn Driver>, job: Encoded, patience: Patience) -> Result<Running, String> {
    let running = Running::default();
    let (progress, cancel) = (running.progress.clone(), running.cancel.clone());
    std::thread::Builder::new()
        .name("cutter-send".into())
        .spawn(move || {
            // The outcome is kept in `progress`, where the app reads it.
            let _ = send(t.as_mut(), driver.as_ref(), &job, patience, &cancel, &progress);
        })
        .map_err(|e| format!("couldn't start sending: {e}"))?;
    Ok(running)
}

/// The web build has no threads to send on yet.
#[cfg(target_arch = "wasm32")]
pub fn spawn(_: Box<dyn Transport>, _: Arc<dyn Driver>, _: Encoded, _: Patience) -> Result<Running, String> {
    Err("sending to a cutter isn't available in the browser yet".into())
}
