//! The Silhouette cutters' driver (fork: `docs/cutting/ARCHITECTURE.md` › 6), written from
//! [`docs/cutting/protocol-silhouette.md`](../../../docs/cutting/protocol-silhouette.md), never
//! from another implementation's code. No I/O: it turns a cut job into the machine's commands and
//! reads its answers; `vectorcraft-cutter`'s sender does the sending.
//!
//! A job is: initialise; the mat; for each operation its tool settings (holder, force, speed,
//! blade offset), then each of its paths as a move to the start with the blade up and cuts along
//! it with the blade down, once per pass (a closed path round and on by the overcut); then the
//! head moved past the work and the end of the job. Commands are ASCII ending in ETX; positions
//! are whole machine units, 20 per millimetre, the vertical value first. Facts not yet checked on
//! a machine are marked "to confirm" in the protocol notes.

#![forbid(unsafe_code)]

use vectorcraft_cutjob::{CutJob, ToolSettings};
use vectorcraft_cutter::{DeviceInfo, DeviceStatus, Driver, Encoded};

/// Graphtec's USB vendor id, which Silhouette's machines use.
pub const VENDOR_ID: u16 = 0x0B4D;
/// Machine units per millimetre.
pub const UNITS_PER_MM: f64 = 20.0;
/// Most bytes in one chunk.
pub const CHUNK_BYTES: usize = 4_096;

const ESC: u8 = 0x1B;
const EOT: u8 = 0x04;
const ENQ: u8 = 0x05;
const ETX: u8 = 0x03;

/// The machines this driver knows: USB product id, name, cut width (mm).
pub const MODELS: [(u16, &str, f64); 6] = [
    (0x112F, "Cameo 3", 304.8),
    (0x1137, "Cameo 4", 304.8),
    (0x1138, "Cameo 4 Plus", 372.0),
    (0x1139, "Cameo 4 Pro", 600.0),
    (0x1140, "Cameo 5", 330.2),
    (0x1141, "Cameo 5 Plus", 372.0),
];

/// The usual blade's offset from its axis, mm: the machine turns the blade round corners for
/// it, so the job's paths are where the tip cuts.
pub const BLADE_OFFSET_MM: f64 = 0.9;

/// A blade offset (mm, `None`: the usual blade's) as the machine's units for `FC`.
fn blade_offset_units(mm: Option<f64>) -> u32 {
    let mm = mm.filter(|m| m.is_finite()).unwrap_or(BLADE_OFFSET_MM).clamp(0.0, 2.0);
    // At most 40 after the clamp, so the conversion is exact.
    (mm * 20.0).round() as u32
}
/// Force steps.
pub const FORCE: (u32, u32) = (1, 33);
/// Speed steps, and the mm/s one step is taken for until it is measured on a machine.
pub const SPEED: (u32, u32) = (1, 30);
pub const MM_S_PER_SPEED_STEP: f64 = 10.0;

/// The machine's force step for force `f` (0–1 of its range).
pub fn force_step(f: f64) -> u32 {
    let span = f64::from(FORCE.1 - FORCE.0);
    let step = f64::from(FORCE.0) + (if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 } * span).round();
    (step as u32).clamp(FORCE.0, FORCE.1)
}

/// Force (0–1) for the machine's force `step`: what [`force_step`] reads back as `step`.
pub fn force_from_step(step: u32) -> f64 {
    f64::from(step.clamp(FORCE.0, FORCE.1) - FORCE.0) / f64::from(FORCE.1 - FORCE.0)
}

/// The machine's speed step for `mm_s` (approximate until measured: [`MM_S_PER_SPEED_STEP`]).
pub fn speed_step(mm_s: f64) -> u32 {
    let step = (mm_s / MM_S_PER_SPEED_STEP).round();
    if step.is_finite() { step.clamp(f64::from(SPEED.0), f64::from(SPEED.1)) as u32 } else { SPEED.0 }
}

/// Speed, mm/s, for the machine's speed `step`: what [`speed_step`] reads back as `step`.
pub fn speed_from_step(step: u32) -> f64 {
    f64::from(step.clamp(SPEED.0, SPEED.1)) * MM_S_PER_SPEED_STEP
}

/// The machine's code for a mat (0: none, the art taped down or on a carrier the machine doesn't know).
fn mat_code(id: &str) -> Option<u32> {
    match id {
        "silhouette.12x12" => Some(1),
        "silhouette.12x24" => Some(2),
        _ => None,
    }
}

/// The model `device` is, if this driver knows it.
fn model(device: &DeviceInfo) -> Option<&'static (u16, &'static str, f64)> {
    (device.vendor_id == VENDOR_ID).then(|| MODELS.iter().find(|m| m.0 == device.product_id)).flatten()
}

/// `mm` as machine units.
fn units(mm: f64) -> i64 {
    (mm * UNITS_PER_MM).round() as i64
}

/// The commands a job is made of, before they are packed into chunks.
#[derive(Default)]
struct Commands {
    list: Vec<Vec<u8>>,
    /// Before the command at each index the sender pauses, for the group named (C5.3).
    pauses: Vec<(usize, String)>,
}

impl Commands {
    /// The sender pauses here, before group `name`.
    fn pause(&mut self, name: &str) {
        self.pauses.push((self.list.len(), name.to_string()));
    }

    fn raw(&mut self, bytes: &[u8]) {
        self.list.push(bytes.to_vec());
    }

    /// `text` and ETX.
    fn cmd(&mut self, text: &str) {
        let mut b = text.as_bytes().to_vec();
        b.push(ETX);
        self.list.push(b);
    }

    /// A move (blade up) or a cut (blade down) to mat point `p`, mm: vertical first.
    fn to(&mut self, down: bool, p: [f64; 2]) {
        self.cmd(&format!("{}{},{}", if down { 'D' } else { 'M' }, units(p[1]), units(p[0])));
    }

    /// Packed into chunks of at most [`CHUNK_BYTES`], split only between commands and at each
    /// pause, which becomes "before chunk n".
    fn chunks(self) -> (Vec<Vec<u8>>, Vec<(usize, String)>) {
        let mut out: Vec<Vec<u8>> = vec![];
        let mut pauses = vec![];
        let mut cur: Vec<u8> = vec![];
        let mut next_pause = self.pauses.into_iter().peekable();
        for (i, c) in self.list.into_iter().enumerate() {
            while let Some((_, name)) = next_pause.next_if(|(at, _)| *at == i) {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                pauses.push((out.len(), name));
            }
            if !cur.is_empty() && cur.len() + c.len() > CHUNK_BYTES {
                out.push(std::mem::take(&mut cur));
            }
            cur.extend(c);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        (out, pauses)
    }
}

/// The machine's force and speed steps for `tool`.
fn steps(tool: &ToolSettings) -> (u32, u32) {
    (force_step(tool.force), speed_step(tool.speed_mm_s))
}

/// The points along closed `points` from its start for `mm` past it (the overcut).
fn overcut(points: &[[f64; 2]], mm: f64) -> Vec<[f64; 2]> {
    let mut out = vec![];
    let ring: Vec<[f64; 2]> = points.iter().chain(points.first()).copied().collect();
    let mut left = mm;
    for w in ring.windows(2) {
        if left <= 0.0 {
            break;
        }
        let (a, b) = (w[0], w[1]);
        let seg = (b[0] - a[0]).hypot(b[1] - a[1]);
        if seg <= 0.0 {
            continue;
        }
        if left < seg {
            let t = left / seg;
            out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
            break;
        }
        out.push(b);
        left -= seg;
    }
    out
}

/// The Silhouette driver.
#[derive(Debug, Default)]
pub struct Silhouette;

impl Silhouette {
    /// `job` as commands for a machine of `width_mm` cut width.
    pub fn encode_for(&self, job: &CutJob, width_mm: f64) -> Result<Encoded, String> {
        job.validate().map_err(|e| e.to_string())?;
        let area = job.mat.cut_area_mm;
        if area.x1 > width_mm + 1e-6 {
            return Err(format!("the job is {:.1} mm wide; this cutter cuts {width_mm:.1} mm", area.x1));
        }
        let mut warnings = vec![];
        let mut c = Commands::default();
        c.raw(&[ESC, EOT]);
        match mat_code(&job.mat.id) {
            Some(code) => c.cmd(&format!("TG{code}")),
            None => {
                c.cmd("TG0");
                warnings.push(format!("the {} mat isn't one the cutter knows: it is told there is no mat", job.mat.id));
            }
        }
        c.cmd("FN0");
        c.cmd("TB50,0");
        if job.track_enhancing {
            // Which of FY0 and FY1 turns it on is to confirm on the machine (protocol notes).
            c.cmd("FY1");
        }
        let mut lowest = 0.0f64;
        // Group by group (one without grouping), a pause before each after the first; in each,
        // every operation's settings again (the tool may have been changed) and its paths.
        let starts: Vec<(usize, &str)> =
            if job.groups.is_empty() { vec![(0, "")] } else { job.groups.iter().map(|g| (g.start, g.name.as_str())).collect() };
        for (gi, &(start, name)) in starts.iter().enumerate() {
            let end = starts.get(gi + 1).map_or(job.paths.len(), |n| n.0);
            let in_group = job.paths.get(start..end).unwrap_or_default();
            if gi > 0 {
                c.pause(name);
            }
            for (op_index, op) in job.operations.iter().enumerate() {
                if !in_group.iter().any(|p| p.op == op_index) {
                    continue;
                }
                let (force, speed) = steps(&op.tool);
                c.cmd("J1");
                c.cmd(&format!("FX{force}"));
                c.cmd(&format!("!{speed}"));
                c.cmd(&format!("FC{}", blade_offset_units(op.tool.blade_offset_mm)));
                if let Some(depth) = op.tool.depth {
                    c.cmd(&format!("TF{depth}"));
                }
                if let Some(accel) = op.tool.accel {
                    c.cmd(&format!("TJ{accel}"));
                }
                for p in in_group.iter().filter(|p| p.op == op_index) {
                    let Some(&first) = p.points.first() else { continue };
                    for _ in 0..op.tool.passes.max(1) {
                        c.to(false, first);
                        for &q in p.points.iter().skip(1) {
                            c.to(true, q);
                        }
                        if p.closed {
                            c.to(true, first);
                            for q in overcut(&p.points, op.tool.overcut_mm) {
                                c.to(true, q);
                            }
                        }
                    }
                    lowest = p.points.iter().fold(lowest, |m, q| m.max(q[1]));
                }
            }
        }
        if !job.operations.is_empty() {
            warnings.push(format!("speeds are approximate until measured on the machine: one step is taken as {MM_S_PER_SPEED_STEP} mm/s"));
        }
        // The head out of the way, below the work, then the end of the job.
        c.to(false, [0.0, lowest.max(area.y0)]);
        c.cmd("SO0");
        let (chunks, pauses) = c.chunks();
        Ok(Encoded { chunks, warnings, pauses })
    }
}

impl Driver for Silhouette {
    fn name(&self) -> &'static str {
        "Silhouette"
    }

    fn drives(&self, device: &DeviceInfo) -> bool {
        model(device).is_some()
    }

    fn model(&self, device: &DeviceInfo) -> String {
        model(device).map_or_else(|| format!("Silhouette {:04x}", device.product_id), |m| m.1.to_string())
    }

    fn encode(&self, job: &CutJob, device: &DeviceInfo) -> Result<Encoded, String> {
        let width = model(device).map_or(304.8, |m| m.2);
        self.encode_for(job, width)
    }

    fn status_query(&self) -> &[u8] {
        &[ESC, ENQ]
    }

    fn parse_status(&self, reply: &[u8]) -> DeviceStatus {
        match reply.iter().copied().find(|b| *b != ETX && !b.is_ascii_whitespace()) {
            Some(b'0') => DeviceStatus::Ready,
            Some(b'1') => DeviceStatus::Moving,
            Some(b'2') => DeviceStatus::Unloaded,
            _ => DeviceStatus::Unknown,
        }
    }
}

/// A Cameo 4 that isn't there (C4.8), for trying the app's sending without a machine: it answers
/// ready, or moving for a while after each part of the job (two seconds and a millisecond a byte,
/// slower than nothing and quicker than a real cut), always with a mat loaded.
#[cfg(not(target_arch = "wasm32"))]
pub fn simulated_cameo4() -> vectorcraft_cutter::simulated::SimulatedMachine {
    use std::time::Duration;
    use vectorcraft_cutter::simulated::{Behaviour, SIMULATED_ID_PREFIX, Sent, SimulatedMachine};
    SimulatedMachine {
        info: DeviceInfo { id: format!("{SIMULATED_ID_PREFIX}cameo4"), vendor_id: VENDOR_ID, product_id: 0x1137, name: "Simulated cutter".into() },
        behaviour: Behaviour {
            query: vec![ESC, ENQ],
            ready: vec![b'0', ETX],
            moving: vec![b'1', ETX],
            busy_per_write: Duration::from_secs(2),
            busy_per_byte: Duration::from_millis(1),
        },
        sent: Sent::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_and_speed_steps() {
        let tool = |force, speed_mm_s| ToolSettings { force, speed_mm_s, passes: 1, overcut_mm: 0.0, ..ToolSettings::default() };
        assert_eq!(steps(&tool(0.0, 1.0)), (1, 1));
        assert_eq!(steps(&tool(1.0, 2000.0)), (33, 30));
        assert_eq!(steps(&tool(0.5, 100.0)), (17, 10));
        assert_eq!(steps(&tool(f64::NAN, f64::NAN)), (1, 1));
    }

    #[test]
    fn status_answers() {
        let s = Silhouette;
        assert_eq!(s.parse_status(b"0\x03"), DeviceStatus::Ready);
        assert_eq!(s.parse_status(b"1\x03"), DeviceStatus::Moving);
        assert_eq!(s.parse_status(b"2\x03"), DeviceStatus::Unloaded);
        assert_eq!(s.parse_status(b""), DeviceStatus::Unknown);
        assert_eq!(s.parse_status(b"x"), DeviceStatus::Unknown);
        assert_eq!(s.status_query(), &[0x1B, 0x05]);
    }

    #[test]
    fn known_machines() {
        let dev = |vendor_id, product_id| DeviceInfo { id: "x".into(), vendor_id, product_id, name: String::new() };
        let s = Silhouette;
        assert!(s.drives(&dev(0x0B4D, 0x1137)));
        assert_eq!(s.model(&dev(0x0B4D, 0x1137)), "Cameo 4");
        assert!(!s.drives(&dev(0x0B4D, 0x9999)));
        assert!(!s.drives(&dev(0x1234, 0x1137)));
    }

    #[test]
    fn the_overcut_runs_round_the_start() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert_eq!(overcut(&sq, 2.5), [[2.5, 0.0]]);
        assert_eq!(overcut(&sq, 12.0), [[10.0, 0.0], [10.0, 2.0]]);
        assert!(overcut(&sq, 0.0).is_empty());
    }
}
