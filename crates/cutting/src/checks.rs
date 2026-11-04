//! Fork: checks before cutting (C18.2). The job's paths, laid on the mat in mm, looked over for
//! what usually spoils a cut, each found reported once per object as a warning naming it (the
//! Send window selects it on a click):
//!
//! - parts narrower than the blade cuts cleanly (`MIN_WIDTH_MM`): what an opening (shrinking by
//!   half the width, then growing back) loses;
//! - pieces and holes too small to weed (`MIN_PIECE_MM` across at most);
//! - the same line twice (cut twice, which can cut through the backing);
//! - pieces of one object that overlap without being welded (script letters cut through each
//!   other); set the object to Cut Outline to weld them;
//! - open paths set to Cut (they leave a slit, not a piece).

use std::collections::{BTreeMap, BTreeSet};

use vectorcraft_cutjob::{JobWarning, OpKind, SourceRef};
use vectorcraft_geom::{FillRule, PathData, Point, Rect, SubPath};
use vectorcraft_pathops::{BoolOp, DEFAULT_PRECISION, Join, offset_path, try_area, try_boolean};

use crate::prepare::Polyline;

/// Parts narrower than this are reported, mm.
pub const MIN_WIDTH_MM: f64 = 0.5;
/// Pieces and holes no larger than this either way are reported, mm.
pub const MIN_PIECE_MM: f64 = 2.0;
/// At most this many closed paths are looked at for narrow parts and overlaps (the slower checks).
const MAX_SHAPES: usize = 500;

fn region(p: &Polyline) -> PathData {
    PathData::single(SubPath::polyline(&p.points.iter().map(|q| Point::new(q[0], q[1])).collect::<Vec<_>>(), true))
}

fn bounds(p: &Polyline) -> Rect {
    p.points.iter().fold(Rect::new(f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |r, q| {
        Rect::new(r.x0.min(q[0]), r.y0.min(q[1]), r.x1.max(q[0]), r.y1.max(q[1]))
    })
}

/// The same line: the same points in the same or the reverse order (a closed one from any
/// start), to a hundredth of a mm.
fn same_line(a: &Polyline, b: &Polyline) -> bool {
    if a.closed != b.closed || a.points.len() != b.points.len() || a.points.is_empty() {
        return false;
    }
    let near = |p: &[f64; 2], q: &[f64; 2]| (p[0] - q[0]).abs() < 0.01 && (p[1] - q[1]).abs() < 0.01;
    let n = a.points.len();
    let starts: Vec<usize> = if a.closed { (0..n).filter(|&k| near(&a.points[0], &b.points[k])).collect() } else { vec![0, n - 1] };
    starts.into_iter().any(|k| {
        let forward = (0..n).all(|i| near(&a.points[i], &b.points[(k + i) % n]));
        let back = (0..n).all(|i| near(&a.points[i], &b.points[(k + n - i) % n]));
        forward || back
    })
}

/// The checks' warnings for `paths` (the job's, mm), each once per object and kind.
pub fn checks(paths: &[Polyline]) -> Vec<JobWarning> {
    let mut found: BTreeMap<(u64, &'static str), SourceRef> = BTreeMap::new();
    let mut say = |source: SourceRef, what: &'static str| {
        found.entry((source.node, what)).or_insert(source);
    };
    let cuts: Vec<&Polyline> = paths.iter().filter(|p| p.op == OpKind::Cut).collect();
    for p in &cuts {
        if !p.closed {
            say(p.source, OPEN);
            continue;
        }
        let b = bounds(p);
        if b.width() <= MIN_PIECE_MM && b.height() <= MIN_PIECE_MM {
            say(p.source, TINY);
        }
    }
    // The same line twice: neighbours by their first point.
    let mut by_start: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    for (i, p) in paths.iter().enumerate() {
        if let Some(b) = p.points.iter().copied().reduce(|a, q| if (q[0], q[1]) < (a[0], a[1]) { q } else { a }) {
            by_start.entry(((b[0] * 50.0).round() as i64, (b[1] * 50.0).round() as i64)).or_default().push(i);
        }
    }
    let mut twice: BTreeSet<usize> = BTreeSet::new();
    for group in by_start.values() {
        for (k, &i) in group.iter().enumerate() {
            for &j in &group[k + 1..] {
                if paths[i].op == paths[j].op && same_line(&paths[i], &paths[j]) && twice.insert(j) {
                    say(paths[j].source, TWICE);
                }
            }
        }
    }
    // Narrow parts and an object's overlapping pieces (closed cuts, the first MAX_SHAPES).
    let closed: Vec<&&Polyline> = cuts.iter().filter(|p| p.closed && p.points.len() >= 3).take(MAX_SHAPES).collect();
    let half = MIN_WIDTH_MM / 2.0;
    for p in &closed {
        let b = bounds(p);
        if b.width() <= MIN_PIECE_MM && b.height() <= MIN_PIECE_MM {
            continue;
        }
        let r = region(p);
        let opened = offset_path(&offset_path(&r, -half, Join::Miter, 4.0), half, Join::Miter, 4.0);
        let (Ok(all), Ok(kept)) = (try_area(&r, FillRule::NonZero), try_area(&opened, FillRule::NonZero)) else { continue };
        // More than a sliver lost: a part narrower than the blade cuts.
        if all.abs() - kept.abs() > (MIN_WIDTH_MM * MIN_WIDTH_MM).max(all.abs() * 0.002) {
            say(p.source, NARROW);
        }
    }
    let mut by_node: BTreeMap<u64, Vec<&Polyline>> = BTreeMap::new();
    for p in &closed {
        by_node.entry(p.source.node).or_default().push(p);
    }
    for pieces in by_node.values() {
        'pairs: for (k, a) in pieces.iter().enumerate() {
            let ba = bounds(a);
            for b in &pieces[k + 1..] {
                let bb = bounds(b);
                let inter = ba.intersect(bb);
                if inter.width() <= 0.0 || inter.height() <= 0.0 {
                    continue;
                }
                let (ra, rb) = (region(a), region(b));
                let Ok(both) = try_boolean(&ra, FillRule::NonZero, &rb, FillRule::NonZero, BoolOp::Intersect, DEFAULT_PRECISION) else { continue };
                let (Ok(ab), Ok(aa), Ok(ab_b)) =
                    (try_area(&both, FillRule::NonZero), try_area(&ra, FillRule::NonZero), try_area(&rb, FillRule::NonZero))
                else {
                    continue;
                };
                let (ab, small) = (ab.abs(), aa.abs().min(ab_b.abs()));
                // Overlapping, but neither wholly inside the other (a hole, an O's counter).
                if ab > 0.01 && ab < small * 0.999 {
                    say(a.source, OVERLAP);
                    break 'pairs;
                }
            }
        }
    }
    found.into_iter().map(|((_, what), source)| JobWarning { source: Some(source), message: what.to_string() }).collect()
}

const OPEN: &str = "an open path is set to Cut: it cuts a slit, not a piece (close it, or set it to Score or Perforate)";
const TINY: &str = "a piece or hole smaller than 2 mm: hard to weed, and it may not cut cleanly";
const TWICE: &str = "the same line is cut twice (two copies on top of each other): it may cut through the backing";
const NARROW: &str = "a part narrower than 0.5 mm: the blade may tear it rather than cut it";
const OVERLAP: &str = "pieces of this object overlap and are cut through each other: set it to Cut Outline to weld them";

#[cfg(test)]
mod tests {
    use super::*;

    fn poly(node: u64, closed: bool, pts: &[[f64; 2]]) -> Polyline {
        Polyline { op: OpKind::Cut, source: SourceRef { node, subpath: 0 }, closed, points: pts.to_vec(), curve: None }
    }

    fn square(node: u64, x: f64, y: f64, s: f64) -> Polyline {
        poly(node, true, &[[x, y], [x + s, y], [x + s, y + s], [x, y + s]])
    }

    fn said(paths: &[Polyline]) -> Vec<&'static str> {
        let mut out: Vec<&'static str> =
            checks(paths).iter().map(|w| [OPEN, TINY, TWICE, NARROW, OVERLAP].into_iter().find(|m| *m == w.message).unwrap_or("?")).collect();
        out.sort_unstable();
        out
    }

    #[test]
    fn a_clean_job_says_nothing() {
        assert!(said(&[square(1, 0.0, 0.0, 20.0), square(2, 30.0, 0.0, 20.0), square(2, 35.0, 5.0, 5.0)]).is_empty(), "a hole inside is fine");
    }

    #[test]
    fn each_problem_is_found() {
        assert_eq!(said(&[poly(1, false, &[[0.0, 0.0], [10.0, 0.0]])]), vec![OPEN]);
        assert_eq!(said(&[square(1, 0.0, 0.0, 1.5)]), vec![TINY]);
        let mut back = square(2, 0.0, 0.0, 20.0);
        back.points.reverse();
        assert_eq!(said(&[square(1, 0.0, 0.0, 20.0), back]), vec![TWICE]);
        // A 20 mm bar 0.3 mm wide.
        assert_eq!(said(&[poly(1, true, &[[0.0, 0.0], [20.0, 0.0], [20.0, 0.3], [0.0, 0.3]])]), vec![NARROW]);
        // Two squares of one object overlapping (script letters).
        assert_eq!(said(&[square(1, 0.0, 0.0, 20.0), square(1, 10.0, 10.0, 20.0)]), vec![OVERLAP]);
        // Of two objects: not this check's business (they are cut as drawn).
        assert!(said(&[square(1, 0.0, 0.0, 20.0), square(2, 10.0, 10.0, 20.0)]).is_empty());
    }
}
