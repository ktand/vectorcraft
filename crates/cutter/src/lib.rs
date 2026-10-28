//! Cutting machines (fork: `docs/cutting/ARCHITECTURE.md` › 6). Machine-specific code lives
//! here and below, never in the cutting core; nothing here does I/O (the app supplies the
//! transport to a real machine).
//!
//! - [`device`]: what a machine is to the app: a [`Transport`] (bytes out, bytes back), found and
//!   opened by a [`DeviceProvider`], spoken to by a [`Driver`] for its family.
//! - [`registry`]: the provider the app installs and the drivers this version has, per process.
//! - [`send`]: an encoded job sent chunk by chunk once the machine is ready, with progress and
//!   cancel, on a thread of its own for the app.
//! - [`MemoryTransport`]: a transport that records and answers from a script, for tests.
//! - [`simulated`]: a machine that isn't there, busy a while for each part of a job, for trying
//!   the app's sending without a cutter; and [`simulated::Providers`], providers listed as one.
//! - [`VirtualCutter`]: a machine that cuts nothing and records what it would have: each stroke
//!   of the blade, down, in order. It plays a job through its [`Simulation`], so what it records
//!   is what the preview plays back, and it is what tests use in place of hardware.
#![forbid(unsafe_code)]

pub mod device;
mod memory;
pub mod registry;
pub mod send;
#[cfg(not(target_arch = "wasm32"))]
pub mod simulated;

pub use device::{DeviceInfo, DeviceProvider, DeviceStatus, Driver, Encoded, Transport, TransportError};
pub use memory::{MemoryTransport, Recorded};

use vectorcraft_cutjob::{CutJob, JobError, OpKind, Simulation};

/// Ids of simulated machines ([`simulated`]) start with this, so the app can say which one isn't
/// real (here, not there, so it is known where they aren't built).
pub const SIMULATED_ID_PREFIX: &str = "simulated:";

/// One stroke of the blade, down from its first point to its last.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// The job's path it cuts.
    pub path: usize,
    pub pass: u8,
    pub op: Option<OpKind>,
    pub points: Vec<[f64; 2]>,
}

/// What a run of the virtual cutter did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recording {
    pub strokes: Vec<Stroke>,
    /// Travelled with the blade up, mm.
    pub travel_mm: f64,
    /// Cut with the blade down, mm.
    pub cut_mm: f64,
    pub seconds: f64,
}

/// A cutter that records instead of cutting.
#[derive(Debug, Default)]
pub struct VirtualCutter {
    last: Option<Recording>,
}

impl VirtualCutter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `job`: refused, as a real machine's driver would refuse it, unless it validates.
    pub fn run(&mut self, job: &CutJob) -> Result<&Recording, JobError> {
        job.validate()?;
        let sim = Simulation::new(job);
        let mut rec = Recording { seconds: sim.total_seconds, ..Recording::default() };
        let mut stroke: Option<Stroke> = None;
        for m in &sim.moves {
            if !m.down {
                rec.travel_mm += m.mm;
                rec.strokes.extend(stroke.take());
                continue;
            }
            rec.cut_mm += m.mm;
            let path = m.path.unwrap_or(0);
            match &mut stroke {
                Some(s) if s.path == path && s.pass == m.pass && s.points.last() == Some(&m.from) => s.points.push(m.to),
                _ => {
                    rec.strokes.extend(stroke.take());
                    let op = job.paths.get(path).and_then(|p| job.operations.get(p.op)).map(|o| o.kind);
                    stroke = Some(Stroke { path, pass: m.pass, op, points: vec![m.from, m.to] });
                }
            }
        }
        rec.strokes.extend(stroke);
        Ok(self.last.insert(rec))
    }

    /// What the last run recorded.
    pub fn last(&self) -> Option<&Recording> {
        self.last.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use vectorcraft_cutjob::{JOB_VERSION, JobMat, Operation, RectMm, SourceRef, ToolSettings, Toolpath};

    use super::*;

    fn tool(passes: u8) -> ToolSettings {
        ToolSettings { force: 0.3, speed_mm_s: 100.0, passes, overcut_mm: 0.0, ..ToolSettings::default() }
    }

    fn job() -> CutJob {
        let path = |op, closed, points: &[[f64; 2]]| Toolpath { op, source: SourceRef::default(), closed, points: points.to_vec() };
        CutJob {
            version: JOB_VERSION,
            mat: JobMat { id: "t".into(), size_mm: [300.0, 300.0], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 } },
            operations: vec![Operation { kind: OpKind::Score, tool: tool(1) }, Operation { kind: OpKind::Cut, tool: tool(2) }],
            paths: vec![
                path(0, false, &[[50.0, 10.0], [70.0, 10.0], [70.0, 30.0]]),
                path(1, true, &[[10.0, 10.0], [20.0, 10.0], [20.0, 20.0], [10.0, 20.0]]),
            ],
            warnings: vec![],
            track_enhancing: false,
            groups: vec![],
        }
    }

    /// The recording is the job: each path, in order, once per pass, a closed one back to its start.
    #[test]
    fn the_recording_replays_the_job() {
        let j = job();
        let mut cutter = VirtualCutter::new();
        let rec = cutter.run(&j).unwrap().clone();
        let mut want = vec![];
        for (i, p) in j.paths.iter().enumerate() {
            let passes = j.operations[p.op].tool.passes;
            let mut pts = p.points.clone();
            if p.closed {
                pts.push(pts[0]);
            }
            for pass in 0..passes {
                want.push(Stroke { path: i, pass, op: Some(j.operations[p.op].kind), points: pts.clone() });
            }
        }
        assert_eq!(rec.strokes, want);
        assert!((rec.cut_mm - (40.0 + 40.0 + 40.0)).abs() < 1e-9);
        assert_eq!(cutter.last(), Some(&rec));
    }

    #[test]
    fn a_job_that_doesnt_validate_is_refused() {
        let mut j = job();
        j.paths[0].points[1] = [400.0, 10.0];
        let mut cutter = VirtualCutter::new();
        assert!(matches!(cutter.run(&j), Err(JobError::OutsideCutArea { path: 0, .. })));
        assert_eq!(cutter.last(), None);
    }
}
