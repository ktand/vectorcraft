//! Fork: the panels that add cut lines to cut the art out (C12.2, one panel each since C14.9): the
//! Contour panel, a contour round the art (C11.1); the Stencil panel, stencil bridges (C11.5); and
//! the Weeding panel, weeding's box and lines (C8.2, C8.3). The designs made from the art (sketch
//! fill, rhinestones, puzzles) are in [`super::patterns`].

use egui::Ui;
use serde_json::{Value, json};

use super::{LABEL_W, mm_field, set};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

/// The Contour panel: the id is the one Cut Lines had, so a saved workspace still opens it.
pub const ID: &str = "cuttingLines";
pub const STENCIL_ID: &str = "cuttingStencil";
pub const WEEDING_ID: &str = "cuttingWeeding";

/// Weeding (C8.2, C8.3): a box round the design and lines from it in to the design, cut by the
/// job (not added to the art), so vinyl's waste comes away easily.
fn weeding_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let s = &info["settings"];
    super::intro(
        ui,
        tl!("For vinyl: extra cuts that make the waste round the design easy to peel away. They are added to the cut, not to your art."),
    );
    let boxed = s["weedBox"].as_bool() == Some(true);
    if super::check_hint(ui, "Weed Box", boxed, "A box cut round the design, Margin mm out.") {
        set(app, json!({ "weedBox": !boxed }));
    }
    if boxed {
        let margin = s["weedMarginMm"].as_f64().unwrap_or(3.0);
        widgets::label_row(ui, tl!("Margin"), LABEL_W, |ui| {
            if let Some(v) = mm_field(ui, "weed-margin", margin, 0.0) {
                set(app, json!({ "weedMarginMm": v.clamp(0.0, 50.0) }));
            }
        });
    }
    let lines = s["weedLines"].as_bool() == Some(true);
    if super::check_hint(
        ui,
        "Weed Lines",
        lines,
        "Lines from the box in towards the design, stopping just short of it, so the waste comes off in strips.",
    ) {
        set(app, json!({ "weedLines": !lines }));
    }
    if lines {
        // C18.24: smart lines through the design's gaps (the default), or evenly spaced.
        let smart = s["weedLinesSmart"].as_bool() != Some(false);
        ui.indent(("cutting", "weed-lines"), |ui| {
            if super::check_hint(
                ui,
                "Smart Placement",
                smart,
                "Lines right across the waste through the gaps between the design's pieces, splitting it into strips no larger than Strip Size; off: evenly spaced lines in to the design.",
            ) {
                set(app, json!({ "weedLinesSmart": !smart }));
            }
            // C18.26: also a line through every gap between pieces, such as between letters.
            let between = s["weedLinesBetween"].as_bool() != Some(false);
            if smart
                && super::check_hint(
                    ui,
                    "Between Pieces",
                    between,
                    "Also a line through every gap between the design's pieces, such as between letters, however small the strips already are.",
                )
            {
                set(app, json!({ "weedLinesBetween": !between }));
            }
        });
        let spacing = s["weedLineSpacingMm"].as_f64().unwrap_or(0.0);
        let label = if smart { tl!("Strip Size") } else { tl!("Spacing") };
        widgets::label_row(ui, label, LABEL_W, |ui| {
            if let Some(v) = mm_field(ui, "weed-spacing", spacing, 0.0) {
                // C18.24: no strip is narrower than the smallest size; 0 stays 0 (50 mm).
                let least = vectorcraft_cutting::weed::SMALLEST_STRIP_MM;
                let v = if smart && v > 0.0 { v.max(least) } else { v };
                set(app, json!({ "weedLineSpacingMm": v.clamp(0.0, 200.0) }));
            }
        });
        if smart {
            widgets::dim_label(ui, tl!("Strip Size 10–200 mm, or 0 for 50 mm. Weeding Map… in Send to Cutter shows the strips."));
        } else {
            widgets::dim_label(ui, tl!("Spacing 0: one line in from the middle of each side."));
        }
    }
}

/// What the Contour section keeps between frames: the distance and holes asked for, and what the
/// last Add Contour said.
#[derive(Clone, Debug)]
struct ContourChoice {
    offset_mm: f64,
    holes: bool,
    note: Option<(bool, String)>,
}

impl Default for ContourChoice {
    fn default() -> Self {
        ContourChoice { offset_mm: vectorcraft_cutting::contour::DEFAULT_OFFSET_MM, holes: false, note: None }
    }
}

/// A cut line round the selected art (C11.1): how far out, whether holes are kept, and Add
/// Contour (`cutting.contour`, one undo step; the art set to No Cut).
fn contour_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: ContourChoice = crate::panels::pstate(&ctx, "cutting-contour");
    super::intro(ui, tl!("A cut line round the selected art, for stickers and printed shapes."));
    widgets::label_row(ui, tl!("Offset"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "contour-offset", c.offset_mm, 0.0) {
            c.offset_mm = v.clamp(0.0, vectorcraft_cutting::contour::MAX_OFFSET_MM);
        }
    });
    if super::check_hint(ui, "Keep Holes", c.holes, "Also cut round the holes inside the art. Off: only the outside is cut.") {
        c.holes = !c.holes;
    }
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    if ui
        .add_enabled(selected, egui::Button::new(tl!("Add Contour")))
        .on_hover_text(tl!("A cut line round the selected art; the art itself is set to No Cut."))
        .clicked()
    {
        c.note = Some(match app.run("cutting.contour", json!({ "offsetMm": c.offset_mm, "holes": c.holes })) {
            Ok(r) => (false, format!("{} {}", r["pieces"], tl!("contour lines on the Cut Contour layer"))),
            Err(e) => (true, e),
        });
    }
    super::select_hint(ui, selected, "Select the art first.");
    // C18.18: the sticker sheet under a heading of its own, saying what it does.
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Sticker Sheet"));
    widgets::dim_label(
        ui,
        tl!(
            "In one step: Print & Cut on, a kiss cut round each selected design at the Offset above, a print bleed, and a stronger cut round the whole sheet."
        ),
    );
    // C18.5: the contour, a print bleed, Print & Cut and a cut round the sheet, in one step.
    if ui
        .add_enabled(selected, egui::Button::new(tl!("Make Sticker Sheet")))
        .on_hover_text(tl!(
            "Print & Cut on, a kiss cut round each selected design at the Offset, a print bleed, and a stronger cut round the whole sheet, cut last."
        ))
        .clicked()
    {
        c.note = Some(match app.run("cutting.stickers", json!({ "offsetMm": c.offset_mm.min(20.0), "sheetCut": true })) {
            Ok(r) => (false, format!("{} {}", r["pieces"], tl!("stickers, and a cut round the sheet"))),
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-contour", c);
}

/// What the Stencil section keeps between frames: the bridges' width and direction, and what the
/// last Add Bridges said.
#[derive(Clone, Debug)]
struct BridgeChoice {
    width_mm: f64,
    horizontal: bool,
    note: Option<(bool, String)>,
}

impl Default for BridgeChoice {
    fn default() -> Self {
        BridgeChoice { width_mm: vectorcraft_cutting::bridges::DEFAULT_WIDTH_MM, horizontal: false, note: None }
    }
}

/// Stencil bridges for the selected art (C11.5): how wide, which way, and Add Bridges
/// (`cutting.bridges`, one undo step; the art set to No Cut and hidden under the stencil).
fn bridge_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: BridgeChoice = crate::panels::pstate(&ctx, "cutting-bridges");
    super::intro(ui, tl!("Narrow bridges that keep the middles of letters (the inside of an O) joined to a stencil sheet."));
    let range = vectorcraft_cutting::bridges::WIDTH_MM;
    widgets::label_row(ui, tl!("Bridge Width"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "bridge-width", c.width_mm, *range.start()) {
            c.width_mm = v.clamp(*range.start(), *range.end());
        }
    });
    let labels = ["Vertical", "Horizontal"];
    widgets::label_row(ui, tl!("Direction"), LABEL_W, |ui| {
        let shown = if c.horizontal { labels[1] } else { labels[0] };
        if let Some(i) = widgets::dropdown(ui, ("cutting", "bridge-direction"), shown, &labels, 110.0) {
            c.horizontal = i == 1;
        }
    });
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    if ui
        .add_enabled(selected, egui::Button::new(tl!("Add Bridges")))
        .on_hover_text(tl!("Joins each hole's middle (the inside of an O) to the art round it, so a stencil keeps it. The stencil is added over the art, which is hidden and set to No Cut."))
        .clicked()
    {
        let direction = if c.horizontal { "horizontal" } else { "vertical" };
        c.note = Some(match app.run("cutting.bridges", json!({ "widthMm": c.width_mm, "direction": direction })) {
            Ok(r) if r["unbridged"].as_u64().unwrap_or(0) > 0 => (true, format!("{} {} · {} {}", r["bridges"], tl!("bridges"), r["unbridged"], tl!("holes missed: try the other direction"))),
            Ok(r) => (false, format!("{} {}", r["bridges"], tl!("bridges"))),
            Err(e) => (true, e),
        });
    }
    super::select_hint(ui, selected, "Select the art first.");
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-bridges", c);
}

/// The Contour panel.
pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if super::has_document(app, ui) {
        contour_rows(app, ui);
    }
}

/// The Stencil panel.
pub fn show_stencil(app: &mut VectorcraftApp, ui: &mut Ui) {
    if super::has_document(app, ui) {
        bridge_rows(app, ui);
    }
}

/// The Weeding panel.
pub fn show_weeding(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    weeding_rows(app, ui, &info);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel_texts(app: &mut VectorcraftApp, ctx: &egui::Context, show: fn(&mut VectorcraftApp, &mut Ui)) -> Vec<String> {
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

    /// C8.2, C8.3: the weeding switches, their numbers shown once they are on.
    #[test]
    fn the_panel_switches_weeding_on() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let texts = panel_texts(&mut app, &ctx, show_weeding);
        assert!(texts.iter().any(|t| t == "Weed Box") && texts.iter().any(|t| t == "Weed Lines"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Margin" || t == "Spacing"), "off: no numbers");
        app.run("cutting.settings.set", json!({"weedBox": true, "weedLines": true, "weedMarginMm": 5})).unwrap();
        let texts = panel_texts(&mut app, &ctx, show_weeding);
        // C18.24: smart placement at first, its strip size; off, the even spacing.
        assert!(
            texts.iter().any(|t| t == "Margin")
                && texts.iter().any(|t| t == "Strip Size")
                && texts.iter().any(|t| t == "Smart Placement")
                && texts.iter().any(|t| t == "Between Pieces"),
            "{texts:?}"
        );
        app.run("cutting.settings.set", json!({"weedLinesSmart": false})).unwrap();
        let texts = panel_texts(&mut app, &ctx, show_weeding);
        assert!(texts.iter().any(|t| t == "Spacing") && !texts.iter().any(|t| t == "Strip Size" || t == "Between Pieces"), "{texts:?}");
        assert!(app.run("cutting.settings.set", json!({"weedMarginMm": 80})).is_err(), "out of range");
    }

    /// C11.1: the Contour section; Add Contour works on the selection.
    #[test]
    fn the_panel_adds_a_contour() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let texts = panel_texts(&mut app, &ctx, show);
        assert!(texts.iter().any(|t| t == "Add Contour") && texts.iter().any(|t| t == "Keep Holes"), "{texts:?}");
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        app.run("cutting.contour", json!({"offsetMm": 2})).unwrap();
        assert!(app.session.active().unwrap().doc.layers.iter().any(|l| l.name.as_deref() == Some("Cut Contour")));
    }

    /// C11.5: the Stencil section; Add Bridges works on the selection.
    #[test]
    fn the_panel_adds_bridges() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let texts = panel_texts(&mut app, &ctx, show_stencil);
        assert!(texts.iter().any(|t| t == "Add Bridges") && texts.iter().any(|t| t == "Vertical"), "{texts:?}");
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let r = app.run("cutting.bridges", json!({})).unwrap();
        assert_eq!(r["bridges"], json!(0), "a square has no hole");
    }
}
