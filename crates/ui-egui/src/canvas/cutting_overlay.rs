//! Fork: View › Cutting View (`docs/cutting/ARCHITECTURE.md`): the cutting mat drawn over the art
//! where the document's cutting settings lay the artboard on it. The mat's outline, its cut area
//! (dashed, where it is smaller), a 10 mm grid with every 50 mm stronger, labels in millimetres
//! from the cut area's origin along the top and left edges, and the artboard in a warning colour
//! when the cut area doesn't hold all of it. Grid lines closer than a few pixels are left out.

use vectorcraft_cutjob::RectMm;
use vectorcraft_cutting::placement::{self, MM_PER_PT};

use super::*;

/// The grid's spacing and the stronger lines', mm.
const MINOR_MM: f64 = 10.0;
const MAJOR_MM: f64 = 50.0;
/// Grid lines closer than this on screen are left out, px.
const MIN_GAP_PX: f64 = 6.0;
/// Labels need this much room between them, px.
const LABEL_GAP_PX: f64 = 40.0;
/// How far labels sit outside the mat's edge, px.
const LABEL_OUT_PX: f32 = 4.0;
/// Most lines one grid direction draws (a 3 m mat at 10 mm is 300).
const MAX_LINES: usize = 400;

/// The mm positions `step` apart strictly inside `0..len`.
fn ticks(len: f64, step: f64) -> impl Iterator<Item = f64> {
    let n = if len.is_finite() && step > 0.0 { ((len / step).ceil() as usize).saturating_sub(1).min(MAX_LINES) } else { 0 };
    (1..=n).map(move |i| i as f64 * step)
}

pub(super) fn cutting_overlay(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(st) = app.session.active() else { return };
    let s = vectorcraft_cutting::settings(&st.doc);
    let Some(pl) = placement::place(&s, &st.doc) else { return };
    let to_doc = pl.mat_to_doc();
    let screen = |x: f64, y: f64| xf.to_screen(to_doc * Point::new(x, y));
    let ring = |r: RectMm| {
        let q = [screen(r.x0, r.y0), screen(r.x1, r.y0), screen(r.x1, r.y1), screen(r.x0, r.y1)];
        q.iter().chain(q.first()).copied().collect::<Vec<Pos2>>()
    };
    let t = Tokens::get(p.ctx());
    let [w, h] = pl.mat.size_mm;
    let px_per_mm = xf.zoom / MM_PER_PT;

    // In the mat's own colour, faint, so the grid isn't taken for ruler guides or the document grid.
    for (step, color) in [(MINOR_MM, t.accent.gamma_multiply(0.18)), (MAJOR_MM, t.accent.gamma_multiply(0.45))] {
        if step * px_per_mm < MIN_GAP_PX {
            continue;
        }
        let stroke = Stroke::new(0.75, color);
        for x in ticks(w, step) {
            p.line_segment([screen(x, 0.0), screen(x, h)], stroke);
        }
        for y in ticks(h, step) {
            p.line_segment([screen(0.0, y), screen(w, y)], stroke);
        }
    }

    let mat = RectMm { x0: 0.0, y0: 0.0, x1: w, y1: h };
    p.add(Shape::line(ring(mat), Stroke::new(1.5, t.accent)));
    let cut = pl.mat.cut_area_mm;
    if cut != mat {
        p.extend(Shape::dashed_line(&ring(cut), Stroke::new(1.0, t.accent), 5.0, 3.0));
    }

    // Labels outside the top and left edges, measured from the cut area's origin. Each sits just
    // outside its edge on screen, anchored on the side facing away from the mat, so it clears the
    // edge whichever way a turned mat or view puts it.
    let font = egui::FontId::proportional(10.0);
    let label = |at: Pos2, inside: Pos2, text: String| {
        let d = (at - inside).normalized();
        let anchor = match (d.x.abs() > d.y.abs(), d.x < 0.0, d.y < 0.0) {
            (true, true, _) => egui::Align2::RIGHT_CENTER,
            (true, false, _) => egui::Align2::LEFT_CENTER,
            (false, _, true) => egui::Align2::CENTER_BOTTOM,
            (false, _, false) => egui::Align2::CENTER_TOP,
        };
        p.text(at + d * LABEL_OUT_PX, anchor, text, font.clone(), t.accent);
    };
    let mut every = MAJOR_MM;
    while every * px_per_mm < LABEL_GAP_PX && every < 1_000.0 {
        every *= 2.0;
    }
    if every * px_per_mm >= LABEL_GAP_PX {
        for x in ticks(w, every) {
            label(screen(x, 0.0), screen(x, 1.0), format!("{} mm", x - cut.x0));
        }
        for y in ticks(h, every) {
            label(screen(0.0, y), screen(1.0, y), format!("{} mm", y - cut.y0));
        }
    }
    // The origin: a corner mark and its 0, off the corner's diagonal.
    let o = screen(cut.x0, cut.y0);
    let arm = 12.0 / px_per_mm;
    let marks = [screen(cut.x0 + arm, cut.y0), o, screen(cut.x0, cut.y0 + arm)];
    p.add(Shape::line(marks.to_vec(), Stroke::new(2.0, t.accent)));
    let d = (o - screen(cut.x0 + 1.0, cut.y0 + 1.0)).normalized();
    p.text(o + d * LABEL_OUT_PX, egui::Align2::RIGHT_BOTTOM, "0", font.clone(), t.accent);

    if !pl.fits() {
        let a = ring(pl.artboard_mm);
        p.add(Shape::line(a.clone(), Stroke::new(2.0, t.bleed)));
        if let Some(top) = a.first() {
            p.text(*top + vec2(4.0, 4.0), egui::Align2::LEFT_TOP, crate::i18n::t("Off the mat"), font, t.bleed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ticks;

    #[test]
    fn ticks_fall_strictly_inside() {
        assert_eq!(ticks(304.8, 50.0).collect::<Vec<_>>(), [50.0, 100.0, 150.0, 200.0, 250.0, 300.0]);
        assert_eq!(ticks(100.0, 50.0).collect::<Vec<_>>(), [50.0]);
        assert_eq!(ticks(f64::NAN, 10.0).count(), 0);
        assert_eq!(ticks(1e12, 10.0).count(), super::MAX_LINES);
    }
}
