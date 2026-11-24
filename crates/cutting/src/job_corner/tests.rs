use vectorcraft_cutjob::{MatGeometry, OpKind, SourceRef};
use vectorcraft_geom::Rect;

use super::*;

/// A 300 × 200 mm mat cut all over, the document laid on it a point to the millimetre.
fn placement() -> Placement {
    let mat = MatGeometry::custom([300.0, 200.0]);
    Placement { artboard_mm: mat.cut_area_mm, mat, doc_to_mat: Affine::IDENTITY }
}

fn square(node: u64, x: f64, y: f64, side: f64) -> Outline {
    Outline {
        op: OpKind::Cut,
        outline: false,
        source: SourceRef { node, subpath: 0 },
        closed: true,
        path: Rect::new(x, y, x + side, y + side).to_path(0.1),
    }
}

fn at(corner: JobCorner) -> CuttingSettings {
    CuttingSettings { job_corner: corner, ..CuttingSettings::default() }
}

/// Where `outlines` land on the mat once moved, mm.
fn landed(s: &CuttingSettings, outlines: &[Outline]) -> (Rect, Vec<JobWarning>) {
    let (p, warnings) = moved(s, &placement(), outlines);
    let b = outlines.iter().map(|o| (p.doc_to_mat * o.path.clone()).bounding_box()).reduce(|a, b| a.union(b));
    (b.unwrap_or_default(), warnings)
}

fn close(a: Rect, want: [f64; 4]) -> bool {
    [a.x0, a.y0, a.x1, a.y1].iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-9)
}

#[test]
fn by_the_artboard_nothing_moves() {
    let (p, warnings) = moved(&at(JobCorner::Artboard), &placement(), &[square(1, 100.0, 80.0, 20.0)]);
    assert_eq!(p, placement());
    assert!(warnings.is_empty());
}

/// A 20 mm square in the middle goes 5 mm (the usual margin) in from each corner's two edges.
#[test]
fn each_corner_takes_the_cut_a_margin_in() {
    let art = [square(1, 100.0, 80.0, 20.0)];
    for (corner, want) in [
        (JobCorner::TopLeft, [5.0, 5.0, 25.0, 25.0]),
        (JobCorner::TopRight, [275.0, 5.0, 295.0, 25.0]),
        (JobCorner::BottomLeft, [5.0, 175.0, 25.0, 195.0]),
        (JobCorner::BottomRight, [275.0, 175.0, 295.0, 195.0]),
    ] {
        let (b, warnings) = landed(&at(corner), &art);
        assert!(close(b, want), "{corner:?}: {b:?}");
        assert!(warnings.is_empty(), "{corner:?}: {warnings:?}");
    }
    let s = CuttingSettings { job_margin_mm: 0.0, ..at(JobCorner::TopLeft) };
    assert!(close(landed(&s, &art).0, [0.0, 0.0, 20.0, 20.0]));
}

/// Pieces keep their places to one another: the whole cut moves, its top-left to the corner.
#[test]
fn the_pieces_move_together() {
    let art = [square(1, 100.0, 80.0, 20.0), square(2, 150.0, 120.0, 10.0)];
    let (p, _) = moved(&at(JobCorner::TopLeft), &placement(), &art);
    let second = (p.doc_to_mat * art[1].path.clone()).bounding_box();
    assert!(close(second, [55.0, 45.0, 65.0, 55.0]), "{second:?}");
    assert!((p.artboard_mm.x0 - (-95.0)).abs() < 1e-9, "the artboard moves with it: {:?}", p.artboard_mm);
}

/// With weeding, the weed box is what goes the margin in: the design its own margin further.
#[test]
fn the_weed_box_goes_into_the_corner() {
    let s = CuttingSettings { weed_box: true, weed_margin_mm: 3.0, ..at(JobCorner::TopLeft) };
    assert!(close(landed(&s, &[square(1, 100.0, 80.0, 20.0)]).0, [8.0, 8.0, 28.0, 28.0]));
}

/// Too large for the margin on one side: as near the corner as it fits, and said; larger than the
/// mat: against the corner's edge (the job then says it runs off the mat).
#[test]
fn a_large_cut_goes_nearer_the_corner() {
    let (b, warnings) = landed(&at(JobCorner::TopRight), &[square(1, 0.0, 0.0, 196.0)]);
    assert!(close(b, [99.0, 4.0, 295.0, 200.0]), "{b:?}");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].message, MARGIN_WARNING);
    let (b, _) = landed(&at(JobCorner::TopLeft), &[square(1, 40.0, 40.0, 250.0)]);
    assert!(close(b, [5.0, 0.0, 255.0, 250.0]), "{b:?}");
}

#[test]
fn print_and_cut_stays_where_the_marks_are() {
    let s = CuttingSettings { print_and_cut: true, ..at(JobCorner::TopLeft) };
    let (p, warnings) = moved(&s, &placement(), &[square(1, 100.0, 80.0, 20.0)]);
    assert_eq!(p, placement());
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].message, PRINT_WARNING);
}

#[test]
fn nothing_to_cut_moves_nothing() {
    let (p, warnings) = moved(&at(JobCorner::BottomRight), &placement(), &[]);
    assert_eq!(p, placement());
    assert!(warnings.is_empty());
}
