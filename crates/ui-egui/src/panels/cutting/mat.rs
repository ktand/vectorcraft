//! Fork: the Cutting Mat panel (C7.2): what is set once for a document, the mat, where the
//! artboard lies on it and whether it fits, Print & Cut and its registration marks, and how the
//! canvas shows the mat. Cut Settings (the parent module) holds what is set per object and per
//! cut.

use egui::Ui;
use serde_json::{Value, json};

use super::{LABEL_W, mm_field, set};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "cuttingMat";

/// The mat, where the artboard lies on it, and whether it fits.
fn mat_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
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

/// Print & Cut (C6.1): the switch, how far in the registration marks go, and the marks layer
/// added, replaced or removed.
fn print_and_cut_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let s = &info["settings"];
    let on = s["printAndCut"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Print & Cut"), on, true) {
        set(app, json!({ "printAndCut": !on }));
    }
    if !on {
        widgets::dim_label(ui, tl!("Print the artboard with registration marks, then cut around the print."));
        return;
    }
    let inset = s["markInsetMm"].as_f64().unwrap_or(vectorcraft_cutjob::marks::DEFAULT_INSET_MM);
    widgets::label_row(ui, tl!("Marks inset"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "mark-inset", inset, vectorcraft_cutjob::marks::INSET_MM.0) {
            let (lo, hi) = vectorcraft_cutjob::marks::INSET_MM;
            set(app, json!({ "markInsetMm": v.clamp(lo, hi) }));
        }
    });
    let has_marks = app.session.active().is_some_and(|st| vectorcraft_cutting::marks::marks_layer(&st.doc).is_some());
    ui.horizontal(|ui| {
        let label = if has_marks { tl!("Update Marks") } else { tl!("Add Marks") };
        if ui.button(label).clicked() {
            let _ = app.run("cutting.marks.add", json!({}));
        }
        if has_marks && ui.button(tl!("Remove Marks")).clicked() {
            let _ = app.run("cutting.marks.remove", json!({}));
        }
    });
    let note = if has_marks {
        tl!("Print the artboard, then put the sheet on the mat with the square mark at the top left.")
    } else {
        tl!("Add the marks, then print the artboard with them.")
    };
    widgets::dim_label(ui, note);
}

/// What the Nesting section keeps between frames: the gap and turns asked for, and what the last
/// Nest said.
#[derive(Clone, Debug)]
struct NestChoice {
    gap_mm: f64,
    rotate: bool,
    note: Option<(bool, String)>,
}

impl Default for NestChoice {
    fn default() -> Self {
        NestChoice { gap_mm: 2.0, rotate: true, note: None }
    }
}

/// Nesting (C8.5): the pieces placed close together on the mat by their true shapes, the selected
/// ones or all, one undo step (`cutting.nest`).
fn nest_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: NestChoice = crate::panels::pstate(&ctx, "cutting-nest");
    widgets::label_row(ui, tl!("Gap"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "nest-gap", c.gap_mm, 0.0) {
            c.gap_mm = v.clamp(0.0, 50.0);
        }
    });
    if widgets::check(ui, tl!("Turn Pieces"), c.rotate, true) {
        c.rotate = !c.rotate;
    }
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    let label = if selected { "Nest Selected" } else { "Nest All" };
    if ui.button(tl!(label)).clicked() {
        c.note = Some(match app.run("cutting.nest", json!({ "gapMm": c.gap_mm, "rotate": c.rotate })) {
            Ok(r) => {
                let moved = r["moved"].as_array().map_or(0, Vec::len);
                let left = r["unplaced"].as_array().map_or(0, Vec::len);
                let used = r["usedLengthMm"].as_f64().unwrap_or(0.0);
                if left > 0 {
                    (true, format!("{moved} {} {used:.0} mm; {left} {}", tl!("nested in"), tl!("didn't fit and stayed where they were")))
                } else {
                    (false, format!("{moved} {} {used:.0} mm {}", tl!("nested in"), tl!("of the mat")))
                }
            }
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-nest", c);
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    mat_rows(app, ui, &info);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Nesting"));
    nest_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Print & Cut"));
    print_and_cut_rows(app, ui, &info);
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

    fn texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 1600.0))), ..Default::default() };
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

    /// The mat, the artboard on it and the view switches; no cutting settings.
    #[test]
    fn the_panel_shows_the_mat() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        assert!(texts(&mut app, &ctx).iter().any(|t| t == "Open a document to cut it."));
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let t = texts(&mut app, &ctx);
        for label in ["Mat", "Orientation", "Offset", "Clip to Mat", "The artboard fits the mat.", "Cutting View", "Show Toolpaths"] {
            assert!(t.iter().any(|x| x == label), "{label}: {t:?}");
        }
        assert!(!t.iter().any(|x| x == "Force" || x == "Send to Cutter…"), "{t:?}");
    }

    /// C8.5: the Nesting section nests every piece, or the selection when there is one.
    #[test]
    fn the_panel_nests_the_pieces() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 1200, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 1000, "y": 20, "width": 100, "height": 100})).unwrap();
        assert!(texts(&mut app, &ctx).iter().any(|t| t == "Nest Selected"), "the new rectangle is selected");
        app.run("select.none", json!({})).unwrap();
        let t = texts(&mut app, &ctx);
        assert!(t.iter().any(|x| x == "Nest All") && t.iter().any(|x| x == "Turn Pieces") && t.iter().any(|x| x == "Gap"), "{t:?}");
        assert!(app.run("cutting.job", json!({})).is_err(), "off the mat");
        app.run("cutting.nest", json!({})).unwrap();
        assert!(app.run("cutting.job", json!({})).is_ok(), "nested onto it");
    }

    /// C6.1: Print & Cut on, the panel adds the marks and then offers to update or remove them.
    #[test]
    fn print_and_cut_adds_the_marks() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 612, "height": 792})).unwrap();
        assert!(texts(&mut app, &ctx).iter().any(|t| t == "Print & Cut"));
        app.run("cutting.settings.set", json!({"printAndCut": true})).unwrap();
        assert!(texts(&mut app, &ctx).iter().any(|t| t == "Add Marks"));
        app.run("cutting.marks.add", json!({})).unwrap();
        let t = texts(&mut app, &ctx);
        assert!(t.iter().any(|x| x == "Update Marks") && t.iter().any(|x| x == "Remove Marks"), "{t:?}");
    }
}
