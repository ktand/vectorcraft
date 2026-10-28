//! What each selected object cuts along (ARCHITECTURE.md › 5.1, steps 2 and 3): its outlines as
//! drawn, in document points, one subpath each.
//!
//! A path or compound path is cut along its path after its geometry effects ([`drawn_art`]), on
//! the path itself: a stroke is cut on its centre line, not around its outline. Type is cut
//! along its glyph outlines, a symbol instance along its symbol's art, and blends, envelopes,
//! repeats and placed documents along their evaluated art. Images and gradient meshes have no
//! line to cut and are left out with a warning. An outline drawn twice in the same object (type's
//! runs under its own fill) is cut once.
//!
//! Art inside clip groups and clipping layers is cut only where it shows ([`crate::clip`]); a
//! clip group inside a symbol's or a live object's own art isn't applied yet (cut whole, with a
//! warning).

use std::collections::HashSet;

use vectorcraft_cutjob::{JobWarning, OpKind, SourceRef};
use vectorcraft_doc::{Document, MAX_DEPTH, Node, NodeKind, SymbolBoxes};
use vectorcraft_effects::preview::drawn_art;
use vectorcraft_geom::{BezPath, PathEl};

use crate::select::Item;

/// One subpath to cut, in document points.
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    pub op: OpKind,
    /// Its object is set to Cut Outline: [`crate::weld`] unites it with the others.
    pub outline: bool,
    pub source: SourceRef,
    pub closed: bool,
    /// One subpath: a move, then lines and curves.
    pub path: BezPath,
}

/// Most subpaths one object gives (type set very small over a page, a dense repeat).
const MAX_SUBPATHS: usize = 100_000;

/// What walking an object's art ran into, each reported once.
#[derive(Default)]
struct Notes {
    images: bool,
    meshes: bool,
    /// A clip group inside the object's own art (a symbol's, a live object's): not applied.
    clipped: bool,
    /// The booleans refused to clip an outline, or it was too large to: cut whole.
    clip_failed: bool,
    too_deep: bool,
    no_outline: bool,
    too_many: bool,
}

/// The outlines `items` cut along, in their order, and what they leave out.
pub fn outlines(d: &Document, items: &[Item]) -> (Vec<Outline>, Vec<JobWarning>) {
    let symbols = d.symbol_boxes();
    let (mut out, mut warnings) = (vec![], vec![]);
    for item in items {
        let Some(n) = d.node(item.id) else { continue };
        let mut paths = vec![];
        let mut notes = Notes::default();
        collect(d, &symbols, n, 0, &mut paths, &mut notes);
        let mut subs: Vec<(BezPath, bool)> = paths.iter().flat_map(subpaths).collect();
        // The clip groups around it, outermost first: each clips what the one outside it left.
        for &clip in &item.clips {
            let Some(region) = crate::clip::region(d, clip) else {
                // Clipped to nothing: hidden, as it is drawn.
                subs.clear();
                break;
            };
            subs = subs
                .into_iter()
                .flat_map(|(p, closed)| {
                    crate::clip::clip(&p, closed, &region).unwrap_or_else(|| {
                        notes.clip_failed = true;
                        vec![(p, closed)]
                    })
                })
                .collect();
        }
        let mut seen = HashSet::new();
        let mut index = 0u32;
        for (path, closed) in subs {
            if index as usize >= MAX_SUBPATHS {
                notes.too_many = true;
                break;
            }
            if !seen.insert((key(&path), closed)) {
                continue;
            }
            out.push(Outline { op: item.op, outline: item.outline, source: SourceRef { node: item.id.0, subpath: index }, closed, path });
            index += 1;
        }
        let source = Some(SourceRef { node: item.id.0, subpath: 0 });
        let mut warn = |on: bool, message: &str| {
            if on {
                warnings.push(JobWarning { source, message: message.to_string() });
            }
        };
        warn(notes.images, "an image has no line to cut and is left out");
        warn(notes.meshes, "a gradient mesh has no line to cut and is left out");
        warn(notes.clipped, "clipping masks inside symbols and live objects aren't applied yet: the art inside one is cut whole");
        warn(notes.clip_failed, "a clipping mask couldn't be applied to some of the art: it is cut whole");
        warn(notes.too_deep, "art nested too deep is left out");
        warn(notes.no_outline, "type couldn't be outlined (is its font missing?) and is left out");
        warn(notes.too_many, "only the first 100000 outlines of an object are cut");
    }
    (out, warnings)
}

/// `n`'s paths as drawn, added to `paths`.
fn collect(d: &Document, symbols: &SymbolBoxes, n: &Node, depth: usize, paths: &mut Vec<BezPath>, notes: &mut Notes) {
    if depth > MAX_DEPTH {
        notes.too_deep = true;
        return;
    }
    if !n.visible {
        return;
    }
    // Geometry effects applied, at every depth of `n`: what is under it then has none left.
    if let Some(art) = drawn_art(symbols, n) {
        return walk(d, symbols, &art, depth, paths, notes);
    }
    walk(d, symbols, n, depth, paths, notes);
}

fn walk(d: &Document, symbols: &SymbolBoxes, n: &Node, depth: usize, paths: &mut Vec<BezPath>, notes: &mut Notes) {
    let next = depth + 1;
    match &n.kind {
        NodeKind::Path { guide: true, .. } | NodeKind::Path { clipping: true, .. } => {}
        NodeKind::Path { path, .. } => paths.push(path.to_bezpath()),
        NodeKind::Compound { children, .. } => paths.extend(children.iter().filter_map(|c| c.path_data()).map(|p| p.to_bezpath())),
        NodeKind::Layer { children, .. } | NodeKind::Group { children, .. } | NodeKind::Intertwine { children } => {
            let clip = matches!(n.kind, NodeKind::Group { clip: true, .. } | NodeKind::Layer { clip: true, .. });
            notes.clipped |= clip;
            for (i, c) in children.iter().enumerate() {
                if !(clip && i == 0) {
                    collect(d, symbols, c, next, paths, notes);
                }
            }
        }
        NodeKind::Text(_) => match vectorcraft_effects::outline_text(n) {
            Some(o) => walk(d, symbols, &o, next, paths, notes),
            None => notes.no_outline = true,
        },
        NodeKind::SymbolInstance { symbol, xf } => {
            if let Some(sym) = d.symbols.iter().find(|s| &s.name == symbol) {
                let mut art = (*sym.art).clone();
                art.transform(*xf, false);
                collect(d, symbols, &art, next, paths, notes);
            }
        }
        NodeKind::Image(_) => notes.images = true,
        NodeKind::Mesh(_) => notes.meshes = true,
        NodeKind::Blend { .. } | NodeKind::Envelope { .. } | NodeKind::Repeat(_) | NodeKind::PlacedDocument(_) => {
            let g = vectorcraft_effects::expand_live_deep(Some(d), n);
            // What expanding gives is plain art; a live object still there didn't evaluate.
            if std::mem::discriminant(&g.kind) == std::mem::discriminant(&n.kind) {
                return;
            }
            walk(d, symbols, &g, next, paths, notes);
        }
    }
}

/// `p` split into subpaths that draw something, each with whether it is closed. A segment after
/// a close with no move before it starts again from the closed subpath's first point.
pub(crate) fn subpaths(p: &BezPath) -> Vec<(BezPath, bool)> {
    let mut out = vec![];
    let mut cur = BezPath::new();
    let mut start = None;
    let mut finish = |cur: &mut BezPath, closed: bool| {
        if cur.elements().len() > 1 {
            out.push((std::mem::take(cur), closed));
        } else {
            cur.truncate(0);
        }
    };
    for el in p.elements() {
        match el {
            PathEl::MoveTo(a) => {
                finish(&mut cur, false);
                start = Some(*a);
                cur.push(*el);
            }
            PathEl::ClosePath => finish(&mut cur, true),
            _ => {
                if cur.elements().is_empty() {
                    let Some(s) = start else { continue };
                    cur.push(PathEl::MoveTo(s));
                }
                cur.push(*el);
            }
        }
    }
    finish(&mut cur, false);
    out
}

/// `p`'s coordinates, bit for bit: equal keys, the same outline.
fn key(p: &BezPath) -> Vec<u64> {
    let mut k = Vec::with_capacity(p.elements().len() * 7);
    for el in p.elements() {
        let (tag, pts) = match *el {
            PathEl::MoveTo(a) => (0, [Some(a), None, None]),
            PathEl::LineTo(a) => (1, [Some(a), None, None]),
            PathEl::QuadTo(a, b) => (2, [Some(a), Some(b), None]),
            PathEl::CurveTo(a, b, c) => (3, [Some(a), Some(b), Some(c)]),
            PathEl::ClosePath => (4, [None, None, None]),
        };
        k.push(tag);
        k.extend(pts.into_iter().flatten().flat_map(|p| [p.x.to_bits(), p.y.to_bits()]));
    }
    k
}

#[cfg(test)]
mod tests;
