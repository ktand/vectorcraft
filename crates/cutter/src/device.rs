//! What a cutting machine is to the rest of the app (ARCHITECTURE.md › 6): a [`Transport`] the
//! app opens (bytes out, bytes back), found and opened by a [`DeviceProvider`] the app installs,
//! and spoken to by a [`Driver`] for its family of machines, which has no I/O of its own.

use vectorcraft_cutjob::CutJob;

/// Why talking to a machine failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// The machine is no longer there (unplugged, switched off).
    Gone,
    /// Nothing came back in time.
    Timeout,
    /// Another program has the machine open, or the system refused access.
    Busy(String),
    /// Anything else the system reported.
    Io(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Gone => write!(f, "the cutter is no longer connected"),
            TransportError::Timeout => write!(f, "the cutter didn't answer in time"),
            TransportError::Busy(why) => write!(f, "the cutter is in use or can't be opened: {why}"),
            TransportError::Io(why) => write!(f, "talking to the cutter failed: {why}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// The connection to one machine: bytes out, bytes back. Implemented by the app (USB through the
/// system's driver) and by [`crate::MemoryTransport`] for tests.
pub trait Transport: Send {
    /// Writes all of `bytes`.
    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    /// Reads what the machine has sent into `buf`, waiting at most `timeout_ms`: how many bytes
    /// (0 when nothing came).
    fn read(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, TransportError>;
}

/// A machine a provider can open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Unique among the provider's devices, stable while it stays connected (a system path).
    pub id: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// What the system calls it.
    pub name: String,
}

/// Finds and opens machines: installed by the app ([`crate::registry::install`]).
pub trait DeviceProvider: Send + Sync {
    /// The machines connected now.
    fn devices(&self) -> Vec<DeviceInfo>;
    /// Opens machine `id`.
    fn open(&self, id: &str) -> Result<Box<dyn Transport>, TransportError>;
}

/// What a machine said when asked how it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Waiting for work.
    Ready,
    /// Working (cutting or moving): ask again.
    Moving,
    /// No mat or material loaded.
    Unloaded,
    /// An answer this version doesn't read.
    Unknown,
}

/// A job made into what one family of machines takes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Encoded {
    /// Sent in order; the sender waits for the machine to be ready before each.
    pub chunks: Vec<Vec<u8>>,
    /// What the job asked for that the machine can't do exactly (a speed clamped, a setting
    /// approximated).
    pub warnings: Vec<String>,
    /// Before chunk n, the sender waits for the user to go on to the group named (a job cut in
    /// groups, C5.3), in order.
    pub pauses: Vec<(usize, String)>,
    /// Answers the machine must give after a chunk before the job goes on (C6.2: it found the
    /// registration marks), in order.
    pub checks: Vec<Check>,
}

/// An answer the machine must give after chunk `after_chunk`, within `timeout_ms`; anything else
/// stops the job with `failure`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub after_chunk: usize,
    pub expect: Vec<u8>,
    pub timeout_ms: u32,
    pub failure: String,
}

/// One family of machines: what it accepts and how it answers. No I/O: the sender does that.
pub trait Driver: Send + Sync {
    /// Its name, for messages ("Silhouette").
    fn name(&self) -> &'static str;
    /// Whether it drives `device`.
    fn drives(&self, device: &DeviceInfo) -> bool;
    /// What `device` should be called, for menus ("Cameo 4").
    fn model(&self, device: &DeviceInfo) -> String;
    /// `job` as what `device` takes.
    fn encode(&self, job: &CutJob, device: &DeviceInfo) -> Result<Encoded, String>;
    /// What asks the machine how it is.
    fn status_query(&self) -> &[u8];
    /// The machine's answer to [`Driver::status_query`].
    fn parse_status(&self, reply: &[u8]) -> DeviceStatus;
    /// Fork (C10.3): what asks the machine which tools are fitted; none for a family that can't
    /// tell.
    fn tools_query(&self) -> Option<&[u8]> {
        None
    }
    /// Whether `answer` (to [`Driver::tools_query`]) is all there.
    fn tools_answer_done(&self, _answer: &[u8]) -> bool {
        false
    }
    /// The machine's answer to [`Driver::tools_query`]: what is in each holder, holder 1 first.
    fn parse_tools(&self, _answer: &[u8]) -> Vec<crate::tools::Fitted> {
        vec![]
    }
}
