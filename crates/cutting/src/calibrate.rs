//! Fork: material calibration (C18.23): a row of test cuts, each with one setting a step apart and
//! the rest as they are, to find the right value by looking rather than by guessing. Each cell is
//! the test cut's shape (a square with a triangle inside, the triangle first), the cells side by
//! side from a corner of the mat's cut area, counted from the corner along the row.

use vectorcraft_cutjob::{CutJob, JOB_VERSION, JobMat, OpKind, Operation, RectMm, SourceRef, ToolSettings, Toolpath};
use vectorcraft_doc::Document;

use crate::BuildError;
use crate::test_cut::{Corner, MARGIN_MM, SQUARE_MM};

/// The space between two cells, mm.
pub const GAP_MM: f64 = 4.0;
/// The most cells in a row.
pub const MAX_CELLS: usize = 9;
/// How far inside a cell its triangle sits, mm (as the test cut's).
const INSET_MM: f64 = 2.0;
/// A cell's mark: a short line cut above the first cell, so the row's start is found on the
/// material, mm.
const MARK_MM: f64 = 3.0;

/// The cells' squares, from `corner` along the cut area's top or bottom edge, or `None` when the
/// row doesn't fit.
pub fn cells_at(area: RectMm, corner: Corner, n: usize) -> Option<Vec<RectMm>> {
    let nf = n as f64;
    let width = nf * SQUARE_MM + (nf - 1.0).max(0.0) * GAP_MM;
    let need_w = width + 2.0 * MARGIN_MM;
    let need_h = SQUARE_MM + 2.0 * MARGIN_MM + MARK_MM + 1.0;
    if n == 0 || !(area.width() >= need_w && area.height() >= need_h) {
        return None;
    }
    let left = matches!(corner, Corner::TopLeft | Corner::BottomLeft);
    let top = matches!(corner, Corner::TopLeft | Corner::TopRight);
    // Room for the first cell's mark between the row and the edge.
    let y0 = if top { area.y0 + MARGIN_MM + MARK_MM + 1.0 } else { area.y1 - MARGIN_MM - MARK_MM - 1.0 - SQUARE_MM };
    Some(
        (0..n)
            .map(|k| {
                let step = k as f64 * (SQUARE_MM + GAP_MM);
                let x0 = if left { area.x0 + MARGIN_MM + step } else { area.x1 - MARGIN_MM - SQUARE_MM - step };
                RectMm { x0, y0, x1: x0 + SQUARE_MM, y1: y0 + SQUARE_MM }
            })
            .collect(),
    )
}

/// The calibration job for `d`'s mat: one cell for each of `tools` (each cell cut with its own),
/// from `corner`, and a short mark above (or below) the first cell.
pub fn calibration_job(d: &Document, tools: &[ToolSettings], corner: Corner) -> Result<CutJob, BuildError> {
    if tools.is_empty() || tools.len() > MAX_CELLS {
        return Err(BuildError::TestCut(format!("a calibration row has 1 to {MAX_CELLS} cells")));
    }
    let s = crate::settings(d);
    let mat = s.mat_geometry();
    let area = mat.cut_area_mm;
    let cells =
        cells_at(area, corner, tools.len()).ok_or_else(|| BuildError::TestCut("the mat's cut area is too small for the calibration row".into()))?;
    let mut paths = vec![];
    for (k, sq) in cells.iter().enumerate() {
        let i = INSET_MM;
        let triangle = vec![[sq.x0 + i, sq.y1 - i], [(sq.x0 + sq.x1) / 2.0, sq.y0 + i], [sq.x1 - i, sq.y1 - i]];
        let square = vec![[sq.x0, sq.y0], [sq.x1, sq.y0], [sq.x1, sq.y1], [sq.x0, sq.y1]];
        for points in [triangle, square] {
            paths.push(Toolpath { op: k, source: SourceRef::default(), closed: true, points });
        }
    }
    // The first cell's mark, outside the row, cut with the first cell's settings.
    if let Some(first) = cells.first() {
        let x = (first.x0 + first.x1) / 2.0;
        let top = matches!(corner, Corner::TopLeft | Corner::TopRight);
        let (a, b) = if top { (first.y0 - 1.0 - MARK_MM, first.y0 - 1.0) } else { (first.y1 + 1.0, first.y1 + 1.0 + MARK_MM) };
        paths.push(Toolpath { op: 0, source: SourceRef::default(), closed: false, points: vec![[x, a], [x, b]] });
    }
    let job = CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: mat.id.to_string(), size_mm: mat.size_mm, cut_area_mm: area },
        operations: tools.iter().map(|t| Operation { kind: OpKind::Cut, tool: t.clone() }).collect(),
        paths,
        warnings: vec![],
        track_enhancing: s.track_enhancing,
        groups: vec![],
        registration: None,
        curves: vec![],
        dashes: vec![],
    };
    job.validate().map_err(BuildError::Invalid)?;
    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> RectMm {
        RectMm { x0: 10.0, y0: 10.0, x1: 300.0, y1: 300.0 }
    }

    /// The cells run from the corner, a gap apart, the first nearest the corner.
    #[test]
    fn the_cells_run_from_the_corner() {
        let c = cells_at(area(), Corner::TopLeft, 5).unwrap();
        assert_eq!(c.len(), 5);
        assert!((c[0].x0 - (10.0 + MARGIN_MM)).abs() < 1e-9);
        assert!((c[1].x0 - c[0].x1 - GAP_MM).abs() < 1e-9);
        let r = cells_at(area(), Corner::BottomRight, 3).unwrap();
        assert!((r[0].x1 - (300.0 - MARGIN_MM)).abs() < 1e-9 && r[1].x1 < r[0].x0, "from the right");
        assert!((r[0].y1 - (300.0 - MARGIN_MM - MARK_MM - 1.0)).abs() < 1e-9, "room for the mark below");
        assert!(cells_at(RectMm { x0: 0.0, y0: 0.0, x1: 50.0, y1: 50.0 }, Corner::TopLeft, 5).is_none(), "too small");
        assert!(cells_at(area(), Corner::TopLeft, 0).is_none());
    }

    /// The job: one operation a cell, its triangle before its square, and the first cell's mark.
    #[test]
    fn each_cell_is_cut_with_its_own_settings() {
        let d = Document::new(800.0, 800.0);
        let tools: Vec<ToolSettings> = (1..=4).map(|k| ToolSettings { force: f64::from(k) / 10.0, ..ToolSettings::default() }).collect();
        let job = calibration_job(&d, &tools, Corner::TopLeft).unwrap();
        assert_eq!(job.operations.len(), 4);
        assert_eq!(job.paths.len(), 4 * 2 + 1);
        assert!(job.paths.iter().take(8).enumerate().all(|(i, p)| p.op == i / 2 && p.closed));
        assert!(!job.paths.last().unwrap().closed, "the mark");
        assert!(calibration_job(&d, &[], Corner::TopLeft).is_err());
        assert!(calibration_job(&d, &vec![ToolSettings::default(); MAX_CELLS + 1], Corner::TopLeft).is_err());
    }
}
