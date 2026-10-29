//! Fork: sending the cut job to a cutting machine (C4.5). The machines are the ones the app's
//! device provider finds (`vectorcraft_cutter::registry`) that a driver this version has drives;
//! a send runs on a thread of its own, one at a time, and these commands start it, report on it
//! and cancel it. Without a provider (the web build, tests, a CLI that installs none) there are
//! no machines, and saying so is the answer.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_cutter::send::{Patience, SendError};
use vectorcraft_cutter::{DeviceInfo, Driver, registry};

use super::*;

const SEND: &str = "cutting.send";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "cutting.devices",
            "Cutting Machines",
            [],
            None,
            "{} the connected cutting machines this version can send to → {devices: [{id, model, name, simulated: a machine that isn't there (the app's --virtual-cutter)}]} (empty without a connected machine or where the app can't reach USB)",
            always,
            devices
        ),
        cmd!(
            query "cutting.send",
            "Send to Cutter",
            [],
            None,
            "{device?: an id from cutting.devices (default: the only one connected)} send the document's cut job to the machine, in the background (cutting.status follows it, cutting.cancel stops it); the mat must be loaded → {device, model, chunks, warnings: [what the machine can't do exactly]}",
            has_doc,
            send
        ),
        cmd!(
            query "cutting.test",
            "Test Cut",
            [],
            None,
            "{device?, corner?: topLeft|topRight|bottomLeft|bottomRight (default topLeft), group?: a name from cutting.groups} cut the test shape (a 10 mm square, a triangle inside it cut first) at that corner of the mat's cut area with the document's cut setup or that group's, in the background as cutting.send does → as cutting.send, warnings including whether it lands on the design",
            has_doc,
            test_cut
        ),
        cmd!(
            query "cutting.test.job",
            "Test Cut Job",
            [],
            None,
            "{corner?, group?} the test cut's job, not sent → {job, warnings: [messages]}",
            has_doc,
            test_cut_job
        ),
        cmd!(
            query "cutting.status",
            "Cutting Status",
            [],
            None,
            "{} the job last sent → {sending: bool, device: name or null, chunksSent, chunks, waitingFor: the group it has paused before (cutting.continue goes on) or null, outcome: null while sending, else done|cancelled|the error}",
            always,
            status
        ),
        cmd!(
            query "cutting.cancel",
            "Cancel Cutting",
            [],
            None,
            "{} stop the job being sent before its next chunk (the machine finishes the chunk it has) → {cancelled: whether one was running}",
            always,
            cancel
        ),
        cmd!(
            query "cutting.continue",
            "Continue Cutting",
            [],
            None,
            "{} go on with a job cut in groups that has paused before its next group (the tool or material changed) → {continued: whether one was waiting}",
            always,
            continue_
        ),
    ]
}

/// The drivers this version has, added once.
fn ensure_drivers() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| registry::add_driver(Arc::new(vectorcraft_cutter_silhouette::Silhouette)));
}

fn machines() -> Vec<(DeviceInfo, Arc<dyn Driver>)> {
    ensure_drivers();
    registry::machines()
}

fn devices(_: &mut Session, _: &Value) -> Result<Value> {
    let list: Vec<Value> = machines()
        .iter()
        .map(|(d, x)| {
            let simulated = d.id.starts_with(vectorcraft_cutter::SIMULATED_ID_PREFIX);
            json!({"id": d.id, "model": x.model(d), "name": d.name, "simulated": simulated})
        })
        .collect();
    Ok(json!({ "devices": list }))
}

/// The machine `p` names, else the only one connected.
fn machine(p: &Value, cmd: &str) -> Result<(DeviceInfo, Arc<dyn Driver>)> {
    let found = machines();
    match str_param(p, "device") {
        Some(id) => found.into_iter().find(|(d, _)| d.id == id).ok_or_else(|| bad(cmd, format!("no cutter {id:?} is connected"))),
        None => match <[_; 1]>::try_from(found) {
            Ok([one]) => Ok(one),
            Err(found) if found.is_empty() => Err(bad(cmd, "no cutter is connected")),
            Err(found) => Err(bad(cmd, format!("{} cutters are connected: say which with device", found.len()))),
        },
    }
}

/// Sends `job` to the machine `p` names, in the background; `before` are warnings of its own.
fn start(s: &Session, job: &vectorcraft_cutjob::CutJob, p: &Value, cmd: &str, before: Vec<String>, test: bool) -> Result<Value> {
    let (device, driver) = machine(p, cmd)?;
    if job.paths.is_empty() {
        return Err(bad(cmd, "the job has nothing to cut"));
    }
    let encoded = driver.encode(job, &device).map_err(|e| bad(cmd, e))?;
    let provider = registry::provider().ok_or_else(|| bad(cmd, "no cutter is connected"))?;
    let transport = provider.open(&device.id).map_err(|e| bad(cmd, e.to_string()))?;
    let chunks = encoded.chunks.len();
    let warnings: Vec<String> = before.into_iter().chain(encoded.warnings.iter().cloned()).collect();
    let model = driver.model(&device);
    let running = vectorcraft_cutter::send::spawn(transport, driver, encoded, Patience::default()).map_err(|e| bad(cmd, e))?;
    registry::start(device.clone(), running.clone()).map_err(|e| bad(cmd, e))?;
    super::history::begin(super::history::entry(s, job, &model, test), running);
    Ok(json!({"device": device.id, "model": model, "chunks": chunks, "warnings": warnings}))
}

fn send(s: &mut Session, p: &Value) -> Result<Value> {
    // Which machine first: without one, that is the answer whatever the document.
    machine(p, SEND)?;
    let job = vectorcraft_cutting::build_job(&s.doc()?.doc).map_err(|e| bad(SEND, e.to_string()))?;
    start(s, &job, p, SEND, vec![], false)
}

/// The test cut's job for `p`'s corner and group.
fn test_job(s: &Session, p: &Value, cmd: &str) -> Result<vectorcraft_cutjob::CutJob> {
    use vectorcraft_cutting::test_cut::Corner;
    let corner = match str_param(p, "corner") {
        None => Corner::default(),
        Some(id) => Corner::parse(id).ok_or_else(|| bad(cmd, format!("corner must be topLeft, topRight, bottomLeft or bottomRight, not {id:?}")))?,
    };
    vectorcraft_cutting::test_cut::test_job(&s.doc()?.doc, corner, str_param(p, "group")).map_err(|e| bad(cmd, e.to_string()))
}

fn test_cut(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.test";
    machine(p, CMD)?;
    let job = test_job(s, p, CMD)?;
    let own = job.warnings.iter().map(|w| w.message.clone()).collect();
    start(s, &job, p, CMD, own, true)
}

fn test_cut_job(s: &mut Session, p: &Value) -> Result<Value> {
    let job = test_job(s, p, "cutting.test.job")?;
    Ok(json!({"job": job.to_value(), "warnings": job.warnings.iter().map(|w| w.message.clone()).collect::<Vec<_>>()}))
}

fn status(_: &mut Session, _: &Value) -> Result<Value> {
    // A send that has stopped finishes its history entry.
    super::history::resolve();
    let Some((device, running)) = registry::current() else {
        return Ok(json!({"sending": false, "device": null, "chunksSent": 0, "chunks": 0, "waitingFor": null, "outcome": null}));
    };
    let p = running.progress();
    let outcome = match &p.outcome {
        None => Value::Null,
        Some(Ok(())) => json!("done"),
        Some(Err(SendError::Cancelled)) => json!("cancelled"),
        Some(Err(e)) => json!(e.to_string()),
    };
    Ok(
        json!({"sending": p.outcome.is_none(), "device": device.name, "chunksSent": p.chunks_sent, "chunks": p.chunks, "waitingFor": p.waiting, "outcome": outcome}),
    )
}

fn cancel(_: &mut Session, _: &Value) -> Result<Value> {
    let running = registry::current().filter(|(_, r)| !r.finished());
    if let Some((_, r)) = &running {
        r.cancel();
    }
    Ok(json!({ "cancelled": running.is_some() }))
}

fn continue_(_: &mut Session, _: &Value) -> Result<Value> {
    let continued = registry::current().is_some_and(|(_, r)| r.resume());
    Ok(json!({ "continued": continued }))
}
