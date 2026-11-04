//! Fork: the curve test (C4.9). Whether the machine cuts GP-GL's own curve commands as well as
//! the short straight moves sent today (protocol notes › Curves): a 10 mm circle and a 50 mm
//! S-curve, cut in three rows from the cut area's top-left, each row its own way:
//!
//! 1. `segments`: both as straight moves, as every job is sent today;
//! 2. `bezier`: both as Bézier pieces (`BZ`), the circle four, the S-curve two;
//! 3. `circle`: the circle as one circle command (`W`), the S-curve as straight moves again.
//!
//! One pass and no overcut in every row, so the rows differ only in how the curves are sent. A
//! row keeps its place when only it is cut, so rows cut on different runs don't overlap. The
//! test isn't kept in the job history: Send Again would cut it as the document's job.

use serde_json::{Value, json};
use vectorcraft_cutjob::{CutJob, JOB_VERSION, JobMat, OpKind, Operation, SourceRef, Toolpath};
use vectorcraft_cutter::send::Patience;
use vectorcraft_cutter::{Native, registry};

use super::{CommandSpec, Result, Session, bad, cmd, has_doc, str_param};

const CMD: &str = "cutting.testCurves";
const JOB_CMD: &str = "cutting.testCurves.job";

/// The rows, top first.
pub const ROWS: [&str; 3] = ["segments", "bezier", "circle"];
/// How far in from the cut area's corner the first row starts, mm.
const MARGIN_MM: f64 = 3.0;
/// The circle's radius, mm.
const RADIUS_MM: f64 = 5.0;
/// The S-curve's width and height, mm.
const S_MM: [f64; 2] = [50.0, 20.0];
/// Between the circle and the S-curve, mm.
const GAP_MM: f64 = 7.0;
/// From one row's top to the next one's, mm.
const ROW_MM: f64 = 25.0;
/// A quarter circle's handles as a share of its radius: 4/3 (√2 − 1).
const QUARTER: f64 = 0.552_284_749_830_793_4;
/// Straight moves along each Bézier piece of the S-curve when it is sent as points.
const PIECE_STEPS: usize = 64;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "cutting.testCurves",
            "Curve Test",
            [],
            None,
            "{only?: segments|bezier|circle (default all three rows), device?: an id from cutting.devices} cut the curve test (hardware checks step 18) with the document's cut tool: a 10 mm circle and a 50 mm S-curve in three rows from the cut area's top-left (straight moves; Bézier curves sent as BZ; the circle sent as W) with one pass and no overcut, in the background as cutting.send does; not kept in the job history → {device, model, chunks, rows, warnings}",
            has_doc,
            send
        ),
        cmd!(
            query "cutting.testCurves.job",
            "Curve Test Job",
            [],
            None,
            "{only?: segments|bezier|circle} the curve test's job without sending it → {job, native: [per path: bezier|circle or null for straight moves], rows}",
            has_doc,
            job_only
        ),
    ]
}

/// The rows `p` asks for.
fn rows(p: &Value, cmd: &str) -> Result<Vec<usize>> {
    match str_param(p, "only") {
        None => Ok((0..ROWS.len()).collect()),
        Some(id) => {
            let i = ROWS.iter().position(|r| *r == id).ok_or_else(|| bad(cmd, format!("only must be segments, bezier or circle, not {id:?}")))?;
            Ok(vec![i])
        }
    }
}

/// The point at angle `a` (radians) round `c`: angle 0 is down the mat from the centre, as the
/// machine's circle starts.
fn round(c: [f64; 2], r: f64, a: f64) -> [f64; 2] {
    [c[0] + r * a.sin(), c[1] + r * a.cos()]
}

/// The circle as straight moves no further than `tolerance` from it.
fn circle_points(c: [f64; 2], r: f64, tolerance: f64) -> Vec<[f64; 2]> {
    let t = if tolerance.is_finite() { tolerance.clamp(0.001, r / 2.0) } else { 0.025 };
    let step = 2.0 * (1.0 - t / r).acos();
    let n = if step > 0.0 { (std::f64::consts::TAU / step).ceil() as usize } else { 360 };
    let n = n.clamp(8, 1440);
    (0..n).map(|k| round(c, r, std::f64::consts::TAU * k as f64 / n as f64)).collect()
}

/// The circle as four Bézier quarters, from angle 0.
fn circle_beziers(c: [f64; 2], r: f64) -> Vec<[[f64; 2]; 4]> {
    let tangent = |a: f64| [r * a.cos(), -r * a.sin()];
    (0..4)
        .map(|k| {
            let (a0, a1) = (std::f64::consts::FRAC_PI_2 * k as f64, std::f64::consts::FRAC_PI_2 * (k + 1) as f64);
            let (p0, p3) = (round(c, r, a0), round(c, r, a1));
            let (t0, t1) = (tangent(a0), tangent(a1));
            [p0, [p0[0] + QUARTER * t0[0], p0[1] + QUARTER * t0[1]], [p3[0] - QUARTER * t1[0], p3[1] - QUARTER * t1[1]], p3]
        })
        .collect()
}

/// The S-curve from its top-left `o`: from the bottom-left corner up to the top-right in two
/// pieces, level at both ends and in the middle.
fn s_beziers(o: [f64; 2]) -> Vec<[[f64; 2]; 4]> {
    let [w, h] = S_MM;
    let (x, y) = (o[0], o[1]);
    vec![
        [[x, y + h], [x + w / 4.0, y + h], [x + w / 4.0, y + h / 2.0], [x + w / 2.0, y + h / 2.0]],
        [[x + w / 2.0, y + h / 2.0], [x + 3.0 * w / 4.0, y + h / 2.0], [x + 3.0 * w / 4.0, y], [x + w, y]],
    ]
}

/// Bézier pieces as straight moves, the start of each after the first left out (it is the end
/// of the one before).
fn flatten(pieces: &[[[f64; 2]; 4]]) -> Vec<[f64; 2]> {
    let mut out = vec![];
    for (i, [p0, p1, p2, p3]) in pieces.iter().enumerate() {
        for k in usize::from(i > 0)..=PIECE_STEPS {
            let t = k as f64 / PIECE_STEPS as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            out.push([a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0], a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1]]);
        }
    }
    out
}

/// The test's job and, per path, its native curve.
fn curve_job(s: &Session, p: &Value, cmd: &str) -> Result<(CutJob, Vec<Option<Native>>, Vec<usize>)> {
    let rows = rows(p, cmd)?;
    let settings = vectorcraft_cutting::settings(&s.doc()?.doc);
    let mat = settings.mat_geometry();
    let area = mat.cut_area_mm;
    let need = [MARGIN_MM + 2.0 * RADIUS_MM + GAP_MM + S_MM[0] + MARGIN_MM, MARGIN_MM + ROW_MM * ROWS.len() as f64];
    if !(area.width() >= need[0] && area.height() >= need[1]) {
        return Err(bad(cmd, format!("the mat's cut area is too small for the curve test: it needs {} × {} mm", need[0], need[1])));
    }
    let tool = vectorcraft_cutjob::ToolSettings { passes: 1, overcut_mm: 0.0, ..settings.operations.cut.clone() };
    let mut paths = vec![];
    let mut native = vec![];
    for &row in &rows {
        let top = area.y0 + MARGIN_MM + ROW_MM * row as f64;
        let centre = [area.x0 + MARGIN_MM + RADIUS_MM, top + S_MM[1] / 2.0];
        let s_curve = s_beziers([area.x0 + MARGIN_MM + 2.0 * RADIUS_MM + GAP_MM, top]);
        paths.push(Toolpath { op: 0, source: SourceRef::default(), closed: true, points: circle_points(centre, RADIUS_MM, settings.tolerance_mm) });
        paths.push(Toolpath { op: 0, source: SourceRef::default(), closed: false, points: flatten(&s_curve) });
        let (circle, s) = match ROWS.get(row).copied() {
            Some("bezier") => (Some(Native::Bezier(circle_beziers(centre, RADIUS_MM))), Some(Native::Bezier(s_curve))),
            Some("circle") => (Some(Native::Circle { centre, radius_mm: RADIUS_MM }), None),
            _ => (None, None),
        };
        native.push(circle);
        native.push(s);
    }
    let job = CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: mat.id.to_string(), size_mm: mat.size_mm, cut_area_mm: area },
        operations: vec![Operation { kind: OpKind::Cut, tool }],
        paths,
        warnings: vec![],
        track_enhancing: settings.track_enhancing,
        groups: vec![],
        registration: None,
    };
    job.validate().map_err(|e| bad(cmd, e.to_string()))?;
    Ok((job, native, rows))
}

fn row_names(rows: &[usize]) -> Vec<&'static str> {
    rows.iter().filter_map(|r| ROWS.get(*r).copied()).collect()
}

fn job_only(s: &mut Session, p: &Value) -> Result<Value> {
    let (job, native, rows) = curve_job(s, p, JOB_CMD)?;
    let kinds: Vec<Value> = native
        .iter()
        .map(|n| match n {
            Some(Native::Bezier(_)) => json!("bezier"),
            Some(Native::Circle { .. }) => json!("circle"),
            None => Value::Null,
        })
        .collect();
    Ok(json!({"job": job.to_value(), "native": kinds, "rows": row_names(&rows)}))
}

fn send(s: &mut Session, p: &Value) -> Result<Value> {
    // Which machine first: without one, that is the answer.
    let (device, driver) = super::devices::machine(p, CMD)?;
    let (job, native, rows) = curve_job(s, p, CMD)?;
    let encoded = driver.encode_native(&job, &native, &device, super::machine::adjust_mm()).map_err(|e| bad(CMD, e))?;
    let provider = registry::provider().ok_or_else(|| bad(CMD, "no cutter is connected"))?;
    let transport = provider.open(&device.id).map_err(|e| bad(CMD, e.to_string()))?;
    let (chunks, warnings) = (encoded.chunks.len(), encoded.warnings.clone());
    let model = driver.model(&device);
    let running = vectorcraft_cutter::send::spawn(transport, driver, encoded, Patience::default()).map_err(|e| bad(CMD, e))?;
    registry::start(device.clone(), running).map_err(|e| bad(CMD, e))?;
    Ok(json!({"device": device.id, "model": model, "chunks": chunks, "rows": row_names(&rows), "warnings": warnings}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The circle's points and Bézier quarters lie on the circle; the S-curve's pieces meet.
    #[test]
    fn the_shapes_are_what_they_say() {
        let c = [50.0, 50.0];
        let pts = circle_points(c, RADIUS_MM, 0.025);
        assert!(pts.len() >= 30 && pts.iter().all(|q| ((q[0] - c[0]).hypot(q[1] - c[1]) - RADIUS_MM).abs() < 1e-9));
        assert_eq!(pts.first(), Some(&[50.0, 55.0]), "starts down the mat, as the machine's circle");
        let quarters = circle_beziers(c, RADIUS_MM);
        let flat = flatten(&quarters);
        let worst = flat.iter().map(|q| ((q[0] - c[0]).hypot(q[1] - c[1]) - RADIUS_MM).abs()).fold(0.0, f64::max);
        assert!(worst < 0.002, "a Bézier circle is within 0.03% of its radius: {worst}");
        assert!((quarters[3][3][0] - 50.0).abs() < 1e-9 && (quarters[3][3][1] - 55.0).abs() < 1e-9, "closed");
        let s = s_beziers([0.0, 0.0]);
        assert_eq!(s[0][3], s[1][0]);
        assert_eq!((s[0][0], s[1][3]), ([0.0, 20.0], [50.0, 0.0]));
        assert_eq!(flatten(&s).len(), 2 * PIECE_STEPS + 1);
    }
}
