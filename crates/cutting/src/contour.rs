//! Fork: a contour cut (C11.1): a line round art, a set distance out, to cut it free (a sticker
//! printed with Print & Cut, a shape cut from card). The art is taken as drawn: each object's
//! outlines from the cutting geometry ([`crate::geometry`], whatever its cut operation), its
//! stroke's outer half, an image's frame. Each is offset by the distance with round corners, all
//! are united, and the holes left inside are dropped unless asked for (a sticker is cut round its
//! outside only). The result is in document points; the engine puts it on a layer of its own.

use std::collections::BTreeMap;

use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_doc::{Appearance, AppearanceItem, Document, ForeignAttribute, LayerColor, Node, NodeId, NodeKind};
use vectorcraft_geom::{BezPath, FillRule, PathData, PathEl, Point, Shape, SubPath};
use vectorcraft_pathops::{Join, offset_path, try_unite_all};

use crate::ops::{NS, PREFIX, set_own_op};
use crate::placement::MM_PER_PT;

/// The largest distance a contour runs out from the art, mm.
pub const MAX_OFFSET_MM: f64 = 50.0;
/// The distance a contour runs out by default, mm.
pub const DEFAULT_OFFSET_MM: f64 = 2.0;
/// Most pieces of art one contour goes round.
const MAX_PIECES: usize = 20_000;

/// How the contour is made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// How far out from the art, mm (0–[`MAX_OFFSET_MM`]).
    pub offset_mm: f64,
    /// Keep the holes the art leaves inside it (a ring cut as a ring), else only the outside.
    pub holes: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { offset_mm: DEFAULT_OFFSET_MM, holes: false }
    }
}

/// The outer half of `n`'s widest visible stroke, points (0 without one).
fn stroke_reach(d: &Document, id: NodeId) -> f64 {
    let Some(n) = d.node(id) else { return 0.0 };
    n.appearance
        .items
        .iter()
        .filter_map(|i| match i {
            AppearanceItem::Stroke(s) if i.visible() && !s.paint.is_none() && s.width.is_finite() => Some(s.width.max(0.0) / 2.0),
            _ => None,
        })
        .fold(0.0, f64::max)
}

/// One outline as a closed or open path.
fn as_path(path: &BezPath, closed: bool) -> PathData {
    let mut bp = path.clone();
    if closed && !matches!(bp.elements().last(), Some(PathEl::ClosePath)) {
        bp.push(PathEl::ClosePath);
    }
    PathData::from_bezpath(&bp)
}

/// The contour round the art of `ids` (and what is inside them), in document points: one or more
/// closed subpaths, or why there is none.
pub fn contour(d: &Document, ids: &[NodeId], opt: Options) -> Result<PathData, String> {
    if !(opt.offset_mm.is_finite() && (0.0..=MAX_OFFSET_MM).contains(&opt.offset_mm)) {
        return Err(format!("the distance is 0 to {MAX_OFFSET_MM} mm"));
    }
    let items = crate::select::art_under(d, ids);
    let (outlines, _) = crate::geometry::outlines(d, &items);
    // Each object's outlines together, so it is offset as the shape it draws.
    let mut by_node: BTreeMap<NodeId, Vec<SubPath>> = BTreeMap::new();
    for o in &outlines {
        let p = as_path(&o.path, o.closed);
        by_node.entry(NodeId(o.source.node)).or_default().extend(p.subpaths);
    }
    // Images have no outline to cut: their frame.
    for item in &items {
        let Some(n) = d.node(item.id) else { continue };
        if matches!(n.kind, NodeKind::Image(_))
            && let Some(b) = n.geometric_bounds().filter(|b| b.width() > 0.0 && b.height() > 0.0)
        {
            let corners = [Point::new(b.x0, b.y0), Point::new(b.x1, b.y0), Point::new(b.x1, b.y1), Point::new(b.x0, b.y1)];
            by_node.entry(n.id).or_default().push(SubPath::polyline(&corners, true));
        }
    }
    if by_node.len() > MAX_PIECES {
        return Err(format!("at most {MAX_PIECES} objects can be gone round at once"));
    }
    let distance = opt.offset_mm / MM_PER_PT;
    let pieces: Vec<PathData> = by_node
        .into_iter()
        .map(|(id, subpaths)| offset_path(&PathData::new(subpaths), distance + stroke_reach(d, id), Join::Round, 4.0))
        .filter(|p| !p.is_empty())
        .collect();
    if pieces.is_empty() {
        return Err("the selection has no art to go round".into());
    }
    let inputs: Vec<(&PathData, FillRule)> = pieces.iter().map(|p| (p, FillRule::NonZero)).collect();
    let united = try_unite_all(&inputs).map_err(|e| format!("the outlines couldn't be united: {e}"))?;
    Ok(if opt.holes { united } else { outside(&united) })
}

/// The contour layer's name, and the attribute that marks it as the contour layer.
pub const LAYER_NAME: &str = "Cut Contour";
const NAME: &str = "contour";

/// The document's contour layer, if it has one (a top-level layer).
pub fn contour_layer(d: &Document) -> Option<NodeId> {
    d.layers.iter().find(|l| l.extras().attrs.as_deref().is_some_and(|a| a.foreign.iter().any(|f| f.ns == NS && f.name == NAME))).map(|l| l.id)
}

/// A new contour layer (ids from `d`): set to Cut, so its contours are cut whatever the default.
pub fn contour_layer_node(d: &mut Document) -> Node {
    let mut layer = Node::layer(d.alloc_id(), LAYER_NAME, LayerColor::Preset(2));
    set_own_op(&mut layer, Some(Op::Cut));
    layer.edit_attrs(|at| {
        at.foreign.push(ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: NAME.to_string(), value: "1".to_string() });
    });
    layer
}

/// The contour as a path: a thin magenta line, no fill, so it shows over the art and isn't
/// printed by mistake for art.
pub fn contour_node(d: &mut Document, path: PathData) -> Node {
    let magenta = Paint::solid(Color::Rgb { r: 1.0, g: 0.0, b: 1.0 });
    let mut n = Node::path(d.alloc_id(), path, Appearance::basic(Paint::None, magenta, 0.5));
    n.name = Some(LAYER_NAME.to_string());
    n
}

/// `p` without the holes: the subpaths inside an odd number of the others are dropped (an island
/// inside a hole is kept, as the outside of its own piece).
fn outside(p: &PathData) -> PathData {
    let rings: Vec<BezPath> = p.subpaths.iter().map(|s| PathData::single(s.clone()).to_bezpath()).collect();
    let kept = p
        .subpaths
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            let Some(q) = s.anchors.first().map(|a| a.p) else { return false };
            let around = rings.iter().enumerate().filter(|(j, r)| j != i && r.winding(q) != 0).count();
            around % 2 == 0
        })
        .map(|(_, s)| s.clone())
        .collect();
    PathData::new(kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::Rect;

    fn doc_with(nodes: Vec<Node>) -> (Document, Vec<NodeId>) {
        let mut d = Document::new(500.0, 500.0);
        let layer = d.layers[0].id;
        let ids = nodes.into_iter().map(|n| d.insert(Some(layer), usize::MAX, n).unwrap()).collect();
        (d, ids)
    }

    fn square(d: &mut Document, x: f64, y: f64, w: f64) -> Node {
        let corners = [Point::new(x, y), Point::new(x + w, y), Point::new(x + w, y + w), Point::new(x, y + w)];
        Node::path(d.alloc_id(), PathData::single(SubPath::polyline(&corners, true)), Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 0.0))
    }

    fn bounds(p: &PathData) -> Rect {
        p.to_bezpath().bounding_box()
    }

    #[test]
    fn a_square_gets_a_contour_the_distance_out() {
        let mut d = Document::new(500.0, 500.0);
        let sq = square(&mut d, 100.0, 100.0, 100.0);
        let layer = d.layers[0].id;
        let id = d.insert(Some(layer), usize::MAX, sq).unwrap();
        let c = contour(&d, &[id], Options { offset_mm: 5.0, holes: false }).unwrap();
        let out = 5.0 / MM_PER_PT;
        let b = bounds(&c);
        assert!((b.x0 - (100.0 - out)).abs() < 0.05 && (b.x1 - (200.0 + out)).abs() < 0.05, "{b:?}");
        assert_eq!(c.subpaths.len(), 1);
        assert!(c.subpaths.iter().all(|s| s.closed));
        // Round corners: the corner itself is cut off.
        let bp = c.to_bezpath();
        assert!(bp.winding(Point::new(100.0 - out + 0.1, 100.0 - out + 0.1)) == 0, "rounded");
    }

    #[test]
    fn close_pieces_join_and_far_ones_stay_apart() {
        let mut d = Document::new(500.0, 500.0);
        let a = square(&mut d, 0.0, 0.0, 50.0);
        let b = square(&mut d, 55.0, 0.0, 50.0);
        let far = square(&mut d, 300.0, 300.0, 50.0);
        let layer = d.layers[0].id;
        let ids: Vec<NodeId> = [a, b, far].into_iter().map(|n| d.insert(Some(layer), usize::MAX, n).unwrap()).collect();
        // 5 pt apart, 2 mm (5.7 pt) out each: the first two touch.
        let c = contour(&d, &ids, Options::default()).unwrap();
        assert_eq!(c.subpaths.len(), 2, "{:?}", c.subpaths.len());
    }

    #[test]
    fn a_ring_loses_its_hole_unless_asked() {
        let mut d = Document::new(500.0, 500.0);
        let outer = [Point::new(0.0, 0.0), Point::new(200.0, 0.0), Point::new(200.0, 200.0), Point::new(0.0, 200.0)];
        let inner = [Point::new(50.0, 50.0), Point::new(50.0, 150.0), Point::new(150.0, 150.0), Point::new(150.0, 50.0)];
        let ring = PathData::new(vec![SubPath::polyline(&outer, true), SubPath::polyline(&inner, true)]);
        let n = Node::path(d.alloc_id(), ring, Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 0.0));
        let (d, ids) = {
            let layer = d.layers[0].id;
            let id = d.insert(Some(layer), usize::MAX, n).unwrap();
            (d, vec![id])
        };
        assert_eq!(contour(&d, &ids, Options::default()).unwrap().subpaths.len(), 1, "the outside only");
        assert_eq!(contour(&d, &ids, Options { holes: true, ..Options::default() }).unwrap().subpaths.len(), 2, "and the hole");
    }

    #[test]
    fn a_stroke_counts_and_nothing_is_refused() {
        let mut d = Document::new(500.0, 500.0);
        let corners = [Point::new(100.0, 100.0), Point::new(200.0, 100.0), Point::new(200.0, 200.0), Point::new(100.0, 200.0)];
        let stroke = Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 20.0);
        let sq = Node::path(d.alloc_id(), PathData::single(SubPath::polyline(&corners, true)), stroke);
        let layer = d.layers[0].id;
        let id = d.insert(Some(layer), usize::MAX, sq).unwrap();
        let c = contour(&d, &[id], Options { offset_mm: 0.0, holes: false }).unwrap();
        assert!((bounds(&c).x0 - 90.0).abs() < 0.05, "half the 20 pt stroke out: {:?}", bounds(&c));
        let (empty, _) = doc_with(vec![]);
        assert!(contour(&empty, &[], Options::default()).is_err());
        assert!(contour(&d, &[id], Options { offset_mm: f64::NAN, holes: false }).is_err());
        assert!(contour(&d, &[id], Options { offset_mm: 80.0, holes: false }).is_err());
    }
}
