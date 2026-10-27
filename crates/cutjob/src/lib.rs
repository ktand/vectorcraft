//! The cutting workspace's machine-independent model (fork: `docs/cutting/ARCHITECTURE.md`).
//!
//! - [`CuttingSettings`]: what a document keeps for cutting (`Document::unknown["cutting"]`): the
//!   mat, where the artboard goes on it, the operation new objects get, flattening tolerance and
//!   each operation's tool settings. Read leniently from untrusted JSON: what is missing, damaged
//!   or not understood takes its default, out-of-range numbers are clamped, and unknown keys are
//!   kept and written back.
//! - [`MatGeometry`] and [`MAT_PRESETS`]: the mats a job is laid out on.
//!
//! Lengths are millimetres throughout. The crate depends on no other workspace crate, so the
//! cutting core and the device drivers share it without depending on each other.
#![forbid(unsafe_code)]

mod mat;
mod read;
mod settings;

pub use mat::{MAT_PRESETS, MatGeometry, RectMm};
pub use settings::{CuttingSettings, DOCUMENT_KEY, Op, Operations, Ordering, Orientation, SETTINGS_VERSION, ToolSettings};
