use std::sync::Arc;

use vectorcraft_cutjob::{CuttingSettings, Op};
use vectorcraft_doc::{Appearance, Document, Node, NodeId, NodeKind};
use vectorcraft_geom::{Point, shapes};

use super::*;
use crate::geometry::outlines;
use crate::ops::set_own_op;
use crate::select::select;

fn square(d: &mut Document, x: f64, y: f64, side: f64) -> Node {
    Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, y, x + side, y + side)), Appearance::default())
}

/// `make`'s objects on a 400 × 400 pt page, each set to `op`, welded.
fn welded(op: Op, make: impl FnOnce(&mut Document) -> Vec<Node>) -> (Vec<Outline>, Vec<JobWarning>) {
    let mut d = Document::new(400.0, 400.0);
    let layer = d.layers[0].id;
    let ids: Vec<NodeId> = make(&mut d).into_iter().map(|n| d.insert(Some(layer), usize::MAX, n).unwrap()).collect();
    for id in ids {
        set_own_op(d.node_mut(id).unwrap(), Some(op));
    }
    let (o, _) = outlines(&d, &select(&d, &CuttingSettings::default()));
    weld(o)
}

fn close(a: Rect, b: Rect) -> bool {
    [(a.x0, b.x0), (a.y0, b.y0), (a.x1, b.x1), (a.y1, b.y1)].iter().all(|(p, q)| (p - q).abs() < 0.05)
}

/// Two squares overlapping at a corner: one closed outline round both, the lines inside left.
#[test]
fn overlapping_shapes_are_cut_as_one() {
    let (out, warnings) = welded(Op::Outline, |d| vec![square(d, 10.0, 10.0, 100.0), square(d, 60.0, 60.0, 100.0)]);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(out[0].closed && out[0].outline);
    assert!(close(out[0].path.bounding_box(), Rect::new(10.0, 10.0, 160.0, 160.0)));
    // The same squares, plain Cut: each cut along all its lines.
    let (plain, _) = welded(Op::Cut, |d| vec![square(d, 10.0, 10.0, 100.0), square(d, 60.0, 60.0, 100.0)]);
    assert_eq!(plain.len(), 2);
}

/// An O (a compound path with a counter) keeps its counter.
#[test]
fn holes_stay_holes() {
    let (out, _) = welded(Op::Outline, |d| {
        let (outer, counter) = (square(d, 10.0, 10.0, 100.0), square(d, 40.0, 40.0, 40.0));
        vec![Node::new(d.alloc_id(), NodeKind::Compound { children: vec![Arc::new(outer), Arc::new(counter)], rule: Default::default() })]
    });
    let mut boxes: Vec<Rect> = out.iter().map(|o| o.path.bounding_box()).collect();
    boxes.sort_by(|a, b| a.width().total_cmp(&b.width()));
    assert_eq!(boxes.len(), 2, "{boxes:?}");
    assert!(close(boxes[0], Rect::new(40.0, 40.0, 80.0, 80.0)) && close(boxes[1], Rect::new(10.0, 10.0, 110.0, 110.0)));
}

#[test]
fn shapes_apart_stay_apart() {
    let (out, _) = welded(Op::Outline, |d| vec![square(d, 10.0, 10.0, 50.0), square(d, 200.0, 200.0, 50.0)]);
    assert_eq!(out.len(), 2);
    let nodes: Vec<u64> = out.iter().map(|o| o.source.node).collect();
    assert_ne!(nodes[0], nodes[1], "each piece names its own object");
}

/// A group set to Cut Outline: its members inherit it and are united.
#[test]
fn a_group_set_to_outline_unites_its_members() {
    let mut d = Document::new(400.0, 400.0);
    let layer = d.layers[0].id;
    let (a, b) = (square(&mut d, 10.0, 10.0, 100.0), square(&mut d, 60.0, 10.0, 100.0));
    let g = Node::group(d.alloc_id(), vec![Arc::new(a), Arc::new(b)]);
    let g = d.insert(Some(layer), usize::MAX, g).unwrap();
    set_own_op(d.node_mut(g).unwrap(), Some(Op::Outline));
    let (out, _) = weld(outlines(&d, &select(&d, &CuttingSettings::default())).0);
    assert_eq!(out.len(), 1);
    assert!(close(out[0].path.bounding_box(), Rect::new(10.0, 10.0, 160.0, 110.0)));
}

#[test]
fn an_open_path_is_cut_as_it_is_with_a_warning() {
    let (out, warnings) =
        welded(Op::Outline, |d| vec![Node::path(d.alloc_id(), shapes::line(Point::new(10.0, 10.0), Point::new(100.0, 10.0)), Appearance::default())]);
    assert_eq!(out.len(), 1);
    assert!(!out[0].closed);
    assert_eq!(warnings.len(), 1);
}
