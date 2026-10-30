//! Fork: stencil bridges (C11.5). Cut from a sheet, a letter's counter (the middle of an O, an A,
//! a B) falls out and is lost: a stencil keeps it joined to the sheet by narrow bridges. Each hole
//! in the art gets a bar through its middle, vertical or horizontal, and the pieces of that bar
//! that cross the art next to the hole are taken out of the art. Cutting what is left cuts round
//! each letter in parts, and the counter stays on the sheet, held by the bridges.
//!
//! The art is taken as filled, as it is cut: each object's closed outlines from the cutting
//! geometry (round its stroke when it is set to its stroke's edge, [`crate::edge`]), even-odd
//! within the object, all objects united. Open lines have no area and add nothing. The result is in document points; the engine adds it as a path.

use std::collections::BTreeMap;

use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{BezPath, FillRule, PathData, PathEl, Point, Rect, Shape, SubPath, shapes};
use vectorcraft_pathops::{BoolOp, DEFAULT_PRECISION, try_boolean, try_unite_all};

use crate::placement::MM_PER_PT;

/// The narrowest and widest bridge, mm.
pub const WIDTH_MM: std::ops::RangeInclusive<f64> = 0.2..=20.0;
/// A bridge's width by default, mm.
pub const DEFAULT_WIDTH_MM: f64 = 1.5;
/// Most holes bridged at once (each is a boolean operation).
const MAX_HOLES: usize = 2_000;
/// Most closed outlines taken in.
const MAX_OUTLINES: usize = 50_000;
/// How far past the art a bridge's cut runs, points: so it leaves no sliver of the edge.
const PAST: f64 = 0.05;
/// How close to a hole a bar's piece must come to be a bridge for it, points.
const NEAR: f64 = 0.01;

/// How the bridges run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    /// Up and down through each hole: above and below it.
    #[default]
    Vertical,
    /// Across each hole: to its left and right.
    Horizontal,
}

impl Direction {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "vertical" => Some(Direction::Vertical),
            "horizontal" => Some(Direction::Horizontal),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// A bridge's width, mm ([`WIDTH_MM`]).
    pub width_mm: f64,
    pub direction: Direction,
}

impl Default for Options {
    fn default() -> Self {
        Options { width_mm: DEFAULT_WIDTH_MM, direction: Direction::Vertical }
    }
}

/// The art with its bridges taken out.
#[derive(Clone, Debug, PartialEq)]
pub struct Bridged {
    /// In document points: the art's area less the bridges.
    pub path: PathData,
    /// How many bridges were made.
    pub bridges: usize,
    /// Holes no bridge could be made for (a bar through the middle misses the art beside them).
    pub unbridged: usize,
}

/// The art of `ids` (and what is inside them) as a stencil: its area with bridges across the art
/// beside each hole, or why there is none.
pub fn bridged(d: &Document, ids: &[NodeId], opt: Options) -> Result<Bridged, String> {
    if !(opt.width_mm.is_finite() && WIDTH_MM.contains(&opt.width_mm)) {
        return Err(format!("the width is {} to {} mm", WIDTH_MM.start(), WIDTH_MM.end()));
    }
    let area = area(d, ids)?;
    let rings: Vec<BezPath> = area.subpaths.iter().map(|s| PathData::single(s.clone()).to_bezpath()).collect();
    let holes: Vec<&BezPath> = rings.iter().enumerate().filter(|(i, _)| depth(&rings, *i) % 2 == 1).map(|(_, r)| r).collect();
    if holes.len() > MAX_HOLES {
        return Err(format!("at most {MAX_HOLES} holes can be bridged at once"));
    }
    let half = opt.width_mm / MM_PER_PT / 2.0;
    let (Some(all), mut cuts, mut unbridged) = (area.bounds(), vec![], 0) else {
        return Err("the selection has no filled art to bridge".into());
    };
    for hole in holes {
        let found = bridges_for(&area, hole, all, half, opt.direction)?;
        if found.is_empty() {
            unbridged += 1;
        }
        cuts.extend(found);
    }
    let bridges = cuts.len();
    let path = if cuts.is_empty() {
        area
    } else {
        let bars = PathData::new(cuts.iter().flat_map(|r| shapes::rectangle(*r).subpaths).collect());
        try_boolean(&area, FillRule::NonZero, &bars, FillRule::NonZero, BoolOp::Difference, DEFAULT_PRECISION)
            .map_err(|e| format!("the bridges couldn't be cut: {e}"))?
    };
    Ok(Bridged { path, bridges, unbridged })
}

/// The filled area of the art under `ids`: each object's closed outlines even-odd, all united.
fn area(d: &Document, ids: &[NodeId]) -> Result<PathData, String> {
    let mut items = crate::select::art_under(d, ids);
    for item in &mut items {
        item.stroke_edge = crate::edge::resolve(d, item.id).is_some_and(|(e, _)| e == crate::edge::Edge::Stroke);
    }
    let (outlines, _) = crate::geometry::outlines(d, &items);
    if outlines.len() > MAX_OUTLINES {
        return Err(format!("at most {MAX_OUTLINES} outlines can be bridged at once"));
    }
    let mut by_node: BTreeMap<u64, Vec<SubPath>> = BTreeMap::new();
    for o in outlines.iter().filter(|o| o.closed) {
        let mut bp = o.path.clone();
        if !matches!(bp.elements().last(), Some(PathEl::ClosePath)) {
            bp.push(PathEl::ClosePath);
        }
        by_node.entry(o.source.node).or_default().extend(PathData::from_bezpath(&bp).subpaths);
    }
    let pieces: Vec<PathData> = by_node.into_values().map(PathData::new).collect();
    let inputs: Vec<(&PathData, FillRule)> = pieces.iter().map(|p| (p, FillRule::EvenOdd)).collect();
    let area = try_unite_all(&inputs).map_err(|e| format!("the art couldn't be united: {e}"))?;
    if area.is_empty() {
        return Err("the selection has no filled art to bridge".into());
    }
    Ok(area)
}

/// How many of the other rings ring `i` lies inside: odd for a hole.
fn depth(rings: &[BezPath], i: usize) -> usize {
    let Some(q) = rings.get(i).and_then(first_point) else { return 0 };
    rings.iter().enumerate().filter(|(j, r)| *j != i && r.winding(q) != 0).count()
}

fn first_point(r: &BezPath) -> Option<Point> {
    r.elements().iter().find_map(|el| match el {
        PathEl::MoveTo(p) => Some(*p),
        _ => None,
    })
}

/// The bridges for `hole`: the pieces of a bar through its middle (across all of the art's bounds
/// `all`, `half` either side) that lie in the art and touch the hole, each as the rectangle to cut,
/// run a little past the art.
fn bridges_for(area: &PathData, hole: &BezPath, all: Rect, half: f64, dir: Direction) -> Result<Vec<Rect>, String> {
    let h = hole.bounding_box();
    let c = h.center();
    let bar = match dir {
        Direction::Vertical => Rect::new(c.x - half, all.y0 - 1.0, c.x + half, all.y1 + 1.0),
        Direction::Horizontal => Rect::new(all.x0 - 1.0, c.y - half, all.x1 + 1.0, c.y + half),
    };
    let crossing = try_boolean(area, FillRule::NonZero, &shapes::rectangle(bar), FillRule::NonZero, BoolOp::Intersect, DEFAULT_PRECISION)
        .map_err(|e| format!("a bridge couldn't be placed: {e}"))?;
    let mut out = vec![];
    for piece in &crossing.subpaths {
        let bp = PathData::single(piece.clone()).to_bezpath();
        let b = bp.bounding_box();
        if !touches(&bp, hole) {
            continue;
        }
        out.push(match dir {
            Direction::Vertical => Rect::new(bar.x0, b.y0 - PAST, bar.x1, b.y1 + PAST),
            Direction::Horizontal => Rect::new(b.x0 - PAST, bar.y0, b.x1 + PAST, bar.y1),
        });
    }
    Ok(out)
}

/// Whether `piece` comes up to `hole`: one of its corners is on the hole's edge, the hole on one
/// side of it and not on the other (an island inside the hole has the hole all round it).
fn touches(piece: &BezPath, hole: &BezPath) -> bool {
    let inside = |x: f64, y: f64| hole.winding(Point::new(x, y)) != 0;
    piece
        .elements()
        .iter()
        .filter_map(PathEl::end_point)
        .any(|p| inside(p.x + NEAR, p.y) != inside(p.x - NEAR, p.y) || inside(p.x, p.y + NEAR) != inside(p.x, p.y - NEAR))
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::{Color, Paint};
    use vectorcraft_doc::{Appearance, Node};

    use super::*;

    /// A square ring: 100 pt across, its hole 60 pt across in the middle.
    fn ring(d: &mut Document, x: f64) -> Node {
        let mut p = shapes::rectangle(Rect::new(x, 0.0, x + 100.0, 100.0));
        p.subpaths.extend(shapes::rectangle(Rect::new(x + 20.0, 20.0, x + 80.0, 80.0)).subpaths);
        Node::path(d.alloc_id(), p, Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 0.0))
    }

    fn doc_with(nodes: impl FnOnce(&mut Document) -> Vec<Node>) -> (Document, Vec<NodeId>) {
        let mut d = Document::new(500.0, 500.0);
        let layer = d.layers[0].id;
        let nodes = nodes(&mut d);
        let ids = nodes.into_iter().map(|n| d.insert(Some(layer), usize::MAX, n).unwrap()).collect();
        (d, ids)
    }

    fn mm(pt: f64) -> f64 {
        pt * MM_PER_PT
    }

    /// A ring gets a bridge above its hole and one below: it is cut in two halves, and the area
    /// lost is the two bridges.
    #[test]
    fn a_ring_is_bridged_above_and_below() {
        let (d, ids) = doc_with(|d| vec![ring(d, 0.0)]);
        let b = bridged(&d, &ids, Options { width_mm: mm(10.0), direction: Direction::Vertical }).unwrap();
        assert_eq!((b.bridges, b.unbridged), (2, 0), "{b:?}");
        assert_eq!(b.path.subpaths.len(), 2, "two halves, no hole left");
        let area = vectorcraft_pathops::area(&b.path, FillRule::NonZero).abs();
        assert!((area - (100.0 * 100.0 - 60.0 * 60.0 - 2.0 * 10.0 * 20.0)).abs() < 1.0, "{area}");
        let h = bridged(&d, &ids, Options { width_mm: mm(10.0), direction: Direction::Horizontal }).unwrap();
        assert_eq!((h.bridges, h.path.subpaths.len()), (2, 2));
    }

    /// Art with no hole is left as it is; two rings get their own bridges only.
    #[test]
    fn only_holes_are_bridged() {
        let (d, ids) = doc_with(|d| vec![Node::path(d.alloc_id(), shapes::rectangle(Rect::new(0.0, 0.0, 50.0, 50.0)), Appearance::default())]);
        let b = bridged(&d, &ids, Options::default()).unwrap();
        assert_eq!((b.bridges, b.path.subpaths.len()), (0, 1));
        let (d, ids) = doc_with(|d| vec![ring(d, 0.0), ring(d, 200.0)]);
        let b = bridged(&d, &ids, Options::default()).unwrap();
        assert_eq!((b.bridges, b.unbridged, b.path.subpaths.len()), (4, 0, 4));
    }

    /// An island inside a hole is its own piece: bridged only if it has a hole of its own.
    #[test]
    fn an_island_in_a_hole_is_left_whole() {
        let (d, ids) =
            doc_with(|d| vec![ring(d, 0.0), Node::path(d.alloc_id(), shapes::rectangle(Rect::new(40.0, 40.0, 60.0, 60.0)), Appearance::default())]);
        let b = bridged(&d, &ids, Options { width_mm: mm(4.0), direction: Direction::Vertical }).unwrap();
        assert_eq!(b.bridges, 2, "the ring's only: {b:?}");
        assert_eq!(b.path.subpaths.len(), 3, "two halves and the island");
    }

    /// A stroked circle set to its stroke's edge is a ring: bridged; along its path it is a disc.
    #[test]
    fn a_stroke_edge_is_bridged_as_cut() {
        let (mut d, ids) = doc_with(|d| {
            let a = Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 20.0);
            vec![Node::path(d.alloc_id(), shapes::ellipse(Rect::new(0.0, 0.0, 100.0, 100.0)), a)]
        });
        let b = bridged(&d, &ids, Options::default()).unwrap();
        assert_eq!((b.bridges, b.path.subpaths.len()), (0, 1), "a disc");
        crate::edge::set_own_edge(d.node_mut(ids[0]).unwrap(), Some(crate::edge::Edge::Stroke));
        let b = bridged(&d, &ids, Options::default()).unwrap();
        assert_eq!((b.bridges, b.path.subpaths.len()), (2, 2), "a ring in two halves");
    }

    #[test]
    fn nothing_and_bad_widths_are_refused() {
        let (d, ids) = doc_with(|d| vec![ring(d, 0.0)]);
        assert!(bridged(&d, &ids, Options { width_mm: 0.0, ..Options::default() }).is_err());
        assert!(bridged(&d, &ids, Options { width_mm: f64::NAN, ..Options::default() }).is_err());
        let (d, ids) = doc_with(|d| vec![Node::path(d.alloc_id(), shapes::line(Point::new(0.0, 0.0), Point::new(9.0, 9.0)), Appearance::default())]);
        assert!(bridged(&d, &ids, Options::default()).unwrap_err().contains("no filled art"));
    }
}
