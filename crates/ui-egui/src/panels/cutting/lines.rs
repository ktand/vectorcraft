//! Fork: the Cut Lines panel (C12.2): what adds cut lines to the design, taken out of Cut
//! Settings: a contour round the art (C11.1), stencil bridges (C11.5), and weeding's box and lines
//! (C8.2, C8.3).

use egui::Ui;
use serde_json::{Value, json};

use super::{LABEL_W, mm_field, set};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "cuttingLines";

/// Weeding (C8.2, C8.3): a box round the design and lines from it in to the design, cut by the
/// job (not added to the art), so vinyl's waste comes away easily.
fn weeding_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let s = &info["settings"];
    let boxed = s["weedBox"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Weed Box"), boxed, true) {
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
    if widgets::check(ui, tl!("Weed Lines"), lines, true) {
        set(app, json!({ "weedLines": !lines }));
    }
    if lines {
        let spacing = s["weedLineSpacingMm"].as_f64().unwrap_or(0.0);
        widgets::label_row(ui, tl!("Every"), LABEL_W, |ui| {
            if let Some(v) = mm_field(ui, "weed-spacing", spacing, 0.0) {
                set(app, json!({ "weedLineSpacingMm": v.clamp(0.0, 200.0) }));
            }
        });
        widgets::dim_label(ui, tl!("0: one line in from the middle of each side."));
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
    widgets::label_row(ui, tl!("Offset"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "contour-offset", c.offset_mm, 0.0) {
            c.offset_mm = v.clamp(0.0, vectorcraft_cutting::contour::MAX_OFFSET_MM);
        }
    });
    if widgets::check(ui, tl!("Keep Holes"), c.holes, true) {
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
    let range = vectorcraft_cutting::bridges::WIDTH_MM;
    widgets::label_row(ui, tl!("Bridges"), LABEL_W, |ui| {
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
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-bridges", c);
}

/// What the Sketch Fill section keeps between frames: the spacing, angle and cross-hatching asked
/// for, and what the last Add Sketch Fill said.
#[derive(Clone, Debug)]
struct SketchChoice {
    spacing_mm: f64,
    angle_deg: f64,
    cross: bool,
    note: Option<(bool, String)>,
}

impl Default for SketchChoice {
    fn default() -> Self {
        use vectorcraft_cutting::hatch::{DEFAULT_ANGLE_DEG, DEFAULT_SPACING_MM};
        SketchChoice { spacing_mm: DEFAULT_SPACING_MM, angle_deg: DEFAULT_ANGLE_DEG, cross: false, note: None }
    }
}

/// Sketch fill for the selected art (C13.3): lines across it for a pen, how far apart, at what
/// angle, crossed or not (`cutting.sketchFill`, one undo step).
fn sketch_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: SketchChoice = crate::panels::pstate(&ctx, "cutting-sketch");
    let (lo, hi) = vectorcraft_cutting::hatch::SPACING_MM;
    widgets::label_row(ui, tl!("Spacing"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "sketch-spacing", c.spacing_mm, lo) {
            c.spacing_mm = v.clamp(lo, hi);
        }
    });
    widgets::label_row(ui, tl!("Angle"), LABEL_W, |ui| {
        if let Some(v) = widgets::spin_plain(ui, ("cutting", "sketch-angle"), c.angle_deg, "°", 0, 80.0, 15.0, -180.0, &[]) {
            c.angle_deg = v.clamp(-180.0, 180.0);
        }
    });
    if widgets::check(ui, tl!("Cross-Hatch"), c.cross, true) {
        c.cross = !c.cross;
    }
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    if ui
        .add_enabled(selected, egui::Button::new(tl!("Add Sketch Fill")))
        .on_hover_text(tl!("Lines across the selected shapes for a pen to draw. Choose the Pen as the tool in Cut Settings to draw them."))
        .clicked()
    {
        let p = json!({ "spacingMm": c.spacing_mm, "angleDeg": c.angle_deg, "cross": c.cross });
        c.note = Some(match app.run("cutting.sketchFill", p) {
            Ok(r) => (false, format!("{} {}", r["lines"], tl!("lines added"))),
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-sketch", c);
}

/// What the Rhinestones section keeps between frames: the hole and gap asked for, whether the
/// inside is filled, and what the last Add Template said.
#[derive(Clone, Debug)]
struct StoneChoice {
    hole_mm: f64,
    gap_mm: f64,
    fill: bool,
    note: Option<(bool, String)>,
}

impl Default for StoneChoice {
    fn default() -> Self {
        use vectorcraft_cutting::stones::{DEFAULT_GAP_MM, DEFAULT_HOLE_MM};
        StoneChoice { hole_mm: DEFAULT_HOLE_MM, gap_mm: DEFAULT_GAP_MM, fill: false, note: None }
    }
}

/// A rhinestone template from the selected art (C13.4): the holes' size and spacing, filled or
/// not (`cutting.rhinestones`, one undo step; the art hidden and set to No Cut).
fn stone_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    use vectorcraft_cutting::stones::{GAP_MM, HOLE_MM};
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: StoneChoice = crate::panels::pstate(&ctx, "cutting-stones");
    widgets::label_row(ui, tl!("Hole"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "stone-hole", c.hole_mm, HOLE_MM.0) {
            c.hole_mm = v.clamp(HOLE_MM.0, HOLE_MM.1);
        }
    });
    widgets::label_row(ui, tl!("Gap"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "stone-gap", c.gap_mm, GAP_MM.0) {
            c.gap_mm = v.clamp(GAP_MM.0, GAP_MM.1);
        }
    });
    if widgets::check(ui, tl!("Fill Inside"), c.fill, true) {
        c.fill = !c.fill;
    }
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    if ui
        .add_enabled(selected, egui::Button::new(tl!("Add Template")))
        .on_hover_text(tl!("Holes for rhinestones round the selected shapes, a little larger than the stones. The art is hidden and set to No Cut."))
        .clicked()
    {
        c.note = Some(match app.run("cutting.rhinestones", json!({ "holeMm": c.hole_mm, "gapMm": c.gap_mm, "fill": c.fill })) {
            Ok(r) => (false, format!("{} {}", r["stones"], tl!("holes"))),
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-stones", c);
}

/// What the Puzzle section keeps between frames: its size, tabs, variety and seed, and what the
/// last Add Puzzle said.
#[derive(Clone, Debug)]
struct PuzzleChoice {
    rows: f64,
    cols: f64,
    tab_pct: f64,
    variety_pct: f64,
    seed: f64,
    note: Option<(bool, String)>,
}

impl Default for PuzzleChoice {
    fn default() -> Self {
        PuzzleChoice { rows: 4.0, cols: 5.0, tab_pct: vectorcraft_cutting::puzzle::DEFAULT_TAB_PCT, variety_pct: 50.0, seed: 1.0, note: None }
    }
}

/// A whole-number field in the Puzzle section, `lo` to `hi`.
fn count_row(ui: &mut Ui, label: &str, id: &'static str, v: &mut f64, suffix: &str, (lo, hi): (f64, f64)) {
    widgets::label_row(ui, label, LABEL_W, |ui| {
        if let Some(n) = widgets::spin_plain(ui, ("cutting", id), *v, suffix, 0, 60.0, 1.0, lo, &[]) {
            *v = n.round().clamp(lo, hi);
        }
    });
}

/// A jigsaw over the selection, or the artboard with nothing selected (C13.5): rows and columns,
/// the tabs' size, how much they vary, and which puzzle (`cutting.puzzle`, one undo step).
fn puzzle_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    use vectorcraft_cutting::puzzle::{ROWS, TAB_PCT};
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: PuzzleChoice = crate::panels::pstate(&ctx, "cutting-puzzle");
    let rows = (f64::from(ROWS.0), f64::from(ROWS.1));
    count_row(ui, tl!("Rows"), "puzzle-rows", &mut c.rows, "", rows);
    count_row(ui, tl!("Columns"), "puzzle-cols", &mut c.cols, "", rows);
    count_row(ui, tl!("Tabs"), "puzzle-tabs", &mut c.tab_pct, " %", TAB_PCT);
    count_row(ui, tl!("Variety"), "puzzle-variety", &mut c.variety_pct, " %", (0.0, 100.0));
    count_row(ui, tl!("Seed"), "puzzle-seed", &mut c.seed, "", (0.0, 1_000_000.0));
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    let label = if selected { tl!("Add Puzzle over Selection") } else { tl!("Add Puzzle over Artboard") };
    if ui
        .button(label)
        .on_hover_text(tl!("A jigsaw's cut lines on top of the art, which stays as it is to be printed. Another seed makes another puzzle."))
        .clicked()
    {
        let p = json!({ "rows": c.rows, "cols": c.cols, "tabPct": c.tab_pct, "variety": c.variety_pct / 100.0, "seed": c.seed });
        c.note = Some(match app.run("cutting.puzzle", p) {
            Ok(r) => (false, format!("{} {}", r["pieces"], tl!("pieces"))),
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-puzzle", c);
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    widgets::section_header(ui, tl!("Contour"));
    contour_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Stencil"));
    bridge_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Sketch Fill"));
    sketch_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Rhinestones"));
    stone_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Puzzle"));
    puzzle_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Weeding"));
    weeding_rows(app, ui, &info);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel_texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
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
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Weed Box") && texts.iter().any(|t| t == "Weed Lines"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Margin" || t == "Every"), "off: no numbers");
        app.run("cutting.settings.set", json!({"weedBox": true, "weedLines": true, "weedMarginMm": 5})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Margin") && texts.iter().any(|t| t == "Every"), "{texts:?}");
        assert!(app.run("cutting.settings.set", json!({"weedMarginMm": 80})).is_err(), "out of range");
    }

    /// C11.1: the Contour section; Add Contour works on the selection.
    #[test]
    fn the_panel_adds_a_contour() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(
            texts.iter().any(|t| t == "Contour") && texts.iter().any(|t| t == "Add Contour") && texts.iter().any(|t| t == "Keep Holes"),
            "{texts:?}"
        );
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
        let texts = panel_texts(&mut app, &ctx);
        assert!(
            texts.iter().any(|t| t == "Stencil") && texts.iter().any(|t| t == "Add Bridges") && texts.iter().any(|t| t == "Vertical"),
            "{texts:?}"
        );
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let r = app.run("cutting.bridges", json!({})).unwrap();
        assert_eq!(r["bridges"], json!(0), "a square has no hole");
    }

    /// C13.3: the Sketch Fill section; its button adds lines to the selection.
    #[test]
    fn the_panel_adds_a_sketch_fill() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(
            texts.iter().any(|t| t == "Sketch Fill") && texts.iter().any(|t| t == "Add Sketch Fill") && texts.iter().any(|t| t == "Cross-Hatch"),
            "{texts:?}"
        );
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 100})).unwrap();
        assert!(app.run("cutting.sketchFill", json!({})).unwrap()["lines"].as_u64().unwrap() > 5);
    }

    /// C13.4: the Rhinestones section; its command makes holes round the selection.
    #[test]
    fn the_panel_adds_a_rhinestone_template() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(
            texts.iter().any(|t| t == "Rhinestones") && texts.iter().any(|t| t == "Add Template") && texts.iter().any(|t| t == "Fill Inside"),
            "{texts:?}"
        );
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 100})).unwrap();
        assert!(app.run("cutting.rhinestones", json!({})).unwrap()["stones"].as_u64().unwrap() > 20);
    }

    /// C13.5: the Puzzle section; with nothing selected it goes over the artboard.
    #[test]
    fn the_panel_adds_a_puzzle() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(
            texts.iter().any(|t| t == "Puzzle") && texts.iter().any(|t| t == "Add Puzzle over Artboard") && texts.iter().any(|t| t == "Seed"),
            "{texts:?}"
        );
        assert_eq!(app.run("cutting.puzzle", json!({})).unwrap()["pieces"], json!(20));
    }
}
