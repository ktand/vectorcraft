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

/// The Cutting workspace's panels (C7.2): the dock's main column keeps Properties · Layers ·
/// Libraries (Layers in front: a cut by layer is set up there) and gains Cut Settings and Cutting
/// Mat below it, taken out of the icon column, so the settings show without opening anything.
pub(crate) fn workspace_layout() -> craft_ui::docking::Layout<String> {
    use craft_ui::docking::{Group, Node};
    let ui = crate::state::UiState { dock_tab: crate::state::DockTab::Layers, ..Default::default() };
    let mut layout = crate::panel_docking::default_layout(&ui);
    let ours = [crate::panels::cutting::ID, crate::panels::cutting::mat::ID];
    if let Some(Node::Split { second, .. }) = layout.root.as_mut()
        && let Node::Dock { columns, .. } = second.as_mut()
    {
        for column in columns.iter_mut() {
            for group in column.groups.iter_mut() {
                group.panels.retain(|p| !ours.contains(&p.as_str()));
                group.active = group.active.min(group.panels.len().saturating_sub(1));
            }
            column.groups.retain(|g| !g.panels.is_empty());
        }
        columns.retain(|c| !c.groups.is_empty());
        if let Some(main) = columns.first_mut() {
            if let Some(tabs) = main.groups.first_mut() {
                tabs.height = Some(LAYERS_HEIGHT);
            }
            main.groups.extend(ours.iter().map(|p| Group::new(vec![p.to_string()], 0)));
        }
    }
    layout
}

/// The height Layers starts at in the Cutting workspace, points.
const LAYERS_HEIGHT: f32 = 200.0;

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

/// Play the blade's run, step from path to path, or scrub along it (C3.7): the Send window's
/// controls under its preview, and (`compact`, one row) the bar over the canvas in Cutting View
/// (C7.3). Both move the one playback the canvas and the preview draw.
pub(crate) fn playback_controls(ui: &mut egui::Ui, sim: &Simulation, compact: bool) {
    let ctx = ui.ctx().clone();
    let mut p = playback(&ctx);
    let total = sim.total_mm;
    let at = p.mm.unwrap_or(total);
    let starts = sim.path_starts_mm();
    let buttons = |ui: &mut egui::Ui, p: &mut Playback| {
        let play = if p.playing { tl!("Pause") } else { tl!("Play") };
        if ui.button(play).clicked() {
            // Playing from the end starts again.
            let from = if at >= total { 0.0 } else { at };
            *p = Playback { mm: Some(from), playing: !p.playing, last: ctx.input(|i| i.time) };
        }
        if ui.button(tl!("Previous")).clicked() {
            let prev = starts.iter().rev().find(|s| **s < at - 1e-6).copied().unwrap_or(0.0);
            *p = Playback { mm: Some(prev), playing: false, ..*p };
        }
        if ui.button(tl!("Next")).clicked() {
            let next = starts.iter().find(|s| **s > at + 1e-6).copied().unwrap_or(total);
            *p = Playback { mm: Some(next), playing: false, ..*p };
        }
        if p.mm.is_some() && ui.button(tl!("Done")).clicked() {
            *p = Playback::default();
        }
    };
    let slider = |ui: &mut egui::Ui, p: &mut Playback, width: f32| {
        let mut v = p.mm.unwrap_or(total);
        ui.spacing_mut().slider_width = width;
        if crate::widgets::slider(ui, egui::Slider::new(&mut v, 0.0..=total.max(f64::MIN_POSITIVE)).show_value(false)).changed() {
            *p = Playback { mm: Some(v), playing: false, ..*p };
        }
    };
    if compact {
        ui.horizontal(|ui| {
            buttons(ui, &mut p);
            slider(ui, &mut p, 220.0);
            playback_label(ui, sim, &p);
        });
    } else {
        ui.horizontal(|ui| buttons(ui, &mut p));
        slider(ui, &mut p, ui.spacing().slider_width);
        playback_label(ui, sim, &p);
    }
    set_playback(&ctx, p);
}

/// How far the playback is: seconds of the machine's, and the path the blade is on.
fn playback_label(ui: &mut egui::Ui, sim: &Simulation, p: &Playback) {
    let blade = sim.at_mm(p.mm.unwrap_or(sim.total_mm));
    let path = blade.path.map_or(String::new(), |i| format!(" · {} {}", tl!("path"), i + 1));
    crate::widgets::dim_label(ui, &format!("{:.0} / {:.0} s{path} (×{SPEEDUP})", blade.done_seconds, sim.total_seconds));
}
