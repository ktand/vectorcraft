//! Fork: templates made to size (C15.1). A template is a function from its parameters (sizes in
//! mm) to a flat net: the outline to cut, the folds to score and the glue areas to mark, in mm with
//! y down and the net's top left at the origin. Sizes are inside sizes (owner, 2026-11-03: what
//! goes in it), each fold taking up the material's thickness; an outside size can be asked for.
//!
//! Every net is our own, worked out from its geometry, or ported with attribution from a source
//! the plan names (`docs/cutting/IMPLEMENTATION_PLAN.md` › Phase 15); none is traced from or
//! compared against another tool's output.

use vectorcraft_geom::{Anchor, PathData, Point, SubPath};

mod boxes;
mod cards;
mod solids;
mod tuck;
mod unfold;

/// What a parameter measures, for its field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Mm,
    Deg,
    Count,
    /// On (1) or off (0): a checkbox.
    Toggle,
}

impl Unit {
    pub fn id(self) -> &'static str {
        match self {
            Unit::Mm => "mm",
            Unit::Deg => "deg",
            Unit::Count => "count",
            Unit::Toggle => "toggle",
        }
    }
}

/// One of a template's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Param {
    /// Its key in the command's params.
    pub key: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub help: &'static str,
}

/// A template's parameters as asked for: each a value, in its range.
#[derive(Clone, Debug, PartialEq)]
pub struct Values {
    nums: Vec<(&'static str, f64)>,
    /// The sizes given are outside sizes (the finished net's), not inside ones.
    pub outside: bool,
}

impl Values {
    /// The value of `key`; 0 if the template has no such parameter.
    pub fn get(&self, key: &str) -> f64 {
        self.nums.iter().find(|(k, _)| *k == key).map_or(0.0, |(_, v)| *v)
    }

    /// Every parameter and its value, in the template's order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, f64)> + '_ {
        self.nums.iter().copied()
    }
}

/// A flat net, in mm, y down, its top left at the origin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Net {
    /// Cut through: the outline, and any slits or holes.
    pub cut: PathData,
    /// Scored, to fold along.
    pub folds: PathData,
    /// Where glue goes (closed areas), marked and not cut.
    pub glue: PathData,
}

impl Net {
    /// The net's size, mm: its outline's bounds.
    pub fn size(&self) -> (f64, f64) {
        self.cut.bounds().map_or((0.0, 0.0), |b| (b.width(), b.height()))
    }
}

/// A kind of template.
#[derive(Clone, Copy, Debug)]
pub struct Kind {
    /// Its id in `cutting.template {kind}`.
    pub id: &'static str,
    pub name: &'static str,
    /// The family it is listed under: Boxes, Envelopes, Solids…
    pub family: &'static str,
    pub params: &'static [Param],
    /// Whether it has inside and outside sizes (a box does; a tag doesn't).
    pub sized: bool,
    make: fn(&Values) -> Result<Net, String>,
}

impl Kind {
    /// `values` for this kind: each given number checked against its range, the rest from
    /// `fallback` (a remade template's old values), else the defaults.
    pub fn values(&self, given: &dyn Fn(&str) -> Option<f64>, fallback: Option<&Values>, outside: bool) -> Result<Values, String> {
        let mut nums = Vec::with_capacity(self.params.len());
        for p in self.params {
            let old = || fallback.and_then(|f| f.nums.iter().find(|(k, _)| *k == p.key)).map(|(_, v)| *v);
            let v = given(p.key).or_else(old).unwrap_or(p.default);
            if !v.is_finite() || v < p.min || v > p.max {
                return Err(format!("{} is {}–{} {}", p.key, p.min, p.max, p.unit.id()));
            }
            if p.unit == Unit::Count && v.fract() != 0.0 {
                return Err(format!("{} must be a whole number", p.key));
            }
            if p.unit == Unit::Toggle && v != 0.0 && v != 1.0 {
                return Err(format!("{} must be true or false", p.key));
            }
            nums.push((p.key, v));
        }
        Ok(Values { nums, outside: outside && self.sized })
    }

    /// The net for `values`.
    pub fn make(&self, values: &Values) -> Result<Net, String> {
        let mut net = (self.make)(values)?;
        // Every net's outline starts at the origin, whatever its curves reach.
        if let Some(b) = net.cut.bounds()
            && b.x0.is_finite()
            && b.y0.is_finite()
        {
            let to = vectorcraft_geom::Affine::translate((-b.x0, -b.y0));
            for p in [&mut net.cut, &mut net.folds, &mut net.glue] {
                p.transform(to);
            }
        }
        // A net that isn't finite or has no size is a template's bug: refused, not drawn.
        let finite = |p: &PathData| p.subpaths.iter().flat_map(|s| &s.anchors).all(|a| a.p.x.is_finite() && a.p.y.is_finite());
        let (w, h) = net.size();
        if !(finite(&net.cut) && finite(&net.folds) && finite(&net.glue)) || w <= 0.0 || h <= 0.0 {
            return Err(format!("the {} couldn't be made with these sizes", self.name.to_lowercase()));
        }
        Ok(net)
    }
}

/// Every template, in the order the panel lists them.
pub const KINDS: &[Kind] = &[
    tuck::STRAIGHT,
    tuck::REVERSE,
    TRAY,
    boxes::TRAY_LID,
    boxes::PILLOW,
    cards::ENVELOPE,
    cards::POINTED_ENVELOPE,
    cards::PLACE_CARD,
    cards::GIFT_TAG,
    cards::EARRING_CARD,
    cards::GIFT_CARD_HOLDER,
    solids::CUBE,
    solids::TETRAHEDRON,
    solids::OCTAHEDRON,
    solids::DODECAHEDRON,
    solids::ICOSAHEDRON,
    solids::PYRAMID,
    solids::PRISM,
    solids::CYLINDER,
    solids::CONE,
];

/// The template `id`.
pub fn kind(id: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.id == id)
}

/// A closed polygon.
fn polygon(points: &[(f64, f64)]) -> SubPath {
    SubPath::polyline(&points.iter().map(|&(x, y)| Point::new(x, y)).collect::<Vec<_>>(), true)
}

/// A straight line.
fn line(a: (f64, f64), b: (f64, f64)) -> SubPath {
    SubPath::polyline(&[Point::new(a.0, a.1), Point::new(b.0, b.1)], false)
}

/// A point, mm.
type P = (f64, f64);

/// A run of curves drawn from a start point: lines, quadratic and circular-arc pieces, each kept as
/// a cubic (`(c1, c2, end)`), so it can be mirrored and walked backwards before it becomes a path.
#[derive(Clone, Debug, PartialEq)]
struct Edge {
    start: P,
    segs: Vec<(P, P, P)>,
}

impl Edge {
    fn from(start: P) -> Edge {
        Edge { start, segs: vec![] }
    }

    fn end(&self) -> P {
        self.segs.last().map_or(self.start, |s| s.2)
    }

    /// A straight line to `p` (none if it is already there).
    fn to(&mut self, p: P) -> &mut Edge {
        let e = self.end();
        if (e.0 - p.0).abs() > 1e-9 || (e.1 - p.1).abs() > 1e-9 {
            self.segs.push((e, p, p));
        }
        self
    }

    /// A quadratic curve to `p` pulled toward `ctrl`.
    fn quad(&mut self, ctrl: P, p: P) -> &mut Edge {
        let e = self.end();
        let third = |a: P, b: P| (a.0 + 2.0 / 3.0 * (b.0 - a.0), a.1 + 2.0 / 3.0 * (b.1 - a.1));
        self.segs.push((third(e, ctrl), third(p, ctrl), p));
        self
    }

    /// A circular arc round `c` of radius `r` from angle `a0` to `a1` (radians, y down: positive
    /// turns clockwise on the page), a straight line first to its start if it isn't there.
    fn arc(&mut self, c: P, r: f64, a0: f64, a1: f64) -> &mut Edge {
        let at = |a: f64| (c.0 + r * a.cos(), c.1 + r * a.sin());
        self.to(at(a0));
        // Pieces of at most a quarter turn, each a cubic with the usual handle length.
        let n = ((a1 - a0).abs() / std::f64::consts::FRAC_PI_2).ceil().clamp(1.0, 8.0);
        let step = (a1 - a0) / n;
        let k = 4.0 / 3.0 * (step / 4.0).tan() * r;
        let mut a = a0;
        for _ in 0..n as usize {
            let (p0, p1) = (at(a), at(a + step));
            let c1 = (p0.0 - k * a.sin(), p0.1 + k * a.cos());
            let c2 = (p1.0 + k * (a + step).sin(), p1.1 - k * (a + step).cos());
            self.segs.push((c1, c2, p1));
            a += step;
        }
        self
    }

    /// The edge mirrored by `f` (applied to every point and handle).
    fn map(&self, f: impl Fn(P) -> P) -> Edge {
        Edge { start: f(self.start), segs: self.segs.iter().map(|&(a, b, c)| (f(a), f(b), f(c))).collect() }
    }

    /// The same edge walked from its end back to its start.
    fn reversed(&self) -> Edge {
        let mut starts: Vec<P> = vec![self.start];
        starts.extend(self.segs.iter().map(|s| s.2));
        let segs = self.segs.iter().zip(starts.iter()).rev().map(|(&(c1, c2, _), &from)| (c2, c1, from)).collect();
        Edge { start: self.end(), segs }
    }

    /// `next` joined on, a straight line between them if they don't meet.
    fn then(&mut self, next: &Edge) -> &mut Edge {
        self.to(next.start);
        self.segs.extend(next.segs.iter().copied());
        self
    }

    fn anchors(&self) -> Vec<Anchor> {
        let pt = |p: P| Point::new(p.0, p.1);
        let mut out = vec![Anchor::corner(pt(self.start))];
        for &(c1, c2, p) in &self.segs {
            if let Some(last) = out.last_mut() {
                last.h_out = pt(c1);
            }
            out.push(Anchor::with_handles(pt(p), pt(c2), pt(p)));
        }
        out
    }

    /// An open path.
    fn open(&self) -> SubPath {
        SubPath::new(self.anchors(), false)
    }

    /// A closed path, back to the start (a line if it doesn't end there).
    fn closed(&self) -> SubPath {
        let mut e = self.clone();
        e.to(self.start);
        let mut a = e.anchors();
        // The last anchor is the start again: its incoming handle goes to the first.
        if a.len() > 1
            && let Some(last) = a.pop()
            && let Some(first) = a.first_mut()
        {
            first.h_in = last.h_in;
        }
        SubPath::new(a, true)
    }
}

/// The material's thickness parameter, shared by every template that folds.
const THICKNESS: Param = Param {
    key: "thicknessMm",
    label: "Thickness",
    unit: Unit::Mm,
    default: 0.3,
    min: 0.0,
    max: 5.0,
    help: "the material's thickness, taken up at each fold",
};

/// The angle a glue tab's ends are cut back by, so it folds in clear of its neighbours.
const TAB_TAPER_DEG: f64 = 15.0;

const TRAY: Kind = Kind {
    id: "tray",
    name: "Open Tray",
    family: "Boxes",
    params: &[
        Param {
            key: "lengthMm",
            label: "Length",
            unit: Unit::Mm,
            default: 100.0,
            min: 10.0,
            max: 1000.0,
            help: "inside, front to back walls' faces",
        },
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 70.0, min: 10.0, max: 1000.0, help: "inside, between the side walls" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 30.0, min: 5.0, max: 500.0, help: "inside, the walls' height" },
        THICKNESS,
        Param {
            key: "tabMm",
            label: "Glue Tab",
            unit: Unit::Mm,
            default: 12.0,
            min: 3.0,
            max: 100.0,
            help: "how far each corner's glue tab reaches along the side wall",
        },
    ],
    sized: true,
    make: tray,
};

/// An open tray: a base, a wall folded up from each side, and at each corner a tab on the end of
/// the front or back wall, folded in and glued inside the side wall.
///
/// Our own geometry. Each 90° fold takes up half the thickness from each panel it joins, so the
/// base is the inside size plus one thickness each way, and a wall the inside height plus half a
/// thickness. Outside sizes are the inside ones plus two thicknesses across (the walls) and one up
/// (the base).
fn tray(v: &Values) -> Result<Net, String> {
    let t = v.get("thicknessMm");
    let (mut l, mut w, mut h) = (v.get("lengthMm"), v.get("widthMm"), v.get("heightMm"));
    if v.outside {
        (l, w, h) = (l - 2.0 * t, w - 2.0 * t, h - t);
        if l <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Err("the outside sizes are too small for the thickness".to_string());
        }
    }
    let a = l + t; // base, along the length (x)
    let b = w + t; // base, across (y)
    let h = h + t / 2.0; // wall
    let g = v.get("tabMm");
    if g > h {
        return Err(format!("the glue tab ({g} mm) can't reach further than the wall is high ({h:.1} mm)"));
    }
    if 2.0 * g > b {
        return Err(format!("two glue tabs ({g} mm each) don't fit inside a side wall {b:.1} mm long"));
    }
    let taper = g * TAB_TAPER_DEG.to_radians().tan();
    // The base spans x0–x1 and y0–y1, walls round it, the corners cut away.
    let (x0, x1, y0, y1) = (h, h + a, h, h + b);
    let outline = [
        (x0, 0.0),
        (x1, 0.0),
        // the front wall's right tab
        (x1 + g, taper),
        (x1 + g, h - taper),
        (x1, y0),
        // the right side wall
        (x1 + h, y0),
        (x1 + h, y1),
        (x1, y1),
        // the back wall's right tab
        (x1 + g, y1 + taper),
        (x1 + g, y1 + h - taper),
        (x1, y1 + h),
        (x0, y1 + h),
        // the back wall's left tab
        (x0 - g, y1 + h - taper),
        (x0 - g, y1 + taper),
        (x0, y1),
        // the left side wall
        (0.0, y1),
        (0.0, y0),
        (x0, y0),
        // the front wall's left tab
        (x0 - g, h - taper),
        (x0 - g, taper),
    ];
    let folds = vec![
        line((x0, y0), (x1, y0)),
        line((x0, y1), (x1, y1)),
        line((x0, y0), (x0, y1)),
        line((x1, y0), (x1, y1)),
        line((x0, 0.0), (x0, y0)),
        line((x1, 0.0), (x1, y0)),
        line((x0, y1), (x0, y1 + h)),
        line((x1, y1), (x1, y1 + h)),
    ];
    let tab = |x: f64, dir: f64, top: f64| polygon(&[(x, top), (x + dir * g, top + taper), (x + dir * g, top + h - taper), (x, top + h)]);
    let glue = vec![tab(x1, 1.0, 0.0), tab(x1, 1.0, y1), tab(x0, -1.0, y1), tab(x0, -1.0, 0.0)];
    Ok(Net { cut: PathData::single(polygon(&outline)), folds: PathData::new(folds), glue: PathData::new(glue) })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn values(k: &Kind, given: &[(&str, f64)], outside: bool) -> Result<Values, String> {
        k.values(&|key| given.iter().find(|(g, _)| *g == key).map(|(_, v)| *v), None, outside)
    }

    /// Whether `path`'s closed outlines don't cross themselves (each curve flattened finely).
    pub(super) fn simple(path: &PathData) -> bool {
        use vectorcraft_geom::ParamCurve;
        path.subpaths.iter().all(|s| {
            let mut pts: Vec<Point> = vec![];
            for i in 0..s.segment_count() {
                let c = s.segment(i);
                pts.extend((0..8).map(|k| c.eval(f64::from(k) / 8.0)));
            }
            let n = pts.len();
            let cross = |a: Point, b: Point, c: Point, d: Point| {
                let o = |p: Point, q: Point, r: Point| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
                let (d1, d2, d3, d4) = (o(a, b, c), o(a, b, d), o(c, d, a), o(c, d, b));
                d1 * d2 < -1e-9 && d3 * d4 < -1e-9
            };
            (0..n).all(|i| (i + 2..n).all(|j| (i == 0 && j == n - 1) || !cross(pts[i], pts[(i + 1) % n], pts[j], pts[(j + 1) % n])))
        })
    }

    /// The edge builder's arcs are circles: their points lie at the radius.
    #[test]
    fn arcs_are_round() {
        use vectorcraft_geom::ParamCurve;
        let mut e = Edge::from((10.0, 0.0));
        e.arc((0.0, 0.0), 10.0, 0.0, std::f64::consts::PI);
        let s = e.open();
        for i in 0..s.segment_count() {
            for k in 0..=10 {
                let p = s.segment(i).eval(f64::from(k) / 10.0);
                assert!((p.to_vec2().hypot() - 10.0).abs() < 0.01, "{p:?}");
            }
        }
        // Walked back and mirrored, it is the same circle.
        let back = e.reversed().map(|p| (p.0, -p.1)).open();
        assert_eq!(back.anchors.len(), s.anchors.len());
        assert!((back.anchors[0].p.x + 10.0).abs() < 1e-9);
    }

    fn len(s: &SubPath) -> f64 {
        let p: Vec<Point> = s.anchors.iter().map(|a| a.p).collect();
        p.windows(2).map(|w| (w[1] - w[0]).hypot()).sum()
    }

    /// The tray's base is the inside size plus a thickness, its walls the height plus half one,
    /// and the net as wide as the base and two walls.
    #[test]
    fn the_tray_is_made_to_its_inside_size() {
        let v = values(&TRAY, &[("lengthMm", 100.0), ("widthMm", 60.0), ("heightMm", 20.0), ("thicknessMm", 0.5)], false).unwrap();
        let net = TRAY.make(&v).unwrap();
        let (w, h) = net.size();
        assert!((w - (100.5 + 2.0 * 20.25)).abs() < 1e-9, "{w}");
        assert!((h - (60.5 + 2.0 * 20.25)).abs() < 1e-9, "{h}");
        // The base's folds are its sides: 100.5 and 60.5 long.
        let folds: Vec<f64> = net.folds.subpaths.iter().map(len).collect();
        assert_eq!(folds.len(), 8);
        assert!((folds[0] - 100.5).abs() < 1e-9 && (folds[2] - 60.5).abs() < 1e-9, "{folds:?}");
        // Each tab's fold is as long as the side wall it is glued to is high.
        assert!(folds[4..].iter().all(|f| (f - 20.25).abs() < 1e-9), "{folds:?}");
        assert_eq!(net.glue.subpaths.len(), 4);
    }

    /// Outside sizes give the same tray as the inside sizes they come to.
    #[test]
    fn outside_sizes_take_the_thickness_off() {
        let inside = values(&TRAY, &[("lengthMm", 99.0), ("widthMm", 59.0), ("heightMm", 19.5), ("thicknessMm", 0.5)], false).unwrap();
        let outside = values(&TRAY, &[("lengthMm", 100.0), ("widthMm", 60.0), ("heightMm", 20.0), ("thicknessMm", 0.5)], true).unwrap();
        let (a, b) = (TRAY.make(&inside).unwrap().size(), TRAY.make(&outside).unwrap().size());
        assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "{a:?} {b:?}");
    }

    /// Values out of range, and tabs that don't fit, are refused in words.
    #[test]
    fn bad_values_are_refused() {
        assert_eq!(values(&TRAY, &[("lengthMm", 5.0)], false).unwrap_err(), "lengthMm is 10–1000 mm");
        assert!(values(&TRAY, &[("lengthMm", f64::NAN)], false).is_err());
        let v = values(&TRAY, &[("heightMm", 5.0), ("tabMm", 20.0)], false).unwrap();
        assert!(TRAY.make(&v).unwrap_err().contains("can't reach further"));
        let v = values(&TRAY, &[("widthMm", 10.0), ("tabMm", 8.0), ("heightMm", 30.0)], false).unwrap();
        assert!(TRAY.make(&v).unwrap_err().contains("don't fit"));
    }

    /// A remade template keeps its old values for what isn't given.
    #[test]
    fn remade_values_fall_back_to_the_old_ones() {
        let old = values(&TRAY, &[("lengthMm", 150.0)], false).unwrap();
        let v = TRAY.values(&|k| (k == "heightMm").then_some(40.0), Some(&old), false).unwrap();
        assert_eq!((v.get("lengthMm"), v.get("heightMm"), v.get("widthMm")), (150.0, 40.0, 70.0));
    }

    /// Every template makes a net with its defaults, its glue inside its outline's bounds.
    #[test]
    fn every_template_makes_its_default() {
        for k in KINDS {
            let net = k.make(&values(k, &[], false).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", k.id));
            let b = net.cut.bounds().unwrap();
            assert!(b.x0.abs() < 1e-9 && b.y0.abs() < 1e-9, "{}: top left at the origin", k.id);
            if let Some(g) = net.glue.bounds() {
                assert!(g.x0 >= b.x0 - 1e-9 && g.y0 >= b.y0 - 1e-9 && g.x1 <= b.x1 + 1e-9 && g.y1 <= b.y1 + 1e-9, "{}", k.id);
            }
        }
    }
}
