//! Fork: templates made to size (C15.1). A template is a function from its parameters (sizes in
//! mm) to a flat net: the outline to cut, the folds to score and the glue areas to mark, in mm with
//! y down and the net's top left at the origin. Sizes are inside sizes (owner, 2026-11-03: what
//! goes in it), each fold taking up the material's thickness; an outside size can be asked for.
//!
//! Every net is our own, worked out from its geometry, or ported with attribution from a source
//! the plan names (`docs/cutting/IMPLEMENTATION_PLAN.md` › Phase 15); none is traced from or
//! compared against another tool's output.

use vectorcraft_geom::{PathData, Point, SubPath};

/// What a parameter measures, for its field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Mm,
    Deg,
    Count,
}

impl Unit {
    pub fn id(self) -> &'static str {
        match self {
            Unit::Mm => "mm",
            Unit::Deg => "deg",
            Unit::Count => "count",
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
            nums.push((p.key, v));
        }
        Ok(Values { nums, outside: outside && self.sized })
    }

    /// The net for `values`.
    pub fn make(&self, values: &Values) -> Result<Net, String> {
        let net = (self.make)(values)?;
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
pub const KINDS: &[Kind] = &[TRAY];

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

    fn values(k: &Kind, given: &[(&str, f64)], outside: bool) -> Result<Values, String> {
        k.values(&|key| given.iter().find(|(g, _)| *g == key).map(|(_, v)| *v), None, outside)
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
            let g = net.glue.bounds().unwrap();
            assert!(g.x0 >= b.x0 - 1e-9 && g.y0 >= b.y0 - 1e-9 && g.x1 <= b.x1 + 1e-9 && g.y1 <= b.y1 + 1e-9, "{}", k.id);
        }
    }
}
