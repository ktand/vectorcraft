//! Fork: job history (C8.4). Each send to a machine (a test cut too) is kept: when, which
//! document and machine, the material and the settings it was cut with, the job's numbers, how
//! long it took and how it ended. The app [`attach`]es a file next to the preferences
//! (`Cutting/History.json`), newest last, at most [`MAX_ENTRIES`]; a send that fails also writes a
//! short report to share (`Cutting/Reports/`). Without a file (tests, the web build) the history
//! is kept for the process only. Entries are read leniently: the file is the user's.
//!
//! A send runs on its own thread; the entry is made when it starts and finished ([`resolve`]) the
//! next time the status is asked for after it stops (the app asks while a send runs).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Value, json};
use vectorcraft_cutjob::{CutJob, CuttingSettings};
use vectorcraft_cutter::send::{Running, SendError};

use super::*;

/// Most entries kept.
pub const MAX_ENTRIES: usize = 50;
/// The largest history file read, bytes.
const MAX_FILE: u64 = 4 << 20;
/// Most reports kept in their folder: the oldest go first.
const MAX_REPORTS: usize = 20;

#[derive(Default)]
struct Store {
    path: Option<PathBuf>,
    entries: Vec<Value>,
    /// The send last started, while its entry waits for its outcome: (its entry's index, it).
    pending: Option<(usize, Running)>,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::default()))
}

/// The entries in `v` (the file's JSON) this version keeps: objects, the newest [`MAX_ENTRIES`].
fn read_entries(v: &Value) -> Vec<Value> {
    let all: Vec<Value> = v.as_array().into_iter().flatten().filter(|e| e.is_object()).cloned().collect();
    let skip = all.len().saturating_sub(MAX_ENTRIES);
    all.into_iter().skip(skip).collect()
}

/// Keep the history in the file at `path` from now on, and take what is already there (a damaged
/// file gives what it can; one too large is left alone, with a log record).
pub fn attach(path: PathBuf) {
    let read = match std::fs::metadata(&path) {
        Ok(m) if m.len() > MAX_FILE => {
            log::warn!("{}: larger than {MAX_FILE} bytes, not read", path.display());
            vec![]
        }
        Ok(_) => std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).map(|v| read_entries(&v)).unwrap_or_default(),
        Err(_) => vec![],
    };
    if let Ok(mut s) = store().lock() {
        *s = Store { path: Some(path), entries: read, pending: None };
    }
}

/// Writes the history to the attached file, through a temporary file renamed over it.
fn save(s: &Store) -> std::result::Result<(), String> {
    let Some(path) = &s.path else { return Ok(()) };
    let text = serde_json::to_string_pretty(&Value::Array(s.entries.clone())).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// `t` (Unix seconds) as the entry shows it: `2026-10-29 14:03 UTC`.
fn shown_time(t: i64) -> String {
    let [y, mo, d, h, mi, _] = vectorcraft_doc::metadata::civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02} UTC")
}

/// The entry for a send just started: everything but how it ends.
pub(super) fn entry(s: &Session, job: &CutJob, model: &str, test: bool) -> Value {
    let (title, settings) = match s.doc() {
        Ok(d) => (d.title(), vectorcraft_cutting::settings(&d.doc)),
        Err(_) => (String::new(), CuttingSettings::default()),
    };
    let stats = job.stats();
    let sim = vectorcraft_cutjob::Simulation::new(job);
    let now = vectorcraft_doc::metadata::now_unix();
    json!({
        "time": now,
        "shownTime": now.map(shown_time),
        "document": title,
        "machine": model,
        "test": test,
        "material": settings.material,
        "grouping": settings.grouping.id(),
        "materials": job.groups.iter().filter_map(|g| g.material.clone()).collect::<Vec<_>>(),
        "paths": stats.path_count,
        "cutMm": (stats.blade_mm * 10.0).round() / 10.0,
        "estimatedSeconds": sim.total_seconds.round(),
        "seconds": null,
        "outcome": "sending",
        "report": null,
        "settings": settings.to_value(),
    })
}

/// Keeps `e` as the newest entry, waiting for `running`'s outcome.
pub(super) fn begin(e: Value, running: Running) {
    let Ok(mut s) = store().lock() else { return };
    finish_pending(&mut s);
    s.entries.push(e);
    let over = s.entries.len().saturating_sub(MAX_ENTRIES);
    s.entries.drain(..over);
    let index = s.entries.len().saturating_sub(1);
    s.pending = Some((index, running));
    if let Err(e) = save(&s) {
        log::warn!("cutting history: {e}");
    }
}

/// Finishes the pending entry if its send has stopped.
pub(super) fn resolve() {
    let Ok(mut s) = store().lock() else { return };
    if finish_pending(&mut s)
        && let Err(e) = save(&s)
    {
        log::warn!("cutting history: {e}");
    }
}

/// Whether the pending entry was finished.
fn finish_pending(s: &mut Store) -> bool {
    let Some((index, running)) = &s.pending else { return false };
    let p = running.progress();
    let (Some(outcome), Some(seconds)) = (&p.outcome, p.seconds) else { return false };
    let text = match outcome {
        Ok(()) => "done".to_string(),
        Err(SendError::Cancelled) => "cancelled".to_string(),
        Err(e) => e.to_string(),
    };
    let failed = !matches!(outcome, Ok(()) | Err(SendError::Cancelled));
    let index = *index;
    let report_dir = s.path.as_ref().and_then(|p| p.parent()).map(|d| d.join("Reports"));
    if let Some(Value::Object(e)) = s.entries.get_mut(index) {
        e.insert("outcome".into(), json!(text));
        e.insert("seconds".into(), json!(seconds.round()));
        e.insert("chunksSent".into(), json!(p.chunks_sent));
        e.insert("chunks".into(), json!(p.chunks));
        if failed && let Some(dir) = report_dir {
            match write_report(&dir, e) {
                Ok(path) => {
                    e.insert("report".into(), json!(path.display().to_string()));
                }
                Err(err) => log::warn!("cutting report: {err}"),
            }
        }
    }
    s.pending = None;
    true
}

/// A failed send's report, for the user to share: what was sent where, with which settings, and
/// how far it got. Nothing of the art itself.
fn write_report(dir: &std::path::Path, e: &Map<String, Value>) -> std::result::Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let t = e.get("time").and_then(Value::as_i64).unwrap_or(0);
    let [y, mo, d, h, mi, sec] = vectorcraft_doc::metadata::civil(t);
    let path = dir.join(format!("cut-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{sec:02}.txt"));
    let field = |k: &str| e.get(k).map_or(String::new(), |v| v.as_str().map_or_else(|| v.to_string(), str::to_string));
    let settings = e.get("settings").and_then(|v| serde_json::to_string_pretty(v).ok()).unwrap_or_default();
    let text = format!(
        "VectorCraft {} cutting report\n\nTime: {}\nMachine: {}\nDocument: {}\nTest cut: {}\nOutcome: {}\nSent: {} of {} chunks after {} s\nJob: {} paths, {} mm cut, about {} s\n\nSettings:\n{settings}\n",
        env!("CARGO_PKG_VERSION"),
        field("shownTime"),
        field("machine"),
        field("document"),
        field("test"),
        field("outcome"),
        field("chunksSent"),
        field("chunks"),
        field("seconds"),
        field("paths"),
        field("cutMm"),
        field("estimatedSeconds"),
    );
    std::fs::write(&path, text).map_err(|err| format!("{}: {err}", path.display()))?;
    // Only the newest reports stay.
    let mut old: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|f| f.ok().map(|f| f.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect())
        .unwrap_or_default();
    old.sort();
    for p in old.iter().take(old.len().saturating_sub(MAX_REPORTS)) {
        // An old report that can't be removed just stays.
        let _ = std::fs::remove_file(p);
    }
    Ok(path)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "cutting.history",
            "Cutting History",
            [],
            None,
            "{} the sends to a machine, newest first → {entries: [{index, time: Unix seconds or null, shownTime, document, machine, test, material, grouping, materials: [each group's], paths, cutMm, estimatedSeconds, seconds, outcome: sending|done|cancelled|the error, report: a failed send's report file or null, settings}], file: the history file or null}",
            always,
            list
        ),
        cmd!(
            "cutting.history.apply",
            "Use Settings from History",
            [],
            None,
            "{index} set the document's tool settings (cut and score, track enhancing, the material named) to those entry `index` was cut with, one undo step → as cutting.settings.get",
            has_doc,
            apply
        ),
        cmd!(
            query "cutting.history.clear",
            "Clear Cutting History",
            [],
            None,
            "{} forget every entry (the reports stay) → {cleared: how many}",
            always,
            clear
        ),
    ]
}

fn list(_: &mut Session, _: &Value) -> Result<Value> {
    resolve();
    let Ok(s) = store().lock() else { return Err(bad("cutting.history", "the history is damaged")) };
    let entries: Vec<Value> = s
        .entries
        .iter()
        .enumerate()
        .rev()
        .map(|(i, e)| {
            let mut e = e.clone();
            if let Value::Object(m) = &mut e {
                m.insert("index".into(), json!(i));
            }
            e
        })
        .collect();
    Ok(json!({"entries": entries, "file": s.path.as_ref().map(|p| p.display().to_string())}))
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.history.apply";
    let index = p.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()).ok_or_else(|| bad(CMD, "index is required"))?;
    let kept = store().lock().ok().and_then(|st| st.entries.get(index).and_then(|e| e.get("settings").cloned()));
    let Some(kept) = kept else { return Err(bad(CMD, format!("no history entry {index}"))) };
    let from = CuttingSettings::from_value(Some(&kept));
    let d = &s.doc()?.doc;
    let current = vectorcraft_cutting::settings(d);
    let new = CuttingSettings {
        operations: from.operations,
        material: from.material,
        track_enhancing: from.track_enhancing,
        color_setups: from.color_setups,
        ..current.clone()
    };
    if new != current {
        s.edit("Use Settings from History", |d, _| {
            vectorcraft_cutting::set_settings(d, &new);
            Ok(())
        })?;
    }
    super::settings_get(s, &json!({}))
}

fn clear(_: &mut Session, _: &Value) -> Result<Value> {
    let Ok(mut s) = store().lock() else { return Err(bad("cutting.history.clear", "the history is damaged")) };
    let n = s.entries.len();
    s.entries.clear();
    s.pending = None;
    save(&s).map_err(|e| bad("cutting.history.clear", e))?;
    Ok(json!({ "cleared": n }))
}
