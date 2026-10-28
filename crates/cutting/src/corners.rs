//! Line segment overcut (C5.2): a drag blade trails its axis, so at a sharp corner it rounds the
//! corner off and leaves it uncut. With an operation's `cornerOvercutMm` set, at each corner
//! that turns by more than [`SHARP_DEG`] the blade runs on past it along the way it came by that
//! much, then back to the corner, and on: the corner is cut through. A run that would leave the
//! cut area isn't made. A closed path's own start, where the cut ends, is the regular overcut's.

use vectorcraft_cutjob::{CutJob, RectMm};

/// Corners turning by more than this, degrees, are overcut.
pub const SHARP_DEG: f64 = 30.0;

fn inside([x, y]: [f64; 2], a: RectMm) -> bool {
    x >= a.x0 && y >= a.y0 && x <= a.x1 && y <= a.y1
}

/// The turn at `b` from `a` to `c`, degrees (0: straight on), and the unit way into `b`.
fn turn(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Option<(f64, [f64; 2])> {
    let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
    let (lu, lv) = (u[0].hypot(u[1]), v[0].hypot(v[1]));
    if lu <= 0.0 || lv <= 0.0 {
        return None;
    }
    let cos = ((u[0] * v[0] + u[1] * v[1]) / (lu * lv)).clamp(-1.0, 1.0);
    Some((cos.acos().to_degrees(), [u[0] / lu, u[1] / lu]))
}

/// `points` with each sharp corner run past by `mm` and back (closed: round to the start, which
/// is left as it is).
fn overcut(points: &[[f64; 2]], closed: bool, mm: f64, area: RectMm) -> Vec<[f64; 2]> {
    let n = points.len();
    let mut out = Vec::with_capacity(n * 3);
    for (i, &b) in points.iter().enumerate() {
        out.push(b);
        let (Some(&a), next) = (
            if i == 0 { None } else { points.get(i - 1) },
            if i + 1 < n {
                points.get(i + 1)
            } else if closed {
                points.first()
            } else {
                None
            },
        ) else {
            continue;
        };
        let Some(&c) = next else { continue };
        if let Some((deg, dir)) = turn(a, b, c)
            && deg > SHARP_DEG
        {
            let past = [b[0] + dir[0] * mm, b[1] + dir[1] * mm];
            if inside(past, area) {
                out.push(past);
                out.push(b);
            }
        }
    }
    out
}

/// Each of `job`'s paths whose operation sets a corner overcut, with its sharp corners overcut.
pub fn overcut_corners(job: &mut CutJob) {
    let area = job.mat.cut_area_mm;
    for p in &mut job.paths {
        let Some(mm) = job.operations.get(p.op).map(|o| o.tool.corner_overcut_mm).filter(|mm| mm.is_finite() && *mm > 0.0) else { continue };
        p.points = overcut(&p.points, p.closed, mm, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: RectMm = RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 };

    /// A square: its three corners after the start each run past by 1 mm along the way in, and back.
    #[test]
    fn a_squares_corners_are_cut_through() {
        let sq = [[10.0, 10.0], [20.0, 10.0], [20.0, 20.0], [10.0, 20.0]];
        let got = overcut(&sq, true, 1.0, AREA);
        assert_eq!(
            got,
            [
                [10.0, 10.0],
                [20.0, 10.0],
                [21.0, 10.0],
                [20.0, 10.0],
                [20.0, 20.0],
                [20.0, 21.0],
                [20.0, 20.0],
                [10.0, 20.0],
                [9.0, 20.0],
                [10.0, 20.0]
            ]
        );
    }

    /// A gentle bend isn't a corner; an open path's ends aren't either.
    #[test]
    fn gentle_bends_and_ends_are_left() {
        let bend = [[0.0, 0.0], [10.0, 0.0], [20.0, 2.0]];
        assert_eq!(overcut(&bend, false, 1.0, AREA), bend);
        let hook = [[50.0, 50.0], [60.0, 50.0], [60.0, 60.0]];
        assert_eq!(overcut(&hook, false, 1.0, AREA).len(), 5, "the one corner between");
    }

    /// At the cut area's edge the run past would leave it: not made.
    #[test]
    fn no_run_off_the_cut_area() {
        let edge = [[290.0, 10.0], [300.0, 10.0], [300.0, 20.0]];
        assert_eq!(overcut(&edge, false, 1.0, AREA), edge);
    }
}
