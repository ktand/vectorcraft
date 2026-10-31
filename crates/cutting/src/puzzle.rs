//! Fork: puzzles (C13.5). A jigsaw's cut lines over a rectangle (the selection's bounds, a printed
//! picture included, or the artboard): its border, and between each row and column of pieces a
//! line with a rounded tab in the middle of every piece's side, sticking out one way or the other.
//! The tab's outline is drawn here from four curves (not taken from anywhere); the variety shifts
//! each tab along its side and changes its size a little, from a seed, so the same settings
//! always make the same puzzle. In document points.

use vectorcraft_geom::{BezPath, PathData, Point, Rect};

/// Rows and columns: the range.
pub const ROWS: (u32, u32) = (2, 50);
/// Most pieces in all.
const MAX_PIECES: u32 = 2_500;
/// A tab's height as a share of the piece's side, %: the range, and the usual.
pub const TAB_PCT: (f64, f64) = (10.0, 35.0);
pub const DEFAULT_TAB_PCT: f64 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub rows: u32,
    pub cols: u32,
    /// A tab's height as a share of the piece's side, % ([`TAB_PCT`]).
    pub tab_pct: f64,
    /// How much the tabs vary, 0 (all alike) to 1.
    pub variety: f64,
    /// Which puzzle of all those the settings can make.
    pub seed: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options { rows: 4, cols: 5, tab_pct: DEFAULT_TAB_PCT, variety: 0.5, seed: 1 }
    }
}

/// A small deterministic generator (xorshift32): the same seed, the same numbers.
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Rng {
        Rng(seed.wrapping_mul(2_654_435_761).max(1))
    }

    /// A number in 0..1.
    fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        f64::from(x) / f64::from(u32::MAX)
    }
}

/// The tab: (along, out) points of four cubic curves from (0, 0) to (1, 0), the knob reaching
/// out to 1 at the middle. Ours: a narrow neck widening to a round head.
const TAB: [[(f64, f64); 3]; 4] = [
    [(0.30, 0.0), (0.42, -0.08), (0.40, 0.30)],
    [(0.38, 0.62), (0.40, 1.0), (0.50, 1.0)],
    [(0.60, 1.0), (0.62, 0.62), (0.60, 0.30)],
    [(0.58, -0.08), (0.70, 0.0), (1.0, 0.0)],
];

/// Adds to `p` (already at `a`) the side from `a` to `b` with a tab `height` high on side `out`
/// (+1: to the left of a→b in page coordinates, −1 to the right), its middle at `mid` (0..1), its
/// width `width` of the side.
fn side(p: &mut BezPath, a: Point, b: Point, height: f64, out: f64, mid: f64, width: f64) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = dx.hypot(dy);
    if len <= 0.0 {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let (nx, ny) = (uy * out, -ux * out);
    let start = (mid - width / 2.0).clamp(0.05, 0.95 - width);
    let at = |along: f64, up: f64| {
        let s = (start + along * width) * len;
        Point::new(a.x + ux * s + nx * up * height, a.y + uy * s + ny * up * height)
    };
    p.line_to(at(0.0, 0.0));
    for [c1, c2, e] in TAB {
        p.curve_to(at(c1.0, c1.1), at(c2.0, c2.1), at(e.0, e.1));
    }
    p.line_to(b);
}

/// The puzzle's cut lines over `r`: the border, then each line between rows and between columns,
/// or why it can't be made.
pub fn puzzle(r: Rect, opt: Options) -> Result<PathData, String> {
    let ok = |n: u32| (ROWS.0..=ROWS.1).contains(&n);
    if !ok(opt.rows) || !ok(opt.cols) || opt.rows * opt.cols > MAX_PIECES {
        return Err(format!("rows and columns are {} to {}, at most {MAX_PIECES} pieces", ROWS.0, ROWS.1));
    }
    if !(opt.tab_pct.is_finite() && (TAB_PCT.0..=TAB_PCT.1).contains(&opt.tab_pct))
        || !(opt.variety.is_finite() && (0.0..=1.0).contains(&opt.variety))
    {
        return Err(format!("the tabs are {} to {} %, the variety 0 to 1", TAB_PCT.0, TAB_PCT.1));
    }
    if !(r.width() > 0.0 && r.height() > 0.0 && r.width().is_finite() && r.height().is_finite()) {
        return Err("there is nothing to make a puzzle of".into());
    }
    let (cw, ch) = (r.width() / f64::from(opt.cols), r.height() / f64::from(opt.rows));
    let height = cw.min(ch) * opt.tab_pct / 100.0;
    let mut rng = Rng::new(opt.seed);
    let v = opt.variety;
    let mut tab = |p: &mut BezPath, a: Point, b: Point| {
        let out = if rng.next() < 0.5 { 1.0 } else { -1.0 };
        let mid = 0.5 + (rng.next() - 0.5) * 0.2 * v;
        let grow = 1.0 + (rng.next() - 0.5) * 0.3 * v;
        side(p, a, b, height * grow, out, mid, 0.34 * grow);
    };
    let mut lines: Vec<BezPath> = vec![];
    let mut border = BezPath::new();
    border.move_to(Point::new(r.x0, r.y0));
    border.line_to(Point::new(r.x1, r.y0));
    border.line_to(Point::new(r.x1, r.y1));
    border.line_to(Point::new(r.x0, r.y1));
    border.close_path();
    lines.push(border);
    for i in 1..opt.rows {
        let y = r.y0 + ch * f64::from(i);
        let mut p = BezPath::new();
        p.move_to(Point::new(r.x0, y));
        for k in 0..opt.cols {
            let (x0, x1) = (r.x0 + cw * f64::from(k), r.x0 + cw * f64::from(k + 1));
            tab(&mut p, Point::new(x0, y), Point::new(x1, y));
        }
        lines.push(p);
    }
    for k in 1..opt.cols {
        let x = r.x0 + cw * f64::from(k);
        let mut p = BezPath::new();
        p.move_to(Point::new(x, r.y0));
        for i in 0..opt.rows {
            let (y0, y1) = (r.y0 + ch * f64::from(i), r.y0 + ch * f64::from(i + 1));
            tab(&mut p, Point::new(x, y0), Point::new(x, y1));
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

    /// 4 × 5: the border, 3 lines across and 4 down, each with a tab per piece side reaching
    /// about the tab height out from its line; the same seed the same puzzle, another another.
    #[test]
    fn a_puzzle_has_its_lines_and_tabs() {
        let p = puzzle(R, Options::default()).unwrap();
        assert_eq!(p.subpaths.len(), 1 + 3 + 4);
        assert!(p.subpaths[0].closed && p.subpaths[1..].iter().all(|s| !s.closed));
        let b = PathData::single(p.subpaths[1].clone()).to_bezpath().bounding_box();
        let height = 100.0 * 0.22;
        assert!((b.x0 - 0.0).abs() < 1e-9 && (b.x1 - 500.0).abs() < 1e-9);
        assert!(b.height() > height * 0.8 && b.height() < height * 2.5, "tabs out from the line at y 100: {b:?}");
        assert_eq!(puzzle(R, Options::default()).unwrap(), p, "the same seed, the same puzzle");
        assert_ne!(puzzle(R, Options { seed: 2, ..Options::default() }).unwrap(), p);
        let all = p.to_bezpath().bounding_box();
        assert!(all.x0 >= -1e-9 && all.y0 >= -1e-9 && all.x1 <= 500.0 + 1e-9 && all.y1 <= 400.0 + 1e-9, "inside the border: {all:?}");
    }

    #[test]
    fn bad_options_are_refused() {
        assert!(puzzle(R, Options { rows: 1, ..Options::default() }).is_err());
        assert!(puzzle(R, Options { rows: 50, cols: 51, ..Options::default() }).is_err());
        assert!(puzzle(R, Options { tab_pct: 50.0, ..Options::default() }).is_err());
        assert!(puzzle(R, Options { variety: f64::NAN, ..Options::default() }).is_err());
        assert!(puzzle(Rect::new(0.0, 0.0, 0.0, 10.0), Options::default()).is_err());
    }
}
