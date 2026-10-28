//! The cutting workspace's core (fork: `docs/cutting/ARCHITECTURE.md`): what a document says
//! about cutting, read from and written to the slots VectorCraft already keeps (§4), so the
//! document model and the native format are unchanged, and the cut job made from it.
//!
//! - [`ops`]: each object's operation, a foreign attribute (`cut:op`), inherited from its
//!   ancestors and otherwise the document's default.
//! - [`settings`] and [`set_settings`]: the document's [`CuttingSettings`], under
//!   `Document::unknown["cutting"]`.
//! - [`placement`]: where the settings' artboard lies on the mat, document points to mat mm.
//! - [`select`]: which objects a job cuts, and with which operation.
//! - [`geometry`]: the outlines each of them cuts along, as drawn.
//! - [`clip`]: those outlines cut to the clipping masks around them.
//! - [`weld`]: the shapes set to Cut Outline united, cut along the outline of the union.
//! - [`prepare`]: those outlines on the mat, in millimetres, flattened.
//! - [`order`]: the order they are cut in.
//! - [`corners`]: sharp corners run past and back, for the operations that ask for it.
//! - [`preview`]: a job as an SVG at the mat's size.
//! - [`build_job`]: all of it, a document to a validated [`CutJob`]. The document isn't changed:
//!   the job is made from it each time.
#![forbid(unsafe_code)]

pub mod clip;
pub mod corners;
pub mod geometry;
pub mod ops;
pub mod order;
pub mod placement;
pub mod prepare;
pub mod preview;
pub mod select;
pub mod weld;

use vectorcraft_cutjob::{CutJob, CuttingSettings, DOCUMENT_KEY, JOB_VERSION, JobError, JobMat, OpKind, Operation, Toolpath};
use vectorcraft_doc::Document;

use crate::prepare::PrepError;

/// The document's cutting settings (the defaults when it has none, read leniently when damaged).
pub fn settings(d: &Document) -> CuttingSettings {
    CuttingSettings::from_value(d.unknown.get(DOCUMENT_KEY))
}

/// Keeps `s` as the document's cutting settings.
pub fn set_settings(d: &mut Document, s: &CuttingSettings) {
    d.unknown.insert(DOCUMENT_KEY.to_string(), s.to_value());
}

/// Why a document can't be made into a job.
#[derive(Clone, Debug, PartialEq)]
pub enum BuildError {
    /// The cutting settings name an artboard the document doesn't have.
    NoArtboard(u32),
    Prepare(PrepError),
    Invalid(JobError),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::NoArtboard(i) => write!(f, "the cutting settings name artboard {i}, which the document doesn't have"),
            BuildError::Prepare(e) => e.fmt(f),
            BuildError::Invalid(e) => write!(f, "the job isn't valid: {e}"),
        }
    }
}

impl std::error::Error for BuildError {}

/// The cut job `d` makes with its cutting settings: what [`select`] picks, cut along its
/// [`geometry`], laid on the mat by [`prepare`] and put in [`order`], validated. The same document
/// and settings always make the same job.
pub fn build_job(d: &Document) -> Result<CutJob, BuildError> {
    let s = settings(d);
    let placement = placement::place(&s, d).ok_or(BuildError::NoArtboard(s.artboard))?;
    let items = select::select(d, &s);
    let (outlines, mut warnings) = geometry::outlines(d, &items);
    let (outlines, welding) = weld::weld(outlines);
    warnings.extend(welding);
    let (polylines, more) = prepare::prepare(&outlines, &placement, s.tolerance_mm, s.clip_to_mat).map_err(BuildError::Prepare)?;
    warnings.extend(more);
    let area = placement.mat.cut_area_mm;
    let ordered = order::order(polylines, s.ordering, [area.x0, area.y0]);
    // One operation for each kind the job uses, scored first.
    let mut kinds: Vec<OpKind> = ordered.iter().map(|p| p.op).collect();
    kinds.sort();
    kinds.dedup();
    let operations: Vec<Operation> = kinds
        .iter()
        .map(|&kind| {
            let tool = match kind {
                OpKind::Cut => s.operations.cut.clone(),
                OpKind::Score => s.operations.score.clone(),
            };
            Operation { kind, tool }
        })
        .collect();
    let paths = ordered
        .into_iter()
        .map(|p| Toolpath { op: kinds.iter().position(|k| *k == p.op).unwrap_or(0), source: p.source, closed: p.closed, points: p.points })
        .collect();
    let mut job = CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: placement.mat.id.to_string(), size_mm: placement.mat.size_mm, cut_area_mm: area },
        operations,
        paths,
        warnings,
        track_enhancing: s.track_enhancing,
    };
    corners::overcut_corners(&mut job);
    job.validate().map_err(BuildError::Invalid)?;
    Ok(job)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_cutjob::Orientation;

    use super::*;

    #[test]
    fn settings_live_in_the_documents_foreign_data() {
        let mut d = Document::new(100.0, 100.0);
        assert_eq!(settings(&d), CuttingSettings::default());
        let s = CuttingSettings { orientation: Orientation::Landscape, ..CuttingSettings::default() };
        set_settings(&mut d, &s);
        assert_eq!(settings(&d), s);
        assert_eq!(d.unknown[DOCUMENT_KEY]["orientation"], json!("landscape"));
        d.unknown.insert(DOCUMENT_KEY.into(), json!("damaged"));
        assert_eq!(settings(&d), CuttingSettings::default());
    }

    #[test]
    fn an_empty_document_makes_an_empty_job() {
        let job = build_job(&Document::new(100.0, 100.0)).unwrap();
        assert!(job.paths.is_empty() && job.operations.is_empty() && job.warnings.is_empty());
    }

    #[test]
    fn a_missing_artboard_is_an_error() {
        let mut d = Document::new(100.0, 100.0);
        set_settings(&mut d, &CuttingSettings { artboard: 3, ..CuttingSettings::default() });
        assert_eq!(build_job(&d), Err(BuildError::NoArtboard(3)));
    }
}
