//! Fork: the machine's origin adjustment (C4.6). A machine and mat whose origin isn't the grid's
//! corner (the owner's Cameo 4 on its mat cut 2 mm left of the grid) cut where the preview shows
//! once every position is moved by the adjustment. It belongs to the machine and mat, not the
//! document: kept for the process and, once the app [`attach`]es a file (next to the
//! preferences), in it, so it is there next time.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};
pub use vectorcraft_cutter::ORIGIN_ADJUST_MM;

use super::{CommandSpec, Result, Session, always, bad, cmd};

const SET: &str = "cutting.machine.set";
/// The largest machine file read, bytes.
const MAX_FILE: u64 = 1 << 16;

#[derive(Default)]
struct Store {
    path: Option<PathBuf>,
    adjust_mm: [f64; 2],
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::default()))
}

/// `v` as an adjustment: two finite numbers within [`ORIGIN_ADJUST_MM`].
fn read_adjust(v: &Value) -> Option<[f64; 2]> {
    let [x, y] = v.as_array()?.as_slice() else { return None };
    let ok = |a: &Value| a.as_f64().filter(|a| a.is_finite() && a.abs() <= ORIGIN_ADJUST_MM);
    Some([ok(x)?, ok(y)?])
}

/// Keep the machine settings in the file at `path` from now on, and take the ones already there
/// (a damaged or too large file leaves no adjustment, with a log record).
pub fn attach(path: PathBuf) {
    let adjust = match std::fs::metadata(&path) {
        Ok(m) if m.len() > MAX_FILE => {
            log::warn!("{}: larger than {MAX_FILE} bytes, not read", path.display());
            None
        }
        Ok(_) => {
            let read = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
            let adjust = read.as_ref().and_then(|v| read_adjust(&v["originAdjustMm"]));
            if adjust.is_none() {
                log::warn!("{}: no origin adjustment in it, none used", path.display());
            }
            adjust
        }
        Err(_) => None,
    };
    if let Ok(mut s) = store().lock() {
        s.path = Some(path);
        s.adjust_mm = adjust.unwrap_or_default();
    }
}

/// Writes the settings to the attached file, through a temporary file renamed over it.
fn save(s: &Store) -> std::result::Result<(), String> {
    let Some(path) = &s.path else { return Ok(()) };
    let text = serde_json::to_string_pretty(&json!({ "originAdjustMm": s.adjust_mm })).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The origin adjustment every send uses, mm (across, down).
pub(super) fn adjust_mm() -> [f64; 2] {
    store().lock().map(|s| s.adjust_mm).unwrap_or_default()
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "cutting.machine.get",
            "Cutter Settings",
            [],
            None,
            "{} the machine settings kept next to the preferences (not in the document) → {originAdjustMm: [across, down] mm, how far every cut is moved on the mat, + right and down}",
            always,
            get
        ),
        cmd!(
            query "cutting.machine.set",
            "Set Cutter Settings",
            [],
            None,
            "{originAdjustMm: [across, down] mm (±10 each)} move every cut that far on the mat (+ right and down), for a machine and mat that cut off the grid: measure a test cut's distance from the grid's corner and set the difference; kept next to the preferences, not undone with the document → as cutting.machine.get",
            always,
            set
        ),
    ]
}

fn get(_: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({ "originAdjustMm": adjust_mm() }))
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let v = p.get("originAdjustMm").ok_or_else(|| bad(SET, "originAdjustMm is required"))?;
    let adjust =
        read_adjust(v).ok_or_else(|| bad(SET, format!("originAdjustMm is [across, down] mm, each at most {ORIGIN_ADJUST_MM} either way")))?;
    {
        let mut st = store().lock().map_err(|_| bad(SET, "the machine settings are unavailable"))?;
        let before = st.adjust_mm;
        st.adjust_mm = adjust;
        if let Err(e) = save(&st) {
            st.adjust_mm = before;
            return Err(bad(SET, e));
        }
    }
    get(s, p)
}
