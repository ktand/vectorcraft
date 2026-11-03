//! Fork: the Templates panel (C15.1): a template made to size (`cutting.template`): which one,
//! its sizes (inside sizes unless Outside Sizes is on), a flat picture of its net, and Add; with a
//! template alone selected, Apply makes it again in place with the panel's values.

use egui::{Color32, Pos2, Stroke, Ui, Vec2};
use serde_json::{Map, Value, json};
use vectorcraft_cutting::template::{self, KINDS, Kind, Net, Unit};

use super::LABEL_W;
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "cuttingTemplates";

/// What the panel keeps between frames: the template chosen, the values asked for, and what the
/// last Add said.
#[derive(Clone, Debug, Default)]
struct TemplateChoice {
    kind: Option<&'static str>,
    values: Vec<(&'static str, f64)>,
    outside: bool,
    /// The folds perforated instead of scored.
    perforate: bool,
    note: Option<(bool, String)>,
}

impl TemplateChoice {
    fn kind(&self) -> Option<&'static Kind> {
        self.kind.and_then(template::kind).or_else(|| KINDS.first())
    }

    fn value(&self, k: &Kind, key: &str) -> f64 {
        let default = k.params.iter().find(|p| p.key == key).map_or(0.0, |p| p.default);
        self.values.iter().find(|(v, _)| *v == key).map_or(default, |(_, v)| *v)
    }

    fn set(&mut self, key: &'static str, v: f64) {
        match self.values.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = v,
            None => self.values.push((key, v)),
        }
    }
}

/// Whether the selection is one template, which Apply makes again in place.
fn template_selected(app: &VectorcraftApp) -> bool {
    app.session.active().is_some_and(|st| {
        let mut ids = st.selection.objects.iter();
        match (ids.next(), ids.next()) {
            (Some(id), None) => st.doc.node(*id).is_some_and(|n| {
                n.extras().attrs.as_deref().is_some_and(|a| a.foreign.iter().any(|f| f.ns == vectorcraft_cutting::ops::NS && f.name == "template"))
            }),
            _ => false,
        }
    })
}

/// The net drawn flat, fitted into `size`: the outline, the folds dashed, the glue shaded.
fn picture(ui: &mut Ui, net: &Net, size: Vec2) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let (w, h) = net.size();
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let pad = 6.0;
    let scale = ((f64::from(rect.width() - 2.0 * pad) / w).min(f64::from(rect.height() - 2.0 * pad) / h)) as f32;
    let origin = rect.center() - Vec2::new(w as f32 * scale, h as f32 * scale) / 2.0;
    let at = |p: vectorcraft_geom::Point| Pos2::new(origin.x + p.x as f32 * scale, origin.y + p.y as f32 * scale);
    // Each subpath's curves as points (8 to a segment), the closing segment included.
    let points = |sp: &vectorcraft_geom::SubPath| -> Vec<Pos2> {
        use vectorcraft_geom::ParamCurve;
        let mut pts: Vec<Pos2> = sp.anchors.first().map(|a| at(a.p)).into_iter().collect();
        for i in 0..sp.segment_count() {
            let c = sp.segment(i);
            pts.extend((1..=8).map(|k| at(c.eval(f64::from(k) / 8.0))));
        }
        pts
    };
    let painter = ui.painter_at(rect);
    let glue = Color32::from_gray(128).gamma_multiply(0.35);
    for s in &net.glue.subpaths {
        let pts: Vec<Pos2> = s.anchors.iter().map(|a| at(a.p)).collect();
        painter.add(egui::Shape::convex_polygon(pts, glue, Stroke::NONE));
    }
    for s in &net.cut.subpaths {
        painter.add(egui::Shape::line(points(s), Stroke::new(1.0, t.text)));
    }
    let fold = Color32::from_rgb(0, 115, 255);
    for s in &net.folds.subpaths {
        painter.extend(egui::Shape::dashed_line(&points(s), Stroke::new(1.0, fold), 3.0, 2.0));
    }
}

fn template_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: TemplateChoice = crate::panels::pstate(&ctx, "cutting-templates");
    let Some(kind) = c.kind() else { return };
    widgets::label_row(ui, tl!("Template"), LABEL_W, |ui| {
        let chosen = widgets::combo(ui, ("cutting", "template-kind"), tl!(kind.name), 160.0, |ui| {
            let mut chosen = None;
            let mut family = "";
            for k in KINDS {
                if k.family != family {
                    family = k.family;
                    widgets::dim_label(ui, tl!(family));
                }
                if ui.add(egui::Button::selectable(k.id == kind.id, tl!(k.name))).clicked() {
                    chosen = Some(k.id);
                }
            }
            chosen
        });
        if let Some(id) = chosen {
            c.kind = Some(id);
        }
    });
    let Some(kind) = c.kind() else { return };
    // The net first, under the choice of template, so it shows without scrolling; the sizes below
    // it change it on the next frame.
    let values = kind.values(&|k| Some(c.value(kind, k)), None, c.outside);
    match values.and_then(|v| kind.make(&v)) {
        Ok(net) => {
            let (w, h) = net.size();
            picture(ui, &net, Vec2::new(ui.available_width().min(260.0), 110.0));
            widgets::dim_label(ui, &format!("{w:.0} × {h:.0} mm"));
        }
        Err(e) => {
            ui.label(egui::RichText::new(e).color(t.warning));
        }
    }
    for p in kind.params {
        if p.unit == Unit::Toggle {
            let mut on = c.value(kind, p.key) > 0.5;
            if ui.checkbox(&mut on, tl!(p.label)).on_hover_text(tl!(p.help)).changed() {
                c.set(p.key, if on { 1.0 } else { 0.0 });
            }
            continue;
        }
        let (suffix, decimals, step) = match p.unit {
            Unit::Mm => (" mm", if p.max <= 10.0 { 2 } else { 1 }, if p.max <= 10.0 { 0.1 } else { 1.0 }),
            Unit::Deg => ("°", 0, 1.0),
            Unit::Count | Unit::Toggle => ("", 0, 1.0),
        };
        let v = c.value(kind, p.key);
        widgets::label_row(ui, tl!(p.label), LABEL_W, |ui| {
            let r = widgets::spin_plain(ui, ("cutting", "template", p.key), v, suffix, decimals, 80.0, step, p.min, &[]);
            if let Some(n) = r {
                c.set(p.key, n.clamp(p.min, p.max));
            }
        })
    }
    if kind.sized {
        ui.checkbox(&mut c.outside, tl!("Outside Sizes"))
            .on_hover_text(tl!("Off: the sizes are what goes inside; on: the finished size, the material's thickness taken off."));
    }
    widgets::label_row(ui, tl!("Folds"), LABEL_W, |ui| {
        ui.selectable_value(&mut c.perforate, false, tl!("Score"));
        ui.selectable_value(&mut c.perforate, true, tl!("Perforate"));
    });
    let mut params: Map<String, Value> = kind.params.iter().map(|p| (p.key.to_string(), json!(c.value(kind, p.key)))).collect();
    let one = template_selected(app);
    let label = if one { tl!("Apply to Selected Template") } else { tl!("Add Template") };
    let hint = tl!("The net as one group: its outline set to Cut, its folds to Score or Perforate, its glue areas marked.");
    if ui.button(label).on_hover_text(hint).clicked() {
        params.insert("kind".to_string(), json!(kind.id));
        params.insert("outside".to_string(), json!(c.outside && kind.sized));
        params.insert("folds".to_string(), json!(if c.perforate { "perforate" } else { "score" }));
        c.note = Some(match app.run("cutting.template", Value::Object(params)) {
            Ok(r) => {
                let said = format!("{:.0} × {:.0} mm", r["widthMm"].as_f64().unwrap_or(0.0), r["heightMm"].as_f64().unwrap_or(0.0));
                match r["warnings"].as_array().and_then(|w| w.first()).and_then(|w| w.as_str()) {
                    Some(w) => (true, format!("{said}. {w}")),
                    None => (false, said),
                }
            }
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-templates", c);
}

/// The Templates panel.
pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if super::has_document(app, ui) {
        template_rows(app, ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 800.0))), ..Default::default() };
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

    /// C15.1: the template, its sizes and its net's size; Add, then Apply with a template selected.
    #[test]
    fn the_panel_adds_a_template() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        assert!(texts(&mut app, &ctx).iter().any(|t| t == "Open a document to cut it."));
        app.run("file.new", json!({"width": 800, "height": 800})).unwrap();
        let t = texts(&mut app, &ctx);
        for label in
            ["Template", "Tuck End Box", "Length", "Width", "Height", "Thickness", "Outside Sizes", "Folds", "Score", "Perforate", "Add Template"]
        {
            assert!(t.iter().any(|x| x == label), "{label}: {t:?}");
        }
        // The first template, a card-deck tuck box: 17.84 + 2 × 63.3 + 2 × 22.3 across.
        assert!(t.iter().any(|x| x == "189 × 162 mm"), "{t:?}");
        let r = app.run("cutting.template", json!({"kind": "tray"})).unwrap();
        app.run("select.set", json!({"ids": [r["id"]]})).unwrap();
        assert!(template_selected(&app));
        assert!(texts(&mut app, &ctx).iter().any(|x| x == "Apply to Selected Template"));
    }
}
