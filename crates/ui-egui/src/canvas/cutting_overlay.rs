//! Fork: View › Cutting View (`docs/cutting/ARCHITECTURE.md`): the cutting mat where the
//! document's cutting settings lay the artboard on it.
//!
//! [`mat_plate`] draws the mat as the physical object, behind the artboards and the art (C2.7): a
//! grey plate with rounded corners reaching past the grid by the mat's margin, the grid printed
//! on it (inches on an imperial mat, numbered each inch; 10 mm on others, numbered each 50 mm),
//! ruler ticks down the right edge, the feed arrow at the top (the edge the machine takes first)
//! and the mat's size in a corner. No maker's logo: the mat is drawn, not copied. Lines and
//! numbers closer than a few pixels are left out.
//!
//! [`cutting_overlay`] draws over the art what the art mustn't hide: the cut area where it is
//! smaller than the mat, the origin, the artboard in a warning colour when the cut area doesn't
//! hold it, and the cut job's toolpaths in cutting order ([`toolpaths`]; the Cutting panel's Show
//! Toolpaths) with its playback, from the job `crate::cutting` keeps for the document.

use vectorcraft_cutjob::RectMm;
use vectorcraft_cutting::placement::{self, MM_PER_PT, Placement};

use super::{Color32, Point, Pos2, Shape, Stroke, Tokens, VectorcraftApp, Xf, pos2, vec2};

/// What Cutting View draws under the art (the canvas's hook): the mat.
pub(super) fn under(app: &VectorcraftApp, painter: &egui::Painter, xf: &Xf) {
    if app.ui.view.cutting {
        mat_plate(app, painter, xf);
    }
}

/// What Cutting View draws over the art (the canvas's hook): the job's overlay and the
/// playback bar.
pub(super) fn over(app: &VectorcraftApp, painter: &egui::Painter, xf: &Xf, ctx: &egui::Context) {
    if app.ui.view.cutting {
        cutting_overlay(app, painter, xf);
        playback_bar(app, ctx, xf);
    }
}

/// Grid lines closer than this on screen are left out, px.
const MIN_GAP_PX: f64 = 6.0;
/// Numbers need this much room between them, px.
const LABEL_GAP_PX: f64 = 28.0;
/// Ticks closer than this are left out, px.
const MIN_TICK_GAP_PX: f64 = 4.0;
/// How far the origin's unit sits off the corner, px.
const LABEL_OUT_PX: f32 = 4.0;
/// Most lines one grid direction draws (a 3 m mat at 5 mm ticks is 600).
const MAX_LINES: usize = 800;
/// The plate's corner radius, mm.
const CORNER_MM: f64 = 5.0;
/// The mat as the physical object is grey in every brightness: its plastic, its edge, and what
/// is printed on it.
const PLATE: Color32 = Color32::from_gray(158);
const PLATE_EDGE: Color32 = Color32::from_gray(112);
const PRINT: Color32 = Color32::from_gray(58);

/// The mm positions `step` apart strictly inside `0..len`.
fn ticks(len: f64, step: f64) -> impl Iterator<Item = f64> {
    // Less a hair: 304.8 / 25.4 is a little over 12 in floating point, and the edge isn't inside.
    let n = if len.is_finite() && step > 0.0 { ((len / step - 1e-9).ceil() as usize).saturating_sub(1).min(MAX_LINES) } else { 0 };
    (1..=n).map(move |i| i as f64 * step)
}

/// Where the active document lies on its mat.
fn placed(app: &VectorcraftApp) -> Option<Placement> {
    let st = app.session.active()?;
    placement::place(&vectorcraft_cutting::settings(&st.doc), &st.doc)
}

/// `n` mm in the mat's own unit, as printed on it.
fn printed(n: f64, inch: bool) -> String {
    let v = if inch { n / 25.4 } else { n };
    format!("{}", (v * 100.0).round() / 100.0)
}

/// The mat as the physical object, behind the art (C2.7).
pub(super) fn mat_plate(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(pl) = placed(app) else { return };
    let to_doc = pl.mat_to_doc();
    let screen = |x: f64, y: f64| xf.to_screen(to_doc * Point::new(x, y));
    let [w, h] = pl.mat.size_mm;
    let [ml, mt, mr, mb] = pl.mat.plate_margin_mm;
    let inch = pl.mat.inch_grid;
    let px_per_mm = xf.zoom / MM_PER_PT;

    // The plate: a rounded rectangle, convex on screen however the mat and the view are turned.
    let (x0, y0, x1, y1) = (-ml, -mt, w + mr, h + mb);
    let r = CORNER_MM.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    let mut plate = Vec::with_capacity(28);
    for (cx, cy, from) in [(x1 - r, y0 + r, -90.0f64), (x1 - r, y1 - r, 0.0), (x0 + r, y1 - r, 90.0), (x0 + r, y0 + r, 180.0)] {
        for k in 0..=6 {
            let a = (from + 15.0 * f64::from(k)).to_radians();
            plate.push(screen(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    p.add(Shape::convex_polygon(plate, PLATE, Stroke::new(1.0, PLATE_EDGE)));

    // The grid, its border, and its numbers along the top and left, inside.
    let unit = if inch { 25.4 } else { 10.0 };
    let line = Stroke::new(1.0, PRINT);
    if unit * px_per_mm >= MIN_GAP_PX {
        for x in ticks(w, unit) {
            p.line_segment([screen(x, 0.0), screen(x, h)], line);
        }
        for y in ticks(h, unit) {
            p.line_segment([screen(0.0, y), screen(w, y)], line);
        }
    }
    let border = [screen(0.0, 0.0), screen(w, 0.0), screen(w, h), screen(0.0, h), screen(0.0, 0.0)];
    p.add(Shape::line(border.to_vec(), Stroke::new(1.0, PRINT)));
    let font = egui::FontId::proportional(9.0);
    let mut every = if inch { 25.4 } else { 50.0 };
    while every * px_per_mm < LABEL_GAP_PX && every < 10_000.0 {
        every *= 2.0;
    }
    if every * px_per_mm >= LABEL_GAP_PX {
        let cut = pl.mat.cut_area_mm;
        for x in ticks(w, every) {
            p.text(screen(x, 0.0) + vec2(-2.0, 2.0), egui::Align2::RIGHT_TOP, printed(x - cut.x0, inch), font.clone(), PRINT);
        }
        for y in ticks(h, every) {
            p.text(screen(0.0, y) + vec2(2.0, -2.0), egui::Align2::LEFT_BOTTOM, printed(y - cut.y0, inch), font.clone(), PRINT);
        }
    }

    // Ruler ticks down the right edge: quarters of an inch, or every 5 mm, longer at each unit.
    let tick = if inch { 25.4 / 4.0 } else { 5.0 };
    if tick * px_per_mm >= MIN_TICK_GAP_PX {
        let per_unit = if inch { 4 } else { 2 };
        for (i, y) in ticks(h, tick).enumerate() {
            let n = i + 1;
            let len = if n % per_unit == 0 {
                4.0
            } else if inch && n % 2 == 0 {
                2.5
            } else {
                1.5
            };
            p.line_segment([screen(w, y), screen(w - len, y)], Stroke::new(0.75, PRINT));
        }
    }

    // The feed arrow, in the top margin's middle, pointing at the machine.
    if mt * px_per_mm >= 8.0 {
        let (cx, top, base) = (w / 2.0, -mt * 0.85, -mt * 0.45);
        let half = mt * 0.35;
        p.add(Shape::convex_polygon(vec![screen(cx, top), screen(cx + half, base), screen(cx - half, base)], PRINT, Stroke::NONE));
        let (stem, stem_end) = (half * 0.45, -mt * 0.15);
        let shaft = vec![screen(cx - stem, base), screen(cx + stem, base), screen(cx + stem, stem_end), screen(cx - stem, stem_end)];
        p.add(Shape::convex_polygon(shaft, PRINT, Stroke::NONE));
    }

    // Its size, in the bottom margin's right-hand corner (where a maker prints its name).
    if mb * px_per_mm >= 10.0 {
        let size = if inch {
            format!("{} × {} in", printed(w, true), printed(h, true))
        } else {
            format!("{} × {} mm", printed(w, false), printed(h, false))
        };
        p.text(screen(w, h + mb / 2.0) + vec2(-2.0, 0.0), egui::Align2::RIGHT_CENTER, size, egui::FontId::proportional(10.0), PRINT);
    }
}

/// What the art mustn't hide: drawn over it.
pub(super) fn cutting_overlay(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(pl) = placed(app) else { return };
    let to_doc = pl.mat_to_doc();
    let screen = |x: f64, y: f64| xf.to_screen(to_doc * Point::new(x, y));
    let ring = |r: RectMm| {
        let q = [screen(r.x0, r.y0), screen(r.x1, r.y0), screen(r.x1, r.y1), screen(r.x0, r.y1)];
        q.iter().chain(q.first()).copied().collect::<Vec<Pos2>>()
    };
    let t = Tokens::get(p.ctx());
    let [w, h] = pl.mat.size_mm;
    let px_per_mm = xf.zoom / MM_PER_PT;
    let cut = pl.mat.cut_area_mm;
    if cut != (RectMm { x0: 0.0, y0: 0.0, x1: w, y1: h }) {
        p.extend(Shape::dashed_line(&ring(cut), Stroke::new(1.0, t.accent), 5.0, 3.0));
    }
    if crate::cutting::shows_toolpaths(p.ctx())
        && let Some(job) = crate::cutting::job(app, p.ctx())
        && let Ok(made) = &*job
    {
        toolpaths(p, &made.job, &screen, &t);
        if let Some(mm) = crate::cutting::tick(p.ctx(), &made.sim).mm {
            playback(p, &made.sim, mm, &screen, &t);
        }
    }
    // The origin: a corner mark, and the unit the mat is numbered in off the corner's diagonal.
    let font = egui::FontId::proportional(10.0);
    let o = screen(cut.x0, cut.y0);
    let arm = 12.0 / px_per_mm;
    let marks = [screen(cut.x0 + arm, cut.y0), o, screen(cut.x0, cut.y0 + arm)];
    p.add(Shape::line(marks.to_vec(), Stroke::new(2.0, t.accent)));
    let d = (o - screen(cut.x0 + 1.0, cut.y0 + 1.0)).normalized();
    let unit = if pl.mat.inch_grid { "in" } else { "mm" };
    p.text(o + d * LABEL_OUT_PX, egui::Align2::RIGHT_BOTTOM, unit, font.clone(), t.accent);

    if !pl.fits() {
        let a = ring(pl.artboard_mm);
        p.add(Shape::line(a.clone(), Stroke::new(2.0, t.bleed)));
        if let Some(top) = a.first() {
            p.text(*top + vec2(4.0, 4.0), egui::Align2::LEFT_TOP, crate::i18n::t("Off the mat"), font, t.bleed);
        }
    }
}

/// The size [`preview`] fills at most `width` wide and `max_height` high: the plate's proportions.
pub(crate) fn preview_size(app: &VectorcraftApp, width: f32, max_height: f32) -> egui::Vec2 {
    let Some(pl) = placed(app) else { return egui::vec2(width, width) };
    let [w, h] = pl.mat.size_mm;
    let [ml, mt, mr, mb] = pl.mat.plate_margin_mm;
    let (full_w, full_h) = (w + ml + mr, h + mt + mb);
    let aspect = if full_w > 0.0 && full_h.is_finite() { (full_h / full_w) as f32 } else { 1.0 };
    let height = (width * aspect).clamp(width * 0.25, max_height);
    egui::vec2((height / aspect).min(width), height)
}

/// The playback bar (C7.3): in Cutting View with the toolpaths shown, the blade's run played,
/// stepped and scrubbed from a bar at the foot of the canvas, so the cutting order can be checked
/// while editing. It moves the same playback as the Send window's.
pub(super) fn playback_bar(app: &VectorcraftApp, ctx: &egui::Context, xf: &Xf) {
    if !crate::cutting::shows_toolpaths(ctx) {
        return;
    }
    let Some(job) = crate::cutting::job(app, ctx) else { return };
    let Ok(made) = &*job else { return };
    if made.job.paths.is_empty() {
        return;
    }
    let t = Tokens::get(ctx);
    let id = egui::Id::unique("cutting-playback-bar");
    // Centred at the foot of the canvas, by its size last frame (a guess before the first).
    let size = ctx.memory(|m| m.area_rect(id)).map_or(vec2(560.0, 40.0), |r| r.size());
    let pos = pos2(xf.rect.center().x - size.x / 2.0, xf.rect.bottom() - size.y - BAR_MARGIN);
    egui::Area::new(id).order(egui::Order::Middle).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::NONE
            .fill(t.panel)
            .stroke(Stroke::new(1.0, t.tool_active))
            .corner_radius(egui::CornerRadius::same(5))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| crate::cutting::playback_controls(ui, &made.sim, true));
    });
}

/// How far above the canvas's foot the playback bar sits, px.
const BAR_MARGIN: f32 = 12.0;

/// The mat as the machine takes it, fitted into `rect` (the Send window, C7.1): the plate, the
/// cut area and origin, and the job's toolpaths with their playback, whatever Show Toolpaths and
/// the view say. The mat is drawn upright, its feed edge at the top, however the artboard lies on
/// it.
pub(crate) fn preview(app: &VectorcraftApp, p: &egui::Painter, rect: egui::Rect) {
    let Some(pl) = placed(app) else { return };
    let to_doc = pl.mat_to_doc();
    let [w, h] = pl.mat.size_mm;
    let [ml, mt, mr, mb] = pl.mat.plate_margin_mm;
    let (full_w, full_h) = (w + ml + mr, h + mt + mb);
    if !(full_w > 0.0 && full_h > 0.0) {
        return;
    }
    let px_per_mm = (f64::from(rect.width()) / full_w).min(f64::from(rect.height()) / full_h);
    let [a, b, ..] = to_doc.as_coeffs();
    let xf = Xf {
        rect,
        zoom: px_per_mm * MM_PER_PT,
        center: to_doc * Point::new((w + mr - ml) / 2.0, (h + mb - mt) / 2.0),
        // The mat's x axis back along the screen's.
        rot: -b.atan2(a),
    };
    if !(xf.zoom.is_finite() && xf.zoom > 0.0) {
        return;
    }
    let p = p.with_clip_rect(rect);
    mat_plate(app, &p, &xf);
    let screen = |x: f64, y: f64| xf.to_screen(to_doc * Point::new(x, y));
    let t = Tokens::get(p.ctx());
    let cut = pl.mat.cut_area_mm;
    if cut != (RectMm { x0: 0.0, y0: 0.0, x1: w, y1: h }) {
        let q = [screen(cut.x0, cut.y0), screen(cut.x1, cut.y0), screen(cut.x1, cut.y1), screen(cut.x0, cut.y1)];
        let ring: Vec<Pos2> = q.iter().chain(q.first()).copied().collect();
        p.extend(Shape::dashed_line(&ring, Stroke::new(1.0, t.accent), 5.0, 3.0));
    }
    if let Some(job) = crate::cutting::job(app, p.ctx())
        && let Ok(made) = &*job
    {
        toolpaths(&p, &made.job, &screen, &t);
        if let Some(mm) = crate::cutting::tick(p.ctx(), &made.sim).mm {
            playback(&p, &made.sim, mm, &screen, &t);
        }
    }
}

/// Most toolpath points one frame draws: past them the rest of a large job isn't drawn.
const MAX_DRAWN_POINTS: usize = 400_000;
/// Jobs of more paths than this aren't numbered.
const MAX_NUMBERS: usize = 500;
/// A path's number is left out when it would sit this close (px) to the one before.
const NUMBER_GAP_PX: f32 = 14.0;

/// The job's toolpaths in cutting order (C3.6): cuts solid and scores dashed, each in its colour,
/// the blade's travel between them faint and dashed, an arrow at each path's start along the way
/// it is cut, and its number in the order.
fn toolpaths(p: &egui::Painter, job: &vectorcraft_cutjob::CutJob, screen: &dyn Fn(f64, f64) -> Pos2, t: &Tokens) {
    use vectorcraft_cutjob::OpKind;
    let travel = Stroke::new(0.75, t.text_dim.gamma_multiply(0.6));
    let font = egui::FontId::proportional(9.0);
    let area = job.mat.cut_area_mm;
    let mut at = screen(area.x0, area.y0);
    let numbered = job.paths.len() <= MAX_NUMBERS;
    let mut last_number: Option<Pos2> = None;
    let mut drawn = 0usize;
    for (i, path) in job.paths.iter().enumerate() {
        if drawn > MAX_DRAWN_POINTS {
            break;
        }
        let pts: Vec<Pos2> = path.points.iter().map(|q| screen(q[0], q[1])).collect();
        let Some(&start) = pts.first() else { continue };
        drawn += pts.len();
        p.extend(Shape::dashed_line(&[at, start], travel, 3.0, 3.0));
        let score = job.operations.get(path.op).is_some_and(|o| o.kind == OpKind::Score);
        let color = if score { t.smart_guide } else { t.selection };
        let mut line = pts.clone();
        if path.closed {
            line.push(start);
        }
        if score {
            p.extend(Shape::dashed_line(&line, Stroke::new(1.5, color), 5.0, 3.0));
        } else {
            p.add(Shape::line(line, Stroke::new(1.5, color)));
        }
        if let Some(&next) = pts.get(1)
            && (next - start).length() > 1.0
        {
            let u = (next - start).normalized();
            let side = vec2(-u.y, u.x) * 3.0;
            p.add(Shape::convex_polygon(vec![start + u * 9.0, start + u * 2.0 + side, start + u * 2.0 - side], color, Stroke::NONE));
        }
        if numbered && last_number.is_none_or(|l| l.distance(start) >= NUMBER_GAP_PX) {
            p.text(start + vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM, (i + 1).to_string(), font.clone(), color);
            last_number = Some(start);
        }
        at = if path.closed { start } else { pts.last().copied().unwrap_or(start) };
    }
}

/// The run played back to `mm` (C3.7): what the blade has cut so far drawn strong over the
/// toolpaths, and the blade where it is, filled while down.
fn playback(p: &egui::Painter, sim: &vectorcraft_cutjob::Simulation, mm: f64, screen: &dyn Fn(f64, f64) -> Pos2, t: &Tokens) {
    let done = Stroke::new(3.0, t.accent_strong);
    let mut left = mm;
    for m in &sim.moves {
        if left <= 0.0 {
            break;
        }
        let k = if m.mm > left { left / m.mm } else { 1.0 };
        left -= m.mm;
        if m.down {
            let to = [m.from[0] + k * (m.to[0] - m.from[0]), m.from[1] + k * (m.to[1] - m.from[1])];
            p.line_segment([screen(m.from[0], m.from[1]), screen(to[0], to[1])], done);
        }
    }
    let blade = sim.at_mm(mm);
    let at = screen(blade.position[0], blade.position[1]);
    if blade.down {
        p.circle_filled(at, 5.0, t.accent_strong);
    } else {
        p.circle_stroke(at, 5.0, Stroke::new(2.0, t.accent_strong));
    }
}

#[cfg(test)]
mod tests {
    use super::ticks;

    #[test]
    fn ticks_fall_strictly_inside() {
        assert_eq!(ticks(304.8, 50.0).collect::<Vec<_>>(), [50.0, 100.0, 150.0, 200.0, 250.0, 300.0]);
        assert_eq!(ticks(100.0, 50.0).collect::<Vec<_>>(), [50.0]);
        assert_eq!(ticks(304.8, 25.4).count(), 11, "12 in: 1 to 11, not the edge");
        assert_eq!(ticks(f64::NAN, 10.0).count(), 0);
        assert_eq!(ticks(1e12, 10.0).count(), super::MAX_LINES);
    }
}
