//! Each object's cutting operation (ARCHITECTURE.md › 4.2).
//!
//! An object's own operation is a foreign attribute in the cutting namespace, `cut:op="score"`,
//! so it is saved natively, kept from SVG export to import, copied with the object and gone
//! with it. An object without one takes its nearest ancestor's (group or layer), and otherwise
//! the document's [`vectorcraft_cutjob::CuttingSettings::default_op`]. A value this version
//! doesn't read (from a newer one) counts as none and is kept until the operation is set.

use vectorcraft_cutjob::Op;
use vectorcraft_doc::{Document, ForeignAttribute, Node, NodeId};

/// The cutting namespace.
pub const NS: &str = "urn:vectorcraft:cutting:1";
/// The prefix SVG export gives the namespace.
pub const PREFIX: &str = "cut";
/// The attribute's local name.
pub const NAME: &str = "op";

fn ours(a: &ForeignAttribute) -> bool {
    a.ns == NS && a.name == NAME
}

/// The operation `n` sets itself, if it sets one this version reads.
pub fn own_op(n: &Node) -> Option<Op> {
    let attrs = n.extras().attrs.as_deref()?;
    attrs.foreign.iter().find(|a| ours(a)).and_then(|a| Op::parse(&a.value))
}

/// `n`'s attributes in the cutting namespace's `op`.
fn current(n: &Node) -> Vec<ForeignAttribute> {
    n.extras().attrs.as_deref().map(|a| a.foreign.iter().filter(|a| ours(a)).cloned().collect()).unwrap_or_default()
}

/// The attributes that set `op`.
fn wanted(op: Option<Op>) -> Vec<ForeignAttribute> {
    op.map(|op| ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: NAME.to_string(), value: op.id().to_string() })
        .into_iter()
        .collect()
}

/// Whether [`set_own_op`] with `op` would change `n`.
pub fn would_change(n: &Node, op: Option<Op>) -> bool {
    current(n) != wanted(op)
}

/// Sets `n`'s own operation (`None`: none, so it inherits). The object's other foreign
/// attributes stay. Whether anything changed.
pub fn set_own_op(n: &mut Node, op: Option<Op>) -> bool {
    let after = wanted(op);
    if current(n) == after {
        return false;
    }
    n.edit_attrs(|a| {
        a.foreign.retain(|f| !ours(f));
        a.foreign.extend(after);
    });
    true
}

/// Where an object's operation comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The object sets it.
    Own,
    /// Its nearest ancestor that sets one, this one.
    Inherited(NodeId),
    /// Nothing on the way to it sets one: the document's default.
    Default,
}

/// An object's operation and where it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub op: Op,
    pub source: Source,
}

/// The operation object `id` is cut with, `default` when neither it nor an ancestor sets one;
/// `None` when the document has no such object.
pub fn resolve(d: &Document, id: NodeId, default: Op) -> Option<Resolved> {
    let ancestry = d.ancestry(id)?;
    for (i, &a) in ancestry.iter().enumerate().rev() {
        if let Some(op) = d.node(a).and_then(own_op) {
            let source = if i + 1 == ancestry.len() { Source::Own } else { Source::Inherited(a) };
            return Some(Resolved { op, source });
        }
    }
    Some(Resolved { op: default, source: Source::Default })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vectorcraft_doc::Appearance;
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    fn square(d: &mut Document) -> Node {
        Node::path(d.alloc_id(), shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), Appearance::default())
    }

    /// A layer holding a group of two squares: (document, layer, group, first, second).
    fn grouped() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut d = Document::new(100.0, 100.0);
        let layer = d.layers[0].id;
        let (a, b) = (square(&mut d), square(&mut d));
        let (a_id, b_id) = (a.id, b.id);
        let g = Node::group(d.alloc_id(), vec![Arc::new(a), Arc::new(b)]);
        let g = d.insert(Some(layer), usize::MAX, g).unwrap();
        (d, layer, g, a_id, b_id)
    }

    #[test]
    fn the_own_op_is_a_foreign_attribute() {
        let mut d = Document::new(10.0, 10.0);
        let mut n = square(&mut d);
        assert_eq!(own_op(&n), None);
        assert!(set_own_op(&mut n, Some(Op::Score)));
        assert_eq!(own_op(&n), Some(Op::Score));
        let a = &n.extras().attrs.as_deref().unwrap().foreign;
        assert_eq!(a, &[ForeignAttribute { ns: NS.into(), prefix: "cut".into(), name: "op".into(), value: "score".into() }]);
        assert!(!set_own_op(&mut n, Some(Op::Score)), "unchanged");
        assert!(set_own_op(&mut n, Some(Op::NoCut)));
        assert_eq!(own_op(&n), Some(Op::NoCut));
        assert!(set_own_op(&mut n, None));
        assert_eq!(own_op(&n), None);
        assert!(n.extras().attrs.is_none(), "no attributes left");
        assert!(!set_own_op(&mut n, None));
    }

    #[test]
    fn other_foreign_attributes_stay() {
        let mut d = Document::new(10.0, 10.0);
        let mut n = square(&mut d);
        let pen = ForeignAttribute { ns: "https://example.org/plotter".into(), prefix: "plotter".into(), name: "pen".into(), value: "2".into() };
        n.edit_attrs(|a| a.foreign.push(pen.clone()));
        set_own_op(&mut n, Some(Op::Cut));
        set_own_op(&mut n, None);
        assert_eq!(n.extras().attrs.as_deref().unwrap().foreign, vec![pen]);
    }

    /// A newer version's value counts as none and stays until the operation is set.
    #[test]
    fn a_value_not_read_is_kept_until_set() {
        let mut d = Document::new(10.0, 10.0);
        let mut n = square(&mut d);
        let newer = ForeignAttribute { ns: NS.into(), prefix: "cut".into(), name: "op".into(), value: "perforate".into() };
        n.edit_attrs(|a| a.foreign.push(newer.clone()));
        assert_eq!(own_op(&n), None);
        assert_eq!(n.extras().attrs.as_deref().unwrap().foreign, vec![newer]);
        assert!(set_own_op(&mut n, Some(Op::Cut)));
        assert_eq!(n.extras().attrs.as_deref().unwrap().foreign.len(), 1);
        assert_eq!(own_op(&n), Some(Op::Cut));
    }

    #[test]
    fn ops_are_inherited_from_the_nearest_ancestor() {
        let (mut d, layer, g, a, b) = grouped();
        let r = |d: &Document, id| resolve(d, id, Op::Cut).unwrap();
        assert_eq!(r(&d, a), Resolved { op: Op::Cut, source: Source::Default });
        set_own_op(d.node_mut(layer).unwrap(), Some(Op::NoCut));
        assert_eq!(r(&d, a), Resolved { op: Op::NoCut, source: Source::Inherited(layer) });
        set_own_op(d.node_mut(g).unwrap(), Some(Op::Score));
        assert_eq!(r(&d, a), Resolved { op: Op::Score, source: Source::Inherited(g) });
        set_own_op(d.node_mut(b).unwrap(), Some(Op::Cut));
        assert_eq!(r(&d, b), Resolved { op: Op::Cut, source: Source::Own });
        assert_eq!(r(&d, a).op, Op::Score);
        assert_eq!(r(&d, g), Resolved { op: Op::Score, source: Source::Own });
        assert_eq!(resolve(&d, NodeId(999_999), Op::Cut), None);
    }

    #[test]
    fn the_default_is_the_documents() {
        let (d, _, _, a, _) = grouped();
        assert_eq!(resolve(&d, a, Op::Score), Some(Resolved { op: Op::Score, source: Source::Default }));
    }
}
