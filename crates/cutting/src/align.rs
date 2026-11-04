//! Fork: alignment marks for layered vinyl (C18.10). In a job cut in groups, a small cross is cut
//! at the same place on the mat in every group, just outside the design: lay each colour's cross
//! over the one before when pressing or applying, and the layers line up (a sign maker's habit).
//! The cross goes at the first of the design's corners with room for it on the cut area.

use vectorcraft_cutjob::{OpKind, RectMm, SourceRef};

use crate::prepare::Polyline;

/// The cross's arms, end to end, mm.
pub const SIZE_MM: f64 = 6.0;
/// Its distance from the design, mm.
pub const GAP_MM: f64 = 3.0;
/// What the cross's paths are made from (no object of the document's).
pub const SOURCE: SourceRef = SourceRef { node: 0, subpath: 0 };

/// The cross for `paths` (the whole job's, mm) on `area`; none when no corner has room.
pub fn cross(paths: &[Polyline], area: RectMm) -> Option<Vec<Polyline>> {
    let mut b: Option<RectMm> = None;
    for q in paths.iter().flat_map(|p| &p.points).filter(|q| q[0].is_finite() && q[1].is_finite()) {
        b = Some(match b {
            None => RectMm { x0: q[0], y0: q[1], x1: q[0], y1: q[1] },
            Some(r) => RectMm { x0: r.x0.min(q[0]), y0: r.y0.min(q[1]), x1: r.x1.max(q[0]), y1: r.y1.max(q[1]) },
        });
    }
    let b = b?;
    let r = SIZE_MM / 2.0;
    let off = GAP_MM + r;
    // Top left, top right, bottom left, bottom right.
    let corners = [(b.x0 - off, b.y0 - off), (b.x1 + off, b.y0 - off), (b.x0 - off, b.y1 + off), (b.x1 + off, b.y1 + off)];
    let (cx, cy) = corners.into_iter().find(|&(x, y)| x - r >= area.x0 && x + r <= area.x1 && y - r >= area.y0 && y + r <= area.y1)?;
    let arm = |a: [f64; 2], z: [f64; 2]| Polyline { op: OpKind::Cut, source: SOURCE, closed: false, points: vec![a, z], curve: None };
    Some(vec![arm([cx - r, cy], [cx + r, cy]), arm([cx, cy - r], [cx, cy + r])])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Polyline {
        Polyline {
            op: OpKind::Cut,
            source: SourceRef { node: 5, subpath: 0 },
            closed: true,
            points: vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]],
            curve: None,
        }
    }

    #[test]
    fn the_cross_goes_by_a_corner_with_room() {
        let area = RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 };
        let c = cross(&[square(50.0, 50.0, 100.0)], area).unwrap();
        assert_eq!(c.len(), 2);
        // Top left: centred 6 mm out from the corner (3 mm gap, 3 mm half arm).
        assert_eq!(c[0].points, vec![[41.0, 44.0], [47.0, 44.0]]);
        // No room at the top left: the top right.
        let c = cross(&[square(2.0, 50.0, 100.0)], area).unwrap();
        assert!(c[0].points[0][0] > 102.0);
        // No room anywhere.
        assert!(cross(&[square(0.0, 0.0, 300.0)], area).is_none());
        assert!(cross(&[], area).is_none());
    }
}
