//! Fork: the calibration sheet (C4.12), our own design, to measure a machine by with a ruler and
//! calipers (hardware checks step 21):
//!
//! - **Scale:** a 100 mm line across and one down: cut, each should measure 100 mm.
//! - **Closing:** a 20 mm square and a 20 mm circle: each should drop out, its start and end
//!   meeting (the overcut).
//! - **Corners:** a star, a 5 mm and a 2 mm square and a small triangle: their corners should be
//!   sharp (the blade offset).
//! - **Registration** (Print & Cut only): four printed crosses with a ring cut round each: the
//!   cross should sit in the ring's middle.
//!
//! In mm from the sheet's top-left corner, across and down.

use vectorcraft_cutjob::Op;
use vectorcraft_geom::{BezPath, Point, Shape};

/// The sheet's width and height, mm.
pub const SIZE_MM: [f64; 2] = [112.0, 112.0];
/// The scale lines' length, mm.
pub const SCALE_MM: f64 = 100.0;
/// The closing square's side and the circle's diameter, mm.
pub const CLOSE_MM: f64 = 20.0;
/// A registration ring's diameter and its cross's arm length, mm.
pub const RING_MM: f64 = 8.0;
const CROSS_MM: f64 = 5.0;
/// Where the registration crosses are, mm.
pub const TARGETS: [[f64; 2]; 4] = [[20.0, 75.0], [95.0, 75.0], [20.0, 100.0], [95.0, 100.0]];

/// One part of the sheet: its name, its outline (mm) and how it is cut (`NoCut`: printed only).
#[derive(Clone, Debug)]
pub struct Part {
    pub name: &'static str,
    pub path: BezPath,
    pub op: Op,
}

fn poly(points: &[[f64; 2]], closed: bool) -> BezPath {
    let mut p = BezPath::new();
    for (i, q) in points.iter().enumerate() {
        let q = Point::new(q[0], q[1]);
        if i == 0 { p.move_to(q) } else { p.line_to(q) }
    }
    if closed {
        p.close_path();
    }
    p
}

fn square(x: f64, y: f64, side: f64) -> BezPath {
    poly(&[[x, y], [x + side, y], [x + side, y + side], [x, y + side]], true)
}

fn circle(c: [f64; 2], diameter: f64) -> BezPath {
    vectorcraft_geom::kurbo::Circle::new(Point::new(c[0], c[1]), diameter / 2.0).to_path(1e-4)
}

/// The sheet's parts; with `print_and_cut`, the registration crosses and their rings too.
pub fn sheet(print_and_cut: bool) -> Vec<Part> {
    let cut = |name, path| Part { name, path, op: Op::Cut };
    let star: Vec<[f64; 2]> = (0..10)
        .map(|i| {
            let a = std::f64::consts::PI * f64::from(i) / 5.0;
            let r = if i % 2 == 0 { 7.0 } else { 2.8 };
            [92.0 + r * a.sin(), 25.0 - r * a.cos()]
        })
        .collect();
    let mut parts = vec![
        cut("Scale Across", poly(&[[10.0, 2.0], [10.0 + SCALE_MM, 2.0]], false)),
        cut("Scale Down", poly(&[[2.0, 10.0], [2.0, 10.0 + SCALE_MM]], false)),
        cut("Closing Square", square(15.0, 15.0, CLOSE_MM)),
        cut("Closing Circle", circle([60.0, 25.0], CLOSE_MM)),
        cut("Star", poly(&star, true)),
        cut("Square 5 mm", square(15.0, 48.0, 5.0)),
        cut("Square 2 mm", square(27.0, 49.5, 2.0)),
        cut("Triangle", poly(&[[36.0, 53.0], [42.0, 53.0], [39.0, 48.0]], true)),
    ];
    if print_and_cut {
        for c in TARGETS {
            let h = CROSS_MM / 2.0;
            let mut cross = poly(&[[c[0] - h, c[1]], [c[0] + h, c[1]]], false);
            cross.extend(poly(&[[c[0], c[1] - h], [c[0], c[1] + h]], false));
            parts.push(Part { name: "Cross", path: cross, op: Op::NoCut });
            parts.push(cut("Ring", circle(c, RING_MM)));
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use vectorcraft_geom::ParamCurve;
    use vectorcraft_geom::kurbo::{ParamCurveArclen, Rect};

    use super::*;

    fn length(p: &BezPath) -> f64 {
        p.segments().map(|s| s.arclen(1e-6)).sum()
    }

    /// The lines are 100 mm, the closing shapes 20 mm and closed, everything on the sheet and
    /// nothing overlapping but the crosses in their rings.
    #[test]
    fn the_sheets_geometry() {
        let parts = sheet(true);
        let named = |n: &str| parts.iter().find(|p| p.name == n).unwrap();
        assert!((length(&named("Scale Across").path) - 100.0).abs() < 1e-9);
        assert!((length(&named("Scale Down").path) - 100.0).abs() < 1e-9);
        let sq = named("Closing Square").path.bounding_box();
        assert!((sq.width() - 20.0).abs() < 1e-9 && (sq.height() - 20.0).abs() < 1e-9);
        let c = named("Closing Circle").path.bounding_box();
        assert!((c.width() - 20.0).abs() < 1e-3 && (length(&named("Closing Circle").path) - 20.0 * std::f64::consts::PI).abs() < 1e-2);
        for p in &parts {
            if p.name != "Cross" && !p.name.starts_with("Scale") {
                let segs: Vec<_> = p.path.segments().collect();
                let (a, b) = (segs.first().unwrap().start(), segs.last().unwrap().end());
                assert!((a - b).hypot() < 1e-9, "{} closes", p.name);
            }
        }
        let sheet_rect = Rect::new(0.0, 0.0, SIZE_MM[0], SIZE_MM[1]);
        let boxes: Vec<(&str, Rect)> = parts.iter().map(|p| (p.name, p.path.bounding_box())).collect();
        for (n, b) in &boxes {
            assert!(sheet_rect.contains(Point::new(b.x0, b.y0)) && sheet_rect.contains(Point::new(b.x1, b.y1)), "{n} on the sheet: {b:?}");
        }
        for (i, (n, a)) in boxes.iter().enumerate() {
            for (m, b) in boxes.iter().skip(i + 1) {
                let pair = (*n == "Cross" && *m == "Ring") || (*n == "Ring" && *m == "Cross");
                if !pair {
                    assert!(a.intersect(*b).area() <= 0.0 || a.intersect(*b).is_zero_area(), "{n} and {m} apart");
                }
            }
        }
        assert_eq!(parts.iter().filter(|p| p.op == Op::NoCut).count(), 4);
        assert_eq!(sheet(false).len(), parts.len() - 8, "no crosses or rings without Print & Cut");
    }
}
