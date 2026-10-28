//! Print & Cut registration marks (C6.1): where they go on the printed page and what they are,
//! generated here from the facts in `docs/cutting/protocol-silhouette.md` › Print & Cut (no maker's
//! artwork). The usual set of three: a filled square at the top left, and L-shaped corners at the
//! top right and bottom left, arms pointing in. Each mark's anchor is its outer corner; the
//! top-left anchor is the origin the machine cuts from once it has found the marks, and the
//! others lie the marks' width to its right and their length below it.

use crate::mat::RectMm;

/// The top-left square's side, mm.
pub const SQUARE_MM: f64 = 5.0;
/// Each arm of an L corner, mm.
pub const ARM_MM: f64 = 20.0;
/// The width the L corners are drawn, mm.
pub const LINE_MM: f64 = 0.3;
/// How far in from the page's edges the marks' anchors go, mm: the range, and the usual.
pub const INSET_MM: (f64, f64) = (5.0, 50.0);
pub const DEFAULT_INSET_MM: f64 = 10.0;
/// Art kept this far from each mark (mm) so the sensor sees only the mark.
pub const KEEP_CLEAR_MM: f64 = 5.0;

/// Where the marks go on a page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkLayout {
    /// The top-left anchor, from the page's top-left, mm.
    pub origin_mm: [f64; 2],
    /// From the top-left anchor across to the top-right one, and down to the bottom-left one, mm.
    pub size_mm: [f64; 2],
}

/// One mark, in page millimetres.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// Filled.
    Square(RectMm),
    /// An L drawn [`LINE_MM`] wide through these three points, the corner in the middle.
    Corner([[f64; 2]; 3]),
}

impl Mark {
    /// What the mark covers, mm.
    pub fn bounds(&self) -> RectMm {
        match self {
            Mark::Square(r) => *r,
            Mark::Corner(p) => {
                p.iter().fold(RectMm { x0: f64::INFINITY, y0: f64::INFINITY, x1: f64::NEG_INFINITY, y1: f64::NEG_INFINITY }, |b, q| RectMm {
                    x0: b.x0.min(q[0]),
                    y0: b.y0.min(q[1]),
                    x1: b.x1.max(q[0]),
                    y1: b.y1.max(q[1]),
                })
            }
        }
    }
}

impl MarkLayout {
    /// The marks on a page `page_mm` (width, height) with their anchors `inset_mm` in from each
    /// edge; `None` when the page is too small to hold them apart (or the numbers aren't finite).
    pub fn for_page(page_mm: [f64; 2], inset_mm: f64) -> Option<MarkLayout> {
        let inset = inset_mm.clamp(INSET_MM.0, INSET_MM.1);
        let size = [page_mm[0] - 2.0 * inset, page_mm[1] - 2.0 * inset];
        let ok = page_mm.iter().chain(&[inset_mm]).all(|v| v.is_finite()) && size.iter().all(|s| *s > 2.0 * ARM_MM);
        ok.then_some(MarkLayout { origin_mm: [inset, inset], size_mm: size })
    }

    /// The three marks, top-left square first.
    pub fn marks(&self) -> [Mark; 3] {
        let [x, y] = self.origin_mm;
        let [w, l] = self.size_mm;
        let (right, bottom) = (x + w, y + l);
        [
            Mark::Square(RectMm { x0: x, y0: y, x1: x + SQUARE_MM, y1: y + SQUARE_MM }),
            Mark::Corner([[right - ARM_MM, y], [right, y], [right, y + ARM_MM]]),
            Mark::Corner([[x, bottom - ARM_MM], [x, bottom], [x + ARM_MM, bottom]]),
        ]
    }

    /// Where art would be too near a mark, mm: each mark's bounds and [`KEEP_CLEAR_MM`] round them.
    pub fn keep_clear(&self) -> Vec<RectMm> {
        let k = KEEP_CLEAR_MM;
        self.marks()
            .iter()
            .map(|m| {
                let b = m.bounds();
                RectMm { x0: b.x0 - k, y0: b.y0 - k, x1: b.x1 + k, y1: b.y1 + k }
            })
            .collect()
    }

    /// The area between the marks' anchors, from the page's top-left, mm: what can be cut.
    pub fn area(&self) -> RectMm {
        let [x, y] = self.origin_mm;
        RectMm { x0: x, y0: y, x1: x + self.size_mm[0], y1: y + self.size_mm[1] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Letter page (215.9 × 279.4 mm), marks 10 mm in.
    #[test]
    fn marks_on_a_page() {
        let m = MarkLayout::for_page([215.9, 279.4], 10.0).unwrap();
        assert_eq!(m.origin_mm, [10.0, 10.0]);
        assert!((m.size_mm[0] - 195.9).abs() < 1e-9 && (m.size_mm[1] - 259.4).abs() < 1e-9);
        let [square, top_right, bottom_left] = m.marks();
        assert_eq!(square, Mark::Square(RectMm { x0: 10.0, y0: 10.0, x1: 15.0, y1: 15.0 }));
        let Mark::Corner(tr) = top_right else { panic!() };
        assert!((tr[1][0] - 205.9).abs() < 1e-9 && tr[1][1] == 10.0, "the corner at the anchor: {tr:?}");
        assert!(tr[0][0] < tr[1][0] && tr[2][1] > tr[1][1], "arms pointing in: {tr:?}");
        let Mark::Corner(bl) = bottom_left else { panic!() };
        assert!(bl[1][0] == 10.0 && (bl[1][1] - 269.4).abs() < 1e-9 && bl[0][1] < bl[1][1] && bl[2][0] > bl[1][0], "{bl:?}");
        assert_eq!(m.keep_clear()[0], RectMm { x0: 5.0, y0: 5.0, x1: 20.0, y1: 20.0 });
    }

    #[test]
    fn a_page_too_small_has_no_marks() {
        assert_eq!(MarkLayout::for_page([60.0, 300.0], 10.0), None);
        assert_eq!(MarkLayout::for_page([f64::NAN, 300.0], 10.0), None);
        assert_eq!(MarkLayout::for_page([300.0, 300.0], 200.0).map(|m| m.origin_mm), Some([50.0, 50.0]), "the inset kept in range");
    }
}
