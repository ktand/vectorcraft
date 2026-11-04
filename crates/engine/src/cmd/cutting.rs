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

use super::{CommandSpec, NodeId, Result, Session, always, bad, check_numbers, checked_ids_or_selection, cmd, f64_or, has_doc, index, str_param};

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
// A cut line round the art (C11.1).
mod contour;
// A document measured in pixels, scaled to its author's pixel size (C11.3).
mod pixels;
// Stencil bridges (C11.5).
mod bridges;
// Sketch fill for the pen (C13.3).
mod sketch;
// Rhinestone templates (C13.4).
mod stones;
// Puzzles (C13.5).
mod popup;
mod puzzle;
// Templates made to size (C15.1).
mod template;
// Papercraft from a 3D model (C15.6).
mod papercraft;
// Tiles larger than the mat (C14.7).
mod tile;
// The machine's origin adjustment (C4.6).
pub mod machine;

const SETTINGS_SET: &str = "cutting.settings.set";
const OP_SET: &str = "cutting.op.set";
const EDGE_SET: &str = "cutting.edge.set";

pub fn specs() -> Vec<CommandSpec> {
    let mut v = devices::specs();
    v.extend(materials::specs());
    v.extend(groups::specs());
    v.extend(history::specs());
    v.extend(nest::specs());
    v.extend(contour::specs());
    v.extend(pixels::specs());
    v.extend(bridges::specs());
    v.extend(sketch::specs());
    v.extend(stones::specs());
    v.extend(puzzle::specs());
    v.extend(popup::specs());
    v.extend(template::specs());
    v.extend(papercraft::specs());
    v.extend(tile::specs());
    v.extend(machine::specs());
    v.extend(vec![
        cmd!(
            query "cutting.settings.get",
            "Cutting Settings",
            [],
            None,
            "{} the document's cutting settings → {settings: {version, mat, customMatMm, orientation, artboard, offsetMm, defaultOp, toleranceMm, operations: {cut, score: {force, speedMmS, passes, overcutMm, depth, accel, cornerOvercutMm, bladeOffsetMm, tool}}, ordering, clipToMat, mirror, trackEnhancing, grouping, printAndCut, markInsetMm, markInsetsMm?, markLengthMm, markLineMm, bleedMm, perforateCutMm, perforateGapMm, weedBox, weedMarginMm, weedLines, weedLineSpacingMm, knockout, knockoutOverlapMm, detailSlow, detailSpeedMmS, detailSizeMm, alignMarks, material?, colorSetups?}, mat: {id, label, sizeMm: [width, height], cutAreaMm: [x0, y0, x1, y1], approximate}, mats: [the built-in mats, as mat], placement: {artboardMm: [x0, y0, x1, y1] where the artboard lies on the mat, fits: whether the cut area holds all of it} or null}",
            has_doc,
            settings_get
        ),
        cmd!(
            "cutting.settings.set",
            "Set Cutting Settings",
            [],
            None,
            "{mat?: a mat id from cutting.settings.get's mats (or custom, or roll: material fed from the roll with no mat, C17.1), customMatMm?: [width, height] mm (10–3000: the custom mat's size, or the roll's width and the length cut), orientation?: portrait|landscape, artboard?: artboard index, offsetMm?: [x, y] mm from the cut area's top-left (±3000), defaultOp?: cut|score|none|outline|perforate (objects that, with their groups and layers, set none), toleranceMm?: curve flattening mm (0.001–1), operations?: {cut?, score?: {force?: 0–1, speedMmS?: 1–2000, passes?: 1–10, overcutMm?: 0–5, depth?: 1–10 or null (an automatic blade's depth; null: the machine's own), accel?: 1–3 or null, cornerOvercutMm?: 0–2 (run past sharp corners and back), bladeOffsetMm?: 0–2 or null (how far the blade's tip trails its axis, for the machine to turn it round corners; 0: a pen; null: the usual blade's), tool?: a tool id from cutting.tools.list or null (the usual, the AutoBlade; the tool decides the holder)}}, ordering?: insideFirst|rollers|document (insideFirst: paths inside others first and the shortest travel; rollers: the same in bands down the mat for the fewest roller moves; document: paint order), clipToMat?: bool (paths off the cut area are cut up to its edge instead of stopping the job), mirror?: bool (cut mirrored left to right, for heat transfer vinyl cut from the back; the art unchanged; not with printAndCut), trackEnhancing?: bool (the machine grips the material harder), grouping?: none|layer|color|fillColor (cut in groups, by top-level layer, by line colour (stroke, else fill) or by fill colour (fill, else stroke), pausing before each group after the first unless it is set not to (cutting.group.set pause), so the tool or material can be changed), printAndCut?: bool (the artboard is printed with registration marks and the machine cuts from them: portrait only, art between the marks), markInsetMm?: 10–84 (how far in from the artboard's edges the marks go; default 15.88, 5/8 in), markInsetsMm?: [left, top, right, bottom] (10–84 each) or null (each edge's own inset, in place of markInsetMm), markLengthMm?: 5–20 (the L marks' arms; default 20), markLineMm?: 0.3–1 (the width the marks are drawn; default 0.5; the machine is told both), bleedMm?: 0.1–5 (how far cutting.bleed.add spreads the print past the art; default 1.27), perforateCutMm?, perforateGapMm?: 0.2–50 (default 3 and 1: the cuts and gaps of objects set to perforate without a dashed stroke of their own), weedBox?: bool (a box cut round the design, each group's, after it), weedMarginMm?: 0–50 (how far the box is from the design), weedLines?: bool (lines from the box in to the design, stopping short of it), weedLineSpacingMm?: 0–200 (0: one from the middle of each side, else one every this many mm), knockout?: bool (C18.1: with grouping, each object cut back to what shows of it under the other groups painted above it, for layered vinyl without bulk), knockoutOverlapMm?: 0–3 (default 0.5: how far a lower colour runs under the one above, so no gap shows), detailSlow?: bool (C18.7: paths no larger than detailSizeMm either way cut at no more than detailSpeedMmS, so fine vinyl lifts less), detailSpeedMmS?: 1–2000 (default 30), detailSizeMm?: 1–100 (default 10), alignMarks?: bool (C18.10: with grouping, a small cross cut at the same place in every group, outside the design, to line the layers up by)} change the given settings as one undo step; a key not listed or a value out of range is an error → as cutting.settings.get",
            has_doc,
            settings_set
        ),
        cmd!(
            "cutting.op.set",
            "Set Cutting Operation",
            [],
            None,
            "{ids?, op: cut|score|none|outline|perforate|inherit} set the cutting operation of the selected objects (or ids: objects, groups or layers); outline cuts along the outline of all the shapes set to it, united (the lines where they overlap not cut); perforate cuts in dashes, the stroke's dash pattern or the settings' perforateCutMm and perforateGapMm; inherit removes their own, so they take their nearest group's or layer's, else defaultOp; one undo step → {ids: those that changed}",
            has_doc,
            op_set
        ),
        cmd!(
            "cutting.edge.set",
            "Set Cut Edge",
            [],
            None,
            "{ids?, edge: path|stroke|inherit} what the selected objects (or ids: objects, groups or layers) are cut along: path, the path itself (a stroke down its centre line); stroke, round the edge of what each path paints (its strokes' outlines, as drawn, united with its fill; a path with no stroke along its path); inherit removes their own, so they take their nearest group's or layer's, else path; one undo step → {ids: those that changed}",
            has_doc,
            edge_set
        ),
        cmd!(
            query "cutting.op.get",
            "Cutting Operation",
            [],
            None,
            "{ids?} the cutting operation of each selected object (or ids) → {objects: [{id, op: cut|score|none|outline|perforate, own: its own op or null, source: own|inherited|default, from?: id of the group or layer it inherits from, edge: path|stroke (what it is cut along, cutting.edge.set), edgeFrom: id of the object, group or layer that sets it, or null}]}",
            has_doc,
            op_get
        ),
        cmd!(
            query "cutting.job",
            "Cutting Job",
            [],
            None,
            "{} the cut job the document makes with its cutting settings, in cutting order (the document isn't changed) → {job: {version, mat: {id, sizeMm, cutAreaMm}, operations: [{kind: score|cut, force, speedMmS, passes, overcutMm}], paths: [{op: index into operations, source: {node: object id, subpath}, closed, points: [[x, y] mm on the mat]}], warnings: [{source, message}]}, stats: {pathCount, pointCount, pathMm, bladeMm, travelMm}, quote: {seconds: the cut's time, groups: [{name, material, sheets, metres: of a roll, cost: null when its material has no price}], cost: the whole job's, or null} (C18.3)}; an error when art leaves the cut area (unless clipToMat) or the settings' artboard is missing",
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
            "cutting.bleed.add",
            "Add Print Bleed",
            [],
            None,
            "{radiusMm?: 0.1–5 (default: the settings' bleedMm)} a print bleed for Print & Cut: the Print Bleed layer (replacing one there is), at the bottom, locked and not cut, holding each printed object's painted outline grown radiusMm with round corners and filled with its stroke's colour, else its fill's, so a cut a little off the print lands in colour; the job warns when the art changes after; one undo step → {layer, objects}",
            has_doc,
            bleed_add
        ),
        cmd!(
            "cutting.bleed.remove",
            "Remove Print Bleed",
            [],
            None,
            "{} the Print Bleed layer taken out, one undo step → {removed: whether there was one}",
            has_doc,
            bleed_remove
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
            query "cutting.jobSheet",
            "Cutting Job Sheet",
            [],
            None,
            "{} the job sheet (C18.8): an A4 SVG page of the document's job, each group in cutting order with its material, cut settings and pressing → {data: the SVG, name: <document>-job.svg}",
            has_doc,
            job_sheet
        ),
        cmd!(
            query "cutting.weedingMap",
            "Cutting Weeding Map",
            [],
            None,
            "{} the weeding map (C18.9): an SVG of the cut sheet as it is cut (mirrored when the job is), what's pulled away shaded and the pieces that stay white, round the design with a 5 mm margin, mm units → {data: the SVG, name: <document>-weeding.svg}",
            has_doc,
            weeding_map
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

/// After each edit ([`Session::edit`]): the registration marks and the print bleed drawn again
/// when what they were drawn from changed, in the same undo step (`vectorcraft_cutting::live`).
/// Nothing to do without a marks or bleed layer.
pub(crate) fn refresh(doc: &mut std::sync::Arc<Document>) {
    if vectorcraft_cutting::marks::marks_layer(doc).is_none() && vectorcraft_cutting::bleed::bleed_layer(doc).is_none() {
        return;
    }
    vectorcraft_cutting::live::refresh(std::sync::Arc::make_mut(doc));
}

fn bleed_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.bleed.add";
    check_numbers(p, &["radiusMm"], CMD)?;
    let radius = p
        .get("radiusMm")
        .and_then(Value::as_f64)
        .unwrap_or_else(|| s.doc().map_or(CuttingSettings::DEFAULT_BLEED_MM, |st| vectorcraft_cutting::settings(&st.doc).bleed_mm));
    let mut added = None;
    s.edit("Add Print Bleed", |d, _| {
        // The distance asked for becomes the document's, so the bleed is kept at it.
        let mut settings = vectorcraft_cutting::settings(d);
        if settings.bleed_mm != radius && (CuttingSettings::BLEED_MM.0..=CuttingSettings::BLEED_MM.1).contains(&radius) {
            settings.bleed_mm = radius;
            vectorcraft_cutting::set_settings(d, &settings);
        }
        if let Some(old) = vectorcraft_cutting::bleed::bleed_layer(d) {
            d.remove(old).map_err(|e| bad(CMD, e.to_string()))?;
        }
        let (layer, objects) = vectorcraft_cutting::bleed::bleed_layer_node(d, radius).map_err(|e| bad(CMD, e))?;
        added = Some((d.insert(None, 0, layer).map_err(|e| bad(CMD, e.to_string()))?, objects));
        vectorcraft_cutting::bleed::prune_images(d);
        Ok(())
    })?;
    let (layer, objects) = added.ok_or_else(|| bad(CMD, "the bleed wasn't added"))?;
    Ok(json!({ "layer": layer.0, "objects": objects }))
}

fn bleed_remove(s: &mut Session, _: &Value) -> Result<Value> {
    let Some(layer) = vectorcraft_cutting::bleed::bleed_layer(&s.doc()?.doc) else {
        return Ok(json!({ "removed": false }));
    };
    s.edit("Remove Print Bleed", |d, _| {
        d.remove(layer).map_err(|e| bad("cutting.bleed.remove", e.to_string()))?;
        vectorcraft_cutting::bleed::prune_images(d);
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

fn weeding_map(s: &mut Session, _: &Value) -> Result<Value> {
    const CMD: &str = "cutting.weedingMap";
    let st = s.doc()?;
    let job = vectorcraft_cutting::build_job(&st.doc).map_err(|e| bad(CMD, e.to_string()))?;
    let name = super::fileio::default_name(&st.doc, "svg").replacen(".svg", "-weeding.svg", 1);
    Ok(json!({"data": vectorcraft_cutting::weedmap::to_svg(&job), "name": name}))
}

fn job_sheet(s: &mut Session, _: &Value) -> Result<Value> {
    const CMD: &str = "cutting.jobSheet";
    let st = s.doc()?;
    let job = vectorcraft_cutting::build_job(&st.doc).map_err(|e| bad(CMD, e.to_string()))?;
    let svg = vectorcraft_cutting::jobsheet::to_svg(&st.title(), &vectorcraft_cutting::settings(&st.doc), &job, &materials::find);
    let name = super::fileio::default_name(&st.doc, "svg").replacen(".svg", "-job.svg", 1);
    Ok(json!({"data": svg, "name": name}))
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
    let d = &s.doc()?.doc;
    let job = vectorcraft_cutting::build_job(d).map_err(|e| bad("cutting.job", e.to_string()))?;
    let price_of = |name: &str| materials::find(name).and_then(|m| m.price);
    let quote = vectorcraft_cutting::quote::quote(&vectorcraft_cutting::settings(d), &job, &price_of);
    let mut job = job;
    // C18.11: a group needing more of a roll than is left of it.
    if vectorcraft_cutting::settings(d).mat == vectorcraft_cutjob::ROLL {
        for g in &quote.shares {
            let left = g.material.as_deref().and_then(materials::find).and_then(|m| m.roll_left_mm.map(|l| (m.name, l)));
            if let Some((name, left)) = left
                && g.metres * 1000.0 > left
            {
                job.warnings.push(vectorcraft_cutjob::JobWarning {
                    source: None,
                    message: format!("the job needs {:.0} mm of {name}, and {left:.0} mm of the roll is left", g.metres * 1000.0),
                });
            }
        }
    }
    Ok(json!({"job": job.to_value(), "stats": job.stats().to_value(), "quote": quote.to_value()}))
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
const TOP_KEYS: [&str; 31] = [
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
    "mirror",
    "trackEnhancing",
    "grouping",
    "printAndCut",
    "markInsetMm",
    "markInsetsMm",
    "markLengthMm",
    "markLineMm",
    "bleedMm",
    "perforateCutMm",
    "perforateGapMm",
    "weedBox",
    "weedMarginMm",
    "weedLines",
    "weedLineSpacingMm",
    "knockout",
    "knockoutOverlapMm",
    "detailSlow",
    "detailSpeedMmS",
    "detailSizeMm",
    "alignMarks",
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
    // C12.1: insets below what can be set (older documents may have them) are refused.
    let lowest = vectorcraft_cutjob::marks::INSET_MM.0;
    let insets = [given.get("markInsetMm").map(|_| new.mark_inset_mm)]
        .into_iter()
        .flatten()
        .chain(given.get("markInsetsMm").and(new.mark_insets_mm).into_iter().flatten());
    if let Some(low) = insets.into_iter().find(|v| *v < lowest) {
        return Err(bad(SETTINGS_SET, format!("the marks' inset must be at least {lowest} mm, not {low}")));
    }
    if given.contains_key("mat") && new.mat != "custom" && new.mat != vectorcraft_cutjob::ROLL && MatGeometry::preset(&new.mat).is_none() {
        let ids: Vec<&str> = MAT_PRESETS.iter().map(|m| m.id).collect();
        return Err(bad(SETTINGS_SET, format!("unknown mat {:?}: one of {}, custom or roll", new.mat, ids.join(", "))));
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
        Some(id) => {
            Some(Op::parse(id).ok_or_else(|| bad(OP_SET, format!("op must be cut, score, none, outline, perforate or inherit, not {id:?}")))?)
        }
        None => return Err(bad(OP_SET, "op is required: cut, score, none, outline, perforate or inherit")),
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

fn edge_set(s: &mut Session, p: &Value) -> Result<Value> {
    use vectorcraft_cutting::edge::{self, Edge};
    let e = match str_param(p, "edge") {
        Some("inherit") => None,
        Some(id) => Some(Edge::parse(id).ok_or_else(|| bad(EDGE_SET, format!("edge must be path, stroke or inherit, not {id:?}")))?),
        None => return Err(bad(EDGE_SET, "edge is required: path, stroke or inherit")),
    };
    let ids = objects(s, p, EDGE_SET)?;
    if ids.is_empty() {
        return Err(bad(EDGE_SET, "no objects: select some or give ids"));
    }
    let d = &s.doc()?.doc;
    let changed: Vec<NodeId> = ids.into_iter().filter(|id| d.node(*id).is_some_and(|n| edge::would_change(n, e))).collect();
    if !changed.is_empty() {
        s.edit("Cut Edge", |d, _| {
            for id in &changed {
                if let Some(n) = d.node_mut(*id) {
                    edge::set_own_edge(n, e);
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
            let (edge, edge_from) = vectorcraft_cutting::edge::resolve(d, id)?;
            o["edge"] = json!(edge.id());
            o["edgeFrom"] = json!(edge_from.map(|a| a.0));
            if let Some(from) = from {
                o["from"] = json!(from);
            }
            Some(o)
        })
        .collect();
    Ok(json!({ "objects": objects }))
}
