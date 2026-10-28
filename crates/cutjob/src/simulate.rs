//! A job as the blade's moves, in order: what the simulator plays back and the virtual cutter
//! records (ARCHITECTURE.md › 5). From the cut area's origin, the blade travels up to each path's
//! start, then cuts along it down, once for each pass of its operation; a closed path is cut
//! round and on past its start by its operation's overcut, an open one is travelled back to its
//! start between passes. Positions along the run are measured in millimetres of blade travel,
//! up and down together, and timed with each operation's speed (travel at [`TRAVEL_MM_S`]).

use crate::job::{CutJob, OpKind};

/// The speed the blade is assumed to travel at, up, for the time estimate (mm/s).
pub const TRAVEL_MM_S: f64 = 200.0;

/// One straight move of the blade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub from: [f64; 2],
    pub to: [f64; 2],
    /// Cutting (down) or travelling (up).
    pub down: bool,
    /// The path cut (an index into the job's paths), for a move down; the path travelled to, up.
    pub path: Option<usize>,
    /// Which pass of the path (from 0).
    pub pass: u8,
    /// Its length, mm.
    pub mm: f64,
    /// Its time, s.
    pub seconds: f64,
}

/// Where the blade is at a point of the run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BladeAt {
    pub position: [f64; 2],
    pub down: bool,
    pub path: Option<usize>,
    pub pass: u8,
    pub op: Option<OpKind>,
    /// How far along the run, mm and s.
    pub done_mm: f64,
    pub done_seconds: f64,
}

/// A job's moves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Simulation {
    pub moves: Vec<Move>,
    /// The kind of each of the job's paths' operations, by path.
    kinds: Vec<Option<OpKind>>,
    pub total_mm: f64,
    pub total_seconds: f64,
    /// The cut area's origin, where the blade starts.
    start: [f64; 2],
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The point `d` mm along `points` (closed: round to the first again and on), and the segments
/// up to it.
fn along(points: &[[f64; 2]], closed: bool, d: f64) -> Vec<[f64; 2]> {
    let mut out = vec![];
    let Some(&first) = points.first() else { return out };
    out.push(first);
    let ring: Vec<[f64; 2]> = if closed { points.iter().chain(points.first()).copied().collect() } else { points.to_vec() };
    let perimeter: f64 = ring.windows(2).map(|w| dist(w[0], w[1])).sum();
    if d.is_nan() || d <= 0.0 || perimeter <= 0.0 {
        return out;
    }
    let mut left = d;
    // An overcut longer than the path goes round again; capped so a damaged value can't loop long.
    for _ in 0..16 {
        for w in ring.windows(2) {
            let seg = dist(w[0], w[1]);
            if seg <= 0.0 {
                continue;
            }
            if left <= seg {
                let t = left / seg;
                out.push([w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1])]);
                return out;
            }
            out.push(w[1]);
            left -= seg;
        }
        if !closed {
            return out;
        }
    }
    out
}

impl Simulation {
    /// The moves of `job` (meaningful for one that validates; never panics on one that doesn't).
    pub fn new(job: &CutJob) -> Self {
        let start = [job.mat.cut_area_mm.x0, job.mat.cut_area_mm.y0];
        let mut sim = Simulation { start, ..Simulation::default() };
        let mut at = start;
        let push = |sim: &mut Simulation, from: [f64; 2], to: [f64; 2], down: bool, path: usize, pass: u8, speed: f64| {
            let mm = dist(from, to);
            if mm <= 0.0 {
                return;
            }
            let seconds = mm / speed.max(f64::MIN_POSITIVE);
            sim.total_mm += mm;
            sim.total_seconds += seconds;
            sim.moves.push(Move { from, to, down, path: Some(path), pass, mm, seconds });
        };
        for (i, p) in job.paths.iter().enumerate() {
            let op = job.operations.get(p.op);
            sim.kinds.push(op.map(|o| o.kind));
            let Some(&first) = p.points.first() else { continue };
            let (passes, speed, overcut) = op.map_or((1, TRAVEL_MM_S, 0.0), |o| (o.tool.passes.max(1), o.tool.speed_mm_s, o.tool.overcut_mm));
            for pass in 0..passes {
                push(&mut sim, at, first, false, i, pass, TRAVEL_MM_S);
                let mut route: Vec<[f64; 2]> = p.points.clone();
                if p.closed {
                    route.push(first);
                    // On past the start by the overcut.
                    route.extend(along(&p.points, true, overcut).into_iter().skip(1));
                }
                for w in route.windows(2) {
                    push(&mut sim, w[0], w[1], true, i, pass, speed);
                }
                at = route.last().copied().unwrap_or(first);
            }
        }
        sim
    }

    /// Where the blade is `mm` along the run (clamped to it).
    pub fn at_mm(&self, mm: f64) -> BladeAt {
        let target = if mm.is_finite() { mm.clamp(0.0, self.total_mm) } else { 0.0 };
        let mut done = 0.0;
        let mut seconds = 0.0;
        let idle = BladeAt { position: self.start, down: false, path: None, pass: 0, op: None, done_mm: 0.0, done_seconds: 0.0 };
        let mut last = idle;
        for m in &self.moves {
            let op = m.path.and_then(|p| self.kinds.get(p).copied().flatten());
            if done + m.mm >= target && m.mm > 0.0 {
                let t = ((target - done) / m.mm).clamp(0.0, 1.0);
                let position = [m.from[0] + t * (m.to[0] - m.from[0]), m.from[1] + t * (m.to[1] - m.from[1])];
                return BladeAt { position, down: m.down, path: m.path, pass: m.pass, op, done_mm: target, done_seconds: seconds + t * m.seconds };
            }
            done += m.mm;
            seconds += m.seconds;
            last = BladeAt { position: m.to, down: false, path: m.path, pass: m.pass, op, done_mm: done, done_seconds: seconds };
        }
        last
    }

    /// Where the blade is at fraction `t` (0–1) of the run.
    pub fn at_fraction(&self, t: f64) -> BladeAt {
        self.at_mm(if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 } * self.total_mm)
    }

    /// The length cut, blade down, mm.
    pub fn down_mm(&self) -> f64 {
        self.moves.iter().filter(|m| m.down).map(|m| m.mm).sum()
    }

    /// How far along the run (mm) the blade first comes down on each path, in order: where
    /// playback steps to.
    pub fn path_starts_mm(&self) -> Vec<f64> {
        let mut out = vec![];
        let mut done = 0.0;
        let mut last = None;
        for m in &self.moves {
            if m.down && m.path != last {
                out.push(done);
                last = m.path;
            }
            done += m.mm;
        }
        out
    }
}

#[cfg(test)]
mod tests;
