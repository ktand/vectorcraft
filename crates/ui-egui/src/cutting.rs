//! Fork: what the Cutting panel and the canvas share (`docs/cutting/ARCHITECTURE.md`). The cut
//! job is made from the active document again only when the document changes (a new document
//! value: an edit, an undo, a settings change), not each frame and not when the selection does.
//! It is kept in the egui context's memory, with the document it was made from.

use std::sync::Arc;

use vectorcraft_cutjob::{CutJob, JobStats};
use vectorcraft_doc::Document;

use crate::VectorcraftApp;

/// A job and its stats, or why the document makes none.
pub(crate) type JobResult = Result<(CutJob, JobStats), String>;

#[derive(Clone)]
struct Cached {
    uid: u64,
    /// Held, so the comparison below can't be fooled by a new document at a freed one's address.
    doc: Arc<Document>,
    job: Arc<JobResult>,
}

fn key() -> egui::Id {
    egui::Id::unique("cutting-job")
}

/// The active document's cut job (`None`: no document).
pub(crate) fn job(app: &VectorcraftApp, ctx: &egui::Context) -> Option<Arc<JobResult>> {
    let st = app.session.active()?;
    if let Some(c) = ctx.data(|d| d.get_temp::<Cached>(key()))
        && c.uid == st.uid
        && Arc::ptr_eq(&c.doc, &st.doc)
    {
        return Some(c.job);
    }
    let job = Arc::new(
        vectorcraft_cutting::build_job(&st.doc)
            .map(|j| {
                let s = j.stats();
                (j, s)
            })
            .map_err(|e| e.to_string()),
    );
    ctx.data_mut(|d| d.insert_temp(key(), Cached { uid: st.uid, doc: st.doc.clone(), job: job.clone() }));
    Some(job)
}

fn toolpaths_key() -> egui::Id {
    egui::Id::unique("cutting-hide-toolpaths")
}

/// Whether the cutting view draws the job's toolpaths over the mat (the Cutting panel's switch).
pub(crate) fn shows_toolpaths(ctx: &egui::Context) -> bool {
    !ctx.data(|d| d.get_temp::<bool>(toolpaths_key())).unwrap_or(false)
}

pub(crate) fn set_shows_toolpaths(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(toolpaths_key(), !on));
}
