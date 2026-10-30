//! Fork: the cutting workspace's commands (`docs/cutting/ARCHITECTURE.md`).
//!
//! The document's cutting settings and each object's operation, read and written through
//! `vectorcraft-cutting`: the settings live in the document's foreign data, an operation is a
//! foreign attribute on the object, so both are saved, undone and copied like any other edit.
//! Unlike a file, which is read leniently, a command's params are checked: a key it doesn't take
//! or a value out of range is an error.

use serde_json::{Map, Value, json};
use vectorcraft_cutjob::{CuttingSettings, MAT_PRESETS, MatGeometry, Op};
use vectorcraft_cutting::ops::{self, Source};
use vectorcraft_cutting::placement;
use vectorcraft_doc::Document;

use super::*;

// Sending the job to a machine (C4.5).
mod devices;
// Materials: named tool settings (C5.1).
pub mod materials;
// Each group's own setup when cut in groups (C7.4).
mod groups;
// Job history: each send kept, a report for one that failed (C8.4).
pub mod history;
// Nesting the pieces on the mat (C8.5).
mod nest;

const SETTINGS_SET: &str = "cutting.settings.set";
const OP_SET: &str = "cutting.op.set";

pub fn specs() -> Vec<CommandSpec> {
    let mut v = devices::specs();
    v.extend(materials::specs());
    v.extend(groups::specs());
    v.extend(history::specs());
    v.extend(nest::specs());
    v.extend(vec![
        cmd!(
            query "cutting.settings.get",
            "Cutting Settings",
            [],
            None,
            "{} the document's cutting settings → {settings: {version, mat, customMatMm, orientation, artboard, offsetMm, defaultOp, toleranceMm, operations: {cut, score: {force, speedMmS, passes, overcutMm, depth, accel, cornerOvercutMm, bladeOffsetMm, tool}}, ordering, clipToMat, trackEnhancing, grouping, printAndCut, markInsetMm, weedBox, weedMarginMm, weedLines, weedLineSpacingMm, material?, colorSetups?}, mat: {id, label, sizeMm: [width, height], cutAreaMm: [x0, y0, x1, y1], approximate}, mats: [the built-in mats, as mat], placement: {artboardMm: [x0, y0, x1, y1] where the artboard lies on the mat, fits: whether the cut area holds all of it} or null}",
            has_doc,
            settings_get
        ),
        cmd!(
            "cutting.settings.set",
            "Set Cutting Settings",
            [],
            None,
            "{mat?: a mat id from cutting.settings.get's mats (or custom), customMatMm?: [width, height] mm (10–3000), orientation?: portrait|landscape, artboard?: artboard index, offsetMm?: [x, y] mm from the cut area's top-left (±3000), defaultOp?: cut|score|none|outline (objects that, with their groups and layers, set none), toleranceMm?: curve flattening mm (0.001–1), operations?: {cut?, score?: {force?: 0–1, speedMmS?: 1–2000, passes?: 1–10, overcutMm?: 0–5, depth?: 1–10 or null (an automatic blade's depth; null: the machine's own), accel?: 1–3 or null, cornerOvercutMm?: 0–2 (run past sharp corners and back), bladeOffsetMm?: 0–2 or null (how far the blade's tip trails its axis, for the machine to turn it round corners; 0: a pen; null: the usual blade's), tool?: a tool id from cutting.tools.list or null (the usual, the AutoBlade; the tool decides the holder)}}, ordering?: insideFirst|document, clipToMat?: bool (paths off the cut area are cut up to its edge instead of stopping the job), trackEnhancing?: bool (the machine grips the material harder), grouping?: none|layer|color (cut in groups, by top-level layer or by stroke colour, else fill, pausing before each group after the first so the tool or material can be changed), printAndCut?: bool (the artboard is printed with registration marks and the machine cuts from them: portrait only, art between the marks), markInsetMm?: 5–50 (how far in from the artboard's edges the marks go), weedBox?: bool (a box cut round the design, each group's, after it), weedMarginMm?: 0–50 (how far the box is from the design), weedLines?: bool (lines from the box in to the design, stopping short of it), weedLineSpacingMm?: 0–200 (0: one from the middle of each side, else one every this many mm)} change the given settings as one undo step; a key not listed or a value out of range is an error → as cutting.settings.get",
            has_doc,
            settings_set
        ),
        cmd!(
            "cutting.op.set",
            "Set Cutting Operation",
            [],
            None,
            "{ids?, op: cut|score|none|outline|inherit} set the cutting operation of the selected objects (or ids: objects, groups or layers); outline cuts along the outline of all the shapes set to it, united (the lines where they overlap not cut); inherit removes their own, so they take their nearest group's or layer's, else defaultOp; one undo step → {ids: those that changed}",
            has_doc,
            op_set
        ),
        cmd!(
            query "cutting.op.get",
            "Cutting Operation",
            [],
            None,
            "{ids?} the cutting operation of each selected object (or ids) → {objects: [{id, op: cut|score|none|outline, own: its own op or null, source: own|inherited|default, from?: id of the group or layer it inherits from}]}",
            has_doc,
            op_get
        ),
        cmd!(
            query "cutting.job",
            "Cutting Job",
            [],
            None,
            "{} the cut job the document makes with its cutting settings, in cutting order (the document isn't changed) → {job: {version, mat: {id, sizeMm, cutAreaMm}, operations: [{kind: score|cut, force, speedMmS, passes, overcutMm}], paths: [{op: index into operations, source: {node: object id, subpath}, closed, points: [[x, y] mm on the mat]}], warnings: [{source, message}]}, stats: {pathCount, pointCount, pathMm, bladeMm, travelMm}}; an error when art leaves the cut area (unless clipToMat) or the settings' artboard is missing",
            has_doc,
            job
        ),
        cmd!(
            query "cutting.simulate",
            "Simulate Cutting",
            [],
            None,
            "{t?: 0–1 of the run (default 1: the end), mm?: mm along the run (instead of t)} where the blade is at that point of the cut job: from the cut area's origin it travels up to each path and cuts it down, once per pass, a closed path on past its start by the overcut → {position: [x, y] mm on the mat, down, path: index into cutting.job's paths or null, pass, op: score|cut|null, doneMm, totalMm, doneSeconds, totalSeconds (estimated: cutting at each operation's speed, travelling at 200 mm/s), pathStartsMm: [where the blade comes down on each path]}",
            has_doc,
            simulate
        ),
        cmd!(
            "cutting.marks.add",
            "Add Registration Marks",
            [],
            None,
            "{} put Print & Cut's registration marks on the settings' artboard as a locked layer at the top that prints and isn't cut (replacing the marks layer there was, which is found by its cut:marks attribute whatever it is called), as one undo step → {layer: its id}",
            has_doc,
            marks_add
        ),
        cmd!(
            "cutting.marks.remove",
            "Remove Registration Marks",
            [],
            None,
            "{} take the registration marks layer out of the document, as one undo step → {removed: whether there was one}",
            has_doc,
            marks_remove
        ),
        cmd!(
            query "cutting.preview.export",
            "Export Cutting Preview",
            [],
            None,
            "{path?, format?: svg (default)|json} the cut job as an SVG at the mat's size (mm user units: the mat, the travel dashed, each path in its operation's colour with data-order and data-op, numbered) or as cutting.job's JSON; with path, written there → {path, bytes}; without → {text, name: <document>-cut.svg|json}",
            has_doc,
            preview_export
        ),
    ]);
    v
}

fn marks_add(s: &mut Session, _: &Value) -> Result<Value> {
    const CMD: &str = "cutting.marks.add";
    let mut added = None;
    s.edit("Add Registration Marks", |d, _| {
        if let Some(old) = vectorcraft_cutting::marks::marks_layer(d) {
            d.remove(old).map_err(|e| bad(CMD, e.to_string()))?;
        }
        let settings = vectorcraft_cutting::settings(d);
        let layer = vectorcraft_cutting::marks::marks_layer_node(&settings, d).map_err(|e| bad(CMD, e))?;
        added = Some(d.insert(None, usize::MAX, layer).map_err(|e| bad(CMD, e.to_string()))?);
        Ok(())
    })?;
    Ok(json!({ "layer": added.map(|id| id.0) }))
}

fn marks_remove(s: &mut Session, _: &Value) -> Result<Value> {
    let Some(layer) = vectorcraft_cutting::marks::marks_layer(&s.doc()?.doc) else {
        return Ok(json!({ "removed": false }));
    };
    s.edit("Remove Registration Marks", |d, _| {
        d.remove(layer).map_err(|e| bad("cutting.marks.remove", e.to_string()))?;
        Ok(())
    })?;
    Ok(json!({ "removed": true }))
}

fn preview_export(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.preview.export";
    let d = &s.doc()?.doc;
    let job = vectorcraft_cutting::build_job(d).map_err(|e| bad(CMD, e.to_string()))?;
    let (text, ext) = match str_param(p, "format").unwrap_or("svg") {
        "svg" => (vectorcraft_cutting::preview::to_svg(&job), "svg"),
        "json" => {
            let v = json!({"job": job.to_value(), "stats": job.stats().to_value()});
            (serde_json::to_string_pretty(&v).map_err(|e| bad(CMD, e.to_string()))?, "json")
        }
        other => return Err(bad(CMD, format!("format must be svg or json, not {other:?}"))),
    };
    let name = super::fileio::default_name(d, ext).replacen(&format!(".{ext}"), &format!("-cut.{ext}"), 1);
    match str_param(p, "path") {
        Some(path) => {
            super::fileio::write_file(path, text.as_bytes())?;
            Ok(json!({"path": path, "bytes": text.len()}))
        }
        None => Ok(json!({"text": text, "name": name})),
    }
}

fn simulate(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.simulate";
    check_numbers(p, &["t", "mm"], CMD)?;
    let job = vectorcraft_cutting::build_job(&s.doc()?.doc).map_err(|e| bad(CMD, e.to_string()))?;
    let sim = vectorcraft_cutjob::Simulation::new(&job);
    let at = match p.get("mm").and_then(Value::as_f64) {
        Some(mm) => sim.at_mm(mm),
        None => sim.at_fraction(f64_or(p, "t", 1.0)),
    };
    Ok(json!({
        "position": at.position,
        "down": at.down,
        "path": at.path,
        "pass": at.pass,
        "op": at.op.map(|k| k.id()),
        "doneMm": at.done_mm,
        "totalMm": sim.total_mm,
        "doneSeconds": at.done_seconds,
        "totalSeconds": sim.total_seconds,
        "pathStartsMm": sim.path_starts_mm(),
    }))
}

fn job(s: &mut Session, _: &Value) -> Result<Value> {
    let job = vectorcraft_cutting::build_job(&s.doc()?.doc).map_err(|e| bad("cutting.job", e.to_string()))?;
    Ok(json!({"job": job.to_value(), "stats": job.stats().to_value()}))
}

fn mat_json(m: &MatGeometry) -> Value {
    let a = m.cut_area_mm;
    json!({"id": m.id, "label": m.label, "sizeMm": m.size_mm, "cutAreaMm": [a.x0, a.y0, a.x1, a.y1], "approximate": m.approximate})
}

/// Where the artboard lies on the mat, and whether the cut area holds all of it (null: the
/// settings' artboard isn't in the document).
fn placement_json(s: &CuttingSettings, d: &Document) -> Value {
    placement::place(s, d).map_or(Value::Null, |p| {
        let a = p.artboard_mm;
        json!({"artboardMm": [a.x0, a.y0, a.x1, a.y1], "fits": p.fits()})
    })
}

fn settings_json(s: &CuttingSettings, d: &Document) -> Value {
    json!({
        "settings": s.to_value(),
        "mat": mat_json(&s.mat_geometry()),
        "mats": MAT_PRESETS.iter().map(mat_json).collect::<Vec<_>>(),
        "placement": placement_json(s, d),
    })
}

fn settings_get(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    Ok(settings_json(&vectorcraft_cutting::settings(d), d))
}

/// The keys `cutting.settings.set` takes at the top, in `operations` and in each tool's.
const TOP_KEYS: [&str; 18] = [
    "mat",
    "customMatMm",
    "orientation",
    "artboard",
    "offsetMm",
    "defaultOp",
    "toleranceMm",
    "operations",
    "ordering",
    "clipToMat",
    "trackEnhancing",
    "grouping",
    "printAndCut",
    "markInsetMm",
    "weedBox",
    "weedMarginMm",
    "weedLines",
    "weedLineSpacingMm",
];
const OP_KEYS: [&str; 2] = ["cut", "score"];
const TOOL_KEYS: [&str; 9] = ["force", "speedMmS", "passes", "overcutMm", "depth", "accel", "cornerOvercutMm", "bladeOffsetMm", "tool"];

/// `m`'s keys are all `allowed`; `at` names where `m` is, for the error.
fn known_keys(m: &Map<String, Value>, allowed: &[&str], at: &str) -> Result<()> {
    match m.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(bad(SETTINGS_SET, format!("unknown key {at}{k}: takes {}", allowed.join(", ")))),
        None => Ok(()),
    }
}

/// `v` at `at` as an object.
fn object_at<'a>(v: &'a Value, at: &str) -> Result<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| bad(SETTINGS_SET, format!("{at} must be an object")))
}

/// The params' keys are the settings' (nested ones too).
fn check_settings_keys(p: &Value) -> Result<()> {
    let top = object_at(p, "params")?;
    known_keys(top, &TOP_KEYS, "")?;
    if let Some(ops) = top.get("operations") {
        let ops = object_at(ops, "operations")?;
        known_keys(ops, &OP_KEYS, "operations.")?;
        for (op, tool) in ops {
            let at = format!("operations.{op}");
            known_keys(object_at(tool, &at)?, &TOOL_KEYS, &format!("{at}."))?;
        }
    }
    Ok(())
}

/// `patch` laid over `target`, objects key by key.
fn merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (k, v) in p {
                match t.get_mut(k) {
                    Some(old) if old.is_object() && v.is_object() => merge(old, v),
                    _ => {
                        t.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (t, p) => *t = p.clone(),
    }
}

/// `a` and `b` are the same value, numbers compared as numbers (`1` is `1.0`).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y)),
        _ => a == b,
    }
}

/// Each value `given` names reads as given in the settings `got` (as their JSON): one that was
/// clamped or read as its default was out of range or not valid.
fn check_taken(given: &Map<String, Value>, got: &Value, at: &str) -> Result<()> {
    for (k, v) in given {
        let r = got.get(k).unwrap_or(&Value::Null);
        if let Some(inner) = v.as_object() {
            check_taken(inner, r, &format!("{at}{k}."))?;
        } else if !same(v, r) {
            return Err(bad(SETTINGS_SET, format!("{at}{k}: {v} is out of range or not valid")));
        }
    }
    Ok(())
}

fn settings_set(s: &mut Session, p: &Value) -> Result<Value> {
    check_settings_keys(p)?;
    let given = object_at(p, "params")?;
    let d = &s.doc()?.doc;
    let current = vectorcraft_cutting::settings(d);
    let mut merged = current.to_value();
    merge(&mut merged, p);
    let new = CuttingSettings::from_value(Some(&merged));
    check_taken(given, &new.to_value(), "")?;
    if given.contains_key("mat") && new.mat != "custom" && MatGeometry::preset(&new.mat).is_none() {
        let ids: Vec<&str> = MAT_PRESETS.iter().map(|m| m.id).collect();
        return Err(bad(SETTINGS_SET, format!("unknown mat {:?}: one of {}, or custom", new.mat, ids.join(", "))));
    }
    // C10.2: a tool given is one this version knows.
    for (op, tool) in [("cut", &new.operations.cut.tool), ("score", &new.operations.score.tool)] {
        let named = given.get("operations").and_then(|o| o.get(op)).is_some_and(|t| t.get("tool").is_some());
        if named
            && let Some(id) = tool
            && vectorcraft_cutter_silhouette::tool(Some(id)).is_none()
        {
            let ids: Vec<&str> = vectorcraft_cutter_silhouette::TOOLS.iter().map(|t| t.id).collect();
            return Err(bad(SETTINGS_SET, format!("unknown tool {id:?} for {op}: one of {}, or null", ids.join(", "))));
        }
    }
    if given.contains_key("artboard") && index(u64::from(new.artboard)) >= d.artboards.len() {
        return Err(bad(SETTINGS_SET, format!("no artboard {}: the document has {}", new.artboard, d.artboards.len())));
    }
    if new != current {
        s.edit("Cutting Settings", |d, _| {
            vectorcraft_cutting::set_settings(d, &new);
            Ok(())
        })?;
    }
    Ok(settings_json(&new, &s.doc()?.doc))
}

/// The objects `p` names in `ids` (each checked; one inside another given one is dropped, as the
/// selection drops it), else the selected ones.
fn objects(s: &Session, p: &Value, cmd: &str) -> Result<Vec<NodeId>> {
    let mut ids = checked_ids_or_selection(s, p, cmd)?;
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
    Ok(ids)
}

fn op_set(s: &mut Session, p: &Value) -> Result<Value> {
    let op = match str_param(p, "op") {
        Some("inherit") => None,
        Some(id) => Some(Op::parse(id).ok_or_else(|| bad(OP_SET, format!("op must be cut, score, none, outline or inherit, not {id:?}")))?),
        None => return Err(bad(OP_SET, "op is required: cut, score, none, outline or inherit")),
    };
    let ids = objects(s, p, OP_SET)?;
    if ids.is_empty() {
        return Err(bad(OP_SET, "no objects: select some or give ids"));
    }
    let d = &s.doc()?.doc;
    let changed: Vec<NodeId> = ids.into_iter().filter(|id| d.node(*id).is_some_and(|n| ops::would_change(n, op))).collect();
    if !changed.is_empty() {
        s.edit("Cutting Operation", |d, _| {
            for id in &changed {
                if let Some(n) = d.node_mut(*id) {
                    ops::set_own_op(n, op);
                }
            }
            Ok(())
        })?;
    }
    Ok(json!({ "ids": changed.iter().map(|i| i.0).collect::<Vec<_>>() }))
}

fn op_get(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = objects(s, p, "cutting.op.get")?;
    let d = &s.doc()?.doc;
    let default = vectorcraft_cutting::settings(d).default_op;
    let objects: Vec<Value> = ids
        .into_iter()
        .filter_map(|id| {
            let r = ops::resolve(d, id, default)?;
            let own = d.node(id).and_then(ops::own_op).map(Op::id);
            let mut o = json!({"id": id.0, "op": r.op.id(), "own": own});
            let (source, from) = match r.source {
                Source::Own => ("own", None),
                Source::Inherited(a) => ("inherited", Some(a.0)),
                Source::Default => ("default", None),
            };
            o["source"] = json!(source);
            if let Some(from) = from {
                o["from"] = json!(from);
            }
            Some(o)
        })
        .collect();
    Ok(json!({ "objects": objects }))
}
