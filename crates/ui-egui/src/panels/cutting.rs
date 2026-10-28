//! Fork: the Cutting panel (`docs/cutting/ARCHITECTURE.md`): the document's cutting settings
//! (mat, orientation, artboard, offset, default operation, Clip to Mat), the selection's
//! operation, and the cut job those make (counts and lengths, or why there is none). It reads
//! the engine's `cutting.*` queries and acts through `app.run`, as every panel does.

use egui::Ui;
use serde_json::{Value, json};
use vectorcraft_doc::Unit;

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
    let ops = ["Cut", "Score", "No Cut"];
    let op_ids = ["cut", "score", "none"];
    let default = op_ids.iter().position(|o| s["defaultOp"] == *o).unwrap_or(0);
    // The operation of objects that, with their groups and layers, set none.
    widgets::label_row(ui, tl!("Default"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "default"), ops[default], &ops, 120.0) {
            set(app, json!({ "defaultOp": op_ids.get(i).copied().unwrap_or("cut") }));
        }
    });
    let clip = s["clipToMat"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Clip to Mat"), clip, true) {
        set(app, json!({ "clipToMat": !clip }));
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
    for (label, op) in [("Cut", "cut"), ("Score", "score"), ("No Cut", "none")] {
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
    widgets::section_header(ui, tl!("Job"));
    job_rows(app, ui);
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
