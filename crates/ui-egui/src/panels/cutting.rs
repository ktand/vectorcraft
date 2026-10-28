//! Fork: the Cutting panel (`docs/cutting/ARCHITECTURE.md`): the document's cutting settings
//! (mat, orientation, artboard, offset, default operation, Clip to Mat), the selection's
//! operation, and the cut job those make (counts and lengths, or why there is none). It reads
//! the engine's `cutting.*` queries and acts through `app.run`, as every panel does.

use egui::Ui;
use serde_json::{Value, json};
use vectorcraft_doc::Unit;

use vectorcraft_cutter_silhouette as silhouette;

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "cutting";

/// Points per millimetre: the number fields take points and show millimetres.
const PT: f64 = 72.0 / 25.4;
const LABEL_W: f32 = 80.0;
/// Most of a job's warnings the panel lists.
const SHOWN_WARNINGS: usize = 3;

fn set(app: &mut VectorcraftApp, p: Value) {
    // A refusal (a value out of range) leaves the settings as they were; the field shows them.
    let _ = app.run("cutting.settings.set", p);
}

fn mm_field(ui: &mut Ui, id: &'static str, mm: f64, min_mm: f64) -> Option<f64> {
    widgets::spin_field(ui, ("cutting", id), Some(mm * PT), Unit::Millimeters, 64.0, PT, min_mm * PT, &[]).map(|pt| pt / PT)
}

fn settings_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let t = Tokens::get(ui.ctx());
    let s = &info["settings"];
    let mats: Vec<&Value> = info["mats"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let mat = s["mat"].as_str().unwrap_or_default();
    let custom = mat == "custom";
    let mut labels: Vec<&str> = mats.iter().map(|m| m["label"].as_str().unwrap_or_default()).collect();
    labels.push(tl!("Custom"));
    let current = if custom { tl!("Custom") } else { mats.iter().find(|m| m["id"] == mat).and_then(|m| m["label"].as_str()).unwrap_or(mat) };
    widgets::label_row(ui, tl!("Mat"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown_names(ui, ("cutting", "mat"), current, &labels, 170.0) {
            let id = mats.get(i).and_then(|m| m["id"].as_str()).unwrap_or("custom");
            set(app, json!({ "mat": id }));
        }
    });
    if custom {
        let size = |i: usize| s["customMatMm"][i].as_f64().unwrap_or(304.8);
        widgets::label_row(ui, tl!("Size"), LABEL_W, |ui| {
            if let Some(w) = mm_field(ui, "mat-w", size(0), 10.0) {
                set(app, json!({ "customMatMm": [w, size(1)] }));
            }
            if let Some(h) = mm_field(ui, "mat-h", size(1), 10.0) {
                set(app, json!({ "customMatMm": [size(0), h] }));
            }
        });
    } else if info["mat"]["approximate"].as_bool() == Some(true) {
        widgets::dim_label(ui, tl!("The cut area isn't from the maker's specifications: check it on the mat."));
    }
    let orientations = ["Portrait", "Landscape"];
    let landscape = s["orientation"] == "landscape";
    widgets::label_row(ui, tl!("Orientation"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "orientation"), orientations[usize::from(landscape)], &orientations, 120.0) {
            set(app, json!({ "orientation": if i == 1 { "landscape" } else { "portrait" } }));
        }
    });
    let artboards: Vec<String> = app.session.active().map(|st| st.doc.artboards.iter().map(|a| a.name.clone()).collect()).unwrap_or_default();
    if artboards.len() > 1 {
        let names: Vec<&str> = artboards.iter().map(String::as_str).collect();
        let i = s["artboard"].as_u64().and_then(|i| usize::try_from(i).ok()).unwrap_or(0);
        widgets::label_row(ui, tl!("Artboard"), LABEL_W, |ui| {
            if let Some(i) = widgets::dropdown_names(ui, ("cutting", "artboard"), names.get(i).copied().unwrap_or_default(), &names, 140.0) {
                set(app, json!({ "artboard": i }));
            }
        });
    }
    let offset = |i: usize| s["offsetMm"][i].as_f64().unwrap_or(0.0);
    widgets::label_row(ui, tl!("Offset"), LABEL_W, |ui| {
        if let Some(x) = mm_field(ui, "offset-x", offset(0), -3_000.0) {
            set(app, json!({ "offsetMm": [x, offset(1)] }));
        }
        if let Some(y) = mm_field(ui, "offset-y", offset(1), -3_000.0) {
            set(app, json!({ "offsetMm": [offset(0), y] }));
        }
    });
    let ops = ["Cut", "Cut Outline", "Score", "No Cut"];
    let op_ids = ["cut", "outline", "score", "none"];
    let default = op_ids.iter().position(|o| s["defaultOp"] == *o).unwrap_or(0);
    // The operation of objects that, with their groups and layers, set none.
    widgets::label_row(ui, tl!("Default"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "default"), ops[default], &ops, 120.0) {
            set(app, json!({ "defaultOp": op_ids.get(i).copied().unwrap_or("cut") }));
        }
    });
    let groupings = ["None", "By Layer", "By Colour"];
    let grouping_ids = ["none", "layer", "color"];
    let grouping = grouping_ids.iter().position(|g| s["grouping"] == *g).unwrap_or(0);
    // Cut in groups, with a pause before each after the first to change the tool or material.
    widgets::label_row(ui, tl!("Groups"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "grouping"), groupings[grouping], &groupings, 120.0) {
            set(app, json!({ "grouping": grouping_ids.get(i).copied().unwrap_or("none") }));
        }
    });
    let clip = s["clipToMat"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Clip to Mat"), clip, true) {
        set(app, json!({ "clipToMat": !clip }));
    }
    let track = s["trackEnhancing"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Track Enhancing"), track, true) {
        set(app, json!({ "trackEnhancing": !track }));
    }
    match info["placement"]["fits"].as_bool() {
        Some(true) => {
            widgets::dim_label(ui, tl!("The artboard fits the mat."));
        }
        Some(false) => {
            ui.label(egui::RichText::new(tl!("The artboard goes off the mat.")).color(t.bleed));
        }
        None => {}
    }
}

/// Cut, Score, No Cut or the group's or layer's, for the selected objects.
fn selection_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let objects = app.session.execute("cutting.op.get", &json!({})).ok().and_then(|r| r["objects"].as_array().cloned()).unwrap_or_default();
    if objects.is_empty() {
        widgets::dim_label(ui, tl!("Select objects to choose how they are cut."));
        return;
    }
    let all = |f: &dyn Fn(&Value) -> bool| objects.iter().all(f);
    for (label, op) in [("Cut", "cut"), ("Cut Outline", "outline"), ("Score", "score"), ("No Cut", "none")] {
        if widgets::radio(ui, label, all(&|o| o["op"] == op), true) {
            let _ = app.run("cutting.op.set", json!({ "op": op }));
        }
    }
    let inherited = all(&|o| o["source"] != "own");
    if widgets::radio(ui, "As Its Group or Layer", inherited, true) {
        let _ = app.run("cutting.op.set", json!({ "op": "inherit" }));
    }
}

fn job_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(job) = crate::cutting::job(app, ui.ctx()) else { return };
    match &*job {
        Ok(made) => {
            let (j, s) = (&made.job, &made.stats);
            ui.label(format!("{}: {}", tl!("Paths"), s.path_count));
            ui.label(format!("{}: {:.0} mm", tl!("Cut length"), s.blade_mm));
            ui.label(format!("{}: {:.0} mm", tl!("Travel"), s.travel_mm));
            ui.label(format!("{}: {:.0} s", tl!("Estimated time"), made.sim.total_seconds));
            for w in j.warnings.iter().take(SHOWN_WARNINGS) {
                ui.label(egui::RichText::new(&w.message).color(t.text_dim));
            }
            if j.warnings.len() > SHOWN_WARNINGS {
                widgets::dim_label(ui, &format!("{} {}", j.warnings.len() - SHOWN_WARNINGS, tl!("more warnings")));
            }
            if !j.paths.is_empty() {
                playback_rows(ui, &made.sim);
            }
        }
        Err(e) => {
            ui.label(egui::RichText::new(e).color(t.bleed));
        }
    }
}

/// Play the blade's run, step from path to path, or scrub along it (C3.7); the canvas shows it.
fn playback_rows(ui: &mut Ui, sim: &vectorcraft_cutjob::Simulation) {
    use crate::cutting::{Playback, playback, set_playback};
    let ctx = ui.ctx().clone();
    let mut p = playback(&ctx);
    let total = sim.total_mm;
    let at = p.mm.unwrap_or(total);
    let starts = sim.path_starts_mm();
    ui.horizontal(|ui| {
        let play = if p.playing { tl!("Pause") } else { tl!("Play") };
        if ui.button(play).clicked() {
            // Playing from the end starts again.
            let from = if at >= total { 0.0 } else { at };
            p = Playback { mm: Some(from), playing: !p.playing, last: ctx.input(|i| i.time) };
        }
        if ui.button(tl!("Previous")).clicked() {
            let prev = starts.iter().rev().find(|s| **s < at - 1e-6).copied().unwrap_or(0.0);
            p = Playback { mm: Some(prev), playing: false, ..p };
        }
        if ui.button(tl!("Next")).clicked() {
            let next = starts.iter().find(|s| **s > at + 1e-6).copied().unwrap_or(total);
            p = Playback { mm: Some(next), playing: false, ..p };
        }
        if p.mm.is_some() && ui.button(tl!("Done")).clicked() {
            p = Playback::default();
        }
    });
    let mut v = p.mm.unwrap_or(total);
    if widgets::slider(ui, egui::Slider::new(&mut v, 0.0..=total.max(f64::MIN_POSITIVE)).show_value(false)).changed() {
        p = Playback { mm: Some(v), playing: false, ..p };
    }
    let blade = sim.at_mm(p.mm.unwrap_or(total));
    let path = blade.path.map_or(String::new(), |i| format!(" · {} {}", tl!("path"), i + 1));
    widgets::dim_label(ui, &format!("{:.0} / {:.0} s{path} (×{})", blade.done_seconds, sim.total_seconds, crate::cutting::SPEEDUP));
    set_playback(&ctx, p);
}

/// The material the tool settings come from (C5.1): built-in starting points and the user's own,
/// whether they were changed by hand since, and saving them as a material of the user's.
fn material_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let Ok(info) = app.session.execute("cutting.materials", &json!({})) else { return };
    let list: Vec<(String, bool)> = info["materials"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| Some((m["name"].as_str()?.to_string(), m["builtin"].as_bool()?))).collect())
        .unwrap_or_default();
    let current = info["current"].as_str().map(str::to_string);
    let names: Vec<&str> = list.iter().map(|(n, _)| n.as_str()).collect();
    let shown = current.as_deref().unwrap_or(tl!("None"));
    widgets::label_row(ui, tl!("Material"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown_names(ui, ("cutting", "material"), shown, &names, 170.0)
            && let Some((name, _)) = list.get(i)
        {
            let _ = app.run("cutting.material.apply", json!({ "name": name }));
        }
    });
    if info["changed"].as_bool() == Some(true) {
        widgets::dim_label(ui, tl!("Changed since the material was chosen."));
    } else if current.as_deref().is_some_and(|c| list.iter().any(|(n, b)| n == c && *b)) {
        widgets::dim_label(ui, tl!("A starting point: make a test cut first."));
    }
    let mut name: String = super::pstate(&ctx, "cutting-material-name");
    let mut error: Option<String> = super::pstate(&ctx, "cutting-material-error");
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).hint_text(tl!("Material name")).desired_width(120.0));
        if ui.add_enabled(!name.trim().is_empty(), egui::Button::new(tl!("Save as Material"))).clicked() {
            error = app.run("cutting.material.save", json!({ "name": name.trim() })).err();
            if error.is_none() {
                name.clear();
            }
        }
        let own = current.as_deref().is_some_and(|c| list.iter().any(|(n, b)| n == c && !*b));
        if own
            && ui.button(tl!("Delete")).clicked()
            && let Some(c) = &current
        {
            error = app.run("cutting.material.delete", json!({ "name": c })).err();
        }
    });
    if let Some(e) = &error {
        ui.label(egui::RichText::new(e).color(t.bleed));
    }
    super::set_pstate(&ctx, "cutting-material-name", name);
    super::set_pstate(&ctx, "cutting-material-error", error);
}

/// How each operation drives the blade, force and speed in the machine's own steps (C5.2), passes,
/// overcut, blade depth, acceleration, the corner overcut and the blade offset, set through `cutting.settings.set`
/// (values typed out of range are brought into it).
fn tool_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    material_rows(app, ui);
    for (op, label) in [("cut", "Cut"), ("score", "Score")] {
        let tool = &info["settings"]["operations"][op];
        let get = |k: &str, d: f64| tool[k].as_f64().unwrap_or(d);
        let set_tool = |app: &mut VectorcraftApp, key: &str, v: f64| set(app, json!({ "operations": { op: { key: v } } }));
        ui.label(egui::RichText::new(tl!(label)).strong());
        // In the machine's own steps (C5.2): what its maker's software and guides use.
        let (f_lo, f_hi) = silhouette::FORCE;
        widgets::label_row(ui, tl!("Force"), LABEL_W, |ui| {
            let step = silhouette::force_step(get("force", 0.3));
            if let Some(v) =
                widgets::spin_plain(ui, ("cutting", op, "force"), f64::from(step), &format!(" ({f_lo}–{f_hi})"), 0, 72.0, 1.0, f64::from(f_lo), &[])
            {
                let step = v.clamp(f64::from(f_lo), f64::from(f_hi)).round() as u32;
                set_tool(app, "force", silhouette::force_from_step(step));
            }
        });
        let (s_lo, s_hi) = silhouette::SPEED;
        widgets::label_row(ui, tl!("Speed"), LABEL_W, |ui| {
            let step = silhouette::speed_step(get("speedMmS", 100.0));
            if let Some(v) =
                widgets::spin_plain(ui, ("cutting", op, "speed"), f64::from(step), &format!(" ({s_lo}–{s_hi})"), 0, 72.0, 1.0, f64::from(s_lo), &[])
            {
                let step = v.clamp(f64::from(s_lo), f64::from(s_hi)).round() as u32;
                set_tool(app, "speedMmS", silhouette::speed_from_step(step));
            }
        });
        widgets::label_row(ui, tl!("Passes"), LABEL_W, |ui| {
            if let Some(v) = widgets::spin_plain(ui, ("cutting", op, "passes"), get("passes", 1.0), "", 0, 48.0, 1.0, 1.0, &[]) {
                set_tool(app, "passes", v.clamp(1.0, 10.0).round());
            }
        });
        widgets::label_row(ui, tl!("Overcut"), LABEL_W, |ui| {
            if let Some(v) = widgets::spin_plain(ui, ("cutting", op, "overcut"), get("overcutMm", 0.0), " mm", 1, 64.0, 0.5, 0.0, &[]) {
                set_tool(app, "overcutMm", (v.clamp(0.0, 5.0) * 10.0).round() / 10.0);
            }
        });
        // 0: the machine's own setting, sent as nothing.
        let set_optional = |app: &mut VectorcraftApp, key: &str, v: f64, hi: f64| {
            let step = v.clamp(0.0, hi).round();
            let value = if step < 1.0 { Value::Null } else { json!(step) };
            set(app, json!({ "operations": { op: { key: value } } }));
        };
        widgets::label_row(ui, tl!("Blade depth"), LABEL_W, |ui| {
            if let Some(v) = widgets::spin_plain(ui, ("cutting", op, "depth"), get("depth", 0.0), " (0: machine's)", 0, 72.0, 1.0, 0.0, &[]) {
                set_optional(app, "depth", v, 10.0);
            }
        });
        widgets::label_row(ui, tl!("Accel."), LABEL_W, |ui| {
            if let Some(v) = widgets::spin_plain(ui, ("cutting", op, "accel"), get("accel", 0.0), " (0: machine's)", 0, 72.0, 1.0, 0.0, &[]) {
                set_optional(app, "accel", v, 3.0);
            }
        });
        widgets::label_row(ui, tl!("Corners"), LABEL_W, |ui| {
            if let Some(v) = widgets::spin_plain(ui, ("cutting", op, "corners"), get("cornerOvercutMm", 0.0), " mm past", 1, 72.0, 0.1, 0.0, &[]) {
                set_tool(app, "cornerOvercutMm", (v.clamp(0.0, 2.0) * 10.0).round() / 10.0);
            }
        });
        // The machine turns the blade round corners by this; the usual blade's until set.
        let offset = tool["bladeOffsetMm"].as_f64().unwrap_or(silhouette::BLADE_OFFSET_MM);
        widgets::label_row(ui, tl!("Blade offset"), LABEL_W, |ui| {
            if let Some(v) = widgets::spin_plain(ui, ("cutting", op, "blade-offset"), offset, " mm", 2, 64.0, 0.05, 0.0, &[]) {
                set_tool(app, "bladeOffsetMm", (v.clamp(0.0, 2.0) * 20.0).round() / 20.0);
            }
        });
    }
}

/// The machines last found: (id, model), and the one chosen.
#[derive(Clone, Debug, Default)]
struct Machines {
    found: Option<Vec<(String, String)>>,
    chosen: usize,
    /// What the last Send said: its warnings, or why it didn't start.
    note: Option<(bool, String)>,
}

fn find_machines(app: &mut VectorcraftApp) -> Vec<(String, String)> {
    let r = app.session.execute("cutting.devices", &json!({})).unwrap_or_default();
    r["devices"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let model = d["model"].as_str()?;
                    // A machine that isn't there says so wherever it is named.
                    let label = if d["simulated"] == true { format!("{model} {}", tl!("(simulated)")) } else { model.to_string() };
                    Some((d["id"].as_str()?.to_string(), label))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The machine to send to, sending and its progress (C4.5). The machines are looked for once and
/// again on Find Cutters, not each frame (that asks the system for its USB devices).
fn cutter_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut m: Machines = super::pstate(&ctx, "cutting-machines");
    let found = match m.found.clone() {
        Some(f) => f,
        None => {
            let f = find_machines(app);
            m.found = Some(f.clone());
            f
        }
    };
    let status = app.session.execute("cutting.status", &json!({})).unwrap_or_default();
    let sending = status["sending"].as_bool() == Some(true);
    let waiting = status["waitingFor"].as_str();
    match found.len() {
        0 => {
            widgets::dim_label(ui, tl!("No cutter is connected."));
        }
        1 => {
            ui.label(found.first().map(|(_, model)| model.as_str()).unwrap_or_default());
        }
        _ => {
            let names: Vec<&str> = found.iter().map(|(_, model)| model.as_str()).collect();
            let current = names.get(m.chosen).copied().unwrap_or_default();
            if let Some(i) = widgets::dropdown_names(ui, ("cutting", "machine"), current, &names, 170.0) {
                m.chosen = i;
            }
        }
    }
    let has_job = crate::cutting::job(app, &ctx).is_some_and(|j| j.as_ref().as_ref().is_ok_and(|made| !made.job.paths.is_empty()));
    ui.horizontal(|ui| {
        if ui.add_enabled(!found.is_empty() && has_job && !sending, egui::Button::new(tl!("Send to Cutter"))).clicked() {
            let device = found.get(m.chosen).or(found.first()).map(|(id, _)| id.clone()).unwrap_or_default();
            m.note = Some(match app.run("cutting.send", json!({ "device": device })) {
                Ok(r) => {
                    (false, r["warnings"].as_array().map(|w| w.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default())
                }
                Err(e) => (true, e),
            });
        }
        if waiting.is_some() && ui.button(tl!("Continue")).clicked() {
            let _ = app.run("cutting.continue", json!({}));
        }
        if sending && ui.button(tl!("Cancel")).clicked() {
            let _ = app.run("cutting.cancel", json!({}));
        }
        if !sending && ui.button(tl!("Find Cutters")).clicked() {
            m.found = None;
            m.chosen = 0;
        }
    });
    if let Some(group) = waiting {
        let text = format!("{} {group}. {}", tl!("Paused before"), tl!("Change the tool or material, then Continue."));
        ui.label(egui::RichText::new(text).color(t.text));
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    } else if sending {
        let (sent, all) = (status["chunksSent"].as_u64().unwrap_or(0), status["chunks"].as_u64().unwrap_or(0));
        widgets::dim_label(ui, &format!("{} {sent} / {all}", tl!("Sending:")));
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    } else if let Some(outcome) = status["outcome"].as_str() {
        let (text, bad) = match outcome {
            "done" => (tl!("Sent.").to_string(), false),
            "cancelled" => (tl!("Cancelled.").to_string(), false),
            other => (other.to_string(), true),
        };
        ui.label(egui::RichText::new(text).color(if bad { t.bleed } else { t.text_dim }));
    }
    if let Some((bad, note)) = &m.note
        && !note.is_empty()
    {
        ui.label(egui::RichText::new(note).color(if *bad { t.bleed } else { t.text_dim }));
    }
    super::set_pstate(&ctx, "cutting-machines", m);
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    settings_rows(app, ui, &info);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Selection"));
    selection_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Tool Settings"));
    tool_rows(app, ui, &info);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Job"));
    job_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Cutter"));
    cutter_rows(app, ui);
    widgets::divider(ui);
    let view = app.ui.view.cutting;
    if widgets::check(ui, tl!("Cutting View"), view, true) {
        let _ = app.run("view.cutting", json!({}));
    }
    let paths = crate::cutting::shows_toolpaths(ui.ctx());
    if widgets::check(ui, tl!("Show Toolpaths"), paths, view) {
        crate::cutting::set_shows_toolpaths(ui.ctx(), !paths);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_draws_headless_with_and_without_a_job() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        let frame = |app: &mut VectorcraftApp| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(app, ui));
            out.textures_delta.clear();
        };
        frame(&mut app);
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        frame(&mut app);
        app.run("cutting.settings.set", json!({"mat": "custom", "offsetMm": [300, 0]})).unwrap();
        frame(&mut app);
        assert!(crate::cutting::job(&app, &ctx).is_some_and(|j| j.is_err()), "off the mat: no job");
    }

    fn panel_texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 2400.0))), ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| show(app, ui));
        out.textures_delta.clear();
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    /// C4.8 and C5.3: a simulated cutter is named as one, and a send cut by layer that has paused
    /// before the second says so and offers Continue. The only test here that installs machines.
    #[test]
    fn a_paused_send_offers_continue() {
        use std::sync::Arc;
        use std::time::Duration;
        use vectorcraft_cutter::simulated::{Behaviour, SIMULATED_ID_PREFIX, Sent, SimulatedMachine};
        let quick = SimulatedMachine {
            info: vectorcraft_cutter::DeviceInfo {
                id: format!("{SIMULATED_ID_PREFIX}quick"),
                vendor_id: vectorcraft_cutter_silhouette::VENDOR_ID,
                product_id: 0x1137,
                name: "Simulated cutter".into(),
            },
            behaviour: Behaviour {
                query: vec![0x1B, 0x05],
                ready: b"0".to_vec(),
                moving: b"1".to_vec(),
                busy_per_write: Duration::ZERO,
                busy_per_byte: Duration::ZERO,
            },
            sent: Sent::default(),
        };
        vectorcraft_cutter::registry::install(Some(Arc::new(quick)));
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        app.run("layer.new", json!({"name": "Gold"})).unwrap();
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
        app.run("cutting.settings.set", json!({"grouping": "layer"})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Cameo 4 (simulated)"), "{texts:?}");
        app.run("cutting.send", json!({})).unwrap();
        for _ in 0..500 {
            if app.session.execute("cutting.status", &json!({})).unwrap()["waitingFor"] == "Gold" {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t.starts_with("Paused before Gold.")), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Continue") && texts.iter().any(|t| t == "Cancel"), "{texts:?}");
        app.run("cutting.cancel", json!({})).unwrap();
        vectorcraft_cutter::registry::install(None);
    }

    /// C5.3: the Groups choice shows what the document cuts by.
    #[test]
    fn the_panel_shows_the_grouping() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("cutting.settings.set", json!({"grouping": "color"})).unwrap();
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 1600.0))), ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| show(&mut app, ui));
        out.textures_delta.clear();
        let texts: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "Groups") && texts.iter().any(|t| t == "By Colour"), "{texts:?}");
    }

    /// Window → Workspace → Cutting is the cutting mode: the mat shows and the Cutting panel opens;
    /// another workspace hides the mat.
    #[test]
    fn the_cutting_workspace_is_the_cutting_mode() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        assert!(!app.ui.view.cutting);
        crate::workspaces::switch(&mut app.ui, crate::workspaces::CUTTING).unwrap();
        assert!(app.ui.view.cutting);
        assert_eq!(app.ui.open_panel.as_deref(), Some(crate::panels::cutting::ID));
        crate::workspaces::switch(&mut app.ui, crate::workspaces::ESSENTIALS).unwrap();
        assert!(!app.ui.view.cutting);
    }
}
