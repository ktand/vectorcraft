//! Fork: what the Cutting panel and the canvas share (`docs/cutting/ARCHITECTURE.md`). The cut
//! job (and the blade's run through it) is made from the active document again only when the
//! document changes (a new document value: an edit, an undo, a settings change), not each frame
//! and not when the selection does. It is kept in the egui context's memory, with the document
//! it was made from, as is the playback of the run (C3.7).

pub(crate) mod send_window;

use std::sync::Arc;

use vectorcraft_cutjob::{CutJob, JobStats, Simulation};
use vectorcraft_doc::Document;

use crate::VectorcraftApp;

/// A document's cut job, its stats and the blade's run through it.
pub(crate) struct Made {
    pub job: CutJob,
    pub stats: JobStats,
    pub sim: Simulation,
}

/// The job, or why the document makes none.
pub(crate) type JobResult = Result<Made, String>;

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
            .map(|job| {
                let stats = job.stats();
                let sim = Simulation::new(&job);
                Made { job, stats, sim }
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

/// Playback runs this many times faster than the machine is estimated to.
pub(crate) const SPEEDUP: f64 = 10.0;

/// The playback of the blade's run (the Cutting panel's controls; the canvas draws it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Playback {
    /// How far along the run, mm; `None`: not played back (the whole job shows).
    pub mm: Option<f64>,
    pub playing: bool,
    /// The frame time it was last moved on at (s).
    pub last: f64,
}

fn playback_key() -> egui::Id {
    egui::Id::unique("cutting-playback")
}

pub(crate) fn playback(ctx: &egui::Context) -> Playback {
    ctx.data(|d| d.get_temp::<Playback>(playback_key())).unwrap_or_default()
}

pub(crate) fn set_playback(ctx: &egui::Context, p: Playback) {
    ctx.data_mut(|d| d.insert_temp(playback_key(), p));
}

/// Moves a playing playback on by the time since the last frame, [`SPEEDUP`] times the
/// machine's estimated pace; it stops at the end. Asks for the next frame while it plays.
pub(crate) fn tick(ctx: &egui::Context, sim: &Simulation) -> Playback {
    let mut p = playback(ctx);
    let now = ctx.input(|i| i.time);
    if p.playing {
        let pace = if sim.total_seconds > 0.0 { sim.total_mm / sim.total_seconds } else { 0.0 };
        let dt = (now - p.last).clamp(0.0, 0.25);
        let mm = (p.mm.unwrap_or(0.0) + dt * pace * SPEEDUP).min(sim.total_mm);
        p.mm = Some(mm);
        if mm >= sim.total_mm {
            p.playing = false;
        }
        ctx.request_repaint();
    }
    p.last = now;
    set_playback(ctx, p);
    p
}
