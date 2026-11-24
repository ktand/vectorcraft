//! A job set against a corner of the mat (C18.32): what it cuts moved, as a whole, so it lies the
//! margin in from that corner's two edges, for a small piece of material pushed into the corner
//! of the mat (a layer of a layered design, an offcut). Only the job moves: the document stays
//! where it is.
//!
//! What is measured is what the design cuts, laid on the mat as it would be (mirrored, turned),
//! the groups that are switched off left out (the job leaves them out before this), so a layered design cut one layer at a time puts
//! each layer in the corner by its own size. With weeding on, the weed box round it is a margin
//! larger, so the box is measured. The alignment cross is placed after, wherever there is room.

use vectorcraft_cutjob::{CuttingSettings, JobCorner, JobWarning, RectMm};
use vectorcraft_geom::{Affine, Shape};

use crate::geometry::Outline;
use crate::placement::Placement;

/// Why a Print & Cut job isn't moved to the corner.
pub const PRINT_WARNING: &str = "Print & Cut is cut where the registration marks are: the job isn't moved to the corner";
/// Why the job is nearer the corner than the margin.
pub const MARGIN_WARNING: &str = "the cut is too large for the margin: it is set nearer the corner";

/// Within this (mm) of the cut area's edge still counts as on it.
const SLACK_MM: f64 = 1e-6;

/// What `outlines` cut on `placement`'s mat, mm; weeding's margin round it when the job weeds.
/// `None` when nothing finite is cut.
fn extent(s: &CuttingSettings, placement: &Placement, outlines: &[Outline]) -> Option<RectMm> {
    let mut b: Option<RectMm> = None;
    for o in outlines {
        let r = (placement.doc_to_mat * o.path.clone()).bounding_box();
        if ![r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()) {
            continue;
        }
        b = Some(match b {
            None => RectMm { x0: r.x0, y0: r.y0, x1: r.x1, y1: r.y1 },
            Some(a) => RectMm { x0: a.x0.min(r.x0), y0: a.y0.min(r.y0), x1: a.x1.max(r.x1), y1: a.y1.max(r.y1) },
        });
    }
    let b = b?;
    let m = if s.weed_box || s.weed_lines { s.weed_margin_mm } else { 0.0 };
    Some(RectMm { x0: b.x0 - m, y0: b.y0 - m, x1: b.x1 + m, y1: b.y1 + m })
}

/// How far to move `lo..hi` so its `low` side (else its high side) is `margin` in from that side of
/// `area_lo..area_hi`, nearer when it is too large for that; whether it had to be nearer.
fn along(lo: f64, hi: f64, area_lo: f64, area_hi: f64, margin: f64, low: bool) -> (f64, bool) {
    let room = (area_hi - area_lo) - (hi - lo);
    let inset = margin.min(room.max(0.0));
    let d = if low { area_lo + inset - lo } else { area_hi - inset - hi };
    (d, inset + SLACK_MM < margin)
}

/// `placement` (the one the job is cut on, already mirrored) moved as `s.job_corner` says by
/// `outlines` (what is cut: the groups switched off already left out), and what to warn of. Unchanged where the artboard decides (the default), for Print & Cut, and when
/// nothing is cut.
pub fn moved(s: &CuttingSettings, placement: &Placement, outlines: &[Outline]) -> (Placement, Vec<JobWarning>) {
    let corner = s.job_corner;
    if corner == JobCorner::Artboard {
        return (placement.clone(), vec![]);
    }
    if s.print_and_cut {
        return (placement.clone(), vec![JobWarning { source: None, message: PRINT_WARNING.into() }]);
    }
    let Some(b) = extent(s, placement, outlines) else { return (placement.clone(), vec![]) };
    let a = placement.mat.cut_area_mm;
    let m = if s.job_margin_mm.is_finite() { s.job_margin_mm.max(0.0) } else { 0.0 };
    let left = matches!(corner, JobCorner::TopLeft | JobCorner::BottomLeft);
    let top = matches!(corner, JobCorner::TopLeft | JobCorner::TopRight);
    let (dx, nearer_x) = along(b.x0, b.x1, a.x0, a.x1, m, left);
    let (dy, nearer_y) = along(b.y0, b.y1, a.y0, a.y1, m, top);
    if !(dx.is_finite() && dy.is_finite()) {
        return (placement.clone(), vec![]);
    }
    let r = placement.artboard_mm;
    let moved = Placement {
        doc_to_mat: Affine::translate((dx, dy)) * placement.doc_to_mat,
        artboard_mm: RectMm { x0: r.x0 + dx, y0: r.y0 + dy, x1: r.x1 + dx, y1: r.y1 + dy },
        mat: placement.mat.clone(),
    };
    let warnings = if nearer_x || nearer_y { vec![JobWarning { source: None, message: MARGIN_WARNING.into() }] } else { vec![] };
    (moved, warnings)
}

#[cfg(test)]
mod tests;
