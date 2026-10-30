//! Fork: asking a machine which tools are fitted (C10.3). A driver that can tell says what to
//! send ([`Driver::tools_query`]) and reads the answer ([`Driver::parse_tools`]); this sends the
//! question and waits a little for the answer. Not while a job is being sent: the answer would
//! be mixed up with the job's.

use crate::device::{Driver, Transport, TransportError};

/// What one holder has in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fitted {
    /// Nothing.
    Empty,
    /// A tool the driver knows: its id (as a job's tool settings name it).
    Tool(String),
    /// Something the driver doesn't know: the machine's code for it.
    Unknown(String),
    /// The machine says it can't tell.
    Error,
}

/// How long the answer may take in all, ms, and each wait for it.
const ANSWER_MS: u32 = 1_000;
const WAIT_MS: u32 = 100;
/// The longest answer read.
const MAX_ANSWER: usize = 64;

/// Asks the machine on `transport` what is in each holder, holder 1 first: none when the driver
/// can't ask, an empty list when the machine didn't answer in time.
pub fn ask(transport: &mut dyn Transport, driver: &dyn Driver) -> Result<Option<Vec<Fitted>>, TransportError> {
    let Some(query) = driver.tools_query() else { return Ok(None) };
    transport.write(query)?;
    let mut answer = Vec::new();
    let mut buf = [0u8; MAX_ANSWER];
    let mut waited = 0;
    while waited < ANSWER_MS && answer.len() < MAX_ANSWER {
        let n = transport.read(&mut buf, WAIT_MS)?;
        if let Some(got) = buf.get(..n) {
            answer.extend_from_slice(got);
        }
        // An answer ends with the terminator the driver reads it by; a pause after some bytes
        // ends it too.
        if (n == 0 && !answer.is_empty()) || driver.tools_answer_done(&answer) {
            break;
        }
        waited += WAIT_MS;
    }
    if answer.is_empty() {
        return Ok(Some(vec![]));
    }
    Ok(Some(driver.parse_tools(&answer)))
}
