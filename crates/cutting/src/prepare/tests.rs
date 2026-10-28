use vectorcraft_cutjob::CuttingSettings;
use vectorcraft_doc::Document;
use vectorcraft_geom::{BezPath, Point, Rect, Shape, shapes};

use super::*;
use crate::placement::{MM_PER_PT, place};

/// Points per millimetre.
const PT: f64 = 1.0 / MM_PER_PT;

/// The default placement of a 300 × 300 mm artboard on the 12 × 12 in mat, offset by `offset`.
fn placement(offset: [f64; 2]) -> Placement {
    let d = Document::new(300.0 * PT, 300.0 * PT);
    place(&CuttingSettings { offset_mm: offset, ..CuttingSettings::default() }, &d).unwrap()
}

fn outline(path: BezPath, closed: bool) -> Outline {
    Outline { op: OpKind::Cut, outline: false, source: SourceRef { node: 9, subpath: 0 }, closed, path }
}

fn run(outlines: &[Outline], offset: [f64; 2], clip: bool) -> Result<(Vec<Polyline>, Vec<JobWarning>), PrepError> {
    prepare(outlines, &placement(offset), 0.025, clip)
}

/// A circle of radius 10 mm at (50, 50) mm: every point on it and every chord within the
/// tolerance of it.
#[test]
fn a_circle_flattens_within_the_tolerance() {
    let c = shapes::ellipse(Rect::new(40.0 * PT, 40.0 * PT, 60.0 * PT, 60.0 * PT)).to_bezpath();
    let (out, warnings) = run(&[outline(c, true)], [0.0, 0.0], false).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(out.len(), 1);
    let p = &out[0];
    assert!(p.closed);
    assert!(p.points.len() > 20, "{} points", p.points.len());
    let r = |q: [f64; 2]| (q[0] - 50.0).hypot(q[1] - 50.0);
    let mut ring = p.points.clone();
    ring.push(p.points[0]);
    for w in ring.windows(2) {
        assert!((r(w[0]) - 10.0).abs() < 0.025, "on the circle: {:?}", w[0]);
        let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
        assert!(10.0 - r(mid) <= 0.025 + 1e-9, "chord within the tolerance: {mid:?}");
    }
}

#[test]
fn the_placement_moves_the_points_onto_the_mat() {
    let line = shapes::line(Point::new(10.0 * PT, 20.0 * PT), Point::new(30.0 * PT, 20.0 * PT)).to_bezpath();
    let (out, _) = run(&[outline(line, false)], [5.0, 7.0], false).unwrap();
    let got = &out[0].points;
    assert!((got[0][0] - 15.0).abs() < 1e-9 && (got[0][1] - 27.0).abs() < 1e-9, "{got:?}");
    assert!((got[1][0] - 35.0).abs() < 1e-9, "{got:?}");
}

/// A square drawn with a repeated corner and midpoints on its sides is its four corners, its
/// closing point not repeated.
#[test]
fn repeats_and_points_on_straight_runs_are_merged() {
    let mut p = BezPath::new();
    for (x, y) in [(10.0, 10.0), (15.0, 10.0), (20.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0), (10.0, 15.0), (10.0, 10.0)] {
        if p.elements().is_empty() {
            p.move_to((x * PT, y * PT));
        } else {
            p.line_to((x * PT, y * PT));
        }
    }
    p.close_path();
    let (out, _) = run(&[outline(p, true)], [0.0, 0.0], false).unwrap();
    let pts: Vec<[f64; 2]> = out[0].points.iter().map(|q| [(q[0] * 1e6).round() / 1e6, (q[1] * 1e6).round() / 1e6]).collect();
    assert_eq!(pts, [[10.0, 10.0], [20.0, 10.0], [20.0, 20.0], [10.0, 20.0]]);
    assert!(out[0].closed);
}

#[test]
fn what_has_no_length_is_dropped() {
    let dot = shapes::line(Point::new(10.0 * PT, 10.0 * PT), Point::new(10.0 * PT, 10.0 * PT)).to_bezpath();
    let (out, _) = run(&[outline(dot, false)], [0.0, 0.0], false).unwrap();
    assert!(out.is_empty());
    // A closed "shape" that is a line there and back is cut as the line.
    let mut flat = BezPath::new();
    flat.move_to((10.0 * PT, 10.0 * PT));
    flat.line_to((30.0 * PT, 10.0 * PT));
    flat.close_path();
    let (out, _) = run(&[outline(flat, true)], [0.0, 0.0], false).unwrap();
    assert_eq!((out.len(), out[0].closed, out[0].points.len()), (1, false, 2));
}

#[test]
fn leaving_the_cut_area_stops_the_job_naming_the_object() {
    let line = shapes::line(Point::new(10.0 * PT, 10.0 * PT), Point::new(310.0 * PT, 10.0 * PT)).to_bezpath();
    let e = run(&[outline(line, false)], [0.0, 0.0], false).unwrap_err();
    assert!(matches!(e, PrepError::OffTheCutArea { source: SourceRef { node: 9, .. }, point } if point[0] > 304.8), "{e:?}");
    assert!(e.to_string().contains("object 9"));
}

/// With Clip to Mat, a square across the mat's right edge is cut up to the edge, in one open
/// piece, with a warning.
#[test]
fn clip_to_mat_cuts_up_to_the_edge() {
    let sq = shapes::rectangle(Rect::new(290.0 * PT, 10.0 * PT, 320.0 * PT, 40.0 * PT)).to_bezpath();
    let (out, warnings) = run(&[outline(sq, true)], [0.0, 0.0], true).unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].source.map(|s| s.node), Some(9));
    assert!(!out.is_empty() && out.iter().all(|p| !p.closed));
    for p in &out {
        assert!(p.points.iter().all(|q| q[0] <= 304.8 + 1e-6), "{:?}", p.points);
    }
    let length: f64 = out.iter().flat_map(|p| p.points.windows(2).map(|w| (w[0][0] - w[1][0]).hypot(w[0][1] - w[1][1]))).sum();
    // Inside: 14.8 along the top and bottom and 30 down the left side.
    assert!((length - (14.8 * 2.0 + 30.0)).abs() < 1e-6, "{length}");
    // Wholly off the mat, nothing is cut.
    let off = shapes::rectangle(Rect::new(400.0 * PT, 10.0 * PT, 420.0 * PT, 40.0 * PT)).to_bezpath();
    assert!(run(&[outline(off, true)], [0.0, 0.0], true).unwrap().0.is_empty());
}

#[test]
fn coordinates_that_arent_numbers_are_left_out_with_a_warning() {
    let mut p = BezPath::new();
    p.move_to((10.0, 10.0));
    p.line_to((f64::NAN, 10.0));
    let (out, warnings) = run(&[outline(p, false)], [0.0, 0.0], false).unwrap();
    assert!(out.is_empty());
    assert_eq!(warnings.len(), 1);
}

/// A curve flattened finer than a job can hold is an error, not an endless list.
#[test]
fn too_many_points_is_an_error() {
    let huge = shapes::ellipse(Rect::new(0.0, 0.0, 860.0, 860.0)).to_bezpath();
    let many: Vec<Outline> = (0..2_000).map(|_| outline(huge.clone(), true)).collect();
    assert_eq!(prepare(&many, &placement([0.0, 0.0]), 0.000_000_1, false).unwrap_err(), PrepError::TooManyPoints);
}

#[test]
fn a_segment_clips_to_the_rectangle() {
    let r = RectMm { x0: 0.0, y0: 0.0, x1: 10.0, y1: 10.0 };
    assert_eq!(clip_segment([-5.0, 5.0], [15.0, 5.0], r), Some(([0.0, 5.0], [10.0, 5.0])));
    assert_eq!(clip_segment([2.0, 2.0], [3.0, 3.0], r), Some(([2.0, 2.0], [3.0, 3.0])));
    assert_eq!(clip_segment([-5.0, -5.0], [-1.0, 20.0], r), None);
    assert_eq!(clip_segment([12.0, 2.0], [12.0, 8.0], r), None);
    let _ = Shape::bounding_box(&BezPath::new());
}
