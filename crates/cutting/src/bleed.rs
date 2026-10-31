//! Fork: print bleed (C12.3). A Print & Cut cut that lands a little off the print leaves a sliver
//! of white paper at the edge; a bleed spreads each printed object's outer colour a set distance
//! past its edge, so a cut a little off still lands in colour. It is a layer of its own at the
//! bottom of the document (locked, not cut, found again by `cut:bleed`, whose value fingerprints
//! the art it was made from): for each printed object, its painted outline (fill and strokes, as
//! cut round a stroke's edge, [`crate::edge`]) grown by the distance with round corners, filled
//! with its stroke's colour if it has one, else its fill's. Behind everything, only the part past
//! the art shows. Images and gradient meshes have no outline to grow and get none.

use vectorcraft_color::Paint;
use vectorcraft_cutjob::Op;
use vectorcraft_doc::appearance::FillLayer;
use vectorcraft_doc::{Appearance, AppearanceItem, Document, ForeignAttribute, LayerColor, Node, NodeId, NodeKind};
use vectorcraft_geom::{FillRule, PathData, PathEl};
use vectorcraft_pathops::{Join, offset_path, try_unite_all};

use crate::ops::{NS, PREFIX, set_own_op};
use crate::placement::MM_PER_PT;

/// How far the bleed runs past the art, mm: the range, and the usual (as the maker's software).
pub const RADIUS_MM: (f64, f64) = (0.1, 5.0);
pub const DEFAULT_RADIUS_MM: f64 = 1.27;
/// The bleed layer's name, and the attribute that marks it.
pub const LAYER_NAME: &str = "Print Bleed";
pub const NAME: &str = "bleed";
/// Most objects one bleed is made for.
const MAX_OBJECTS: usize = 20_000;

fn marker(n: &Node) -> Option<&str> {
    n.extras().attrs.as_deref()?.foreign.iter().find(|f| f.ns == NS && f.name == NAME).map(|f| f.value.as_str())
}

/// The document's bleed layer, if it has one (a top-level layer).
pub fn bleed_layer(d: &Document) -> Option<NodeId> {
    d.layers.iter().find(|l| marker(l).is_some()).map(|l| l.id)
}

/// The printed objects a bleed is made for: the visible art on every layer but the bleed, the
/// registration marks and the cut contour (cut lines, not print).
fn sources(d: &Document) -> Vec<crate::select::Item> {
    let skip = [bleed_layer(d), crate::marks::marks_layer(d), crate::contour::contour_layer(d)];
    let layers: Vec<NodeId> = d.layers.iter().map(|l| l.id).filter(|id| !skip.contains(&Some(*id))).collect();
    crate::select::art_under(d, &layers)
}

/// The colour the bleed of `n` is filled with: its first painted leaf's stroke colour if it has
/// one, else its fill's (type through its outlines, a symbol through its art).
fn outer_paint(d: &Document, n: &Node) -> Option<Paint> {
    let own = |n: &Node| {
        let a = &n.appearance;
        a.painted_strokes().next().map(|s| s.paint.stroke_as_fill()).or_else(|| {
            a.items.iter().find_map(|i| match i {
                AppearanceItem::Fill(f) if f.visible && !f.paint.is_none() => Some(f.paint.clone()),
                _ => None,
            })
        })
    };
    let mut found = None;
    let mut look = |n: &Node| {
        n.walk(&mut |c| {
            if found.is_none() {
                found = own(c);
            }
        });
    };
    match &n.kind {
        NodeKind::Text(_) => look(&vectorcraft_effects::outline_text(n)?),
        NodeKind::SymbolInstance { symbol, .. } => look(&d.symbols.iter().find(|s| &s.name == symbol)?.art),
        _ => look(n),
    }
    found
}

/// Each printed object's painted area, document points (even-odd, closed), and its bleed colour.
fn painted(d: &Document) -> Result<Vec<(NodeId, PathData, Paint)>, String> {
    let items = sources(d);
    if items.len() > MAX_OBJECTS {
        return Err(format!("at most {MAX_OBJECTS} objects can have a bleed"));
    }
    let mut out = vec![];
    for item in items {
        let Some(n) = d.node(item.id) else { continue };
        let Some(paint) = outer_paint(d, n) else { continue };
        let edge = crate::select::Item { stroke_edge: true, ..item };
        let (outlines, _) = crate::geometry::outlines(d, std::slice::from_ref(&edge));
        let mut subpaths = vec![];
        for o in outlines {
            let mut bp = o.path;
            if !matches!(bp.elements().last(), Some(PathEl::ClosePath)) {
                bp.push(PathEl::ClosePath);
            }
            subpaths.extend(PathData::from_bezpath(&bp).subpaths);
        }
        if !subpaths.is_empty() {
            out.push((edge.id, PathData::new(subpaths), paint));
        }
    }
    Ok(out)
}

/// A fingerprint of what the bleed is made from (each object's area and colour, and the
/// distance): 64-bit FNV-1a, in hex. A bleed layer whose fingerprint differs is out of date.
fn fingerprint(art: &[(NodeId, PathData, Paint)], radius_mm: f64) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(&radius_mm.to_bits().to_le_bytes());
    for (id, path, paint) in art {
        eat(&id.0.to_le_bytes());
        for s in &path.subpaths {
            for a in &s.anchors {
                for v in [a.p.x, a.p.y, a.h_in.x, a.h_in.y, a.h_out.x, a.h_out.y] {
                    eat(&v.to_bits().to_le_bytes());
                }
            }
        }
        eat(format!("{paint:?}").as_bytes());
    }
    format!("{h:016x}")
}

/// A new bleed layer for `d`'s art, `radius_mm` past it (ids from `d`), and how many objects it
/// bleeds, or why there can't be one.
pub fn bleed_layer_node(d: &mut Document, radius_mm: f64) -> Result<(Node, usize), String> {
    if !(radius_mm.is_finite() && (RADIUS_MM.0..=RADIUS_MM.1).contains(&radius_mm)) {
        return Err(format!("the bleed is {} to {} mm", RADIUS_MM.0, RADIUS_MM.1));
    }
    let art = painted(d)?;
    let print = fingerprint(&art, radius_mm);
    let distance = radius_mm / MM_PER_PT;
    let mut children = vec![];
    for (_, area, paint) in &art {
        let clean = try_unite_all(&[(area, FillRule::EvenOdd)]).map_err(|e| format!("an object's outline couldn't be made: {e}"))?;
        let grown = offset_path(&clean, distance, Join::Round, 4.0);
        if grown.is_empty() {
            continue;
        }
        let fill = Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(paint.clone()))], ..Default::default() };
        children.push(std::sync::Arc::new(Node::path(d.alloc_id(), grown, fill)));
    }
    if children.is_empty() {
        return Err("there is no printed art to bleed".into());
    }
    let count = children.len();
    let mut layer = Node::layer(d.alloc_id(), LAYER_NAME, LayerColor::Preset(3));
    layer.locked = true;
    set_own_op(&mut layer, Some(Op::NoCut));
    layer.edit_attrs(|at| at.foreign.push(ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: NAME.to_string(), value: print }));
    if let Some(c) = layer.children_mut() {
        *c = children;
    }
    Ok((layer, count))
}

/// The warning for a bleed made from art that has changed since.
pub const STALE_BLEED: &str = "the print bleed was made before the art changed: press Update Bleed, then print again";

/// Whether `d` has a bleed layer made from other art or another distance than `radius_mm`.
pub fn is_stale(d: &Document, radius_mm: f64) -> bool {
    let Some(layer) = bleed_layer(d).and_then(|id| d.node(id)) else { return false };
    let Some(made) = marker(layer) else { return false };
    painted(d).map_or(true, |art| fingerprint(&art, radius_mm) != made)
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::Color;
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    fn doc() -> (Document, NodeId) {
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        let red = Appearance::basic(Paint::solid(Color::rgb(1.0, 0.0, 0.0)), Paint::None, 0.0);
        let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(100.0, 100.0, 200.0, 200.0)), red);
        let id = d.insert(Some(layer), usize::MAX, sq).unwrap();
        (d, id)
    }

    fn add(d: &mut Document, radius: f64) -> NodeId {
        let (layer, _) = bleed_layer_node(d, radius).unwrap();
        d.insert(None, 0, layer).unwrap()
    }

    /// A red square's bleed: red, the distance past it all round, round at the corners; the
    /// layer at the bottom, locked, not cut, found again.
    #[test]
    fn a_square_bleeds_its_colour() {
        let (mut d, _) = doc();
        let (layer, count) = bleed_layer_node(&mut d, 2.0).unwrap();
        assert_eq!(count, 1);
        assert!(layer.locked && crate::ops::own_op(&layer) == Some(Op::NoCut));
        let id = d.insert(None, 0, layer).unwrap();
        assert_eq!(bleed_layer(&d), Some(id));
        let n = &d.node(id).unwrap().children().unwrap()[0];
        let b = n.geometric_bounds().unwrap();
        let r = 2.0 / MM_PER_PT;
        assert!((b.x0 - (100.0 - r)).abs() < 1e-3 && (b.y1 - (200.0 + r)).abs() < 1e-3, "{b:?}");
        assert_eq!(n.appearance.fill().map(|f| f.paint.clone()), Some(Paint::solid(Color::rgb(1.0, 0.0, 0.0))));
        let job = crate::build_job(&d).unwrap();
        assert_eq!(job.paths.len(), 1, "the bleed isn't cut");
    }

    /// A stroked line bleeds its stroke's colour round the stroke.
    #[test]
    fn a_stroke_bleeds_round_the_stroke() {
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        let blue = Appearance::basic(Paint::None, Paint::solid(Color::rgb(0.0, 0.0, 1.0)), 10.0);
        let line =
            Node::path(d.alloc_id(), shapes::line(vectorcraft_geom::Point::new(50.0, 100.0), vectorcraft_geom::Point::new(250.0, 100.0)), blue);
        d.insert(Some(layer), usize::MAX, line).unwrap();
        let id = add(&mut d, 1.0);
        let n = &d.node(id).unwrap().children().unwrap()[0];
        let b = n.geometric_bounds().unwrap();
        let r = 1.0 / MM_PER_PT;
        assert!((b.y0 - (95.0 - r)).abs() < 1e-3 && (b.y1 - (105.0 + r)).abs() < 1e-3, "the stroke's width and the bleed: {b:?}");
        assert_eq!(n.appearance.fill().map(|f| f.paint.clone()), Some(Paint::solid(Color::rgb(0.0, 0.0, 1.0))));
    }

    /// Moving the art or changing the distance makes the bleed out of date; making it again
    /// doesn't bleed the old bleed.
    #[test]
    fn a_changed_art_makes_the_bleed_stale() {
        let (mut d, sq) = doc();
        assert!(!is_stale(&d, 2.0), "no bleed, nothing stale");
        let id = add(&mut d, 2.0);
        assert!(!is_stale(&d, 2.0));
        assert!(is_stale(&d, 3.0), "another distance");
        d.node_mut(sq).unwrap().transform(vectorcraft_geom::Affine::translate((10.0, 0.0)), false);
        assert!(is_stale(&d, 2.0));
        d.remove(id).unwrap();
        let (layer, count) = bleed_layer_node(&mut d, 2.0).unwrap();
        assert_eq!(count, 1, "the art only, not the old bleed");
        d.insert(None, 0, layer).unwrap();
        assert!(!is_stale(&d, 2.0));
    }

    #[test]
    fn nothing_and_bad_distances_are_refused() {
        let (mut d, sq) = doc();
        assert!(bleed_layer_node(&mut d, 0.0).is_err());
        assert!(bleed_layer_node(&mut d, f64::NAN).is_err());
        d.node_mut(sq).unwrap().appearance = Appearance::basic(Paint::None, Paint::None, 0.0).into();
        assert!(bleed_layer_node(&mut d, 1.0).unwrap_err().contains("no printed art"));
    }
}
