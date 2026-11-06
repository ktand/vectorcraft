//! Weeding aids (C8.2, C8.3): cuts the job adds round a design so the waste comes away easily, as
//! vinyl wants. They are made here, in the job, never added to the art: the document stays the
//! design. Their toolpaths name node 0, which no object has (a document's ids start at 1).
//!
//! - The **weed box**: a rectangle round the design a margin away, so the sheet's waste round it
//!   can be lifted as one piece. Cut last, so the material is held while the design is cut.
//! - **Weed lines**: from the box in to the design, across the waste, stopping just short of the
//!   design so they never cut into it, so the waste comes off in strips. One from the middle of
//!   each side, or one every `weedLineSpacingMm` along them; a line that meets nothing on its way
//!   (it passes between shapes) runs across the box.
//! - **Smart weed lines** (C18.24, the default): the box's waste split into strips no larger than
//!   the spacing (50 mm when it is 0), each split a line right across the strip through a gap in
//!   the design, as near its middle as there is one, never closer than a millimetre to the design;
//!   where a strip has no such gap, lines in from both sides to the design, as above.

use vectorcraft_cutjob::{CuttingSettings, JobWarning, OpKind, RectMm, SourceRef};

use crate::prepare::Polyline;

/// Where a weed line stops short of the design, mm: the blade runs on a little past its end.
pub const GAP_MM: f64 = 0.3;
/// A weed line shorter than this isn't cut, mm.
const MIN_LINE_MM: f64 = 1.0;
/// Most weed lines on each side.
const MAX_LINES_PER_SIDE: usize = 60;
/// Most line-against-segment tests weeding makes for one group, so a huge design can't stall it.
const MAX_WORK: usize = 50_000_000;
/// Smart lines: the largest strip when the spacing is 0, mm.
pub const STRIP_MM: f64 = 50.0;
/// Smart lines: no strip narrower than this is split off, mm.
const MIN_STRIP_MM: f64 = 5.0;
/// Smart lines: the smallest strip size asked for, mm (a smaller one is taken as this).
pub const SMALLEST_STRIP_MM: f64 = 2.0 * MIN_STRIP_MM;
/// Smart lines: how close a line may come to the design, mm.
const CLEAR_MM: f64 = 1.0;
/// Smart lines: most lines for one group.
const MAX_SMART_LINES: usize = 200;

/// The source weeding's toolpaths name: no object of the document.
pub const SOURCE: SourceRef = SourceRef { node: 0, subpath: 0 };

/// The weeding cuts for one group's paths (laid on the mat), in the order they are cut (the lines,
/// then the box), all cuts; inside `area` (the cut area, or the marks' for Print & Cut).
pub fn weeding(paths: &[Polyline], area: RectMm, s: &CuttingSettings) -> (Vec<Polyline>, Vec<JobWarning>) {
    if !s.weed_box && !s.weed_lines {
        return (vec![], vec![]);
    }
    let Some(b) = bounds(paths) else { return (vec![], vec![]) };
    let m = s.weed_margin_mm;
    let boxed = RectMm { x0: (b.x0 - m).max(area.x0), y0: (b.y0 - m).max(area.y0), x1: (b.x1 + m).min(area.x1), y1: (b.y1 + m).min(area.y1) };
    if !(boxed.width() > 0.0 && boxed.height() > 0.0) {
        return (vec![], vec![]);
    }
    let mut out = vec![];
    let mut warnings = vec![];
    if s.weed_lines && s.weed_lines_smart {
        let strip = if s.weed_line_spacing_mm > 0.0 { s.weed_line_spacing_mm.max(SMALLEST_STRIP_MM) } else { STRIP_MM };
        let mut budget = MAX_WORK;
        out.extend(smart_lines(paths, boxed, strip, s.weed_lines_between, &mut budget));
        if budget == 0 {
            warnings.push(JobWarning { source: None, message: "the design is too large for all its weed lines: some were left out".into() });
        }
    } else if s.weed_lines {
        let segments: usize = paths.iter().map(|p| p.points.len()).sum();
        let xs = positions(boxed.x0, boxed.x1, s.weed_line_spacing_mm);
        let ys = positions(boxed.y0, boxed.y1, s.weed_line_spacing_mm);
        if (xs.len() + ys.len()).saturating_mul(segments) > MAX_WORK {
            warnings.push(JobWarning { source: None, message: "the design is too large for weed lines: they were left out".into() });
        } else {
            for x in xs {
                out.extend(lines_across(paths, x, boxed.y0, boxed.y1, false));
            }
            for y in ys {
                out.extend(lines_across(paths, y, boxed.x0, boxed.x1, true));
            }
        }
    }
    if s.weed_box {
        let corners = vec![[boxed.x0, boxed.y0], [boxed.x1, boxed.y0], [boxed.x1, boxed.y1], [boxed.x0, boxed.y1]];
        out.push(Polyline { op: OpKind::Cut, source: SOURCE, closed: true, points: corners, curve: None, dash: None });
    }
    (out, warnings)
}

/// What the paths cover, mm.
fn bounds(paths: &[Polyline]) -> Option<RectMm> {
    let mut points = paths.iter().flat_map(|p| p.points.iter());
    let first = points.next()?;
    let r = points.fold(RectMm { x0: first[0], y0: first[1], x1: first[0], y1: first[1] }, |r, q| RectMm {
        x0: r.x0.min(q[0]),
        y0: r.y0.min(q[1]),
        x1: r.x1.max(q[0]),
        y1: r.y1.max(q[1]),
    });
    [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()).then_some(r)
}

/// Where the lines go along one side from `lo` to `hi`: its middle, or every `spacing` mm.
fn positions(lo: f64, hi: f64, spacing: f64) -> Vec<f64> {
    if !(spacing.is_finite() && spacing > 0.0) {
        return vec![(lo + hi) / 2.0];
    }
    let n = ((hi - lo) / spacing).floor();
    let n = if n.is_finite() && n >= 1.0 { (n as usize).min(MAX_LINES_PER_SIDE) } else { 0 };
    if n == 0 {
        return vec![(lo + hi) / 2.0];
    }
    // Centred: the same room left at both ends.
    let start = lo + ((hi - lo) - spacing * (n as f64 - 1.0)) / 2.0;
    (0..n).map(|k| start + spacing * k as f64).collect()
}

/// Smart weed lines (C18.24) in `boxed`: each strip larger than `strip` split by a line across it
/// through a gap in the design, as near its middle as there is one; a strip with no gap gets lines
/// in from both sides to the design. Every test of a line against a segment uses up `budget`.
fn smart_lines(paths: &[Polyline], boxed: RectMm, strip: f64, between: bool, budget: &mut usize) -> Vec<Polyline> {
    let mut out = vec![];
    let mut todo = vec![boxed];
    // The strips no line splits further.
    let mut leaves = vec![];
    while let Some(r) = todo.pop() {
        if out.len() >= MAX_SMART_LINES || *budget == 0 {
            break;
        }
        let (w, h) = (r.width(), r.height());
        if w <= strip && h <= strip {
            leaves.push(r);
            continue;
        }
        // The longer side first: a vertical line (not horizontal) splits the width.
        let order = if w >= h { [false, true] } else { [true, false] };
        let mut split = false;
        for horizontal in order {
            let (lo, hi, a0, a1) = if horizontal { (r.y0, r.y1, r.x0, r.x1) } else { (r.x0, r.x1, r.y0, r.y1) };
            if hi - lo <= strip {
                continue;
            }
            if let Some(at) = free_line(paths, lo, hi, a0, a1, horizontal, budget) {
                out.push(segment(at, a0, a1, horizontal));
                if horizontal {
                    todo.push(RectMm { y1: at, ..r });
                    todo.push(RectMm { y0: at, ..r });
                } else {
                    todo.push(RectMm { x1: at, ..r });
                    todo.push(RectMm { x0: at, ..r });
                }
                split = true;
                break;
            }
        }
        if !split {
            // No gap: lines in from both sides to the design, across the longer side's middle.
            let horizontal = h > w;
            let (lo, hi, a0, a1) = if horizontal { (r.y0, r.y1, r.x0, r.x1) } else { (r.x0, r.x1, r.y0, r.y1) };
            *budget = budget.saturating_sub(paths.iter().map(|p| p.points.len()).sum());
            out.extend(lines_across(paths, (lo + hi) / 2.0, a0, a1, horizontal));
            leaves.push(r);
        }
    }
    // C18.26: a line through every gap between pieces (between letters) in each strip.
    if between {
        for r in leaves {
            for horizontal in [false, true] {
                if out.len() >= MAX_SMART_LINES || *budget == 0 {
                    break;
                }
                out.extend(gap_lines(paths, r, horizontal, budget));
            }
        }
    }
    out.truncate(MAX_SMART_LINES);
    out
}

/// Lines across `r` through each gap between the design's pieces: a run of places where a line
/// right across clears the design, with design on both sides of it in `r`, gets one line through
/// its middle. Vertical lines (gaps side by side, as between letters), or horizontal ones.
fn gap_lines(paths: &[Polyline], r: RectMm, horizontal: bool, budget: &mut usize) -> Vec<Polyline> {
    let (lo, hi, a0, a1) = if horizontal { (r.y0, r.y1, r.x0, r.x1) } else { (r.x0, r.x1, r.y0, r.y1) };
    let step = ((hi - lo) / 2000.0).max(0.5);
    let n = ((hi - lo) / step).floor();
    if !(n.is_finite() && n >= 2.0) {
        return vec![];
    }
    let places: Vec<(f64, bool)> = (0..=n as usize)
        .map(|k| {
            let at = lo + step * k as f64;
            let band = if horizontal {
                RectMm { x0: a0, x1: a1, y0: at - CLEAR_MM, y1: at + CLEAR_MM }
            } else {
                RectMm { x0: at - CLEAR_MM, x1: at + CLEAR_MM, y0: a0, y1: a1 }
            };
            (at, clear(paths, band, budget))
        })
        .collect();
    let mut out = vec![];
    let mut k = 0;
    while k < places.len() {
        if !places.get(k).is_some_and(|p| p.1) {
            k += 1;
            continue;
        }
        let start = k;
        while places.get(k).is_some_and(|p| p.1) {
            k += 1;
        }
        // Design on both sides: not a run that reaches the strip's edge.
        if start > 0
            && k < places.len()
            && let (Some(a), Some(b)) = (places.get(start), places.get(k - 1))
        {
            out.push(segment((a.0 + b.0) / 2.0, a0, a1, horizontal));
        }
    }
    out
}

/// Where a line across from `a0` to `a1` (at a place between `lo` and `hi`) clears the design,
/// the nearest to the middle; none when every place meets it.
fn free_line(paths: &[Polyline], lo: f64, hi: f64, a0: f64, a1: f64, horizontal: bool, budget: &mut usize) -> Option<f64> {
    let mid = (lo + hi) / 2.0;
    let half = (hi - lo) / 2.0 - MIN_STRIP_MM;
    if !half.is_finite() || half < 0.0 {
        return None;
    }
    let step = ((hi - lo) / 120.0).max(0.5);
    let n = (half / step).floor().min(1000.0) as usize;
    for k in 0..=n {
        for side in [1.0, -1.0] {
            if k == 0 && side < 0.0 {
                continue;
            }
            let at = mid + side * step * k as f64;
            let band = if horizontal {
                RectMm { x0: a0, x1: a1, y0: at - CLEAR_MM, y1: at + CLEAR_MM }
            } else {
                RectMm { x0: at - CLEAR_MM, x1: at + CLEAR_MM, y0: a0, y1: a1 }
            };
            if clear(paths, band, budget) {
                return Some(at);
            }
            if *budget == 0 {
                return None;
            }
        }
    }
    None
}

/// Whether no part of the design comes into `band`.
fn clear(paths: &[Polyline], band: RectMm, budget: &mut usize) -> bool {
    for p in paths {
        let n = p.points.len();
        let count = if p.closed { n } else { n.saturating_sub(1) };
        for k in 0..count {
            if *budget == 0 {
                return false;
            }
            *budget -= 1;
            let (Some(a), Some(b)) = (p.points.get(k), p.points.get((k + 1) % n.max(1))) else { continue };
            if meets(*a, *b, band) {
                return false;
            }
        }
        // A single point (a dot) has no segment.
        if n == 1 && p.points.first().is_some_and(|q| q[0] >= band.x0 && q[0] <= band.x1 && q[1] >= band.y0 && q[1] <= band.y1) {
            return false;
        }
    }
    true
}

/// Whether the segment `a`–`b` meets `r` (clipped to it, Liang–Barsky).
fn meets(a: [f64; 2], b: [f64; 2], r: RectMm) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a[0] - r.x0), (dx, r.x1 - a[0]), (-dy, a[1] - r.y0), (dy, r.y1 - a[1])] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// A weed line at `at` from `a0` to `a1`.
fn segment(at: f64, a0: f64, a1: f64, horizontal: bool) -> Polyline {
    let point = |along: f64| if horizontal { [along, at] } else { [at, along] };
    Polyline { op: OpKind::Cut, source: SOURCE, closed: false, points: vec![point(a0), point(a1)], curve: None, dash: None }
}

/// The weed lines along `at` (x of a vertical line, or y of a horizontal one when `horizontal`)
/// from `lo` to `hi`: from each end in to where the design starts, or one line across when it
/// meets none of it.
fn lines_across(paths: &[Polyline], at: f64, lo: f64, hi: f64, horizontal: bool) -> Vec<Polyline> {
    // Where the line crosses each of the design's segments, along it.
    let mut first = f64::INFINITY;
    let mut last = f64::NEG_INFINITY;
    for p in paths {
        let n = p.points.len();
        let count = if p.closed { n } else { n.saturating_sub(1) };
        for k in 0..count {
            let (Some(a), Some(b)) = (p.points.get(k), p.points.get((k + 1) % n.max(1))) else { continue };
            let (a_at, a_along, b_at, b_along) = if horizontal { (a[1], a[0], b[1], b[0]) } else { (a[0], a[1], b[0], b[1]) };
            if (a_at - at) * (b_at - at) > 0.0 {
                continue;
            }
            let along = if (b_at - a_at).abs() < 1e-12 {
                // Lying along the line: it meets it at its nearer end.
                a_along.min(b_along)
            } else {
                a_along + (at - a_at) / (b_at - a_at) * (b_along - a_along)
            };
            let far = if (b_at - a_at).abs() < 1e-12 { a_along.max(b_along) } else { along };
            first = first.min(along);
            last = last.max(far);
        }
    }
    let point = |along: f64| if horizontal { [along, at] } else { [at, along] };
    let line = |from: f64, to: f64| Polyline {
        op: OpKind::Cut,
        source: SOURCE,
        closed: false,
        points: vec![point(from), point(to)],
        curve: None,
        dash: None,
    };
    if !first.is_finite() {
        return vec![line(lo, hi)];
    }
    let mut out = vec![];
    let to = (first - GAP_MM).min(hi);
    if to - lo >= MIN_LINE_MM {
        out.push(line(lo, to));
    }
    let from = (last + GAP_MM).max(lo);
    if hi - from >= MIN_LINE_MM {
        out.push(line(hi, from));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, side: f64) -> Polyline {
        Polyline {
            op: OpKind::Cut,
            source: SourceRef { node: 1, subpath: 0 },
            closed: true,
            points: vec![[x0, y0], [x0 + side, y0], [x0 + side, y0 + side], [x0, y0 + side]],
            curve: None,
            dash: None,
        }
    }

    const MAT: RectMm = RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 };

    /// Evenly spaced weed lines, as before C18.24.
    fn on(weed_box: bool, weed_lines: bool) -> CuttingSettings {
        CuttingSettings { weed_box, weed_lines, weed_lines_smart: false, ..CuttingSettings::default() }
    }

    fn smart(spacing: f64) -> CuttingSettings {
        CuttingSettings { weed_box: true, weed_lines: true, weed_line_spacing_mm: spacing, ..CuttingSettings::default() }
    }

    /// Whether `l` keeps `CLEAR_MM` (less a hair) from every segment of `design`.
    fn keeps_clear(l: &Polyline, design: &[Polyline]) -> bool {
        let (a, b) = (l.points[0], l.points[1]);
        let band = RectMm { x0: a[0].min(b[0]) - 0.99, y0: a[1].min(b[1]) - 0.99, x1: a[0].max(b[0]) + 0.99, y1: a[1].max(b[1]) + 0.99 };
        let mut budget = usize::MAX;
        clear(design, band, &mut budget)
    }

    /// C18.24: a row of squares in a long box: smart lines run right across through the gaps,
    /// splitting the waste into strips no wider than the spacing, never near a square.
    #[test]
    fn smart_lines_split_the_waste_through_the_gaps() {
        let design: Vec<Polyline> = (0..6).map(|k| square(20.0 + 30.0 * f64::from(k), 50.0, 20.0)).collect();
        let (out, w) = weeding(&design, MAT, &smart(40.0));
        assert!(w.is_empty());
        let lines: Vec<&Polyline> = out.iter().filter(|l| !l.closed).collect();
        assert!(!lines.is_empty());
        // Every line runs from the box's top to its bottom (through a gap) and keeps clear.
        for l in &lines {
            assert_eq!((l.points[0][1], l.points[1][1]), (47.0, 73.0), "{l:?}");
            assert!(keeps_clear(l, &design), "{l:?}");
        }
        // The strips between them are no wider than 40 mm.
        let mut xs: Vec<f64> = lines.iter().map(|l| l.points[0][0]).chain([17.0, 193.0]).collect();
        xs.sort_by(f64::total_cmp);
        assert!(xs.windows(2).all(|p| p[1] - p[0] <= 40.0 + 1e-9), "{xs:?}");
        assert!(out.last().is_some_and(|p| p.closed), "the box last");
    }

    /// C18.26: with a strip size larger than the box, still a line through every gap between the
    /// squares (as between letters), clear of them; none without the option.
    #[test]
    fn lines_go_between_the_pieces() {
        let design: Vec<Polyline> = (0..4).map(|k| square(20.0 + 25.0 * f64::from(k), 50.0, 20.0)).collect();
        let s = CuttingSettings { weed_line_spacing_mm: 200.0, ..smart(0.0) };
        let (out, _) = weeding(&design, MAT, &s);
        let lines: Vec<&Polyline> = out.iter().filter(|l| !l.closed).collect();
        assert_eq!(lines.len(), 3, "one in each of the three gaps: {lines:?}");
        for l in &lines {
            assert!(keeps_clear(l, &design), "{l:?}");
            let x = l.points[0][0];
            assert!(((x - 20.0) % 25.0 - 22.5).abs() < 0.6, "in the middle of a gap: {x}");
        }
        let off = CuttingSettings { weed_lines_between: false, ..s };
        assert!(weeding(&design, MAT, &off).0.iter().all(|l| l.closed), "the box alone");
    }

    /// One large shape with no gap: lines come in from both sides and stop short of it; a small
    /// box needs no lines at all.
    #[test]
    fn without_a_gap_smart_lines_stop_short() {
        let design = [square(50.0, 50.0, 150.0)];
        let (out, _) = weeding(&design, MAT, &smart(0.0));
        let lines: Vec<&Polyline> = out.iter().filter(|l| !l.closed).collect();
        assert!(lines.len() >= 2, "{out:?}");
        for l in &lines {
            let inside = |q: [f64; 2]| q[0] > 50.0 && q[0] < 200.0 && q[1] > 50.0 && q[1] < 200.0;
            assert!(!inside(l.points[0]) && !inside(l.points[1]), "{l:?}");
        }
        let (small, _) = weeding(&[square(50.0, 50.0, 20.0)], MAT, &smart(0.0));
        assert_eq!(small.len(), 1, "the box alone: its waste comes off in one piece");
    }

    /// A design of very many segments stops at the budget with a warning, not a stall.
    #[test]
    fn a_huge_design_is_bounded() {
        let mut budget = 10;
        let design: Vec<Polyline> = (0..50).map(|k| square(10.0 + 5.0 * f64::from(k), 10.0, 2.0)).collect();
        let _ = smart_lines(&design, RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 }, 20.0, true, &mut budget);
        assert_eq!(budget, 0);
    }

    #[test]
    fn the_box_goes_round_the_design_a_margin_away_and_last() {
        let (out, w) = weeding(&[square(50.0, 50.0, 20.0)], MAT, &on(true, false));
        assert!(w.is_empty());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].points, [[47.0, 47.0], [73.0, 47.0], [73.0, 73.0], [47.0, 73.0]], "3 mm by default");
        assert!(out[0].closed && out[0].source == SOURCE);
        // Kept inside the area it may cut.
        let (out, _) = weeding(&[square(1.0, 1.0, 20.0)], MAT, &on(true, false));
        assert_eq!(out[0].points[0], [0.0, 0.0]);
        assert!(weeding(&[square(1.0, 1.0, 20.0)], MAT, &on(false, false)).0.is_empty(), "off by default");
        assert!(weeding(&[], MAT, &on(true, true)).0.is_empty(), "nothing to weed round");
    }

    /// One line in from the middle of each side, stopping short of the square; the box after them.
    #[test]
    fn weed_lines_run_in_to_the_design() {
        let (out, _) = weeding(&[square(50.0, 50.0, 20.0)], MAT, &on(true, true));
        assert_eq!(out.len(), 5, "four lines and the box: {out:?}");
        assert!(out.last().is_some_and(|p| p.closed), "the box last");
        let top = &out[0];
        assert_eq!(top.points, [[60.0, 47.0], [60.0, 50.0 - GAP_MM]], "from the box down to the square, short of it");
        for l in &out[..4] {
            assert!(!l.closed && l.points.len() == 2);
        }
    }

    /// Two squares with a gap between them: the line down the gap meets nothing and runs across;
    /// no line ever crosses a square.
    #[test]
    fn a_line_between_shapes_runs_across_and_none_cuts_into_one() {
        let design = [square(50.0, 50.0, 20.0), square(80.0, 50.0, 20.0)];
        let s = CuttingSettings { weed_line_spacing_mm: 5.0, ..on(true, true) };
        let (out, _) = weeding(&design, MAT, &s);
        let across = out.iter().filter(|l| !l.closed && l.points.first().map(|p| p[1]) == Some(47.0) && l.points.last().map(|p| p[1]) == Some(73.0));
        assert!(across.count() >= 1, "down the gap: {out:?}");
        for l in out.iter().filter(|l| !l.closed) {
            for sq in &design {
                let r = RectMm { x0: sq.points[0][0], y0: sq.points[0][1], x1: sq.points[2][0], y1: sq.points[2][1] };
                let inside = |q: &[f64; 2]| q[0] > r.x0 && q[0] < r.x1 && q[1] > r.y0 && q[1] < r.y1;
                let mid = [(l.points[0][0] + l.points[1][0]) / 2.0, (l.points[0][1] + l.points[1][1]) / 2.0];
                assert!(!inside(&l.points[0]) && !inside(&l.points[1]) && !inside(&mid), "{l:?} cuts into {r:?}");
            }
        }
    }

    #[test]
    fn line_positions_are_centred_and_capped() {
        assert_eq!(positions(0.0, 10.0, 0.0), [5.0]);
        assert_eq!(positions(0.0, 10.0, 4.0), [3.0, 7.0]);
        assert_eq!(positions(0.0, 3.0, 5.0), [1.5], "too short for the spacing: the middle");
        assert_eq!(positions(0.0, 1e9, 0.001).len(), MAX_LINES_PER_SIDE);
        assert_eq!(positions(0.0, 10.0, f64::NAN), [5.0]);
    }
}
