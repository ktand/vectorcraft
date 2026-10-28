use std::sync::Arc;

use serde_json::json;
use vectorcraft_doc::text::{CharStyle, TextObject};
use vectorcraft_doc::{Appearance, Effect, ImageObject, NodeId, Symbol};
use vectorcraft_geom::{Affine, Point, Rect, Shape, shapes};

use super::*;

fn square(d: &mut Document, x: f64, y: f64, side: f64) -> Node {
    Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, y, x + side, y + side)), Appearance::default())
}

fn item(id: NodeId) -> Item {
    Item { id, op: OpKind::Cut, clips: vec![] }
}

/// `n` added to a fresh 200 × 200 pt document: the document and its id.
fn with(n: impl FnOnce(&mut Document) -> Node) -> (Document, NodeId) {
    let mut d = Document::new(200.0, 200.0);
    let layer = d.layers[0].id;
    let n = n(&mut d);
    let id = d.insert(Some(layer), usize::MAX, n).unwrap();
    (d, id)
}

fn bounds(o: &Outline) -> Rect {
    o.path.bounding_box()
}

fn close(a: Rect, b: Rect) -> bool {
    [(a.x0, b.x0), (a.y0, b.y0), (a.x1, b.x1), (a.y1, b.y1)].iter().all(|(p, q)| (p - q).abs() < 1e-6)
}

#[test]
fn a_path_is_its_subpath() {
    let (d, id) = with(|d| square(d, 10.0, 20.0, 30.0));
    let (out, warnings) = outlines(&d, &[item(id)]);
    assert!(warnings.is_empty());
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].op, out[0].source, out[0].closed), (OpKind::Cut, SourceRef { node: id.0, subpath: 0 }, true));
    assert!(close(bounds(&out[0]), Rect::new(10.0, 20.0, 40.0, 50.0)));
}

/// A stroke is cut on its centre line: the path, not the stroke's outline.
#[test]
fn an_open_stroked_path_is_cut_on_its_line() {
    let (d, id) = with(|d| {
        // However wide its stroke: the appearance isn't read.
        Node::path(d.alloc_id(), shapes::line(Point::new(0.0, 50.0), Point::new(100.0, 50.0)), Appearance::default())
    });
    let (out, _) = outlines(&d, &[item(id)]);
    assert_eq!(out.len(), 1);
    assert!(!out[0].closed);
    assert!(close(bounds(&out[0]), Rect::new(0.0, 50.0, 100.0, 50.0)));
}

#[test]
fn a_compound_path_cuts_each_subpath_once() {
    let (d, id) = with(|d| {
        let (outer, inner, again) = (square(d, 0.0, 0.0, 50.0), square(d, 10.0, 10.0, 10.0), square(d, 10.0, 10.0, 10.0));
        Node::new(d.alloc_id(), NodeKind::Compound { children: vec![Arc::new(outer), Arc::new(inner), Arc::new(again)], rule: Default::default() })
    });
    let (out, _) = outlines(&d, &[item(id)]);
    assert_eq!(out.iter().map(|o| o.source.subpath).collect::<Vec<_>>(), [0, 1], "the inner square drawn twice is cut once");
    assert!(close(bounds(&out[1]), Rect::new(10.0, 10.0, 20.0, 20.0)));
}

/// A geometry effect changes what is cut: Offset Path by 5 pt grows the square by 5 on each side.
#[test]
fn geometry_effects_are_applied() {
    let (d, id) = with(|d| {
        let mut n = square(d, 50.0, 50.0, 40.0);
        n.appearance.effects.push(Effect { id: "path.offsetPath".into(), params: json!({"offset": 5.0}), visible: true });
        n
    });
    let (out, _) = outlines(&d, &[item(id)]);
    assert_eq!(out.len(), 1);
    assert!(close(bounds(&out[0]), Rect::new(45.0, 45.0, 95.0, 95.0)), "{:?}", bounds(&out[0]));
}

#[test]
fn type_is_cut_along_its_glyphs() {
    let (d, id) = with(|d| {
        let t = TextObject::point(Point::new(10.0, 100.0), "OIL", CharStyle { size: 48.0, ..CharStyle::default() });
        Node::new(d.alloc_id(), NodeKind::Text(Box::new(t)))
    });
    let (out, warnings) = outlines(&d, &[item(id)]);
    assert!(warnings.is_empty(), "{warnings:?}");
    // O has a counter, I and L none: four closed outlines, all within the type's line.
    assert_eq!(out.len(), 4, "{out:?}");
    assert!(out.iter().all(|o| o.closed && o.source.node == id.0));
    let all = out.iter().map(bounds).reduce(|a, b| a.union(b)).unwrap();
    assert!(all.x0 >= 10.0 && all.y1 <= 101.0 && all.y0 > 50.0, "{all:?}");
}

#[test]
fn a_symbol_instance_is_cut_along_its_placed_art() {
    let mut d = Document::new(200.0, 200.0);
    let art = square(&mut d, 0.0, 0.0, 10.0);
    d.symbols.push(Symbol { name: "dot".into(), art: Arc::new(art) });
    let layer = d.layers[0].id;
    let inst = Node::new(d.alloc_id(), NodeKind::SymbolInstance { symbol: "dot".into(), xf: Affine::translate((100.0, 50.0)) });
    let id = d.insert(Some(layer), usize::MAX, inst).unwrap();
    let (out, _) = outlines(&d, &[item(id)]);
    assert_eq!(out.len(), 1);
    assert!(close(bounds(&out[0]), Rect::new(100.0, 50.0, 110.0, 60.0)));
}

#[test]
fn images_are_left_out_with_a_warning() {
    let (d, id) = with(|d| {
        let im =
            ImageObject { key: "k".into(), width: 10, height: 10, xf: Affine::IDENTITY, link: None, placement: Default::default(), pixels: None };
        Node::new(d.alloc_id(), NodeKind::Image(Box::new(im)))
    });
    let (out, warnings) = outlines(&d, &[item(id)]);
    assert!(out.is_empty());
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].source.map(|s| s.node), Some(id.0));
    assert!(warnings[0].message.contains("image"));
}

#[test]
fn hidden_members_of_a_symbols_art_arent_cut() {
    let mut d = Document::new(200.0, 200.0);
    let shown = square(&mut d, 0.0, 0.0, 10.0);
    let mut hidden = square(&mut d, 20.0, 0.0, 10.0);
    hidden.visible = false;
    let g = Node::group(d.alloc_id(), vec![Arc::new(shown), Arc::new(hidden)]);
    d.symbols.push(Symbol { name: "pair".into(), art: Arc::new(g) });
    let layer = d.layers[0].id;
    let inst = Node::new(d.alloc_id(), NodeKind::SymbolInstance { symbol: "pair".into(), xf: Affine::IDENTITY });
    let id = d.insert(Some(layer), usize::MAX, inst).unwrap();
    assert_eq!(outlines(&d, &[item(id)]).0.len(), 1);
}

#[test]
fn subpaths_split_at_moves_and_after_closes() {
    let mut p = BezPath::new();
    p.move_to((0.0, 0.0));
    p.line_to((10.0, 0.0));
    p.line_to((10.0, 10.0));
    p.close_path();
    // No move: starts again from (0, 0).
    p.line_to((0.0, 20.0));
    p.move_to((50.0, 50.0));
    // A move alone draws nothing.
    p.move_to((60.0, 60.0));
    p.line_to((70.0, 60.0));
    let s = subpaths(&p);
    assert_eq!(s.iter().map(|(_, c)| *c).collect::<Vec<_>>(), [true, false, false]);
    assert_eq!(s[1].0.elements()[0], PathEl::MoveTo(Point::new(0.0, 0.0)));
    assert_eq!(s[2].0.elements()[0], PathEl::MoveTo(Point::new(60.0, 60.0)));
}
