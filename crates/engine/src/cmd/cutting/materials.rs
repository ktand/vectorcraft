//! Fork: materials (C5.1): named tool settings for cutting and scoring. The built-in ones are
//! starting points (`vectorcraft_cutjob::builtin_materials`); the user's are kept for the process
//! and, once the app [`attach`]es a file (next to the preferences), in it, so they are there next
//! time. Applying one to the document sets its tool settings and remembers its name; the
//! document's settings are then "changed" once a number is set by hand. A material cut from the
//! back (C14.12) turns Cut Mirrored on when applied, and one saved while it is on is marked so.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};
use vectorcraft_cutjob::{CuttingSettings, Material, Peel, Press, Pressure, Price, ToolSettings, builtin_materials, read_materials, valid_name};

use super::{CommandSpec, Result, Session, bad, cmd, has_doc, str_param};

/// The largest materials file read, bytes.
const MAX_FILE: u64 = 1 << 20;

#[derive(Default)]
struct Store {
    path: Option<PathBuf>,
    user: Vec<Material>,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::default()))
}

/// Keep the user's materials in the file at `path` from now on, and take the ones already there
/// (a damaged file gives what it can; one too large is left alone, with a log record).
pub fn attach(path: PathBuf) {
    let read = match std::fs::metadata(&path) {
        Ok(m) if m.len() > MAX_FILE => {
            log::warn!("{}: larger than {MAX_FILE} bytes, not read", path.display());
            vec![]
        }
        Ok(_) => std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).map(|v| read_materials(&v)).unwrap_or_default(),
        Err(_) => vec![],
    };
    if let Ok(mut s) = store().lock() {
        s.path = Some(path);
        s.user = read;
    }
}

/// Writes the user's materials to the attached file, through a temporary file renamed over it.
fn save(s: &Store) -> std::result::Result<(), String> {
    let Some(path) = &s.path else { return Ok(()) };
    let text = serde_json::to_string_pretty(&Value::Array(s.user.iter().map(Material::to_value).collect())).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every material: the built-in ones, then the user's.
fn all() -> Vec<Material> {
    let mut v = builtin_materials();
    if let Ok(s) = store().lock() {
        v.extend(s.user.iter().cloned());
    }
    v
}

pub(super) fn find(name: &str) -> Option<Material> {
    all().into_iter().find(|m| m.name.eq_ignore_ascii_case(name.trim()))
}

/// `s`'s tool settings are no longer those of the material it names.
fn changed(s: &CuttingSettings) -> bool {
    s.material.as_deref().and_then(find).is_some_and(|m| !m.cut.same_setup(&s.operations.cut) || !m.score.same_setup(&s.operations.score))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "cutting.materials",
            "Cutting Materials",
            [],
            None,
            "{} the materials (built-in starting points first, then the user's) → {materials: [{name, builtin, mirror: cut from the back (Send asks whether to cut mirrored), price: {amount, per: sheet|metre, wastePct} or null (C18.3), rollLeftMm: what's left of its roll, mm, or null (C18.11), press: {tempC, seconds, pressure, peel} or null (C18.8), handDepthMm: how deep to set a blade adjusted by hand, mm, or null (C18.14; never sent), cut, score: {force, speedMmS, passes, overcutMm, depth, accel, cornerOvercutMm, bladeOffsetMm}}], current: the material the document's tool settings were set from or null, changed: whether they were changed by hand since}",
            has_doc,
            list
        ),
        cmd!(
            "cutting.material.apply",
            "Apply Cutting Material",
            [],
            None,
            "{name} set the document's cut and score tool settings to the material's, and Cut Mirrored to whether it is cut from the back, one undo step → as cutting.materials",
            has_doc,
            apply
        ),
        cmd!(
            "cutting.material.save",
            "Save Cutting Material",
            [],
            None,
            "{name} keep the document's cut and score tool settings as the user's material `name` (replacing one of that name; a built-in name is refused; cut from the back when Cut Mirrored is on), next to the preferences, and name it as the document's material → as cutting.materials",
            has_doc,
            save_cmd
        ),
        cmd!(
            "cutting.material.price",
            "Set Cutting Material Price",
            [],
            None,
            "{name, amount: 0–1000000 or null (no price), per?: sheet|metre (default sheet: a mat's worth; metre: of a roll), wastePct?: 0–100 (default 10: added to a roll's length)} the price of the user's material `name` (built-in ones can't be changed: save it as your own first), for the job's quote (cutting.job's quote) → as cutting.materials",
            has_doc,
            price_cmd
        ),
        cmd!(
            "cutting.material.roll",
            "Set Cutting Material Roll",
            [],
            None,
            "{name, leftMm: 0–1000000 or null (not kept)} what's left of the roll of the user's material `name`, mm; each send cut on a roll (mat roll) from that material takes its length off (C18.11) → as cutting.materials",
            has_doc,
            roll_cmd
        ),
        cmd!(
            "cutting.material.press",
            "Set Cutting Material Pressing",
            [],
            None,
            "{name, tempC: 50–250 or null (none), seconds?: 1–300 (default 15), pressure?: light|medium|firm (default medium), peel?: hot|warm|cold (default warm)} how the user's material `name` is pressed (heat transfer vinyl), for the job sheet (C18.8) → as cutting.materials",
            has_doc,
            press_cmd
        ),
        cmd!(
            "cutting.material.import",
            "Import Cutting Materials",
            [],
            None,
            "{path? | text?: a material list exported from Silhouette Studio (CSV, at most 8 MB), machine?: the file's cutter class (default cameo4), depth?: file|auto (default file: the AutoBlade's depth from the file's mm, 0.1 mm a step of 1–10; auto: left to the machine; other blades' depth is set by hand: kept as the material's handDepthMm, never sent), replace?: bool (default false: a material of yours of the same name is skipped), preview?: bool (nothing kept)} the list's materials for the machine as the user's own (C18.14): a cut row (automatic, Kraft or Deep-Cut blade) and its score row each; force and speed the machine's steps as they are → as cutting.materials, and imported: [names], skipped: [{name, reason}], machines: [the file's cutter classes]",
            has_doc,
            import_cmd
        ),
        cmd!(
            "cutting.material.delete",
            "Delete Cutting Material",
            [],
            None,
            "{name} delete the user's material `name` (built-in ones can't be) → as cutting.materials",
            has_doc,
            delete
        ),
    ]
}

fn listing(s: &Session) -> Result<Value> {
    let settings = vectorcraft_cutting::settings(&s.doc()?.doc);
    let materials: Vec<Value> = all()
        .iter()
        .map(|m| json!({"name": m.name, "builtin": m.builtin, "mirror": m.mirror, "price": m.price.map(|p| p.to_value()), "rollLeftMm": m.roll_left_mm, "press": m.press.map(|p| p.to_value()), "handDepthMm": m.hand_depth_mm, "cut": m.cut.to_value(), "score": m.score.to_value()}))
        .collect();
    Ok(json!({"materials": materials, "current": settings.material, "changed": changed(&settings)}))
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    listing(s)
}

/// The document's settings with `m`'s tool settings and name, as one undo step (none if nothing changes).
fn set_material(s: &mut Session, m: &Material, label: &str) -> Result<()> {
    let mut settings = vectorcraft_cutting::settings(&s.doc()?.doc);
    let before = settings.clone();
    // Keys a newer version kept in the tool settings stay.
    let (cut_extra, score_extra) = (settings.operations.cut.extra.clone(), settings.operations.score.extra.clone());
    // The tool is the machine's (C10.2): a material leaves the one chosen.
    let (cut_tool, score_tool) = (settings.operations.cut.tool.clone(), settings.operations.score.tool.clone());
    settings.operations.cut = ToolSettings { extra: cut_extra, tool: cut_tool, ..m.cut.clone() };
    settings.operations.score = ToolSettings { extra: score_extra, tool: score_tool, ..m.score.clone() };
    settings.material = Some(m.name.clone());
    settings.mirror = m.mirror;
    if settings != before {
        s.edit(label, |d, _| {
            vectorcraft_cutting::set_settings(d, &settings);
            Ok(())
        })?;
    }
    Ok(())
}

fn name_param(p: &Value, cmd: &str) -> Result<String> {
    let name = str_param(p, "name").ok_or_else(|| bad(cmd, "name is required"))?;
    valid_name(name).ok_or_else(|| bad(cmd, format!("a material's name is 1 to {} characters", vectorcraft_cutjob::MAX_NAME)))
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.material.apply";
    let name = name_param(p, CMD)?;
    let m = find(&name).ok_or_else(|| bad(CMD, format!("no material {name:?}")))?;
    set_material(s, &m, "Apply Material")?;
    listing(s)
}

fn price_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.material.price";
    let name = name_param(p, CMD)?;
    super::check_numbers(p, &["wastePct"], CMD)?;
    let price = match p.get("amount") {
        Some(Value::Null) => None,
        Some(v) => {
            let amount = v
                .as_f64()
                .filter(|a| a.is_finite() && (Price::AMOUNT.0..=Price::AMOUNT.1).contains(a))
                .ok_or_else(|| bad(CMD, "amount is 0–1000000, or null"))?;
            let per_metre = match p.get("per").and_then(Value::as_str) {
                None | Some("sheet") => false,
                Some("metre") => true,
                Some(other) => return Err(bad(CMD, format!("per is sheet or metre, not {other:?}"))),
            };
            let waste_pct = p.get("wastePct").and_then(Value::as_f64).unwrap_or(10.0);
            if !(Price::WASTE_PCT.0..=Price::WASTE_PCT.1).contains(&waste_pct) {
                return Err(bad(CMD, "wastePct is 0–100"));
            }
            Some(Price { amount, per_metre, waste_pct })
        }
        None => return Err(bad(CMD, "amount is required (null: no price)")),
    };
    if builtin_materials().iter().any(|m| m.name.eq_ignore_ascii_case(&name)) {
        return Err(bad(CMD, format!("{name:?} is a built-in material: save it as your own to give it a price")));
    }
    {
        let mut st = store().lock().map_err(|_| bad(CMD, "the materials are damaged"))?;
        let m = st
            .user
            .iter_mut()
            .find(|u| u.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| bad(CMD, format!("no material of yours named {name:?}")))?;
        m.price = price;
        save(&st).map_err(|e| bad(CMD, format!("the price is kept until VectorCraft closes, but couldn't be saved: {e}")))?;
    }
    listing(s)
}

fn press_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.material.press";
    let name = name_param(p, CMD)?;
    super::check_numbers(p, &["seconds"], CMD)?;
    let press = match p.get("tempC") {
        Some(Value::Null) => None,
        Some(v) => {
            let temp_c = v
                .as_f64()
                .filter(|t| t.is_finite() && (Press::TEMP_C.0..=Press::TEMP_C.1).contains(t))
                .ok_or_else(|| bad(CMD, "tempC is 50–250, or null"))?;
            let seconds = p.get("seconds").and_then(Value::as_f64).unwrap_or(15.0);
            if !(Press::SECONDS.0..=Press::SECONDS.1).contains(&seconds) {
                return Err(bad(CMD, "seconds is 1–300"));
            }
            let pick = |key: &str, default: &'static str| p.get(key).map_or(Some(default), Value::as_str);
            let pressure = pick("pressure", "medium").and_then(Pressure::parse).ok_or_else(|| bad(CMD, "pressure is light, medium or firm"))?;
            let peel = pick("peel", "warm").and_then(Peel::parse).ok_or_else(|| bad(CMD, "peel is hot, warm or cold"))?;
            Some(Press { temp_c, seconds, pressure, peel })
        }
        None => return Err(bad(CMD, "tempC is required (null: no pressing)")),
    };
    if builtin_materials().iter().any(|m| m.name.eq_ignore_ascii_case(&name)) {
        return Err(bad(CMD, format!("{name:?} is a built-in material: save it as your own to give it its pressing")));
    }
    {
        let mut st = store().lock().map_err(|_| bad(CMD, "the materials are damaged"))?;
        let m = st
            .user
            .iter_mut()
            .find(|u| u.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| bad(CMD, format!("no material of yours named {name:?}")))?;
        m.press = press;
        save(&st).map_err(|e| bad(CMD, format!("the pressing is kept until VectorCraft closes, but couldn't be saved: {e}")))?;
    }
    listing(s)
}

fn roll_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.material.roll";
    let name = name_param(p, CMD)?;
    let left = match p.get("leftMm") {
        Some(Value::Null) => None,
        Some(v) => Some(
            v.as_f64()
                .filter(|l| l.is_finite() && (0.0..=vectorcraft_cutjob::MAX_ROLL_MM).contains(l))
                .ok_or_else(|| bad(CMD, "leftMm is 0–1000000, or null"))?,
        ),
        None => return Err(bad(CMD, "leftMm is required (null: not kept)")),
    };
    if builtin_materials().iter().any(|m| m.name.eq_ignore_ascii_case(&name)) {
        return Err(bad(CMD, format!("{name:?} is a built-in material: save it as your own to keep its roll")));
    }
    {
        let mut st = store().lock().map_err(|_| bad(CMD, "the materials are damaged"))?;
        let m = st
            .user
            .iter_mut()
            .find(|u| u.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| bad(CMD, format!("no material of yours named {name:?}")))?;
        m.roll_left_mm = left;
        save(&st).map_err(|e| bad(CMD, format!("the roll is kept until VectorCraft closes, but couldn't be saved: {e}")))?;
    }
    listing(s)
}

/// C18.11: `mm` taken off what's left of the roll of material `name`, if it is the user's and
/// keeps one (a failure to save is logged: the send has started).
pub(super) fn use_roll(name: &str, mm: f64) {
    let Ok(mut st) = store().lock() else { return };
    let Some(m) = st.user.iter_mut().find(|u| u.name.eq_ignore_ascii_case(name)) else { return };
    let Some(left) = m.roll_left_mm else { return };
    m.roll_left_mm = Some((left - mm.max(0.0)).max(0.0));
    if let Err(e) = save(&st) {
        log::warn!("cutting materials: {e}");
    }
}

fn save_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.material.save";
    let name = name_param(p, CMD)?;
    if builtin_materials().iter().any(|m| m.name.eq_ignore_ascii_case(&name)) {
        return Err(bad(CMD, format!("{name:?} is a built-in material: choose another name")));
    }
    let settings = vectorcraft_cutting::settings(&s.doc()?.doc);
    let m = Material {
        name: name.clone(),
        cut: ToolSettings { tool: None, ..settings.operations.cut.clone() },
        score: ToolSettings { tool: None, ..settings.operations.score.clone() },
        builtin: false,
        mirror: settings.mirror,
        // Saved again under its name, it keeps its price (C18.3) and what's left of its roll (C18.11).
        price: find(&name).and_then(|old| old.price),
        roll_left_mm: find(&name).and_then(|old| old.roll_left_mm),
        press: find(&name).and_then(|old| old.press),
        hand_depth_mm: find(&name).and_then(|old| old.hand_depth_mm),
    };
    {
        let mut st = store().lock().map_err(|_| bad(CMD, "the materials are damaged"))?;
        match st.user.iter_mut().find(|u| u.name.eq_ignore_ascii_case(&name)) {
            Some(u) => *u = m.clone(),
            None => {
                if st.user.len() >= vectorcraft_cutjob::MAX_MATERIALS {
                    return Err(bad(CMD, format!("at most {} materials are kept: delete one first", vectorcraft_cutjob::MAX_MATERIALS)));
                }
                st.user.push(m.clone());
            }
        }
        save(&st).map_err(|e| bad(CMD, format!("the material is kept until VectorCraft closes, but couldn't be saved: {e}")))?;
    }
    set_material(s, &m, "Save Material")?;
    listing(s)
}

/// C18.14: materials from a list the user exported from the maker's software (only the file
/// given is read).
fn import_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_cutter_silhouette::studio;
    const CMD: &str = "cutting.material.import";
    let text = match (str_param(p, "path"), str_param(p, "text")) {
        (Some(path), None) => {
            let len = std::fs::metadata(path).map_err(|e| bad(CMD, format!("{path}: {e}")))?.len();
            if len > studio::MAX_BYTES as u64 {
                return Err(bad(CMD, format!("{path}: larger than {} MB", studio::MAX_BYTES >> 20)));
            }
            let bytes = std::fs::read(path).map_err(|e| bad(CMD, format!("{path}: {e}")))?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        (None, Some(text)) => text.to_string(),
        _ => return Err(bad(CMD, "give path or text")),
    };
    let machine = str_param(p, "machine").unwrap_or(studio::DEFAULT_MACHINE);
    let depth = match str_param(p, "depth") {
        None | Some("file") => studio::Depth::File,
        Some("auto") => studio::Depth::Auto,
        Some(other) => return Err(bad(CMD, format!("depth is file or auto, not {other:?}"))),
    };
    let flag = |key: &str| p.get(key).and_then(Value::as_bool).unwrap_or(false);
    let (replace, preview) = (flag("replace"), flag("preview"));
    let got = studio::import(&text, machine, depth).map_err(|e| bad(CMD, e))?;
    let mut skipped: Vec<Value> = got.skipped.iter().map(|k| json!({"name": k.name, "reason": k.reason})).collect();
    let mut imported = vec![];
    {
        let mut st = store().lock().map_err(|_| bad(CMD, "the materials are damaged"))?;
        let mut user = st.user.clone();
        for mut m in got.materials {
            if builtin_materials().iter().any(|b| b.name.eq_ignore_ascii_case(&m.name)) {
                skipped.push(json!({"name": m.name, "reason": "the name of a built-in material"}));
                continue;
            }
            let full = user.len() >= vectorcraft_cutjob::MAX_MATERIALS;
            match user.iter_mut().find(|u| u.name.eq_ignore_ascii_case(&m.name)) {
                Some(_) if !replace => skipped.push(json!({"name": m.name, "reason": "already one of yours"})),
                Some(u) => {
                    // Its price, roll and pressing stay, as when it is saved again.
                    (m.price, m.roll_left_mm, m.press, m.mirror) = (u.price, u.roll_left_mm, u.press, u.mirror);
                    imported.push(m.name.clone());
                    *u = m;
                }
                None if full => {
                    skipped.push(json!({"name": m.name, "reason": format!("at most {} materials are kept", vectorcraft_cutjob::MAX_MATERIALS)}));
                }
                None => {
                    imported.push(m.name.clone());
                    user.push(m);
                }
            }
        }
        if !preview && !imported.is_empty() {
            st.user = user;
            save(&st).map_err(|e| bad(CMD, format!("the materials are kept until VectorCraft closes, but couldn't be saved: {e}")))?;
        }
    }
    let mut out = listing(s)?;
    if let Some(o) = out.as_object_mut() {
        o.insert("imported".into(), json!(imported));
        o.insert("skipped".into(), Value::Array(skipped));
        o.insert("machines".into(), json!(got.machines));
    }
    Ok(out)
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.material.delete";
    let name = name_param(p, CMD)?;
    if builtin_materials().iter().any(|m| m.name.eq_ignore_ascii_case(&name)) {
        return Err(bad(CMD, format!("{name:?} is built in and can't be deleted")));
    }
    {
        let mut st = store().lock().map_err(|_| bad(CMD, "the materials are damaged"))?;
        let before = st.user.len();
        st.user.retain(|u| !u.name.eq_ignore_ascii_case(&name));
        if st.user.len() == before {
            return Err(bad(CMD, format!("no material {name:?} of yours")));
        }
        save(&st).map_err(|e| bad(CMD, e))?;
    }
    listing(s)
}
