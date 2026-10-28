//! The cut job: what to cut, in which order, with which settings, in mat millimetres
//! (ARCHITECTURE.md › 5). Made from a document by `vectorcraft-cutting`, read by drivers and the
//! simulator, and checked by [`CutJob::validate`] before either sees it.

use serde_json::{Value, json};

use crate::mat::RectMm;
use crate::settings::ToolSettings;

/// The job schema this version writes.
pub const JOB_VERSION: u32 = 1;
/// Most paths one job holds.
pub const MAX_PATHS: usize = 200_000;
/// Most points all of a job's paths hold together.
pub const MAX_POINTS: usize = 5_000_000;
/// A point this far (mm) outside the cut area still counts as on it: rounding, not an overhang.
const AREA_SLACK_MM: f64 = 1e-6;

/// What an operation does to the material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpKind {
    /// Score: part way through, to fold along. Scored before anything is cut free.
    Score,
    /// Through the material.
    Cut,
}

impl OpKind {
    pub fn id(self) -> &'static str {
        match self {
            OpKind::Score => "score",
            OpKind::Cut => "cut",
        }
    }
}

/// One operation of the job: what it does and how it drives the tool.
#[derive(Clone, Debug, PartialEq)]
pub struct Operation {
    pub kind: OpKind,
    pub tool: ToolSettings,
}

/// The mat the job is laid out on.
#[derive(Clone, Debug, PartialEq)]
pub struct JobMat {
    pub id: String,
    /// The mat, width × height, mm.
    pub size_mm: [f64; 2],
    /// Where the blade may go, in mat coordinates.
    pub cut_area_mm: RectMm,
}

/// Where a toolpath comes from in the document: for the preview's hover and for messages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceRef {
    /// The object's id (`NodeId`).
    pub node: u64,
    /// Which of its subpaths.
    pub subpath: u32,
}

/// One stroke of the blade: down at the first point, through the rest, up at the end (a closed
/// path back at the first point first).
#[derive(Clone, Debug, PartialEq)]
pub struct Toolpath {
    /// Index into [`CutJob::operations`].
    pub op: usize,
    pub source: SourceRef,
    pub closed: bool,
    /// Mat millimetres: origin at the mat's top-left, +x right, +y toward the user.
    pub points: Vec<[f64; 2]>,
}

/// Something a job leaves out or approximates, for the user.
#[derive(Clone, Debug, PartialEq)]
pub struct JobWarning {
    pub source: Option<SourceRef>,
    pub message: String,
}

/// A device-independent cut job. Paths are in execution order.
#[derive(Clone, Debug, PartialEq)]
pub struct CutJob {
    pub version: u32,
    pub mat: JobMat,
    pub operations: Vec<Operation>,
    pub paths: Vec<Toolpath>,
    pub warnings: Vec<JobWarning>,
    /// The machine grips the material harder as it feeds (where it can).
    pub track_enhancing: bool,
    /// Cut in groups, a pause before each after the first; none: one group.
    pub groups: Vec<JobGroup>,
}

/// One group of a job cut in groups (C5.3): its name, and where its paths start.
#[derive(Clone, Debug, PartialEq)]
pub struct JobGroup {
    pub name: String,
    /// Index of its first path; its paths run to the next group's start.
    pub start: usize,
}

/// Why a job can't be run.
#[derive(Clone, Debug, PartialEq)]
pub enum JobError {
    TooManyPaths(usize),
    TooManyPoints(usize),
    /// The mat's size or cut area isn't a finite, non-empty rectangle on the mat.
    BadMat,
    /// Operation `op`'s setting is out of range or not finite.
    BadSetting {
        op: usize,
        setting: &'static str,
    },
    /// Path `path` names an operation the job doesn't have.
    NoSuchOperation {
        path: usize,
    },
    /// Path `path` has too few points to cut (two for an open path, three for a closed one).
    TooFewPoints {
        path: usize,
    },
    /// Path `path` has a point that isn't finite.
    NotFinite {
        path: usize,
    },
    /// Path `path` leaves the cut area at `point`.
    OutsideCutArea {
        path: usize,
        point: [f64; 2],
    },
    /// The groups don't start at the first path, in order, within the job.
    BadGroups,
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobError::TooManyPaths(n) => write!(f, "{n} paths: a job holds at most {MAX_PATHS}"),
            JobError::TooManyPoints(n) => write!(f, "{n} points: a job holds at most {MAX_POINTS}"),
            JobError::BadMat => write!(f, "the mat's size or cut area isn't valid"),
            JobError::BadSetting { op, setting } => write!(f, "operation {op}: {setting} is out of range"),
            JobError::NoSuchOperation { path } => write!(f, "path {path} names an operation the job doesn't have"),
            JobError::TooFewPoints { path } => write!(f, "path {path} has too few points to cut"),
            JobError::NotFinite { path } => write!(f, "path {path} has a point that isn't a finite number"),
            JobError::OutsideCutArea { path, point } => write!(f, "path {path} leaves the cut area at ({:.2}, {:.2}) mm", point[0], point[1]),
            JobError::BadGroups => write!(f, "the job's groups aren't in order"),
        }
    }
}

impl std::error::Error for JobError {}

/// What a job adds up to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JobStats {
    pub path_count: usize,
    pub point_count: usize,
    /// The paths' length, mm (a closed path's closing segment included).
    pub path_mm: f64,
    /// What the blade travels down, mm: each path's length times its operation's passes.
    pub blade_mm: f64,
    /// What the blade travels up, mm: from the cut area's origin to the first path, and from each
    /// path's end to the next one's start.
    pub travel_mm: f64,
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

impl Toolpath {
    /// Where the blade lifts: the last point, or the first for a closed path.
    pub fn end(&self) -> Option<[f64; 2]> {
        if self.closed { self.points.first().copied() } else { self.points.last().copied() }
    }

    /// The path's length, mm (a closed one back to its first point).
    pub fn length_mm(&self) -> f64 {
        let open: f64 = self
            .points
            .windows(2)
            .map(|w| match w {
                [a, b] => dist(*a, *b),
                _ => 0.0,
            })
            .sum();
        let closing = match (self.closed, self.points.first(), self.points.last()) {
            (true, Some(a), Some(b)) => dist(*a, *b),
            _ => 0.0,
        };
        open + closing
    }
}

impl CutJob {
    /// The job can be run: within the caps, its settings in range, and every path finite,
    /// long enough, naming an operation it has and inside the cut area. Never panics.
    pub fn validate(&self) -> Result<(), JobError> {
        if self.paths.len() > MAX_PATHS {
            return Err(JobError::TooManyPaths(self.paths.len()));
        }
        let points = self.paths.iter().fold(0usize, |n, p| n.saturating_add(p.points.len()));
        if points > MAX_POINTS {
            return Err(JobError::TooManyPoints(points));
        }
        let a = self.mat.cut_area_mm;
        let [w, h] = self.mat.size_mm;
        let finite = [a.x0, a.y0, a.x1, a.y1, w, h].iter().all(|v| v.is_finite());
        if !finite || a.x0 < 0.0 || a.y0 < 0.0 || a.x1 > w || a.y1 > h || a.width() <= 0.0 || a.height() <= 0.0 {
            return Err(JobError::BadMat);
        }
        let starts_ok = self.groups.first().is_none_or(|g| g.start == 0)
            && self.groups.windows(2).all(|w| w[0].start < w[1].start)
            && self.groups.last().is_none_or(|g| g.start < self.paths.len().max(1));
        if !starts_ok || self.groups.len() > MAX_PATHS {
            return Err(JobError::BadGroups);
        }
        for (op, o) in self.operations.iter().enumerate() {
            if let Some(setting) = o.tool.problem() {
                return Err(JobError::BadSetting { op, setting });
            }
        }
        for (i, p) in self.paths.iter().enumerate() {
            if p.op >= self.operations.len() {
                return Err(JobError::NoSuchOperation { path: i });
            }
            if p.points.len() < if p.closed { 3 } else { 2 } {
                return Err(JobError::TooFewPoints { path: i });
            }
            for &[x, y] in &p.points {
                if !x.is_finite() || !y.is_finite() {
                    return Err(JobError::NotFinite { path: i });
                }
                let s = AREA_SLACK_MM;
                if x < a.x0 - s || y < a.y0 - s || x > a.x1 + s || y > a.y1 + s {
                    return Err(JobError::OutsideCutArea { path: i, point: [x, y] });
                }
            }
        }
        Ok(())
    }

    /// Counts and lengths. Meaningful for a job that [`CutJob::validate`]s; never panics on one
    /// that doesn't.
    pub fn stats(&self) -> JobStats {
        let mut s = JobStats { path_count: self.paths.len(), ..JobStats::default() };
        let mut at = [self.mat.cut_area_mm.x0, self.mat.cut_area_mm.y0];
        for p in &self.paths {
            s.point_count = s.point_count.saturating_add(p.points.len());
            let len = p.length_mm();
            s.path_mm += len;
            let passes = self.operations.get(p.op).map_or(1, |o| o.tool.passes);
            s.blade_mm += len * f64::from(passes);
            if let (Some(start), Some(end)) = (p.points.first(), p.end()) {
                s.travel_mm += dist(at, *start);
                at = end;
            }
        }
        s
    }

    /// The job as JSON, for `cutting.job`, previews and golden tests.
    pub fn to_value(&self) -> Value {
        let a = self.mat.cut_area_mm;
        let source = |s: &SourceRef| json!({"node": s.node, "subpath": s.subpath});
        json!({
            "version": self.version,
            "trackEnhancing": self.track_enhancing,
            "groups": self.groups.iter().map(|g| json!({"name": g.name, "start": g.start})).collect::<Vec<_>>(),
            "mat": {"id": self.mat.id, "sizeMm": self.mat.size_mm, "cutAreaMm": [a.x0, a.y0, a.x1, a.y1]},
            "operations": self.operations.iter().map(|o| json!({
                "kind": o.kind.id(),
                "force": o.tool.force,
                "speedMmS": o.tool.speed_mm_s,
                "passes": o.tool.passes,
                "overcutMm": o.tool.overcut_mm,
                "depth": o.tool.depth,
                "accel": o.tool.accel,
                "cornerOvercutMm": o.tool.corner_overcut_mm,
                "bladeOffsetMm": o.tool.blade_offset_mm,
            })).collect::<Vec<_>>(),
            "paths": self.paths.iter().map(|p| json!({
                "op": p.op,
                "source": source(&p.source),
                "closed": p.closed,
                "points": p.points,
            })).collect::<Vec<_>>(),
            "warnings": self.warnings.iter().map(|w| json!({
                "source": w.source.as_ref().map(source),
                "message": w.message,
            })).collect::<Vec<_>>(),
        })
    }
}

impl JobStats {
    pub fn to_value(&self) -> Value {
        json!({
            "pathCount": self.path_count,
            "pointCount": self.point_count,
            "pathMm": self.path_mm,
            "bladeMm": self.blade_mm,
            "travelMm": self.travel_mm,
        })
    }
}

#[cfg(test)]
mod tests;
