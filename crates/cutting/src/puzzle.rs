//! Fork: puzzles (C13.5). A jigsaw's cut lines over a rectangle (the selection's bounds, a printed
//! picture included, or the artboard): its border, and the lines between the pieces, each piece's
//! side with a knob, a round head on a narrow neck, sticking out one way or the other. The knob's
//! outline is drawn here (a neck flaring into a near-circle made of arcs; not taken from
//! anywhere). Three amounts shape it, as the maker's software names them (protocol notes [ST]):
//! `curve`, the knob's size; `ripple`, how much the side waves either side of the knob; `wobble`,
//! how far the grid's inner corners and the knobs stray from even, so no two pieces are alike.
//! `curvature` off makes a plain grid. A seed picks the strays: the same settings and seed always
//! make the same puzzle. In document points.

use vectorcraft_geom::{BezPath, PathData, Point, Rect};

/// Rows and columns: the range.
pub const ROWS: (u32, u32) = (2, 50);
/// Most pieces in all.
const MAX_PIECES: u32 = 2_500;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub rows: u32,
    pub cols: u32,
    /// Knobs and waves; off, a plain grid.
    pub curvature: bool,
    /// The knob's size, 0 (small) to 1 (full).
    pub curve: f64,
    /// How much a side waves either side of its knob, 0 to 1.
    pub ripple: f64,
    /// How far the grid's corners and the knobs stray from even, 0 to 1.
    pub wobble: f64,
    /// Which puzzle of all those the settings can make.
    pub seed: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options { rows: 4, cols: 4, curvature: true, curve: 1.0, ripple: 1.0, wobble: 1.0, seed: 1 }
    }
}

/// A small deterministic generator (xorshift32): the same seed, the same numbers.
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Rng {
        Rng(seed.wrapping_mul(2_654_435_761).max(1))
    }

    /// A number in −1..1.
    fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        f64::from(x) / f64::from(u32::MAX) * 2.0 - 1.0
    }
}

/// One side's shape, in units of the side's length: where along it the knob sits, which way it
/// sticks out (+1 to the left of the side's direction on the page), and its size and waves.
#[derive(Clone, Copy, Debug)]
struct Knob {
    mid: f64,
    out: f64,
    curve: f64,
    ripple: f64,
}

/// The cubic curves of a side from (0, 0) to (1, 0) with `k`'s knob, as (along, out) triples of
/// control, control, end.
fn knob_curves(k: Knob) -> Vec<[(f64, f64); 3]> {
    // The head: a circle of radius `r` whose centre stands `cy` out, on a neck `n` wide each side.
    let s = 0.5 + 0.5 * k.curve.clamp(0.0, 1.0);
    let (r, n, m) = (0.10 * s, 0.045 * s, k.mid);
    let cy = 0.12 * s + r * 0.35;
    // Where the neck meets the head: the circle at 210° and −30° (y up).
    let (c30, s30) = (3f64.sqrt() / 2.0, 0.5);
    let j1 = (m - r * c30, cy - r * s30);
    let j2 = (m + r * c30, cy - r * s30);
    let top = (m, cy + r);
    // A 120° arc's handles: 4/3·tan(30°) of the radius, along each end's tangent.
    let h = 4.0 / 3.0 * (std::f64::consts::PI / 6.0).tan() * r;
    let (a, b) = (m - n - 0.06, m + n + 0.06);
    let w = 0.035 * k.ripple.clamp(0.0, 1.0);
    vec![
        // The shoulder: waving down, then back up to the neck's foot.
        [(a * 0.35, -w), (a * 0.75, w * 0.6), (a, 0.0)],
        // The neck rising, narrowing in, to the head.
        [(m - n + 0.01, 0.0), (m - n - 0.005, j1.1 - r * 0.45), j1],
        // Round the head: 210° to 90°, then 90° to −30°.
        [(j1.0 - h * s30, j1.1 + h * c30), (top.0 - h, top.1), top],
        [(top.0 + h, top.1), (j2.0 + h * s30, j2.1 + h * c30), j2],
        // Down the neck, and the other shoulder.
        [(m + n + 0.005, j2.1 - r * 0.45), (m + n - 0.01, 0.0), (b, 0.0)],
        [(b + (1.0 - b) * 0.25, w * 0.6), (b + (1.0 - b) * 0.65, -w), (1.0, 0.0)],
    ]
}

/// Adds to `p` (already at `a`) the side from `a` to `b` shaped as `k` says (a straight line
/// without one).
fn side(p: &mut BezPath, a: Point, b: Point, k: Option<Knob>) {
    let Some(k) = k else {
        p.line_to(b);
        return;
    };
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    // Across the side, scaled with it: its left on the page (y down) when `out` is +1.
    let (nx, ny) = (dy * k.out, -dx * k.out);
    let at = |(t, u): (f64, f64)| Point::new(a.x + dx * t + nx * u, a.y + dy * t + ny * u);
    for [c1, c2, e] in knob_curves(k) {
        p.curve_to(at(c1), at(c2), at(e));
    }
}

/// The puzzle's cut lines over `r`: the border, then each line between rows and between columns,
/// or why it can't be made.
pub fn puzzle(r: Rect, opt: Options) -> Result<PathData, String> {
    let ok = |n: u32| (ROWS.0..=ROWS.1).contains(&n);
    if !ok(opt.rows) || !ok(opt.cols) || opt.rows * opt.cols > MAX_PIECES {
        return Err(format!("rows and columns are {} to {}, at most {MAX_PIECES} pieces", ROWS.0, ROWS.1));
    }
    let unit = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
    if !unit(opt.curve) || !unit(opt.ripple) || !unit(opt.wobble) {
        return Err("curve, ripple and wobble are 0 to 100 %".into());
    }
    if !(r.width() > 0.0 && r.height() > 0.0 && r.width().is_finite() && r.height().is_finite()) {
        return Err("there is nothing to make a puzzle of".into());
    }
    let (rows, cols) = (opt.rows as usize, opt.cols as usize);
    let (cw, ch) = (r.width() / f64::from(opt.cols), r.height() / f64::from(opt.rows));
    let mut rng = Rng::new(opt.seed);
    // The grid's corners: the inner ones strayed by the wobble (those on the border stay on it).
    let stray = if opt.curvature { 0.08 * opt.wobble } else { 0.0 };
    let mut corner = vec![vec![Point::ZERO; cols + 1]; rows + 1];
    for (i, line) in corner.iter_mut().enumerate() {
        for (k, q) in line.iter_mut().enumerate() {
            let (inner_x, inner_y) = (k > 0 && k < cols, i > 0 && i < rows);
            let jx = if inner_x { rng.next() * stray * cw } else { 0.0 };
            let jy = if inner_y { rng.next() * stray * ch } else { 0.0 };
            *q = Point::new(r.x0 + cw * k as f64 + jx, r.y0 + ch * i as f64 + jy);
        }
    }
    let knob = |rng: &mut Rng| {
        opt.curvature.then(|| Knob {
            mid: 0.5 + rng.next() * 0.08 * opt.wobble,
            out: if rng.next() < 0.0 { -1.0 } else { 1.0 },
            curve: opt.curve * (1.0 - 0.15 * opt.wobble * (rng.next() + 1.0) / 2.0),
            ripple: opt.ripple,
        })
    };
    let mut lines: Vec<BezPath> = vec![];
    let mut border = BezPath::new();
    border.move_to(Point::new(r.x0, r.y0));
    border.line_to(Point::new(r.x1, r.y0));
    border.line_to(Point::new(r.x1, r.y1));
    border.line_to(Point::new(r.x0, r.y1));
    border.close_path();
    lines.push(border);
    for row in corner.iter().take(rows).skip(1) {
        let Some(first) = row.first() else { continue };
        let mut p = BezPath::new();
        p.move_to(*first);
        for pair in row.windows(2) {
            side(&mut p, pair[0], pair[1], knob(&mut rng));
        }
        lines.push(p);
    }
    for k in 1..cols {
        let column: Vec<Point> = corner.iter().filter_map(|row| row.get(k).copied()).collect();
        let Some(first) = column.first() else { continue };
        let mut p = BezPath::new();
        p.move_to(*first);
        for pair in column.windows(2) {
            side(&mut p, pair[0], pair[1], knob(&mut rng));
        }
        lines.push(p);
    }
    Ok(PathData::new(lines.iter().flat_map(|l| PathData::from_bezpath(l).subpaths).collect()))
}

#[cfg(test)]
mod tests {
    use vectorcraft_geom::Shape;

    use super::*;

    const R: Rect = Rect { x0: 0.0, y0: 0.0, x1: 500.0, y1: 400.0 };

    fn bounds(p: &PathData, i: usize) -> Rect {
        PathData::single(p.subpaths[i].clone()).to_bezpath().bounding_box()
    }

    /// 4 × 5: the border and 7 lines, the knobs reaching a fifth or so of a side out; the same
    /// seed the same puzzle, another another; all inside the border.
    #[test]
    fn a_puzzle_has_its_lines_and_knobs() {
        let opt = Options { rows: 4, cols: 5, ..Options::default() };
        let p = puzzle(R, opt).unwrap();
        assert_eq!(p.subpaths.len(), 1 + 3 + 4);
        assert!(p.subpaths[0].closed && p.subpaths[1..].iter().all(|s| !s.closed));
        let b = bounds(&p, 1);
        assert!(b.height() > 2.0 * 100.0 * 0.15 && b.height() < 2.0 * 100.0 * 0.45, "knobs both ways off the line at y 100: {b:?}");
        assert_eq!(puzzle(R, opt).unwrap(), p, "the same seed, the same puzzle");
        assert_ne!(puzzle(R, Options { seed: 2, ..opt }).unwrap(), p);
        let all = p.to_bezpath().bounding_box();
        assert!(all.x0 >= -1e-9 && all.y0 >= -1e-9 && all.x1 <= 500.0 + 1e-9 && all.y1 <= 400.0 + 1e-9, "inside the border: {all:?}");
    }

    /// The knob's head is wider than its neck, so pieces lock; and it stands well out.
    #[test]
    fn the_head_is_wider_than_the_neck() {
        let c = knob_curves(Knob { mid: 0.5, out: 1.0, curve: 1.0, ripple: 0.0 });
        let neck = 0.5 - 0.045;
        let head_left = c[2].iter().map(|p| p.0).fold(f64::INFINITY, f64::min).min(c[1][2].0);
        assert!(head_left < neck - 0.03, "the head bulges past the neck: {head_left}");
        assert!(c[2][2].1 > 0.2, "the head stands out: {:?}", c[2][2]);
    }

    /// Without curvature, a plain grid: straight lines; with no wobble, the grid even.
    #[test]
    fn curvature_off_is_a_grid() {
        let p = puzzle(R, Options { curvature: false, ..Options::default() }).unwrap();
        assert!(bounds(&p, 1).height().abs() < 1e-9, "a straight line");
        let even = puzzle(R, Options { wobble: 0.0, ripple: 0.0, ..Options::default() }).unwrap();
        assert_eq!(even.subpaths[1].anchors.first().map(|a| a.p), Some(Point::new(0.0, 100.0)));
    }

    #[test]
    fn bad_options_are_refused() {
        assert!(puzzle(R, Options { rows: 1, ..Options::default() }).is_err());
        assert!(puzzle(R, Options { rows: 50, cols: 51, ..Options::default() }).is_err());
        assert!(puzzle(R, Options { curve: 1.5, ..Options::default() }).is_err());
        assert!(puzzle(R, Options { wobble: f64::NAN, ..Options::default() }).is_err());
        assert!(puzzle(Rect::new(0.0, 0.0, 0.0, 10.0), Options::default()).is_err());
    }
}
