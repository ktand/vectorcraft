//! Fork: what an object is cut along (C11.4, ARCHITECTURE.md › 4.2 and R6). By default a path is
//! cut on the path itself, so a stroke is cut down its centre line. Set to its stroke's edge, it
//! is cut round what it paints instead: its strokes' outlines (width, caps, joins, dashes,
//! alignment and arrowheads, as on the canvas) united with its fill. Thick lettering and line art
//! then cut as they look.
//!
//! Kept as a second foreign attribute in the cutting namespace, `cut:edge="stroke"`, inherited
//! like the operation ([`crate::ops`]): an object without one takes its nearest group's or
//! layer's, else the path. A value this version doesn't read counts as none and is kept until the
//! edge is set.

use vectorcraft_doc::appearance::AppearanceItem;
use vectorcraft_doc::{Document, ForeignAttribute, Node, NodeId};
use vectorcraft_geom::{FillRule, PathData};

use crate::ops::{NS, PREFIX};

/// The attribute's local name.
pub const NAME: &str = "edge";

/// What an object is cut along.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Edge {
    /// The path itself (a stroke down its centre line).
    #[default]
    Path,
    /// The edge of what it paints: its strokes' outlines and its fill.
    Stroke,
}

impl Edge {
    pub fn id(self) -> &'static str {
        match self {
            Edge::Path => "path",
            Edge::Stroke => "stroke",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "path" => Some(Edge::Path),
            "stroke" => Some(Edge::Stroke),
            _ => None,
        }
    }
}

fn ours(a: &ForeignAttribute) -> bool {
    a.ns == NS && a.name == NAME
}

/// The edge `n` sets itself, if it sets one this version reads.
pub fn own_edge(n: &Node) -> Option<Edge> {
    let attrs = n.extras().attrs.as_deref()?;
    attrs.foreign.iter().find(|a| ours(a)).and_then(|a| Edge::parse(&a.value))
}

fn current(n: &Node) -> Vec<ForeignAttribute> {
    n.extras().attrs.as_deref().map(|a| a.foreign.iter().filter(|a| ours(a)).cloned().collect()).unwrap_or_default()
}

fn wanted(edge: Option<Edge>) -> Vec<ForeignAttribute> {
    edge.map(|e| ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: NAME.to_string(), value: e.id().to_string() })
        .into_iter()
        .collect()
}

/// Whether [`set_own_edge`] with `edge` would change `n`.
pub fn would_change(n: &Node, edge: Option<Edge>) -> bool {
    current(n) != wanted(edge)
}

/// Sets `n`'s own edge (`None`: none, so it inherits); its other foreign attributes stay.
/// Whether anything changed.
pub fn set_own_edge(n: &mut Node, edge: Option<Edge>) -> bool {
    let after = wanted(edge);
    if current(n) == after {
        return false;
    }
    n.edit_attrs(|a| {
        a.foreign.retain(|f| !ours(f));
        a.foreign.extend(after);
    });
    true
}

/// The edge object `id` is cut along: its own, else its nearest ancestor's, else the path; with
/// the object or ancestor that sets it (`None`: nothing does). `None` without such an object.
pub fn resolve(d: &Document, id: NodeId) -> Option<(Edge, Option<NodeId>)> {
    let ancestry = d.ancestry(id)?;
    for &a in ancestry.iter().rev() {
        if let Some(e) = d.node(a).and_then(own_edge) {
            return Some((e, Some(a)));
        }
    }
    Some((Edge::Path, None))
}

/// The edge of what `n` paints along `path` (fill rule `rule`): its painted strokes' outlines and,
/// when it is filled, the fill, united. `None` when it paints no stroke (it is cut along its path)
/// or the outlines couldn't be united.
pub fn painted_edge(n: &Node, path: &PathData, rule: FillRule) -> Option<PathData> {
    let strokes: Vec<PathData> =
        n.appearance.painted_strokes().map(|st| vectorcraft_effects::stroke::outline_region(path, rule, st)).filter(|p| !p.is_empty()).collect();
    if strokes.is_empty() {
        return None;
    }
    let filled = n.appearance.items.iter().any(|i| matches!(i, AppearanceItem::Fill(f) if f.visible && !f.paint.is_none()));
    let mut inputs: Vec<(&PathData, FillRule)> = strokes.iter().map(|p| (p, FillRule::NonZero)).collect();
    if filled {
        inputs.push((path, rule));
    }
    vectorcraft_pathops::try_unite_all(&inputs).ok().filter(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::{Color, Paint};
    use vectorcraft_doc::{Appearance, NodeKind};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    /// A 100 × 50 pt rectangle with a black stroke `width` wide, filled or not.
    fn stroked(d: &mut Document, width: f64, filled: bool) -> Node {
        let fill = if filled { Paint::solid(Color::BLACK) } else { Paint::None };
        Node::path(d.alloc_id(), shapes::rectangle(Rect::new(0.0, 0.0, 100.0, 50.0)), Appearance::basic(fill, Paint::solid(Color::BLACK), width))
    }

    fn edge_of(n: &Node) -> Option<PathData> {
        let NodeKind::Path { path, rule, .. } = &n.kind else { return None };
        painted_edge(n, path, *rule)
    }

    #[test]
    fn the_edge_is_a_foreign_attribute_and_inherited() {
        let mut d = Document::new(200.0, 200.0);
        let layer = d.layers[0].id;
        let n = stroked(&mut d, 4.0, false);
        let id = d.insert(Some(layer), usize::MAX, n).unwrap();
        assert_eq!(resolve(&d, id), Some((Edge::Path, None)));
        assert!(set_own_edge(d.node_mut(layer).unwrap(), Some(Edge::Stroke)));
        assert_eq!(resolve(&d, id), Some((Edge::Stroke, Some(layer))));
        assert!(set_own_edge(d.node_mut(id).unwrap(), Some(Edge::Path)));
        assert_eq!(resolve(&d, id), Some((Edge::Path, Some(id))), "its own wins");
        assert!(!set_own_edge(d.node_mut(id).unwrap(), Some(Edge::Path)), "unchanged");
        let a = &d.node(id).unwrap().extras().attrs.as_deref().unwrap().foreign;
        assert_eq!(a, &[ForeignAttribute { ns: NS.into(), prefix: "cut".into(), name: "edge".into(), value: "path".into() }]);
        assert!(set_own_edge(d.node_mut(id).unwrap(), None));
        assert_eq!(resolve(&d, id), Some((Edge::Stroke, Some(layer))));
        assert_eq!(resolve(&d, NodeId(999_999)), None);
    }

    /// The operation and the edge are separate attributes: setting one leaves the other.
    #[test]
    fn the_edge_and_the_operation_are_kept_apart() {
        let mut d = Document::new(200.0, 200.0);
        let mut n = stroked(&mut d, 4.0, false);
        crate::ops::set_own_op(&mut n, Some(vectorcraft_cutjob::Op::Score));
        set_own_edge(&mut n, Some(Edge::Stroke));
        crate::ops::set_own_op(&mut n, None);
        assert_eq!(own_edge(&n), Some(Edge::Stroke));
        set_own_edge(&mut n, None);
        assert!(n.extras().attrs.is_none(), "no attributes left");
    }

    /// A 10 pt stroke round a rectangle: its edge runs 5 pt outside the path, and 5 pt inside
    /// unless it's filled.
    #[test]
    fn a_strokes_edge_is_its_outline() {
        let mut d = Document::new(200.0, 200.0);
        let edge = edge_of(&stroked(&mut d, 10.0, false)).unwrap();
        let b = edge.bounds().unwrap();
        assert!((b.x0 + 5.0).abs() < 1e-6 && (b.x1 - 105.0).abs() < 1e-6 && (b.y1 - 55.0).abs() < 1e-6, "{b:?}");
        assert_eq!(edge.subpaths.len(), 2, "the outer edge and the inner one");
        let edge = edge_of(&stroked(&mut d, 10.0, true)).unwrap();
        assert_eq!(edge.subpaths.len(), 1, "the fill fills the inside");
        assert!((edge.bounds().unwrap().x0 + 5.0).abs() < 1e-6);
    }

    #[test]
    fn no_stroke_no_edge() {
        let mut d = Document::new(200.0, 200.0);
        assert_eq!(edge_of(&stroked(&mut d, 0.0, true)), None);
        let mut n = stroked(&mut d, 10.0, true);
        n.appearance = Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 10.0).into();
        assert_eq!(edge_of(&n), None);
    }
}
