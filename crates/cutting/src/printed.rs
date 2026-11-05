//! Fork: what Print & Cut prints (owner, 2026-11-05). A shape often has a thin stroke only to
//! show where it is cut; printed, that line shows beside a cut that lands a little off. With
//! Print & Cut on, File › Print and its preview leave out every stroke of a hairline's width
//! ([`HAIRLINE_PT`] or thinner) on the objects that are cut or scored; their fills, wider
//! strokes and everything else print as they are. The document itself is left alone: the
//! canvas and the cut preview still show the hairlines.

use vectorcraft_cutjob::OpKind;
use vectorcraft_doc::{AppearanceItem, Document};

/// The widest stroke left out, pt (about 0.09 mm).
pub const HAIRLINE_PT: f64 = 0.25;

/// `d` as Print & Cut prints it: a copy without the hairline strokes of the objects it cuts or
/// scores; `None` when nothing would change (Print & Cut off, or no such stroke).
pub fn for_print(d: &Document) -> Option<Document> {
    let s = crate::settings(d);
    if !s.print_and_cut {
        return None;
    }
    let items = crate::select::select(d, &s);
    let hairline = |i: &AppearanceItem| matches!(i, AppearanceItem::Stroke(k) if k.width <= HAIRLINE_PT);
    let ids: Vec<_> = items
        .iter()
        .filter(|i| matches!(i.op, OpKind::Cut | OpKind::Score))
        .map(|i| i.id)
        .filter(|id| d.node(*id).is_some_and(|n| n.appearance.items.iter().any(hairline)))
        .collect();
    if ids.is_empty() {
        return None;
    }
    let mut out = d.clone();
    for id in ids {
        if let Some(n) = out.node_mut(id) {
            n.appearance.items.retain(|i| !hairline(i));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::{Color, Paint};
    use vectorcraft_doc::{Appearance, Node};
    use vectorcraft_geom::{PathData, Point, SubPath};

    use super::*;

    fn square(d: &mut Document, x: f64, width: f64) -> vectorcraft_doc::NodeId {
        let pts = [Point::new(x, 10.0), Point::new(x + 50.0, 10.0), Point::new(x + 50.0, 60.0), Point::new(x, 60.0)];
        let n = Node::path(
            d.alloc_id(),
            PathData::single(SubPath::polyline(&pts, true)),
            Appearance::basic(Paint::solid(Color::BLACK), Paint::solid(Color::BLACK), width),
        );
        let layer = d.layers.last().map(|l| l.id);
        d.insert(layer, usize::MAX, n).unwrap()
    }

    fn strokes(d: &Document, id: vectorcraft_doc::NodeId) -> usize {
        d.node(id).unwrap().appearance.items.iter().filter(|i| matches!(i, AppearanceItem::Stroke(_))).count()
    }

    /// With Print & Cut on, a hairline's stroke is left out of the print and a wider one kept,
    /// the fills printed; off, nothing changes; the document itself keeps both.
    #[test]
    fn hairline_cut_lines_dont_print() {
        let mut d = Document::new(400.0, 400.0);
        let thin = square(&mut d, 10.0, 0.25);
        let wide = square(&mut d, 100.0, 1.0);
        assert!(for_print(&d).is_none(), "Print & Cut off");
        let mut s = crate::settings(&d);
        s.print_and_cut = true;
        crate::set_settings(&mut d, &s);
        let p = for_print(&d).unwrap();
        assert_eq!((strokes(&p, thin), strokes(&p, wide)), (0, 1));
        assert!(p.node(thin).unwrap().appearance.items.iter().any(|i| matches!(i, AppearanceItem::Fill(_))));
        assert_eq!(strokes(&d, thin), 1, "the document keeps it");
        // An object left out of the cut keeps its hairline in the print.
        crate::ops::set_own_op(d.node_mut(thin).unwrap(), Some(vectorcraft_cutjob::Op::NoCut));
        assert!(for_print(&d).is_none());
    }
}
