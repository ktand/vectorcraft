use vectorcraft_cutjob::{OpKind, SourceRef};

use super::*;

fn poly(node: u64, op: OpKind, closed: bool, points: &[[f64; 2]]) -> Polyline {
    Polyline { op, source: SourceRef { node, subpath: 0 }, closed, points: points.to_vec() }
}

fn square(node: u64, x: f64, y: f64, side: f64) -> Polyline {
    poly(node, OpKind::Cut, true, &[[x, y], [x + side, y], [x + side, y + side], [x, y + side]])
}

fn nodes(v: &[Polyline]) -> Vec<u64> {
    v.iter().map(|p| p.source.node).collect()
}

/// Blade travel from `origin` through `v`.
fn travel(v: &[Polyline], origin: [f64; 2]) -> f64 {
    let mut at = origin;
    let mut sum = 0.0;
    for p in v {
        sum += dist(at, p.points[0]);
        at = end_of(p).unwrap();
    }
    sum
}

#[test]
fn scoring_comes_before_cutting() {
    let v = vec![square(1, 0.0, 0.0, 10.0), poly(2, OpKind::Score, false, &[[50.0, 0.0], [60.0, 0.0]]), square(3, 20.0, 0.0, 10.0)];
    for ordering in [Ordering::InsideFirst, Ordering::Document] {
        let out = order(v.clone(), ordering, [0.0, 0.0]);
        assert_eq!(out[0].op, OpKind::Score, "{ordering:?}");
    }
    assert_eq!(nodes(&order(v, Ordering::Document, [0.0, 0.0])), [2, 1, 3], "otherwise paint order");
}

/// The counter of an O is cut before the O, though the O is nearer and comes first.
#[test]
fn inner_paths_go_first() {
    let o = square(1, 0.0, 0.0, 50.0);
    let counter = square(2, 20.0, 20.0, 10.0);
    let dot = poly(3, OpKind::Cut, false, &[[24.0, 25.0], [26.0, 25.0]]);
    let out = order(vec![o, counter, dot], Ordering::InsideFirst, [0.0, 0.0]);
    let n = nodes(&out);
    let pos = |id| n.iter().position(|x| *x == id).unwrap();
    assert!(pos(2) < pos(1) && pos(3) < pos(1), "{n:?}");
    assert!(pos(3) < pos(2), "the dot inside the counter before the counter: {n:?}");
}

#[test]
fn the_nearest_path_goes_next_and_open_paths_turn_round() {
    // In document order: far, near, middle; the middle line is drawn away from the near one.
    let far = poly(1, OpKind::Cut, false, &[[200.0, 0.0], [210.0, 0.0]]);
    let near = poly(2, OpKind::Cut, false, &[[10.0, 0.0], [20.0, 0.0]]);
    let middle = poly(3, OpKind::Cut, false, &[[100.0, 0.0], [30.0, 0.0]]);
    let input = vec![far, near, middle];
    let out = order(input.clone(), Ordering::InsideFirst, [0.0, 0.0]);
    assert_eq!(nodes(&out), [2, 3, 1]);
    assert_eq!(out[1].points, [[30.0, 0.0], [100.0, 0.0]], "reversed to start at the nearer end");
    assert!(travel(&out, [0.0, 0.0]) < travel(&input, [0.0, 0.0]));
}

#[test]
fn a_closed_path_starts_at_its_point_nearest_the_blade() {
    let out = order(vec![square(1, 100.0, 100.0, 10.0)], Ordering::InsideFirst, [200.0, 200.0]);
    assert_eq!(out[0].points[0], [110.0, 110.0]);
    assert_eq!(out[0].points.len(), 4);
}

/// Squares side by side, none inside another, are all cut, each once.
#[test]
fn every_path_is_cut_once() {
    let v: Vec<Polyline> = (0..30).map(|i| square(i, (i % 6) as f64 * 20.0, (i / 6) as f64 * 20.0, 10.0)).collect();
    let mut n = nodes(&order(v, Ordering::InsideFirst, [0.0, 0.0]));
    n.sort();
    assert_eq!(n, (0..30).collect::<Vec<_>>());
}

/// Past the search's limit the inner paths still go first.
#[test]
fn many_paths_keep_inner_ones_first() {
    let mut v = vec![square(0, 0.0, 0.0, 300.0)];
    v.extend((1..=MAX_OPTIMIZED as u64).map(|i| square(i, 1.0 + (i % 60) as f64 * 4.5, 1.0 + (i / 60) as f64 * 4.0, 1.0)));
    let out = order(v, Ordering::InsideFirst, [0.0, 0.0]);
    assert_eq!(out.len(), MAX_OPTIMIZED + 1);
    assert_eq!(out.last().map(|p| p.source.node), Some(0), "the big square last");
}

#[test]
fn points_in_polygons() {
    let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    assert!(in_polygon([5.0, 5.0], &sq));
    assert!(!in_polygon([15.0, 5.0], &sq));
    assert!(!in_polygon([5.0, 5.0], &[]));
}

/// C5.6: nearest-next alone goes 51 → 48 → 44 → 53.5 (17.5 mm up); the shortened order goes
/// out and back once instead (13 mm).
#[test]
fn the_order_is_shortened_where_nearest_next_isnt_best() {
    let dash = |node, x: f64| poly(node, OpKind::Cut, false, &[[x, 0.0], [x + 0.001, 0.0]]);
    let v = vec![dash(1, 51.0), dash(2, 48.0), dash(3, 44.0), dash(4, 53.5)];
    let out = order(v, Ordering::InsideFirst, [50.0, 0.0]);
    let t = travel(&out, [50.0, 0.0]);
    assert!(t < 13.1, "{t} mm up: {:?}", nodes(&out));
    assert_eq!(nodes(&out), [1, 4, 2, 3]);
}

/// Shortening never cuts a path before one inside it, and every path is still cut once.
#[test]
fn shortening_keeps_inner_paths_first() {
    // Rings of squares round a common centre, and loose dashes around them.
    let mut v = vec![];
    for (k, side) in [80.0, 60.0, 40.0, 20.0].iter().enumerate() {
        let o = (100.0 - side) / 2.0;
        v.push(square(k as u64 + 1, 100.0 + o, o, *side));
    }
    for k in 0..30u64 {
        let x = (k * 37 % 200) as f64;
        let y = (k * 53 % 120) as f64 + 120.0;
        v.push(poly(100 + k, OpKind::Cut, false, &[[x, y], [x + 3.0, y + 1.0]]));
    }
    let out = order(v.clone(), Ordering::InsideFirst, [0.0, 0.0]);
    let n = nodes(&out);
    let pos = |id| n.iter().position(|x| *x == id).unwrap();
    assert!(pos(4) < pos(3) && pos(3) < pos(2) && pos(2) < pos(1), "inside first: {n:?}");
    let mut sorted = n.clone();
    sorted.sort();
    let mut want = nodes(&v);
    want.sort();
    assert_eq!(sorted, want, "each path once");
}

/// C5.6: the most paths the search orders, scattered: the shortening stays within its work
/// budget (a fraction of a second in a release build) and only ever shortens.
#[test]
fn many_scattered_paths_are_shortened_within_the_budget() {
    let mut seed = 12345u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / f64::from(u32::MAX >> 1)
    };
    let v: Vec<Polyline> = (0..MAX_OPTIMIZED as u64)
        .map(|k| {
            let (x, y) = (next() * 290.0, next() * 290.0);
            poly(k, OpKind::Cut, false, &[[x, y], [x + 2.0, y + 1.0]])
        })
        .collect();
    let started = std::time::Instant::now();
    let nearest = order_group(v.clone(), [0.0, 0.0], 0);
    let took_nearest = started.elapsed();
    let out = order_group(v.clone(), [0.0, 0.0], MAX_IMPROVE_WORK);
    let (before, after) = (travel(&nearest, [0.0, 0.0]), travel(&out, [0.0, 0.0]));
    eprintln!("nearest next {took_nearest:?}, {before:.0} mm up; shortened {:?} in all, {after:.0} mm up", started.elapsed() - took_nearest);
    assert_eq!(out.len(), v.len());
    assert!(after < before, "shortened: {after} < {before}");
}
