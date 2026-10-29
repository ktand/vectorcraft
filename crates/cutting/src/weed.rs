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
    if s.weed_lines {
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
        out.push(Polyline { op: OpKind::Cut, source: SOURCE, closed: true, points: corners });
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
    let line = |from: f64, to: f64| Polyline { op: OpKind::Cut, source: SOURCE, closed: false, points: vec![point(from), point(to)] };
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
        }
    }

    const MAT: RectMm = RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 };

    fn on(weed_box: bool, weed_lines: bool) -> CuttingSettings {
        CuttingSettings { weed_box, weed_lines, ..CuttingSettings::default() }
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
