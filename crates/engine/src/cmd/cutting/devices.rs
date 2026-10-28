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
            "{} the connected cutting machines this version can send to → {devices: [{id, model, name}]} (empty without a connected machine or where the app can't reach USB)",
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
            query "cutting.status",
            "Cutting Status",
            [],
            None,
            "{} the job last sent → {sending: bool, device: name or null, chunksSent, chunks, outcome: null while sending, else done|cancelled|the error}",
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
    let list: Vec<Value> = machines().iter().map(|(d, x)| json!({"id": d.id, "model": x.model(d), "name": d.name})).collect();
    Ok(json!({ "devices": list }))
}

fn send(s: &mut Session, p: &Value) -> Result<Value> {
    let found = machines();
    let (device, driver) = match str_param(p, "device") {
        Some(id) => found.into_iter().find(|(d, _)| d.id == id).ok_or_else(|| bad(SEND, format!("no cutter {id:?} is connected")))?,
        None => match <[_; 1]>::try_from(found) {
            Ok([one]) => one,
            Err(found) if found.is_empty() => return Err(bad(SEND, "no cutter is connected")),
            Err(found) => return Err(bad(SEND, format!("{} cutters are connected: say which with device", found.len()))),
        },
    };
    let job = vectorcraft_cutting::build_job(&s.doc()?.doc).map_err(|e| bad(SEND, e.to_string()))?;
    if job.paths.is_empty() {
        return Err(bad(SEND, "the job has nothing to cut"));
    }
    let encoded = driver.encode(&job, &device).map_err(|e| bad(SEND, e))?;
    let provider = registry::provider().ok_or_else(|| bad(SEND, "no cutter is connected"))?;
    let transport = provider.open(&device.id).map_err(|e| bad(SEND, e.to_string()))?;
    let (chunks, warnings) = (encoded.chunks.len(), encoded.warnings.clone());
    let model = driver.model(&device);
    let running = vectorcraft_cutter::send::spawn(transport, driver, encoded, Patience::default()).map_err(|e| bad(SEND, e))?;
    registry::start(device.clone(), running).map_err(|e| bad(SEND, e))?;
    Ok(json!({"device": device.id, "model": model, "chunks": chunks, "warnings": warnings}))
}

fn status(_: &mut Session, _: &Value) -> Result<Value> {
    let Some((device, running)) = registry::current() else {
        return Ok(json!({"sending": false, "device": null, "chunksSent": 0, "chunks": 0, "outcome": null}));
    };
    let p = running.progress();
    let outcome = match &p.outcome {
        None => Value::Null,
        Some(Ok(())) => json!("done"),
        Some(Err(SendError::Cancelled)) => json!("cancelled"),
        Some(Err(e)) => json!(e.to_string()),
    };
    Ok(json!({"sending": p.outcome.is_none(), "device": device.name, "chunksSent": p.chunks_sent, "chunks": p.chunks, "outcome": outcome}))
}

fn cancel(_: &mut Session, _: &Value) -> Result<Value> {
    let running = registry::current().filter(|(_, r)| !r.finished());
    if let Some((_, r)) = &running {
        r.cancel();
    }
    Ok(json!({ "cancelled": running.is_some() }))
}
