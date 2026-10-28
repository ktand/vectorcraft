//! Where the document lies on the mat (ARCHITECTURE.md › 5.1, step 4).
//!
//! The settings' artboard is laid on the mat with its top-left corner at the cut area's top-left
//! plus `offsetMm`. Portrait keeps its top along the mat's top (the edge fed first); landscape
//! turns it a quarter turn clockwise, its top along the mat's right side, and `offsetMm` then
//! places the turned artboard's top-left corner. Document points become millimetres
//! (25.4 / 72 each); the document's y axis already grows down, as the mat's does toward the user.

use vectorcraft_cutjob::{CuttingSettings, MatGeometry, Orientation, RectMm};
use vectorcraft_doc::Document;
use vectorcraft_geom::{Affine, Rect};

/// Millimetres per point.
pub const MM_PER_PT: f64 = 25.4 / 72.0;

/// Within this (mm) of the cut area's edge still counts as on it: rounding, not a real overhang.
const FIT_SLACK_MM: f64 = 1e-6;

/// How the document lies on the mat.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub mat: MatGeometry,
    /// Document points → mat millimetres.
    pub doc_to_mat: Affine,
    /// The artboard on the mat, mm.
    pub artboard_mm: RectMm,
}

impl Placement {
    /// Mat millimetres → document points (to draw the mat over the art).
    pub fn mat_to_doc(&self) -> Affine {
        self.doc_to_mat.inverse()
    }

    /// The whole artboard lies inside the cut area.
    pub fn fits(&self) -> bool {
        let (a, c) = (self.artboard_mm, self.mat.cut_area_mm);
        a.x0 >= c.x0 - FIT_SLACK_MM && a.y0 >= c.y0 - FIT_SLACK_MM && a.x1 <= c.x1 + FIT_SLACK_MM && a.y1 <= c.y1 + FIT_SLACK_MM
    }
}

/// `artboard` (document points) laid on `mat` as `s` says.
fn place_rect(s: &CuttingSettings, mat: MatGeometry, artboard: Rect) -> Placement {
    let k = MM_PER_PT;
    let origin = [mat.cut_area_mm.x0 + s.offset_mm[0], mat.cut_area_mm.y0 + s.offset_mm[1]];
    let (w, h) = (artboard.width() * k, artboard.height() * k);
    // kurbo: x' = a·x + c·y + e, y' = b·x + d·y + f.
    let (doc_to_mat, size) = match s.orientation {
        Orientation::Portrait => (Affine::new([k, 0.0, 0.0, k, origin[0] - k * artboard.x0, origin[1] - k * artboard.y0]), (w, h)),
        // The artboard's (u, v) from its top-left goes to (h − v, u): its top edge to the right.
        Orientation::Landscape => (Affine::new([0.0, k, -k, 0.0, origin[0] + h + k * artboard.y0, origin[1] - k * artboard.x0]), (h, w)),
    };
    let artboard_mm = RectMm { x0: origin[0], y0: origin[1], x1: origin[0] + size.0, y1: origin[1] + size.1 };
    Placement { mat, doc_to_mat, artboard_mm }
}

/// How `d` lies on the mat its cutting settings `s` name; `None` when the settings' artboard
/// isn't in the document.
pub fn place(s: &CuttingSettings, d: &Document) -> Option<Placement> {
    let i = usize::try_from(s.artboard).ok()?;
    let artboard = d.artboards.get(i)?.rect;
    Some(place_rect(s, s.mat_geometry(), artboard))
}

#[cfg(test)]
mod tests {
    use vectorcraft_geom::Point;

    use super::*;

    /// A 100 × 50 mm artboard, placed with its top-left at (20, 30) pt, as the document has it.
    fn artboard() -> Rect {
        Rect::new(20.0, 30.0, 20.0 + 100.0 / MM_PER_PT, 30.0 + 50.0 / MM_PER_PT)
    }

    fn corners(r: Rect) -> [Point; 4] {
        [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)]
    }

    fn assert_points(got: [Point; 4], want: [[f64; 2]; 4]) {
        for (g, w) in got.iter().zip(want) {
            assert!((g.x - w[0]).abs() < 1e-9 && (g.y - w[1]).abs() < 1e-9, "{got:?}, want {want:?}");
        }
    }

    fn settings(orientation: Orientation, offset_mm: [f64; 2]) -> CuttingSettings {
        CuttingSettings { orientation, offset_mm, ..CuttingSettings::default() }
    }

    /// Top-left, top-right, bottom-right and bottom-left of the artboard on the mat.
    fn mapped(p: &Placement) -> [Point; 4] {
        corners(artboard()).map(|c| p.doc_to_mat * c)
    }

    #[test]
    fn portrait_keeps_the_artboard_upright_at_the_cut_areas_corner() {
        let s = settings(Orientation::Portrait, [0.0, 0.0]);
        let p = place_rect(&s, s.mat_geometry(), artboard());
        assert_points(mapped(&p), [[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]]);
        assert_eq!(p.artboard_mm, RectMm { x0: 0.0, y0: 0.0, x1: 100.0, y1: 50.0 });
        assert!(p.fits());
    }

    #[test]
    fn the_offset_moves_the_artboard() {
        let s = settings(Orientation::Portrait, [10.0, 25.0]);
        let p = place_rect(&s, s.mat_geometry(), artboard());
        assert_points(mapped(&p), [[10.0, 25.0], [110.0, 25.0], [110.0, 75.0], [10.0, 75.0]]);
    }

    /// Landscape turns it clockwise: its top-left corner to the top right, its top edge down
    /// the right side; the turned artboard (50 × 100) starts at the offset.
    #[test]
    fn landscape_turns_the_artboard_clockwise() {
        let s = settings(Orientation::Landscape, [10.0, 25.0]);
        let p = place_rect(&s, s.mat_geometry(), artboard());
        assert_points(mapped(&p), [[60.0, 25.0], [60.0, 125.0], [10.0, 125.0], [10.0, 25.0]]);
        assert_eq!(p.artboard_mm, RectMm { x0: 10.0, y0: 25.0, x1: 60.0, y1: 125.0 });
        let det = p.doc_to_mat.determinant();
        assert!(det > 0.0, "a turn, not a mirror: {det}");
    }

    #[test]
    fn mat_to_doc_undoes_it() {
        for o in [Orientation::Portrait, Orientation::Landscape] {
            let s = settings(o, [3.0, -4.0]);
            let p = place_rect(&s, s.mat_geometry(), artboard());
            for c in corners(artboard()) {
                let back = p.mat_to_doc() * (p.doc_to_mat * c);
                assert!((back - c).hypot() < 1e-9, "{o:?}: {back:?} for {c:?}");
            }
        }
    }

    #[test]
    fn an_artboard_off_the_cut_area_doesnt_fit() {
        let s = settings(Orientation::Portrait, [250.0, 0.0]);
        assert!(!place_rect(&s, s.mat_geometry(), artboard()).fits(), "100 mm from 250 mm passes 304.8 mm");
        let s = settings(Orientation::Portrait, [-1.0, 0.0]);
        assert!(!place_rect(&s, s.mat_geometry(), artboard()).fits());
        // A whole 12 × 12 in artboard just fits the 12 × 12 in mat.
        let full = Rect::new(0.0, 0.0, 864.0, 864.0);
        let s = settings(Orientation::Landscape, [0.0, 0.0]);
        assert!(place_rect(&s, s.mat_geometry(), full).fits());
    }

    #[test]
    fn the_settings_artboard_must_exist() {
        let d = Document::new(200.0, 100.0);
        let s = CuttingSettings::default();
        let p = place(&s, &d).unwrap();
        assert_eq!(p.artboard_mm, RectMm { x0: 0.0, y0: 0.0, x1: 200.0 * MM_PER_PT, y1: 100.0 * MM_PER_PT });
        let s = CuttingSettings { artboard: 5, ..CuttingSettings::default() };
        assert_eq!(place(&s, &d), None);
    }
}
