//! Outlines laid on the mat as polylines (ARCHITECTURE.md › 5.1, steps 4–6): document points to
//! mat millimetres ([`Placement::doc_to_mat`]), curves flattened to the settings' tolerance,
//! repeated and collinear points merged, and what has no length dropped. A path that leaves the
//! cut area stops the job, naming its object; with `clipToMat` it is cut up to the area's edge
//! instead, in pieces, with a warning.

use vectorcraft_cutjob::{JobWarning, MAX_POINTS, Native, OpKind, Piece, RectMm, SourceRef};
use vectorcraft_geom::{BezPath, PathEl};

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
    /// Fork (C4.14): the curve the points were flattened from, for a machine that cuts curves;
    /// `None` once a step changes the points other than by starting or reversing them.
    pub curve: Option<Box<Curve>>,
    /// Fork (C4.15): cut in dashes of this cut and gap, mm, from its start, by a machine that
    /// dashes by itself where it can: kept whole until the job is made, then cut into its dashes.
    pub dash: Option<[f64; 2]>,
}

/// Fork (C4.14): a polyline's curve, and where its pieces start among the points.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub native: Native,
    /// `(point, piece)`: the point where each kept piece starts, in order; the first is `(0, 0)`.
    /// A closed path may be started only at these, so its curve can start there too.
    pub starts: Vec<(usize, usize)>,
}

impl Polyline {
    /// Where a closed path may start: any point, or with a curve the points where a piece starts.
    pub(crate) fn start_points(&self) -> Vec<usize> {
        match &self.curve {
            Some(c) => c.starts.iter().map(|s| s.0).collect(),
            None => (0..self.points.len()).collect(),
        }
    }

    /// A closed path started at point `i` instead, its curve with it.
    pub(crate) fn start_at(&mut self, i: usize) {
        if i == 0 || i >= self.points.len() {
            return;
        }
        self.points.rotate_left(i);
        let n = self.points.len();
        self.curve = self.curve.take().and_then(|c| {
            let &(_, k) = c.starts.iter().find(|s| s.0 == i)?;
            let Native::Path { pieces, .. } = &c.native else { return None };
            let m = pieces.len();
            let mut starts: Vec<(usize, usize)> = c.starts.iter().map(|&(p, q)| ((p + n - i) % n, (q + m - k) % m)).collect();
            starts.sort_unstable();
            Some(Box::new(Curve { native: c.native.started_at(k), starts }))
        });
    }

    /// An open path cut the other way round, its curve with it.
    pub(crate) fn reverse(&mut self) {
        self.points.reverse();
        self.curve =
            if self.closed { None } else { self.curve.take().map(|c| Box::new(Curve { native: c.native.reversed(), starts: vec![(0, 0)] })) };
    }
}

/// Fork (C4.14): `path` (one subpath, on the mat) as lines and cubics, and where among `points`
/// (what it was flattened and tidied to) each piece starts. `None` when the points don't start
/// where the curve does.
fn curve_of(path: &BezPath, closed: bool, points: &[[f64; 2]]) -> Option<Box<Curve>> {
    let mut start: Option<[f64; 2]> = None;
    let mut at = [0.0, 0.0];
    let mut pieces = vec![];
    let xy = |p: vectorcraft_geom::Point| [p.x, p.y];
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                if start.is_some() {
                    // A second subpath: not one stroke.
                    return None;
                }
                start = Some(xy(p));
                at = xy(p);
            }
            PathEl::LineTo(p) => {
                pieces.push(Piece::Line(xy(p)));
                at = xy(p);
            }
            PathEl::QuadTo(q, p) => {
                // The same curve as a cubic: its handles two thirds of the way to the control point.
                let (q, p) = (xy(q), xy(p));
                let third = |a: [f64; 2]| [a[0] + 2.0 / 3.0 * (q[0] - a[0]), a[1] + 2.0 / 3.0 * (q[1] - a[1])];
                pieces.push(Piece::Cubic(third(at), third(p), p));
                at = p;
            }
            PathEl::CurveTo(a, b, p) => {
                pieces.push(Piece::Cubic(xy(a), xy(b), xy(p)));
                at = xy(p);
            }
            PathEl::ClosePath => {}
        }
    }
    let start = start?;
    if closed && dist(at, start) >= SAME_MM {
        pieces.push(Piece::Line(start));
    }
    if pieces.is_empty()
        || !pieces
            .iter()
            .flat_map(|p| match *p {
                Piece::Line(q) => vec![q],
                Piece::Cubic(a, b, q) => vec![a, b, q],
            })
            .all(|q| q.iter().all(|v| v.is_finite()))
    {
        return None;
    }
    // Each piece's start among the points, in order.
    let mut starts = vec![];
    let mut from = 0;
    let mut piece_start = start;
    for (k, piece) in pieces.iter().enumerate() {
        if let Some(i) = points.iter().enumerate().skip(from).find(|(_, q)| dist(**q, piece_start) < SAME_MM).map(|(i, _)| i) {
            starts.push((i, k));
            from = i;
        }
        piece_start = piece.end();
    }
    if starts.first() != Some(&(0, 0)) {
        return None;
    }
    starts.dedup_by_key(|s| s.0);
    Some(Box::new(Curve { native: Native::Path { start, pieces }, starts }))
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
        let Some(mut p) = tidy(o, points) else { continue };
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
        // Whole on the mat: its curve kept beside its points (C4.14). A clipped one has none.
        p.curve = curve_of(&on_mat, p.closed, &p.points);
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
    (kept.len() >= 2 && length >= SAME_MM).then_some(Polyline { op: o.op, source: o.source, closed, points: kept, curve: None, dash: None })
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
        .map(|points| Polyline { op: p.op, source: p.source, closed: false, points, curve: None, dash: None })
        .collect()
}

#[cfg(test)]
mod tests;
