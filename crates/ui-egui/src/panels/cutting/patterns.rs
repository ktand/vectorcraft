//! Fork: the Patterns panel (C14.0, taken out of Cut Lines): designs made from the selected art
//! or the artboard for the machine to draw or cut: a sketch fill for the pen (C13.3, patterns
//! C14.5), a rhinestone template (C13.4, stone sizes and a radial fill C14.5) and a puzzle (C13.5).

use egui::Ui;
use serde_json::json;
use vectorcraft_cutting::hatch::Pattern;
use vectorcraft_cutting::stones::Fill;

use super::{LABEL_W, mm_field};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "cuttingPatterns";

/// What the Sketch Fill section keeps between frames: the pattern, spacing, angle and
/// cross-hatching asked for, and what the last Add Sketch Fill said.
#[derive(Clone, Debug)]
struct SketchChoice {
    pattern: Pattern,
    spacing_mm: f64,
    angle_deg: f64,
    cross: bool,
    note: Option<(bool, String)>,
}

impl Default for SketchChoice {
    fn default() -> Self {
        use vectorcraft_cutting::hatch::{DEFAULT_ANGLE_DEG, DEFAULT_SPACING_MM};
        SketchChoice { pattern: Pattern::Lines, spacing_mm: DEFAULT_SPACING_MM, angle_deg: DEFAULT_ANGLE_DEG, cross: false, note: None }
    }
}

/// Sketch fill for the selected art (C13.3): lines across it for a pen, rings or a spiral, how far
/// apart, and for lines at what angle, crossed or not (`cutting.sketchFill`, one undo step).
fn sketch_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: SketchChoice = crate::panels::pstate(&ctx, "cutting-sketch");
    let patterns = [Pattern::Lines, Pattern::Concentric, Pattern::Spiral];
    let labels = ["Lines", "Concentric", "Spiral"];
    let shown = patterns.iter().position(|p| *p == c.pattern).unwrap_or(0);
    widgets::label_row(ui, tl!("Pattern"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "sketch-pattern"), labels[shown], &labels, 120.0) {
            c.pattern = patterns.get(i).copied().unwrap_or_default();
        }
    });
    let (lo, hi) = vectorcraft_cutting::hatch::SPACING_MM;
    widgets::label_row(ui, tl!("Spacing"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "sketch-spacing", c.spacing_mm, lo) {
            c.spacing_mm = v.clamp(lo, hi);
        }
    });
    // The angle and crossing are the straight lines' alone.
    if c.pattern == Pattern::Lines {
        widgets::label_row(ui, tl!("Angle"), LABEL_W, |ui| {
            if let Some(v) = widgets::spin_plain(ui, ("cutting", "sketch-angle"), c.angle_deg, "°", 0, 80.0, 15.0, -180.0, &[]) {
                c.angle_deg = v.clamp(-180.0, 180.0);
            }
        });
        if widgets::check(ui, tl!("Cross-Hatch"), c.cross, true) {
            c.cross = !c.cross;
        }
    }
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    if ui
        .add_enabled(selected, egui::Button::new(tl!("Add Sketch Fill")))
        .on_hover_text(tl!("Lines across the selected shapes for a pen to draw. Choose the Pen as the tool in Cut Settings to draw them."))
        .clicked()
    {
        let p = json!({ "pattern": c.pattern.id(), "spacingMm": c.spacing_mm, "angleDeg": c.angle_deg, "cross": c.cross });
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

/// What the Rhinestones section keeps between frames: the hole and gap asked for, what fills the
/// inside, and what the last Add Template said.
#[derive(Clone, Debug)]
struct StoneChoice {
    hole_mm: f64,
    gap_mm: f64,
    fill: Fill,
    note: Option<(bool, String)>,
}

impl Default for StoneChoice {
    fn default() -> Self {
        use vectorcraft_cutting::stones::{DEFAULT_GAP_MM, DEFAULT_HOLE_MM};
        StoneChoice { hole_mm: DEFAULT_HOLE_MM, gap_mm: DEFAULT_GAP_MM, fill: Fill::None, note: None }
    }
}

/// A rhinestone template from the selected art (C13.4): the stone (a size that sets the hole) or
/// the hole itself, the spacing, and what fills the inside (`cutting.rhinestones`, one undo step;
/// the art hidden and set to No Cut).
fn stone_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    use vectorcraft_cutting::stones::{GAP_MM, HOLE_ALLOWANCE_MM, HOLE_MM, STONES};
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: StoneChoice = crate::panels::pstate(&ctx, "cutting-stones");
    // Stone sizes are names (SS6…), shown as they are; any other hole is Custom.
    let mut sizes: Vec<String> = STONES.iter().map(|(id, mm)| format!("{} ({mm} mm)", id.to_uppercase())).collect();
    sizes.push(tl!("Custom").to_string());
    let names: Vec<&str> = sizes.iter().map(String::as_str).collect();
    let shown = STONES.iter().position(|(_, mm)| (mm + HOLE_ALLOWANCE_MM - c.hole_mm).abs() < 1e-6).unwrap_or(STONES.len());
    widgets::label_row(ui, tl!("Stone"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown_names(ui, ("cutting", "stone-size"), names.get(shown).copied().unwrap_or_default(), &names, 140.0)
            && let Some((_, mm)) = STONES.get(i)
        {
            c.hole_mm = mm + HOLE_ALLOWANCE_MM;
        }
    });
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
    let fills = [Fill::None, Fill::Grid, Fill::Radial];
    let fill_labels = ["Outline Only", "Grid", "Radial"];
    let shown = fills.iter().position(|f| *f == c.fill).unwrap_or(0);
    widgets::label_row(ui, tl!("Fill Inside"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "stone-fill"), fill_labels[shown], &fill_labels, 120.0) {
            c.fill = fills.get(i).copied().unwrap_or_default();
        }
    });
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    if ui
        .add_enabled(selected, egui::Button::new(tl!("Add Template")))
        .on_hover_text(tl!("Holes for rhinestones round the selected shapes, a little larger than the stones. The art is hidden and set to No Cut."))
        .clicked()
    {
        let fill = match c.fill {
            Fill::None => "none",
            Fill::Grid => "grid",
            Fill::Radial => "radial",
        };
        c.note = Some(match app.run("cutting.rhinestones", json!({ "holeMm": c.hole_mm, "gapMm": c.gap_mm, "fill": fill })) {
            Ok(r) => (false, format!("{} {}", r["stones"], tl!("holes"))),
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-stones", c);
}

/// What the Puzzle section keeps between frames: its size and shape, the seed, and what the last
/// Add Puzzle said.
#[derive(Clone, Debug)]
struct PuzzleChoice {
    rows: f64,
    cols: f64,
    curvature: bool,
    curve: f64,
    ripple: f64,
    wobble: f64,
    seed: u32,
    note: Option<(bool, String)>,
}

impl Default for PuzzleChoice {
    fn default() -> Self {
        PuzzleChoice { rows: 4.0, cols: 4.0, curvature: true, curve: 100.0, ripple: 100.0, wobble: 100.0, seed: 1, note: None }
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

/// Whether the selection is one puzzle, which Randomize makes again in place.
fn puzzle_selected(app: &VectorcraftApp) -> bool {
    app.session.active().is_some_and(|st| {
        let mut ids = st.selection.objects.iter();
        match (ids.next(), ids.next()) {
            (Some(id), None) => st.doc.node(*id).is_some_and(|n| {
                n.extras().attrs.as_deref().is_some_and(|a| a.foreign.iter().any(|f| f.ns == vectorcraft_cutting::ops::NS && f.name == "puzzle"))
            }),
            _ => false,
        }
    })
}

/// A jigsaw over the selection, or the artboard with nothing selected (C13.5): rows and columns,
/// the knobs' curve, the sides' ripple and the grid's wobble; Randomize makes the selected puzzle
/// again with another seed (`cutting.puzzle`, one undo step).
fn puzzle_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: PuzzleChoice = crate::panels::pstate(&ctx, "cutting-puzzle");
    let (lo, hi) = vectorcraft_cutting::puzzle::ROWS;
    let rows = (f64::from(lo), f64::from(hi));
    count_row(ui, tl!("Rows"), "puzzle-rows", &mut c.rows, "", rows);
    count_row(ui, tl!("Columns"), "puzzle-cols", &mut c.cols, "", rows);
    if widgets::check(ui, tl!("Curvature"), c.curvature, true) {
        c.curvature = !c.curvature;
    }
    if c.curvature {
        count_row(ui, tl!("Curve"), "puzzle-curve", &mut c.curve, " %", (0.0, 100.0));
        count_row(ui, tl!("Ripple"), "puzzle-ripple", &mut c.ripple, " %", (0.0, 100.0));
        count_row(ui, tl!("Wobble"), "puzzle-wobble", &mut c.wobble, " %", (0.0, 100.0));
    }
    let one = puzzle_selected(app);
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    let run = |app: &mut VectorcraftApp, c: &mut PuzzleChoice| {
        // The fields hold whole numbers as f64; the command takes integers.
        let (rows, cols) = (c.rows.round() as u32, c.cols.round() as u32);
        let p =
            json!({ "rows": rows, "cols": cols, "curvature": c.curvature, "curve": c.curve, "ripple": c.ripple, "wobble": c.wobble, "seed": c.seed });
        c.note = Some(match app.run("cutting.puzzle", p) {
            Ok(r) => (false, format!("{} {}", r["pieces"], tl!("pieces"))),
            Err(e) => (true, e),
        });
    };
    let label = if one {
        tl!("Apply to Selected Puzzle")
    } else if selected {
        tl!("Add Puzzle over Selection")
    } else {
        tl!("Add Puzzle over Artboard")
    };
    if ui.button(label).on_hover_text(tl!("A jigsaw's cut lines on top of the art, which stays as it is to be printed.")).clicked() {
        run(app, &mut c);
    }
    if ui.add_enabled(one, egui::Button::new(tl!("Randomize Selected Puzzle"))).clicked() {
        // Another seed, the next in a fixed sequence: deterministic, never the same twice running.
        c.seed = c.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 1_000_000;
        run(app, &mut c);
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-puzzle", c);
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if app.session.active().is_none() {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    }
    widgets::section_header(ui, tl!("Sketch Fill"));
    sketch_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Rhinestones"));
    stone_rows(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Puzzle"));
    puzzle_rows(app, ui);
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
        // C14.5: the pattern; rings have no angle.
        assert!(texts.iter().any(|t| t == "Lines"), "{texts:?}");
        let mut c: SketchChoice = crate::panels::pstate(&ctx, "cutting-sketch");
        c.pattern = Pattern::Concentric;
        crate::panels::set_pstate(&ctx, "cutting-sketch", c);
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Concentric") && !texts.iter().any(|t| t == "Cross-Hatch"), "{texts:?}");
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
        // C14.5: the stone's size, the hole a little larger.
        assert!(texts.iter().any(|t| t == "SS10 (2.8 mm)") && texts.iter().any(|t| t == "Outline Only"), "{texts:?}");
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 100})).unwrap();
        assert!(app.run("cutting.rhinestones", json!({})).unwrap()["stones"].as_u64().unwrap() > 20);
    }

    /// C13.5: the Puzzle section; with nothing selected it goes over the artboard, and with the
    /// puzzle selected it is made again in place.
    #[test]
    fn the_panel_adds_a_puzzle() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        for label in ["Puzzle", "Add Puzzle over Artboard", "Curvature", "Curve", "Ripple", "Wobble", "Randomize Selected Puzzle"] {
            assert!(texts.iter().any(|t| t == label), "{label}: {texts:?}");
        }
        let r = app.run("cutting.puzzle", json!({})).unwrap();
        assert_eq!(r["pieces"], json!(16));
        app.run("select.set", json!({"ids": [r["id"]]})).unwrap();
        assert!(super::puzzle_selected(&app));
        assert!(panel_texts(&mut app, &ctx).iter().any(|t| t == "Apply to Selected Puzzle"));
    }
}
