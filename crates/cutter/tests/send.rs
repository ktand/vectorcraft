//! C4.1: the sender and the registry against a recording transport and a test driver.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use vectorcraft_cutjob::CutJob;
use vectorcraft_cutter::send::{Controls, Patience, Progress, SendError, send, spawn};
use vectorcraft_cutter::{DeviceInfo, DeviceProvider, DeviceStatus, Driver, Encoded, MemoryTransport, Transport, TransportError, registry};

/// Asks with `?`; `R` is ready, `M` moving, `U` unloaded.
struct TestDriver;

impl Driver for TestDriver {
    fn name(&self) -> &'static str {
        "Test"
    }
    fn drives(&self, d: &DeviceInfo) -> bool {
        d.vendor_id == 0x1234
    }
    fn model(&self, _: &DeviceInfo) -> String {
        "Test machine".into()
    }
    fn encode(&self, job: &CutJob, _: &DeviceInfo) -> Result<Encoded, String> {
        Ok(Encoded {
            chunks: job.paths.iter().map(|p| format!("{}", p.points.len()).into_bytes()).collect(),
            warnings: vec![],
            pauses: vec![],
            checks: vec![],
        })
    }
    fn status_query(&self) -> &[u8] {
        b"?"
    }
    fn parse_status(&self, reply: &[u8]) -> DeviceStatus {
        match reply {
            b"R" => DeviceStatus::Ready,
            b"M" => DeviceStatus::Moving,
            b"U" => DeviceStatus::Unloaded,
            _ => DeviceStatus::Unknown,
        }
    }
}

fn job(chunks: &[&str]) -> Encoded {
    Encoded { chunks: chunks.iter().map(|c| c.as_bytes().to_vec()).collect(), warnings: vec![], pauses: vec![], checks: vec![] }
}

const QUICK: Patience = Patience { polls: 5, poll_ms: 0 };

fn run(t: &mut MemoryTransport, chunks: &[&str]) -> (Result<(), SendError>, Progress) {
    let progress = Mutex::new(Progress::default());
    let r = send(t, &TestDriver, &job(chunks), QUICK, &Controls::default(), &progress);
    (r, progress.into_inner().unwrap())
}

#[test]
fn each_chunk_goes_once_the_machine_is_ready() {
    let mut t = MemoryTransport::answering(b"R");
    t.state.lock().unwrap().replies.extend([b"M".to_vec(), b"M".to_vec()]);
    let (r, p) = run(&mut t, &["a", "b"]);
    assert_eq!(r, Ok(()));
    assert_eq!((p.chunks_sent, p.chunks, p.outcome), (2, 2, Some(Ok(()))));
    let writes = t.state.lock().unwrap().writes.clone();
    let want: Vec<&[u8]> = vec![b"?", b"?", b"?", b"a", b"?", b"b"];
    assert_eq!(writes, want, "asked until ready, twice moving first");
}

#[test]
fn no_mat_stops_the_job() {
    let mut t = MemoryTransport::answering(b"U");
    let (r, p) = run(&mut t, &["a"]);
    assert_eq!(r, Err(SendError::Unloaded));
    assert_eq!(p.chunks_sent, 0);
    assert_eq!(r.unwrap_err().to_string(), "load the mat into the cutter first");
}

#[test]
fn a_machine_that_stays_busy_stops_the_job() {
    let mut t = MemoryTransport::answering(b"M");
    assert_eq!(run(&mut t, &["a"]).0, Err(SendError::StillMoving));
    // Silence (a read that times out) counts as not ready either.
    let mut t = MemoryTransport::default();
    assert_eq!(run(&mut t, &["a"]).0, Err(SendError::StillMoving));
}

#[test]
fn a_transport_failure_stops_the_job() {
    let mut t = MemoryTransport::answering(b"R");
    t.state.lock().unwrap().fail_after = Some((2, TransportError::Gone));
    let (r, p) = run(&mut t, &["a", "b"]);
    assert_eq!(r, Err(SendError::Transport(TransportError::Gone)));
    assert_eq!(p.chunks_sent, 1);
    assert_eq!(p.outcome, Some(Err(SendError::Transport(TransportError::Gone))));
}

#[test]
fn cancelling_stops_before_the_next_chunk() {
    let mut t = MemoryTransport::answering(b"R");
    let progress = Mutex::new(Progress::default());
    let r = send(&mut t, &TestDriver, &job(&["a", "b"]), QUICK, &Controls { cancel: AtomicBool::new(true), ..Controls::default() }, &progress);
    assert_eq!(r, Err(SendError::Cancelled));
    assert!(t.written().is_empty(), "nothing went");
}

#[test]
fn a_spawned_send_reports_its_outcome() {
    let t = MemoryTransport::answering(b"R");
    let seen = t.clone();
    let running = spawn(Box::new(t), Arc::new(TestDriver), job(&["a", "b", "c"]), QUICK).unwrap();
    for _ in 0..500 {
        if running.finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(running.progress().outcome, Some(Ok(())));
    assert_eq!(seen.written(), b"?a?b?c");
}

struct OneMachine;

impl DeviceProvider for OneMachine {
    fn devices(&self) -> Vec<DeviceInfo> {
        vec![
            DeviceInfo { id: "usb-1".into(), vendor_id: 0x1234, product_id: 1, name: "a test machine".into() },
            DeviceInfo { id: "usb-2".into(), vendor_id: 0x9999, product_id: 1, name: "a printer".into() },
        ]
    }
    fn open(&self, id: &str) -> Result<Box<dyn Transport>, TransportError> {
        if id == "usb-1" { Ok(Box::new(MemoryTransport::answering(b"R"))) } else { Err(TransportError::Gone) }
    }
}

/// The registry lists the connected machines a driver drives (the other device isn't one).
#[test]
fn the_registry_lists_the_machines_its_drivers_drive() {
    registry::install(Some(Arc::new(OneMachine)));
    registry::add_driver(Arc::new(TestDriver));
    let machines = registry::machines();
    assert_eq!(machines.len(), 1);
    assert_eq!((machines[0].0.id.as_str(), machines[0].1.model(&machines[0].0)), ("usb-1", "Test machine".to_string()));
    assert!(registry::provider().unwrap().open("usb-1").is_ok());
    registry::install(None);
    assert!(registry::machines().is_empty());
}

fn paused_job() -> Encoded {
    Encoded { chunks: vec![b"a".to_vec(), b"b".to_vec()], pauses: vec![(1, "Blue".into())], ..Encoded::default() }
}

fn until(running: &vectorcraft_cutter::send::Running, done: impl Fn(&Progress) -> bool) {
    for _ in 0..500 {
        if done(&running.progress()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("not reached: {:?}", running.progress());
}

/// C5.3: the send stops before the next group and goes on when told.
#[test]
fn a_paused_send_waits_and_goes_on() {
    let t = MemoryTransport::answering(b"R");
    let seen = t.clone();
    let running = spawn(Box::new(t), Arc::new(TestDriver), paused_job(), QUICK).unwrap();
    until(&running, |p| p.waiting.is_some());
    assert_eq!(running.progress().waiting.as_deref(), Some("Blue"));
    assert_eq!(seen.written(), b"?a?", "the first group went, and the machine finished it");
    assert!(running.resume());
    until(&running, |p| p.outcome.is_some());
    assert_eq!(running.progress().outcome, Some(Ok(())));
    assert!(seen.written().ends_with(b"b"));
}

#[test]
fn a_paused_send_can_be_cancelled() {
    let t = MemoryTransport::answering(b"R");
    let seen = t.clone();
    let running = spawn(Box::new(t), Arc::new(TestDriver), paused_job(), QUICK).unwrap();
    until(&running, |p| p.waiting.is_some());
    running.cancel();
    until(&running, |p| p.outcome.is_some());
    assert_eq!(running.progress().outcome, Some(Err(SendError::Cancelled)));
    assert!(!seen.written().ends_with(b"b"), "the second group never went");
}

fn checked_job() -> Encoded {
    let check = vectorcraft_cutter::Check { after_chunk: 0, expect: b"OK".to_vec(), timeout_ms: 1_000, failure: "the marks weren't found".into() };
    Encoded { chunks: vec![b"find".to_vec(), b"cut".to_vec()], checks: vec![check], ..Encoded::default() }
}

fn run_checked(answer: &[u8]) -> (Result<(), SendError>, Vec<u8>) {
    let mut t = MemoryTransport::answering(b"R");
    t.state.lock().unwrap().replies = [b"R".to_vec(), answer.to_vec()].into();
    let progress = Mutex::new(Progress::default());
    let r = send(&mut t, &TestDriver, &checked_job(), QUICK, &Controls::default(), &progress);
    (r, t.written())
}

/// C6.2: after the chunk that asks, the machine's answer is read: the right one and the job goes
/// on, anything else (or nothing) and it stops there with the reason.
#[test]
fn a_checked_answer_lets_the_job_go_on_or_stops_it() {
    let (r, written) = run_checked(b"OK");
    assert_eq!(r, Ok(()));
    assert!(written.ends_with(b"cut"));
    for wrong in [&b"NO"[..], b""] {
        let (r, written) = run_checked(wrong);
        assert_eq!(r, Err(SendError::Refused("the marks weren't found".into())), "{wrong:?}");
        assert!(!written.ends_with(b"cut"), "nothing cut after a wrong answer");
    }
}
