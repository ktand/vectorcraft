//! Fork (C4.9, C4.14): a path as the machine's own curves, for a driver that can send them in place
//! of the path's points. The points stay in the job for the preview, the simulator, the history and
//! the drivers that can't: the curve is what they were flattened from, on the mat in millimetres.

/// One piece of a [`Native::Path`], from where the one before it ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    Line([f64; 2]),
    /// A cubic Bézier: two handles, then its end.
    Cubic([f64; 2], [f64; 2], [f64; 2]),
}

impl Piece {
    pub fn end(&self) -> [f64; 2] {
        match *self {
            Piece::Line(p) | Piece::Cubic(_, _, p) => p,
        }
    }

    /// The point at `t` (0 to 1) along it from `from`.
    pub fn at(&self, from: [f64; 2], t: f64) -> [f64; 2] {
        match *self {
            Piece::Line(p) => lerp(from, p, t),
            Piece::Cubic(a, b, p) => {
                let u = 1.0 - t;
                let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                [k0 * from[0] + k1 * a[0] + k2 * b[0] + k3 * p[0], k0 * from[1] + k1 * a[1] + k2 * b[1] + k3 * p[1]]
            }
        }
    }

    /// The part of it from `from` to `t` (0 to 1), as a piece of its own.
    pub fn first_part(&self, from: [f64; 2], t: f64) -> Piece {
        match *self {
            Piece::Line(p) => Piece::Line(lerp(from, p, t)),
            Piece::Cubic(a, b, p) => {
                // de Casteljau: the first half of the split at t.
                let (ab, bc, cd) = (lerp(from, a, t), lerp(a, b, t), lerp(b, p, t));
                let (abc, bcd) = (lerp(ab, bc, t), lerp(bc, cd, t));
                Piece::Cubic(ab, abc, lerp(abc, bcd, t))
            }
        }
    }

    /// Its length from `from`, measured along it in `steps` straight parts.
    fn length(&self, from: [f64; 2], steps: usize) -> f64 {
        match self {
            Piece::Line(p) => dist(from, *p),
            Piece::Cubic(..) => {
                let mut at = from;
                let mut sum = 0.0;
                for k in 1..=steps {
                    let q = self.at(from, k as f64 / steps as f64);
                    sum += dist(at, q);
                    at = q;
                }
                sum
            }
        }
    }
}

fn lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

/// Straight parts a cubic's length is measured in.
const LENGTH_STEPS: usize = 32;

/// A path as the machine's own curves, mat millimetres.
#[derive(Clone, Debug, PartialEq)]
pub enum Native {
    /// Cubic Bézier pieces end to end, each `[start, handle, handle, end]`, each starting where
    /// the one before ends (the curve test, C4.9).
    Bezier(Vec<[[f64; 2]; 4]>),
    /// A whole circle (the curve test).
    Circle { centre: [f64; 2], radius_mm: f64 },
    /// From `start`, lines and cubics end to end: a design's path as it is drawn (C4.14). A
    /// closed one ends back at `start`.
    Path { start: [f64; 2], pieces: Vec<Piece> },
}

impl Native {
    /// The first `mm` along a closed path, from its start: what its overcut cuts again. `None` for
    /// what isn't a path.
    pub fn overcut(&self, mm: f64) -> Option<Vec<Piece>> {
        let Native::Path { start, pieces } = self else { return None };
        let mut left = if mm.is_finite() { mm.max(0.0) } else { 0.0 };
        let mut out = vec![];
        let mut at = *start;
        for piece in pieces {
            if left <= 0.0 {
                break;
            }
            let len = piece.length(at, LENGTH_STEPS);
            if len <= 0.0 {
                at = piece.end();
                continue;
            }
            if left >= len {
                out.push(*piece);
                left -= len;
                at = piece.end();
                continue;
            }
            if let Piece::Line(q) = piece {
                out.push(Piece::Line(lerp(at, *q, left / len)));
                break;
            }
            // Where along a cubic the length runs out: found by halving, measured each time.
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..30 {
                let mid = (lo + hi) / 2.0;
                if piece.first_part(at, mid).length(at, LENGTH_STEPS) < left {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            out.push(piece.first_part(at, hi));
            break;
        }
        Some(out)
    }

    /// The same path cut the other way round. A circle stays as it is.
    pub fn reversed(&self) -> Native {
        match self {
            Native::Path { start, pieces } => {
                let ends: Vec<[f64; 2]> = std::iter::once(*start).chain(pieces.iter().map(Piece::end)).collect();
                let mut out = vec![];
                for (k, piece) in pieces.iter().enumerate().rev() {
                    let to = ends.get(k).copied().unwrap_or(*start);
                    out.push(match *piece {
                        Piece::Line(_) => Piece::Line(to),
                        Piece::Cubic(a, b, _) => Piece::Cubic(b, a, to),
                    });
                }
                Native::Path { start: ends.last().copied().unwrap_or(*start), pieces: out }
            }
            Native::Bezier(pieces) => Native::Bezier(pieces.iter().rev().map(|[a, b, c, d]| [*d, *c, *b, *a]).collect()),
            Native::Circle { .. } => self.clone(),
        }
    }

    /// A closed path started at the start of piece `k` instead, going round the same way.
    pub fn started_at(&self, k: usize) -> Native {
        match self {
            Native::Path { start, pieces } if k > 0 && k < pieces.len() => {
                let from = pieces.get(k - 1).map_or(*start, Piece::end);
                let mut out: Vec<Piece> = pieces.get(k..).unwrap_or_default().to_vec();
                out.extend_from_slice(pieces.get(..k).unwrap_or_default());
                Native::Path { start: from, pieces: out }
            }
            _ => self.clone(),
        }
    }

    /// Every point that defines it, handles included, for checks of where it lies.
    pub fn points(&self) -> Vec<[f64; 2]> {
        match self {
            Native::Bezier(pieces) => pieces.iter().flatten().copied().collect(),
            Native::Circle { centre, radius_mm } => {
                let r = *radius_mm;
                vec![[centre[0] - r, centre[1] - r], [centre[0] + r, centre[1] + r]]
            }
            Native::Path { start, pieces } => std::iter::once(*start)
                .chain(pieces.iter().flat_map(|p| match *p {
                    Piece::Line(q) => vec![q],
                    Piece::Cubic(a, b, q) => vec![a, b, q],
                }))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Native {
        Native::Path {
            start: [0.0, 0.0],
            pieces: vec![Piece::Line([10.0, 0.0]), Piece::Line([10.0, 10.0]), Piece::Line([0.0, 10.0]), Piece::Line([0.0, 0.0])],
        }
    }

    #[test]
    fn the_overcut_runs_on_along_the_start() {
        assert_eq!(square().overcut(4.0), Some(vec![Piece::Line([4.0, 0.0])]));
        assert_eq!(square().overcut(12.0), Some(vec![Piece::Line([10.0, 0.0]), Piece::Line([10.0, 2.0])]));
        assert_eq!(square().overcut(0.0), Some(vec![]));
        // A quarter circle of radius 10 (length ~15.71): 5 mm along it ends a third of the way round.
        let k = 0.552_284_749_830_793_4 * 10.0;
        let arc = Native::Path { start: [10.0, 0.0], pieces: vec![Piece::Cubic([10.0, k], [k, 10.0], [0.0, 10.0])] };
        let Some(v) = arc.overcut(5.0) else { panic!() };
        let end = v[0].end();
        let angle = end[1].atan2(end[0]);
        assert!((angle - 0.5).abs() < 0.01 && (end[0].hypot(end[1]) - 10.0).abs() < 0.01, "{end:?}");
        assert!(Native::Circle { centre: [0.0, 0.0], radius_mm: 1.0 }.overcut(1.0).is_none());
    }

    #[test]
    fn reversed_and_restarted() {
        let r = square().reversed();
        assert_eq!(
            r,
            Native::Path {
                start: [0.0, 0.0],
                pieces: vec![Piece::Line([0.0, 10.0]), Piece::Line([10.0, 10.0]), Piece::Line([10.0, 0.0]), Piece::Line([0.0, 0.0])]
            }
        );
        let open = Native::Path { start: [0.0, 0.0], pieces: vec![Piece::Cubic([1.0, 0.0], [2.0, 1.0], [3.0, 3.0]), Piece::Line([5.0, 3.0])] };
        assert_eq!(
            open.reversed(),
            Native::Path { start: [5.0, 3.0], pieces: vec![Piece::Line([3.0, 3.0]), Piece::Cubic([2.0, 1.0], [1.0, 0.0], [0.0, 0.0])] }
        );
        assert_eq!(open.reversed().reversed(), open);
        let s = square().started_at(2);
        assert_eq!(
            s,
            Native::Path {
                start: [10.0, 10.0],
                pieces: vec![Piece::Line([0.0, 10.0]), Piece::Line([0.0, 0.0]), Piece::Line([10.0, 0.0]), Piece::Line([10.0, 10.0])]
            }
        );
        assert_eq!(square().started_at(0), square());
        assert_eq!(square().started_at(9), square());
    }
}
