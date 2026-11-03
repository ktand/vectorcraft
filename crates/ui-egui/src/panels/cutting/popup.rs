//! Fork: the Pop Up panel (C16.1): the selected design made into a pop-up card
//! (`cutting.popup`): the base's height, the struts' width, the folds' dashes and the card's
//! margin; with a pop-up alone selected, it is made again in place with them.

use egui::Ui;
use serde_json::json;
use vectorcraft_cutting::popup::{BASE_PCT, DASH_MM, MARGIN_MM, Options, STRUT_MM};

use super::LABEL_W;
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "cuttingPopUp";

/// What the panel keeps between frames: the settings asked for, and what the last Make said.
#[derive(Clone, Debug, Default)]
struct PopUpChoice {
    opt: Options,
    note: Option<(bool, String)>,
}

/// A number field shown with `suffix` and `decimals`, stepped by `step`, kept to `lo`–`hi`.
fn number_row(ui: &mut Ui, label: &str, id: &'static str, v: &mut f64, (suffix, decimals, step): (&str, usize, f64), (lo, hi): (f64, f64)) {
    widgets::label_row(ui, label, LABEL_W, |ui| {
        if let Some(n) = widgets::spin_plain(ui, ("cutting", id), *v, suffix, decimals, 80.0, step, lo, &[]) {
            *v = n.clamp(lo, hi);
        }
    });
}

/// Whether the selection is one pop-up, which Make makes again in place.
fn popup_selected(app: &VectorcraftApp) -> bool {
    app.session.active().is_some_and(|st| {
        let mut ids = st.selection.objects.iter();
        match (ids.next(), ids.next()) {
            (Some(id), None) => st.doc.node(*id).is_some_and(|n| {
                n.extras().attrs.as_deref().is_some_and(|a| a.foreign.iter().any(|f| f.ns == vectorcraft_cutting::ops::NS && f.name == "popup"))
            }),
            _ => false,
        }
    })
}

fn popup_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: PopUpChoice = crate::panels::pstate(&ctx, "cutting-popup");
    number_row(ui, tl!("Base Width"), "popup-base", &mut c.opt.base_pct, (" %", 1, 1.0), BASE_PCT);
    number_row(ui, tl!("Strut Width"), "popup-strut", &mut c.opt.strut_mm, (" mm", 2, 0.5), STRUT_MM);
    number_row(ui, tl!("Dash Pitch"), "popup-dash", &mut c.opt.dash_mm, (" mm", 2, 0.25), DASH_MM);
    number_row(ui, tl!("Margin"), "popup-margin", &mut c.opt.margin_mm, (" mm", 1, 1.0), MARGIN_MM);
    let one = popup_selected(app);
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    let label = if one { tl!("Apply to Selected Pop Up") } else { tl!("Make Pop Up") };
    let hint = tl!("A card that folds in half, the design standing up when it is opened at a right angle; the design is left to be printed.");
    if ui.add_enabled(selected, egui::Button::new(label)).on_hover_text(hint).clicked() {
        let o = c.opt;
        let p = json!({ "baseWidthPct": o.base_pct, "strutWidthMm": o.strut_mm, "dashPitchMm": o.dash_mm, "marginMm": o.margin_mm });
        c.note = Some(match app.run("cutting.popup", p) {
            Ok(r) => (false, format!("{} {} · {} {}", r["layers"], tl!("layers"), r["struts"], tl!("struts"))),
            Err(e) => (true, e),
        });
    }
    if !selected {
        widgets::dim_label(ui, tl!("Select the design to pop up."));
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-popup", c);
}

/// The Pop Up panel.
pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if super::has_document(app, ui) {
        popup_rows(app, ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 800.0))), ..Default::default() };
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

    /// C16.1: the settings, Make Pop Up once something is selected, Apply with a pop-up selected.
    #[test]
    fn the_panel_makes_a_pop_up() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        assert!(texts(&mut app, &ctx).iter().any(|t| t == "Open a document to cut it."));
        app.run("file.new", json!({"width": 400, "height": 400})).unwrap();
        let t = texts(&mut app, &ctx);
        for label in ["Base Width", "Strut Width", "Dash Pitch", "Margin", "Make Pop Up", "Select the design to pop up."] {
            assert!(t.iter().any(|x| x == label), "{label}: {t:?}");
        }
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 100, "height": 80})).unwrap();
        assert!(!texts(&mut app, &ctx).iter().any(|x| x == "Select the design to pop up."));
        let r = app.run("cutting.popup", json!({})).unwrap();
        app.run("select.set", json!({"ids": [r["id"]]})).unwrap();
        assert!(popup_selected(&app));
        assert!(texts(&mut app, &ctx).iter().any(|x| x == "Apply to Selected Pop Up"));
    }
}
