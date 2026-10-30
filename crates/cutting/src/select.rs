//! Which objects a job cuts, and how (ARCHITECTURE.md › 5.1, step 1).
//!
//! The objects on the settings' artboard that are shown: hidden objects and layers, template
//! layers and guides are left out; locked ones are cut. Each is cut with its operation
//! ([`crate::ops`]: its own, else its nearest group's or layer's, else the default), and those
//! set to No Cut are left out. A layer that doesn't print is still cut: print-and-cut work often
//! keeps its cut lines on one. Clipping paths are masks, not art, and aren't cut; the clip groups
//! around an object are noted for the geometry step to clip it to.
//!
//! Layers, groups and Intertwine groups are looked through. Every other object is one item:
//! a compound path is one shape, and type, symbols, blends, envelopes and repeats are made into
//! outlines in the geometry step.

use vectorcraft_cutjob::{CuttingSettings, Op, OpKind};
use vectorcraft_doc::{Document, Node, NodeId, NodeKind};
use vectorcraft_geom::Rect;

use crate::edge::{Edge, own_edge};
use crate::ops::own_op;

/// An object the job cuts.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: NodeId,
    pub op: OpKind,
    /// Set to Cut Outline: cut, along the outline it makes with the others set to it
    /// ([`crate::weld`]).
    pub outline: bool,
    /// Cut round the edge of what it paints, not along its path (C11.4, [`crate::edge`]).
    pub stroke_edge: bool,
    /// The clip groups and clipping layers around it, outermost first.
    pub clips: Vec<NodeId>,
}

/// The objects `d` cuts with settings `s`, in paint order (bottom first). Nothing when the
/// settings' artboard isn't in the document.
pub fn select(d: &Document, s: &CuttingSettings) -> Vec<Item> {
    let Some(artboard) = usize::try_from(s.artboard).ok().and_then(|i| d.artboards.get(i)).map(|a| a.rect) else { return vec![] };
    let mut out = vec![];
    let mut clips = vec![];
    for layer in &d.layers {
        visit(layer, (s.default_op, Edge::Path), artboard, &mut clips, &mut out, false);
    }
    out
}

/// The objects `d` would cut with settings `s` wherever they lie, on the artboard or off it (for
/// nesting, which brings them onto it).
pub fn select_everywhere(d: &Document, s: &CuttingSettings) -> Vec<Item> {
    let everywhere = Rect::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::INFINITY);
    let mut out = vec![];
    let mut clips = vec![];
    for layer in &d.layers {
        visit(layer, (s.default_op, Edge::Path), everywhere, &mut clips, &mut out, false);
    }
    out
}

/// The visible art in and under `ids`, wherever it lies and whatever it is set to cut as (C11.1:
/// what a contour goes round), each as a cut.
pub fn art_under(d: &Document, ids: &[NodeId]) -> Vec<Item> {
    let everywhere = Rect::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::INFINITY);
    let mut out = vec![];
    let mut clips = vec![];
    for id in ids {
        if let Some(n) = d.node(*id) {
            visit(n, (Op::Cut, Edge::Path), everywhere, &mut clips, &mut out, true);
        }
    }
    out
}

/// Whether `n` clips its other children with its first one.
fn clips_children(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Group { clip: true, .. } | NodeKind::Layer { clip: true, .. })
}

/// `inherited`: the operation and edge from the groups and layer around `n`. `every`: every
/// object, whatever it is set to cut as (each as a cut along its path).
fn visit(n: &Node, inherited: (Op, Edge), artboard: Rect, clips: &mut Vec<NodeId>, out: &mut Vec<Item>, every: bool) {
    if !n.visible || n.is_template() {
        return;
    }
    let op = if every { Op::Cut } else { own_op(n).unwrap_or(inherited.0) };
    let edge = if every { Edge::Path } else { own_edge(n).unwrap_or(inherited.1) };
    match &n.kind {
        NodeKind::Layer { children, .. } | NodeKind::Group { children, .. } | NodeKind::Intertwine { children } => {
            let clipping = clips_children(n);
            if clipping {
                clips.push(n.id);
            }
            for (i, c) in children.iter().enumerate() {
                // The clipping path itself: a mask, not art.
                if clipping && i == 0 {
                    continue;
                }
                visit(c, (op, edge), artboard, clips, out, every);
            }
            if clipping {
                clips.pop();
            }
        }
        NodeKind::Path { guide: true, .. } | NodeKind::Path { clipping: true, .. } => {}
        _ => {
            let kind = match op {
                Op::Cut | Op::Outline => OpKind::Cut,
                Op::Score => OpKind::Score,
                Op::NoCut => return,
            };
            // Bounds the model can't give here (type before layout, symbols) are left to the
            // geometry step; known ones off the artboard leave the object out.
            if n.geometric_bounds().is_some_and(|b| !touches(b, artboard)) {
                return;
            }
            out.push(Item { id: n.id, op: kind, outline: op == Op::Outline, stroke_edge: edge == Edge::Stroke, clips: clips.clone() });
        }
    }
}

/// `b` meets `artboard`, a line or point on it included (an open horizontal path has no area).
fn touches(b: Rect, artboard: Rect) -> bool {
    b.x0 <= artboard.x1 && b.x1 >= artboard.x0 && b.y0 <= artboard.y1 && b.y1 >= artboard.y0
}

#[cfg(test)]
mod tests;
