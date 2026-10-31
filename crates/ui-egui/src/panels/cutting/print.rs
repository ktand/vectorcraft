//! Fork: the Print & Cut panel (C12.2, taken out of Cutting Mat): the switch, the registration
//! marks' length, thickness and inset (each edge's own behind a fold, C12.1), and the marks layer
//! added, replaced or removed.

use egui::Ui;
use serde_json::{Value, json};

use super::{LABEL_W, mm_field, set};
use crate::VectorcraftApp;
use crate::widgets;

pub const ID: &str = "cuttingPrint";

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
    mark_rows(app, ui, s);
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

/// The registration marks (C12.1): the arms' length, the line width and the inset, one for all
/// edges or, behind Each Edge, each its own; Restore Defaults puts the usual marks back.
fn mark_rows(app: &mut VectorcraftApp, ui: &mut Ui, s: &Value) {
    use vectorcraft_cutjob::marks::{DEFAULT_INSET_MM, DEFAULT_LENGTH_MM, DEFAULT_LINE_MM, INSET_MM, LENGTH_MM, LINE_MM};
    let num = |key: &str, default: f64| s[key].as_f64().unwrap_or(default);
    let length = num("markLengthMm", DEFAULT_LENGTH_MM);
    widgets::label_row(ui, tl!("Mark length"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "mark-length", length, LENGTH_MM.0) {
            set(app, json!({ "markLengthMm": v.clamp(LENGTH_MM.0, LENGTH_MM.1) }));
        }
    });
    let line = num("markLineMm", DEFAULT_LINE_MM);
    widgets::label_row(ui, tl!("Thickness"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "mark-line", line, LINE_MM.0) {
            set(app, json!({ "markLineMm": v.clamp(LINE_MM.0, LINE_MM.1) }));
        }
    });
    let inset = num("markInsetMm", DEFAULT_INSET_MM);
    let each: Option<Vec<f64>> = s["markInsetsMm"].as_array().map(|a| a.iter().map(|v| v.as_f64().unwrap_or(inset)).collect());
    if each.is_none() {
        widgets::label_row(ui, tl!("Inset"), LABEL_W, |ui| {
            if let Some(v) = mm_field(ui, "mark-inset", inset, INSET_MM.0) {
                set(app, json!({ "markInsetMm": v.clamp(INSET_MM.0, INSET_MM.1) }));
            }
        });
    }
    if super::fold(ui, "cutting-mark-edges", tl!("Each Edge")) {
        let sides = each.clone().unwrap_or_else(|| vec![inset; 4]);
        let names = [tl!("Left"), tl!("Top"), tl!("Right"), tl!("Bottom")];
        let ids = ["mark-inset-left", "mark-inset-top", "mark-inset-right", "mark-inset-bottom"];
        for (k, (name, id)) in names.iter().zip(ids).enumerate() {
            let v = sides.get(k).copied().unwrap_or(inset);
            widgets::label_row(ui, name, LABEL_W, |ui| {
                if let Some(v) = mm_field(ui, id, v, INSET_MM.0) {
                    let mut new = sides.clone();
                    if let Some(slot) = new.get_mut(k) {
                        *slot = v.clamp(INSET_MM.0, INSET_MM.1);
                    }
                    set(app, json!({ "markInsetsMm": new }));
                }
            });
        }
        if each.is_some() && ui.button(tl!("Same on Every Edge")).clicked() {
            set(app, json!({ "markInsetsMm": null }));
        }
    }
    if ui.button(tl!("Restore Defaults")).on_hover_text(tl!("The usual marks: 20 mm long, 0.5 mm thick, 15.88 mm in from each edge.")).clicked() {
        set(app, json!({ "markLengthMm": DEFAULT_LENGTH_MM, "markLineMm": DEFAULT_LINE_MM, "markInsetMm": DEFAULT_INSET_MM, "markInsetsMm": null }));
    }
}

/// Print bleed (C12.3): how far, and the bleed layer added, made again or removed; what the last
/// Add said.
fn bleed_rows(app: &mut VectorcraftApp, ui: &mut Ui, s: &Value) {
    let (lo, hi) = vectorcraft_cutjob::CuttingSettings::BLEED_MM;
    let radius = s["bleedMm"].as_f64().unwrap_or(vectorcraft_cutjob::CuttingSettings::DEFAULT_BLEED_MM);
    widgets::label_row(ui, tl!("Bleed"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "bleed", radius, lo) {
            set(app, json!({ "bleedMm": v.clamp(lo, hi) }));
        }
    });
    let ctx = ui.ctx().clone();
    let mut note: Option<(bool, String)> = crate::panels::pstate(&ctx, "cutting-bleed");
    let has = app.session.active().is_some_and(|st| vectorcraft_cutting::bleed::bleed_layer(&st.doc).is_some());
    ui.horizontal(|ui| {
        let label = if has { tl!("Update Bleed") } else { tl!("Add Bleed") };
        let hover = tl!("Spreads each printed object's outer colour past its edge on a layer at the bottom, so a cut a little off the print still lands in colour.");
        if ui.button(label).on_hover_text(hover).clicked() {
            note = Some(match app.run("cutting.bleed.add", json!({})) {
                Ok(r) => (false, format!("{} {}", r["objects"], tl!("objects bleed"))),
                Err(e) => (true, e),
            });
        }
        if has && ui.button(tl!("Remove Bleed")).clicked() {
            let _ = app.run("cutting.bleed.remove", json!({}));
            note = None;
        }
    });
    if let Some((bad, text)) = &note {
        let t = crate::theme::Tokens::get(ui.ctx());
        ui.label(egui::RichText::new(text).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-bleed", note);
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    print_and_cut_rows(app, ui, &info);
    if info["settings"]["printAndCut"].as_bool() == Some(true) {
        widgets::divider(ui);
        widgets::section_header(ui, tl!("Print Bleed"));
        bleed_rows(app, ui, &info["settings"]);
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

    /// C12.1: the marks' length, thickness and inset; each edge's own behind a fold, and the
    /// defaults back in one click.
    #[test]
    fn the_marks_can_be_set() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 612, "height": 792})).unwrap();
        app.run("cutting.settings.set", json!({"printAndCut": true})).unwrap();
        let t = texts(&mut app, &ctx);
        for label in ["Mark length", "Thickness", "Inset", "Each Edge", "Restore Defaults"] {
            assert!(t.iter().any(|x| x == label), "{label}: {t:?}");
        }
        assert!(!t.iter().any(|x| x == "Left"), "folded");
        app.run("cutting.settings.set", json!({"markInsetsMm": [10, 20, 30, 40]})).unwrap();
        let t = texts(&mut app, &ctx);
        assert!(!t.iter().any(|x| x == "Inset"), "each edge's own: {t:?}");
    }

    /// C12.3: Print Bleed shows with Print & Cut on, and offers Update and Remove once added.
    #[test]
    fn the_panel_adds_a_bleed() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 612, "height": 792})).unwrap();
        assert!(!texts(&mut app, &ctx).iter().any(|x| x == "Add Bleed"), "Print & Cut off");
        app.run("cutting.settings.set", json!({"printAndCut": true})).unwrap();
        let t = texts(&mut app, &ctx);
        assert!(t.iter().any(|x| x == "Print Bleed") && t.iter().any(|x| x == "Add Bleed"), "{t:?}");
        app.run("shape.rectangle", json!({"x": 200, "y": 300, "width": 100, "height": 100})).unwrap();
        app.run("cutting.bleed.add", json!({})).unwrap();
        let t = texts(&mut app, &ctx);
        assert!(t.iter().any(|x| x == "Update Bleed") && t.iter().any(|x| x == "Remove Bleed"), "{t:?}");
    }
}
