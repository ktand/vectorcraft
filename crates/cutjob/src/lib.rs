//! The cutting workspace's machine-independent model (fork: `docs/cutting/ARCHITECTURE.md`).
//!
//! - [`CuttingSettings`]: what a document keeps for cutting (`Document::unknown["cutting"]`): the
//!   mat, where the artboard goes on it, the operation new objects get, flattening tolerance and
//!   each operation's tool settings. Read leniently from untrusted JSON: what is missing, damaged
//!   or not understood takes its default, out-of-range numbers are clamped, and unknown keys are
//!   kept and written back.
//! - [`MatGeometry`] and [`MAT_PRESETS`]: the mats a job is laid out on.
//! - [`CutJob`]: what to cut, in which order, with which settings, in mat mm;
//!   [`CutJob::validate`] is the gate before a driver or the simulator sees one.
//!
//! Lengths are millimetres throughout. The crate depends on no other workspace crate, so the
//! cutting core and the device drivers share it without depending on each other.
#![forbid(unsafe_code)]

mod job;
pub mod marks;
mod mat;
mod material;
mod read;
mod settings;
mod simulate;

pub use job::{
    CutJob, JOB_VERSION, JobError, JobGroup, JobMat, JobRegistration, JobStats, JobWarning, MAX_PATHS, MAX_POINTS, OpKind, Operation, SourceRef,
    Toolpath,
};
pub use marks::{Mark, MarkLayout};
pub use mat::{MAT_PRESETS, MatGeometry, RectMm};
pub use material::{MAX_MATERIALS, MAX_NAME, Material, builtin_materials, read_materials, valid_name};
pub use settings::{CuttingSettings, DOCUMENT_KEY, Grouping, Op, Operations, Ordering, Orientation, SETTINGS_VERSION, ToolSettings};
pub use simulate::{BladeAt, Move, Simulation, TRAVEL_MM_S};
