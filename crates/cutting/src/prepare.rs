//! Outlines laid on the mat as polylines (ARCHITECTURE.md › 5.1, steps 4–6): document points to
//! mat millimetres ([`Placement::doc_to_mat`]), curves flattened to the settings' tolerance,
//! repeated and collinear points merged, and what has no length dropped. A path that leaves the
//! cut area stops the job, naming its object; with `clipToMat` it is cut up to the area's edge
//! instead, in pieces, with a warning.

use vectorcraft_cutjob::{JobWarning, MAX_POINTS, OpKind, RectMm, SourceRef};
use vectorcraft_geom::PathEl;

use crate::geometry::Outline;
use crate::placement::Placement;

/// One stroke of the blade, in mat millimetres.
#[derive(Clone, Debug, PartialEq)]
pub struct Polyline {
    pub op: OpKind,
    pub source: SourceRef,
    /// Back to its first point at the end (which isn't repeated).
    pub closed: bool,
    pub points: Vec<[f64; 2]>,
}

/// Why outlines can't be laid on the mat.
#[derive(Clone, Debug, PartialEq)]
pub enum PrepError {
    /// The object's outline leaves the cut area at this point (mm), and `clipToMat` is off.
    OffTheCutArea { source: SourceRef, point: [f64; 2] },
    /// Flattened, the outlines need more points than a job holds.
    TooManyPoints,
}

impl std::fmt::Display for PrepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrepError::OffTheCutArea { source, point } => write!(
                f,
                "object {} goes off the mat's cut area at ({:.1}, {:.1}) mm: move it, or turn on Clip to Mat",
                source.node, point[0], point[1]
            ),
            PrepError::TooManyPoints => write!(f, "the art needs more than {MAX_POINTS} points to cut: simplify it or raise the tolerance"),
        }
    }
}

impl std::error::Error for PrepError {}

/// Points closer than this (mm) are one point; a point off the line through its neighbours by
/// less than this is on it.
const SAME_MM: f64 = 1e-6;

/// `outlines` laid on the mat by `placement`, flattened to `tolerance_mm`.
pub fn prepare(
    outlines: &[Outline],
    placement: &Placement,
    tolerance_mm: f64,
    clip_to_mat: bool,
) -> Result<(Vec<Polyline>, Vec<JobWarning>), PrepError> {
    let tolerance = if tolerance_mm.is_finite() && tolerance_mm > 0.0 { tolerance_mm } else { 0.025 };
    let area = placement.mat.cut_area_mm;
    let (mut out, mut warnings) = (vec![], vec![]);
    let mut budget = MAX_POINTS;
    let mut clipped: Vec<SourceRef> = vec![];
    let mut invalid: Vec<SourceRef> = vec![];
    for o in outlines {
        let on_mat = placement.doc_to_mat * o.path.clone();
        let mut points: Vec<[f64; 2]> = vec![];
        let mut finite = true;
        vectorcraft_geom::flatten(on_mat.elements().iter().copied(), tolerance, |el| match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                finite &= p.x.is_finite() && p.y.is_finite();
                if points.len() < budget {
                    points.push([p.x, p.y]);
                }
            }
            _ => {}
        });
        if !finite {
            invalid.push(o.source);
            continue;
        }
        if points.len() >= budget {
            return Err(PrepError::TooManyPoints);
        }
        let Some(p) = tidy(o, points) else { continue };
        if let Some(point) = p.points.iter().copied().find(|q| !inside(*q, area)) {
            if !clip_to_mat {
                return Err(PrepError::OffTheCutArea { source: o.source, point });
            }
            if clipped.last() != Some(&SourceRef { subpath: 0, ..o.source }) {
                clipped.push(SourceRef { subpath: 0, ..o.source });
            }
            for piece in clip(&p, area) {
                budget = budget.saturating_sub(piece.points.len());
                out.push(piece);
            }
            continue;
        }
        budget = budget.saturating_sub(p.points.len());
        out.push(p);
    }
    for source in clipped {
        warnings.push(JobWarning { source: Some(source), message: "goes off the mat's cut area: cut up to its edge".into() });
    }
    for source in invalid {
        warnings.push(JobWarning { source: Some(source), message: "an outline with coordinates that aren't numbers is left out".into() });
    }
    Ok((out, warnings))
}

fn inside([x, y]: [f64; 2], a: RectMm) -> bool {
    x >= a.x0 - SAME_MM && y >= a.y0 - SAME_MM && x <= a.x1 + SAME_MM && y <= a.y1 + SAME_MM
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// `b` lies on the segment from `a` to `c`: off its line by less than [`SAME_MM`], between them.
fn between(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let (ux, uy) = (c[0] - a[0], c[1] - a[1]);
    let len = ux.hypot(uy);
    if len < SAME_MM {
        return false;
    }
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let off = (ux * vy - uy * vx).abs() / len;
    let along = (ux * vx + uy * vy) / len;
    off < SAME_MM && along > 0.0 && along < len
}

/// `points` of outline `o` without repeats or points on a straight run; `None` when nothing
/// with length is left. A closed outline loses its repeated closing point, and one left with two
/// points is cut as a line.
fn tidy(o: &Outline, points: Vec<[f64; 2]>) -> Option<Polyline> {
    let mut kept: Vec<[f64; 2]> = Vec::with_capacity(points.len());
    for p in points {
        if kept.last().is_some_and(|q| dist(*q, p) < SAME_MM) {
            continue;
        }
        if let [.., a, b] = kept[..]
            && between(a, b, p)
        {
            kept.pop();
        }
        kept.push(p);
    }
    let mut closed = o.closed;
    if closed {
        if let (Some(first), Some(last)) = (kept.first().copied(), kept.last().copied())
            && kept.len() > 1
            && dist(first, last) < SAME_MM
        {
            kept.pop();
        }
        // The closing point back at the start may be on the run into it, or out of it.
        if let [.., a, b] = kept[..]
            && let Some(&first) = kept.first()
            && between(a, b, first)
        {
            kept.pop();
        }
        if let [first, second, .., last] = kept[..]
            && between(last, first, second)
        {
            kept.remove(0);
        }
        if kept.len() < 3 {
            closed = false;
        }
    }
    let length: f64 = kept.windows(2).map(|w| dist(w[0], w[1])).sum();
    (kept.len() >= 2 && length >= SAME_MM).then_some(Polyline { op: o.op, source: o.source, closed, points: kept })
}

/// The parts of segment `a`–`b` inside `r` (Liang–Barsky): its ends there, or `None`.
fn clip_segment(a: [f64; 2], b: [f64; 2], r: RectMm) -> Option<([f64; 2], [f64; 2])> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a[0] - r.x0), (dx, r.x1 - a[0]), (-dy, a[1] - r.y0), (dy, r.y1 - a[1])] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    let at = |t: f64| [a[0] + t * dx, a[1] + t * dy];
    Some((at(t0), at(t1)))
}

/// `p` cut to `r`: the runs of it inside, each an open piece.
fn clip(p: &Polyline, r: RectMm) -> Vec<Polyline> {
    let mut pts = p.points.clone();
    if p.closed
        && let Some(&first) = pts.first()
    {
        pts.push(first);
    }
    let mut pieces: Vec<Vec<[f64; 2]>> = vec![];
    let mut cur: Vec<[f64; 2]> = vec![];
    for w in pts.windows(2) {
        let [a, b] = [w[0], w[1]];
        match clip_segment(a, b, r) {
            Some((s, e)) => {
                if cur.last().is_none_or(|q| dist(*q, s) >= SAME_MM) {
                    if cur.len() >= 2 {
                        pieces.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    cur.push(s);
                }
                cur.push(e);
                // Left the area at `e`: the piece ends there.
                if dist(e, b) >= SAME_MM {
                    pieces.push(std::mem::take(&mut cur));
                }
            }
            None => {
                if cur.len() >= 2 {
                    pieces.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
    }
    if cur.len() >= 2 {
        pieces.push(cur);
    }
    pieces
        .into_iter()
        .filter(|pc| pc.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>() >= SAME_MM)
        .map(|points| Polyline { op: p.op, source: p.source, closed: false, points })
        .collect()
}

#[cfg(test)]
mod tests;
