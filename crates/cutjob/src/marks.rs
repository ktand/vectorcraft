//! Print & Cut registration marks (C6.1, C12.1): where they go on the printed page and what they
//! are, generated here from the facts in `docs/cutting/protocol-silhouette.md` › Print & Cut (no
//! maker's artwork). The usual set of three: a filled square at the top left, and L-shaped corners
//! at the top right and bottom left, arms pointing in. Each mark's anchor is its outer corner; the
//! top-left anchor is the origin the machine cuts from once it has found the marks, and the
//! others lie the marks' width to its right and their length below it. The arms' length, the line
//! width and the inset from each edge can be set ([`MarkStyle`]); round the marks is an area the
//! sensor must see clear ([`MarkLayout::no_go`]).

use crate::mat::RectMm;

/// The top-left square's side, mm.
pub const SQUARE_MM: f64 = 5.0;
/// Each arm of an L corner, mm: the range, and the usual (what the machine is told by default,
/// `TB51,400` at 20 steps a mm).
pub const LENGTH_MM: (f64, f64) = (5.0, 20.0);
pub const DEFAULT_LENGTH_MM: f64 = 20.0;
/// The width the marks are drawn, mm: the range, and the usual (`TB53,10`).
pub const LINE_MM: (f64, f64) = (0.3, 1.0);
pub const DEFAULT_LINE_MM: f64 = 0.5;
/// How far in from the page's edges the marks' anchors go, mm: the range that can be set, the one
/// read from a file (documents from before C12.1 went down to 5), and the usual (5/8 in).
pub const INSET_MM: (f64, f64) = (10.0, 84.0);
pub const READ_INSET_MM: (f64, f64) = (5.0, 84.0);
pub const DEFAULT_INSET_MM: f64 = 15.88;
/// How far past a mark's arms the area kept clear for the sensor runs, mm (estimated from the
/// maker's software as the owner saw it, protocol notes › Print & Cut).
pub const NO_GO_MARGIN_MM: f64 = 2.0;

/// How the marks are drawn and where: the inset from each edge (left, top, right, bottom), the
/// arms' length and the line width, mm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkStyle {
    pub insets_mm: [f64; 4],
    pub length_mm: f64,
    pub line_mm: f64,
}

impl Default for MarkStyle {
    fn default() -> Self {
        MarkStyle { insets_mm: [DEFAULT_INSET_MM; 4], length_mm: DEFAULT_LENGTH_MM, line_mm: DEFAULT_LINE_MM }
    }
}

/// Where the marks go on a page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkLayout {
    /// The top-left anchor, from the page's top-left, mm.
    pub origin_mm: [f64; 2],
    /// From the top-left anchor across to the top-right one, and down to the bottom-left one, mm.
    pub size_mm: [f64; 2],
    /// The page, mm.
    pub page_mm: [f64; 2],
    /// Each L's arms, and the width the marks are drawn, mm.
    pub length_mm: f64,
    pub line_mm: f64,
}

/// One mark, in page millimetres.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// Filled.
    Square(RectMm),
    /// An L drawn [`MarkLayout::line_mm`] wide through these three points, the corner in the middle.
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
    /// The marks on a page `page_mm` (width, height) drawn as `style` says (each number kept in
    /// its range); `None` when the page is too small to hold them apart (or the numbers aren't
    /// finite).
    pub fn for_page(page_mm: [f64; 2], style: MarkStyle) -> Option<MarkLayout> {
        let finite = page_mm.iter().chain(&style.insets_mm).chain(&[style.length_mm, style.line_mm]).all(|v| v.is_finite());
        if !finite {
            return None;
        }
        let [l, t, r, b] = style.insets_mm.map(|v| v.clamp(READ_INSET_MM.0, READ_INSET_MM.1));
        let length = style.length_mm.clamp(LENGTH_MM.0, LENGTH_MM.1);
        let size = [page_mm[0] - l - r, page_mm[1] - t - b];
        (size.iter().all(|s| *s > 2.0 * length)).then_some(MarkLayout {
            origin_mm: [l, t],
            size_mm: size,
            page_mm,
            length_mm: length,
            line_mm: style.line_mm.clamp(LINE_MM.0, LINE_MM.1),
        })
    }

    /// The three marks, top-left square first.
    pub fn marks(&self) -> [Mark; 3] {
        let [x, y] = self.origin_mm;
        let [w, l] = self.size_mm;
        let (right, bottom, arm) = (x + w, y + l, self.length_mm);
        [
            Mark::Square(RectMm { x0: x, y0: y, x1: x + SQUARE_MM, y1: y + SQUARE_MM }),
            Mark::Corner([[right - arm, y], [right, y], [right, y + arm]]),
            Mark::Corner([[x, bottom - arm], [x, bottom], [x + arm, bottom]]),
        ]
    }

    /// Where nothing may be printed or cut, page mm, as the maker's software hatches it (protocol
    /// notes [ST]): the band along the top, from edge to edge down to the marks' line, and below
    /// it each mark's square, from the page's edges at its corner to [`NO_GO_MARGIN_MM`] past its
    /// arms. The areas don't overlap.
    pub fn no_go(&self) -> Vec<RectMm> {
        let [x, y] = self.origin_mm;
        let [w, h] = self.size_mm;
        let [pw, ph] = self.page_mm;
        let reach = self.length_mm.max(SQUARE_MM) + NO_GO_MARGIN_MM;
        vec![
            RectMm { x0: 0.0, y0: 0.0, x1: pw, y1: y },
            RectMm { x0: 0.0, y0: y, x1: x + reach, y1: y + reach },
            RectMm { x0: x + w - reach, y0: y, x1: pw, y1: y + reach },
            RectMm { x0: 0.0, y0: y + h - reach, x1: x + reach, y1: ph },
        ]
    }

    /// The area between the marks' anchors, from the page's top-left, mm: what can be cut.
    pub fn area(&self) -> RectMm {
        let [x, y] = self.origin_mm;
        RectMm { x0: x, y0: y, x1: x + self.size_mm[0], y1: y + self.size_mm[1] }
    }

    /// What the machine is told the marks are, in its steps of 1/20 mm: (arm length, line width).
    pub fn machine_steps(&self) -> (i64, i64) {
        // Finite and in range (`for_page`): the casts can't overflow.
        ((self.length_mm * 20.0).round() as i64, (self.line_mm * 20.0).round() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(inset: f64) -> MarkStyle {
        MarkStyle { insets_mm: [inset; 4], ..MarkStyle::default() }
    }

    /// A Letter page (215.9 × 279.4 mm), marks 10 mm in.
    #[test]
    fn marks_on_a_page() {
        let m = MarkLayout::for_page([215.9, 279.4], style(10.0)).unwrap();
        assert_eq!(m.origin_mm, [10.0, 10.0]);
        assert!((m.size_mm[0] - 195.9).abs() < 1e-9 && (m.size_mm[1] - 259.4).abs() < 1e-9);
        let [square, top_right, bottom_left] = m.marks();
        assert_eq!(square, Mark::Square(RectMm { x0: 10.0, y0: 10.0, x1: 15.0, y1: 15.0 }));
        let Mark::Corner(tr) = top_right else { panic!() };
        assert!((tr[1][0] - 205.9).abs() < 1e-9 && tr[1][1] == 10.0, "the corner at the anchor: {tr:?}");
        assert!(tr[0][0] < tr[1][0] && tr[2][1] > tr[1][1], "arms pointing in: {tr:?}");
        let Mark::Corner(bl) = bottom_left else { panic!() };
        assert!(bl[1][0] == 10.0 && (bl[1][1] - 269.4).abs() < 1e-9 && bl[0][1] < bl[1][1] && bl[2][0] > bl[1][0], "{bl:?}");
        assert_eq!(m.no_go()[0], RectMm { x0: 0.0, y0: 0.0, x1: 215.9, y1: 10.0 }, "the band along the top, to the marks' line");
        assert_eq!(m.no_go()[1], RectMm { x0: 0.0, y0: 10.0, x1: 32.0, y1: 32.0 }, "the square mark's corner, below it");
        let z = m.no_go();
        let overlap = |a: &RectMm, b: &RectMm| a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1;
        assert!(z.iter().enumerate().all(|(i, a)| z.iter().skip(i + 1).all(|b| !overlap(a, b))), "no two overlap: {z:?}");
        let bl = m.no_go()[3];
        assert!(bl.x0 == 0.0 && bl.x1 == 32.0 && (bl.y0 - 247.4).abs() < 1e-9 && bl.y1 == 279.4, "the bottom-left mark to the page's corner: {bl:?}");
        assert_eq!(m.machine_steps(), (400, 10), "the usual marks are what the machine was always told");
    }

    /// Each side's own inset, and the arms and line kept in their ranges.
    #[test]
    fn each_side_has_its_inset() {
        let s = MarkStyle { insets_mm: [12.0, 20.0, 30.0, 40.0], length_mm: 50.0, line_mm: 0.1 };
        let m = MarkLayout::for_page([215.9, 279.4], s).unwrap();
        assert_eq!(m.origin_mm, [12.0, 20.0]);
        assert!((m.size_mm[0] - 173.9).abs() < 1e-9 && (m.size_mm[1] - 219.4).abs() < 1e-9);
        assert_eq!((m.length_mm, m.line_mm), (20.0, 0.3));
        let short = MarkLayout::for_page([215.9, 279.4], MarkStyle { length_mm: 5.0, line_mm: 1.0, ..style(10.0) }).unwrap();
        assert_eq!(short.machine_steps(), (100, 20));
    }

    #[test]
    fn a_page_too_small_has_no_marks() {
        assert_eq!(MarkLayout::for_page([60.0, 300.0], style(10.0)), None);
        assert_eq!(MarkLayout::for_page([f64::NAN, 300.0], style(10.0)), None);
        assert_eq!(MarkLayout::for_page([300.0, 300.0], style(200.0)).map(|m| m.origin_mm), Some([84.0, 84.0]), "the inset kept in range");
    }
}
