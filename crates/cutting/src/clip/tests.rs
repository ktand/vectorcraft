use std::sync::Arc;

use vectorcraft_cutjob::CuttingSettings;
use vectorcraft_doc::{Appearance, Node, NodeKind};
use vectorcraft_geom::{Rect, Shape, shapes};

use super::*;
use crate::geometry::{Outline, outlines};
use crate::select::select;

fn rect(d: &mut Document, r: Rect) -> Node {
    Node::path(d.alloc_id(), shapes::rectangle(r), Appearance::default())
}

fn line(d: &mut Document, a: (f64, f64), b: (f64, f64)) -> Node {
    Node::path(d.alloc_id(), shapes::line(Point::new(a.0, a.1), Point::new(b.0, b.1)), Appearance::default())
}

/// `mask` clipping `art` in a clip group on a 400 × 400 pt document; the outlines it cuts.
fn clipped(make: impl FnOnce(&mut Document) -> (Node, Vec<Node>)) -> (Vec<Outline>, Vec<vectorcraft_cutjob::JobWarning>) {
    let mut d = Document::new(400.0, 400.0);
    let layer = d.layers[0].id;
    let (mut mask, art) = make(&mut d);
    if let NodeKind::Path { clipping, .. } = &mut mask.kind {
        *clipping = true;
    }
    let children = std::iter::once(mask).chain(art).map(Arc::new).collect();
    let g = Node::new(d.alloc_id(), NodeKind::Group { children, clip: true });
    d.insert(Some(layer), usize::MAX, g).unwrap();
    let items = select(&d, &CuttingSettings::default());
    outlines(&d, &items)
}

fn bounds(o: &Outline) -> Rect {
    o.path.bounding_box()
}

fn close(a: Rect, b: Rect) -> bool {
    [(a.x0, b.x0), (a.y0, b.y0), (a.x1, b.x1), (a.y1, b.y1)].iter().all(|(p, q)| (p - q).abs() < 0.05)
}

/// A square half under the mask is cut as the half that shows, closed.
#[test]
fn a_closed_shape_is_cut_where_it_shows() {
    let (out, warnings) = clipped(|d| (rect(d, Rect::new(0.0, 0.0, 100.0, 200.0)), vec![rect(d, Rect::new(50.0, 50.0, 150.0, 150.0))]));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(out.len(), 1);
    assert!(out[0].closed);
    assert!(close(bounds(&out[0]), Rect::new(50.0, 50.0, 100.0, 150.0)), "{:?}", bounds(&out[0]));
}

/// A line across the mask's edge is cut inside it only.
#[test]
fn an_open_line_is_cut_inside_the_mask() {
    let (out, _) = clipped(|d| (rect(d, Rect::new(0.0, 0.0, 100.0, 100.0)), vec![line(d, (-50.0, 50.0), (150.0, 50.0))]));
    assert_eq!(out.len(), 1);
    assert!(!out[0].closed);
    assert!(close(bounds(&out[0]), Rect::new(0.0, 50.0, 100.0, 50.0)), "{:?}", bounds(&out[0]));
}

/// A ring-shaped mask (a hole, even-odd): a line through it is cut in two pieces, not across the
/// hole.
#[test]
fn a_line_through_a_ring_mask_is_cut_on_the_ring() {
    let (out, _) = clipped(|d| {
        let mut ring = shapes::rectangle(Rect::new(0.0, 0.0, 300.0, 300.0)).to_bezpath();
        ring.extend(shapes::rectangle(Rect::new(100.0, 100.0, 200.0, 200.0)).to_bezpath());
        let mut mask = Node::path(d.alloc_id(), PathData::from_bezpath(&ring), Appearance::default());
        if let NodeKind::Path { rule, .. } = &mut mask.kind {
            *rule = FillRule::EvenOdd;
        }
        (mask, vec![line(d, (-10.0, 150.0), (310.0, 150.0))])
    });
    let mut spans: Vec<(f64, f64)> = out.iter().map(|o| (bounds(o).x0, bounds(o).x1)).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(spans.len(), 2, "{spans:?}");
    assert!((spans[0].0 - 0.0).abs() < 0.05 && (spans[0].1 - 100.0).abs() < 0.05, "{spans:?}");
    assert!((spans[1].0 - 200.0).abs() < 0.05 && (spans[1].1 - 300.0).abs() < 0.05, "{spans:?}");
}

/// Art wholly outside its mask isn't cut.
#[test]
fn hidden_art_isnt_cut() {
    let (out, _) = clipped(|d| (rect(d, Rect::new(0.0, 0.0, 50.0, 50.0)), vec![rect(d, Rect::new(200.0, 200.0, 250.0, 250.0))]));
    assert!(out.is_empty(), "{out:?}");
}

/// A clip group inside another: the art is cut where both show.
#[test]
fn nested_masks_both_clip() {
    let mut d = Document::new(400.0, 400.0);
    let layer = d.layers[0].id;
    let mask = |d: &mut Document, r: Rect| {
        let mut m = rect(d, r);
        if let NodeKind::Path { clipping, .. } = &mut m.kind {
            *clipping = true;
        }
        m
    };
    let art = rect(&mut d, Rect::new(0.0, 0.0, 300.0, 300.0));
    let inner_mask = mask(&mut d, Rect::new(100.0, 0.0, 300.0, 300.0));
    let inner = Node::new(d.alloc_id(), NodeKind::Group { children: vec![Arc::new(inner_mask), Arc::new(art)], clip: true });
    let outer_mask = mask(&mut d, Rect::new(0.0, 0.0, 200.0, 200.0));
    let outer = Node::new(d.alloc_id(), NodeKind::Group { children: vec![Arc::new(outer_mask), Arc::new(inner)], clip: true });
    d.insert(Some(layer), usize::MAX, outer).unwrap();
    let (out, _) = outlines(&d, &select(&d, &CuttingSettings::default()));
    assert_eq!(out.len(), 1);
    assert!(close(bounds(&out[0]), Rect::new(100.0, 0.0, 200.0, 200.0)), "{:?}", bounds(&out[0]));
}

#[test]
fn segments_cross() {
    let t = crossing(Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(4.0, -1.0), Point::new(4.0, 1.0));
    assert!((t.unwrap() - 0.4).abs() < 1e-12);
    assert_eq!(crossing(Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(0.0, 1.0), Point::new(10.0, 1.0)), None);
}
