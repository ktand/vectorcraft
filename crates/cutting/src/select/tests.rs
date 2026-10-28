use std::sync::Arc;

use vectorcraft_doc::{Appearance, LayerColor};
use vectorcraft_geom::{Point, shapes};

use super::*;
use crate::ops::{resolve, set_own_op};

fn square(d: &mut Document, x: f64, y: f64) -> Node {
    Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, y, x + 10.0, y + 10.0)), Appearance::default())
}

/// A 200 × 200 pt document; its one layer's id.
fn doc() -> (Document, NodeId) {
    let d = Document::new(200.0, 200.0);
    let layer = d.layers[0].id;
    (d, layer)
}

fn add(d: &mut Document, parent: NodeId, n: Node) -> NodeId {
    d.insert(Some(parent), usize::MAX, n).unwrap()
}

fn ids(items: &[Item]) -> Vec<NodeId> {
    items.iter().map(|i| i.id).collect()
}

fn cut(d: &Document) -> Vec<Item> {
    select(d, &CuttingSettings::default())
}

#[test]
fn shown_objects_are_cut_in_paint_order() {
    let (mut d, layer) = doc();
    let (a, b) = (square(&mut d, 10.0, 10.0), square(&mut d, 50.0, 10.0));
    let (a, b) = (add(&mut d, layer, a), add(&mut d, layer, b));
    assert_eq!(
        cut(&d),
        [Item { id: a, op: OpKind::Cut, outline: false, clips: vec![] }, Item { id: b, op: OpKind::Cut, outline: false, clips: vec![] }]
    );
}

#[test]
fn hidden_template_guide_and_clipping_paths_are_left_out() {
    let (mut d, layer) = doc();
    let mut hidden = square(&mut d, 10.0, 10.0);
    hidden.visible = false;
    add(&mut d, layer, hidden);
    let mut guide = square(&mut d, 10.0, 10.0);
    if let NodeKind::Path { guide: g, .. } = &mut guide.kind {
        *g = true;
    }
    add(&mut d, layer, guide);
    let mut locked = square(&mut d, 10.0, 10.0);
    locked.locked = true;
    let locked = add(&mut d, layer, locked);

    let mut hidden_layer = Node::layer(d.alloc_id(), "Hidden", LayerColor::Preset(1));
    hidden_layer.visible = false;
    let hidden_layer = d.insert(None, usize::MAX, hidden_layer).unwrap();
    let s = square(&mut d, 10.0, 10.0);
    add(&mut d, hidden_layer, s);

    let mut template = Node::layer(d.alloc_id(), "Template", LayerColor::Preset(2));
    if let NodeKind::Layer { template: t, .. } = &mut template.kind {
        *t = true;
    }
    let template = d.insert(None, usize::MAX, template).unwrap();
    let s = square(&mut d, 10.0, 10.0);
    add(&mut d, template, s);

    // A layer that doesn't print is still cut (print-and-cut keeps cut lines on one).
    let mut no_print = Node::layer(d.alloc_id(), "Cut lines", LayerColor::Preset(3));
    if let NodeKind::Layer { printable, .. } = &mut no_print.kind {
        *printable = false;
    }
    let no_print = d.insert(None, usize::MAX, no_print).unwrap();
    let s = square(&mut d, 10.0, 10.0);
    let on_no_print = add(&mut d, no_print, s);

    assert_eq!(ids(&cut(&d)), [locked, on_no_print]);
}

#[test]
fn objects_off_the_artboard_are_left_out() {
    let (mut d, layer) = doc();
    let on = square(&mut d, 10.0, 10.0);
    let on = add(&mut d, layer, on);
    let off = square(&mut d, 300.0, 10.0);
    let off = add(&mut d, layer, off);
    let edge = Node::path(d.alloc_id(), shapes::line(Point::new(0.0, 200.0), Point::new(50.0, 200.0)), Appearance::default());
    let edge = add(&mut d, layer, edge);
    let partly = square(&mut d, 195.0, 10.0);
    let partly = add(&mut d, layer, partly);
    assert_eq!(ids(&cut(&d)), [on, edge, partly], "a line along the edge and a square partly on it count");
    // The second artboard cuts what is on it.
    let mut second = d.artboards[0].clone();
    second.rect = Rect::new(250.0, 0.0, 450.0, 200.0);
    d.artboards.push(second);
    let s = CuttingSettings { artboard: 1, ..CuttingSettings::default() };
    assert_eq!(ids(&select(&d, &s)), [off]);
    let s = CuttingSettings { artboard: 9, ..CuttingSettings::default() };
    assert!(select(&d, &s).is_empty());
}

/// Each object's operation is the one `cutting.op.get` reports ([`resolve`]); No Cut leaves it out.
#[test]
fn ops_resolve_as_the_commands_report_them() {
    let (mut d, layer) = doc();
    let (a, b, c) = (square(&mut d, 10.0, 10.0), square(&mut d, 30.0, 10.0), square(&mut d, 50.0, 10.0));
    let (a_id, b_id) = (a.id, b.id);
    let g = Node::group(d.alloc_id(), vec![Arc::new(a), Arc::new(b)]);
    let g = add(&mut d, layer, g);
    let c = add(&mut d, layer, c);
    set_own_op(d.node_mut(g).unwrap(), Some(Op::Score));
    set_own_op(d.node_mut(b_id).unwrap(), Some(Op::NoCut));
    let items = cut(&d);
    assert_eq!(ids(&items), [a_id, c]);
    for item in &items {
        let r = resolve(&d, item.id, Op::Cut).unwrap();
        assert_eq!(
            Some(item.op),
            match r.op {
                Op::Cut | Op::Outline => Some(OpKind::Cut),
                Op::Score => Some(OpKind::Score),
                Op::NoCut => None,
            }
        );
    }
    assert_eq!(items[0].op, OpKind::Score);
    // The document's default reaches what sets nothing.
    let s = CuttingSettings { default_op: Op::NoCut, ..CuttingSettings::default() };
    assert_eq!(ids(&select(&d, &s)), [a_id]);
    set_own_op(d.node_mut(layer).unwrap(), Some(Op::NoCut));
    assert_eq!(ids(&cut(&d)), [a_id], "the group's own Score wins over its layer's No Cut");
}

#[test]
fn clip_groups_are_noted_and_their_clipping_path_isnt_cut() {
    let (mut d, layer) = doc();
    let mut mask = square(&mut d, 0.0, 0.0);
    if let NodeKind::Path { clipping, .. } = &mut mask.kind {
        *clipping = true;
    }
    let art = square(&mut d, 5.0, 5.0);
    let art_id = art.id;
    let mut g = Node::group(d.alloc_id(), vec![Arc::new(mask), Arc::new(art)]);
    if let NodeKind::Group { clip, .. } = &mut g.kind {
        *clip = true;
    }
    let g = add(&mut d, layer, g);
    assert_eq!(cut(&d), [Item { id: art_id, op: OpKind::Cut, outline: false, clips: vec![g] }]);
}

#[test]
fn a_compound_path_is_one_item() {
    let (mut d, layer) = doc();
    let (outer, inner) = (square(&mut d, 10.0, 10.0), square(&mut d, 12.0, 12.0));
    let compound = Node::new(d.alloc_id(), NodeKind::Compound { children: vec![Arc::new(outer), Arc::new(inner)], rule: Default::default() });
    let compound = add(&mut d, layer, compound);
    assert_eq!(ids(&cut(&d)), [compound]);
}
