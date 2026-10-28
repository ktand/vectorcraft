//! A cut job as an SVG at the mat's size (C3.8): for bug reports, agents and a look outside the
//! app. Millimetre user units on a mat-sized page: the mat (and its cut area, where smaller),
//! the blade's travel dashed, each path stroked in its operation's colour with its place in the
//! order and its operation as `data-order` and `data-op`, and its number at its start.

use std::fmt::Write as _;

use vectorcraft_cutjob::{CutJob, OpKind};

const CUT: &str = "#0057d9";
const SCORE: &str = "#d4007a";
const GREY: &str = "#999999";

/// `v` with at most 4 decimals, trailing zeros dropped.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// `job` as an SVG document.
pub fn to_svg(job: &CutJob) -> String {
    let [w, h] = job.mat.size_mm;
    let a = job.mat.cut_area_mm;
    let mut s = String::new();
    // Writing to a String can't fail: the results are ignored.
    let _ = writeln!(s, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}">"#, w = num(w), h = num(h));
    let _ = writeln!(s, "  <title>Cut job preview ({} paths)</title>", job.paths.len());
    let _ = writeln!(s, r#"  <rect id="mat" width="{}" height="{}" fill="none" stroke="{GREY}" stroke-width="0.3"/>"#, num(w), num(h));
    if (a.x0, a.y0, a.x1, a.y1) != (0.0, 0.0, w, h) {
        let _ = writeln!(
            s,
            r#"  <rect id="cut-area" x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{GREY}" stroke-width="0.2" stroke-dasharray="2 1"/>"#,
            num(a.x0),
            num(a.y0),
            num(a.width()),
            num(a.height())
        );
    }
    let mut travel = String::new();
    let mut paths = String::new();
    let mut numbers = String::new();
    let mut at = [a.x0, a.y0];
    for (i, p) in job.paths.iter().enumerate() {
        let Some(&first) = p.points.first() else { continue };
        let _ = write!(travel, "M{} {}L{} {}", num(at[0]), num(at[1]), num(first[0]), num(first[1]));
        let kind = job.operations.get(p.op).map_or(OpKind::Cut, |o| o.kind);
        let (color, dash) = match kind {
            OpKind::Cut => (CUT, ""),
            OpKind::Score => (SCORE, r#" stroke-dasharray="1.5 1""#),
        };
        let mut d = String::new();
        for (k, q) in p.points.iter().enumerate() {
            let _ = write!(d, "{}{} {}", if k == 0 { "M" } else { "L" }, num(q[0]), num(q[1]));
        }
        if p.closed {
            d.push('Z');
        }
        let _ = writeln!(paths, r#"    <path data-order="{}" data-op="{}" stroke="{color}"{dash} d="{d}"/>"#, i + 1, kind.id());
        let _ = writeln!(numbers, r#"    <text x="{}" y="{}" fill="{color}">{}</text>"#, num(first[0] + 0.8), num(first[1] - 0.8), i + 1);
        at = if p.closed { first } else { p.points.last().copied().unwrap_or(first) };
    }
    if !travel.is_empty() {
        let _ = writeln!(s, r#"  <path id="travel" d="{travel}" fill="none" stroke="{GREY}" stroke-width="0.2" stroke-dasharray="1 1"/>"#);
    }
    let _ = writeln!(s, r#"  <g id="paths" fill="none" stroke-width="0.3" stroke-linejoin="round">"#);
    s.push_str(&paths);
    let _ = writeln!(s, "  </g>");
    let _ = writeln!(s, r#"  <g id="numbers" font-family="sans-serif" font-size="3">"#);
    s.push_str(&numbers);
    let _ = writeln!(s, "  </g>");
    s.push_str("</svg>\n");
    s
}

#[cfg(test)]
mod tests {
    use vectorcraft_cutjob::{JOB_VERSION, JobMat, Operation, RectMm, SourceRef, ToolSettings, Toolpath};

    use super::*;

    #[test]
    fn numbers_are_short() {
        assert_eq!(
            (num(10.0), num(0.12345), num(-0.00001), num(f64::NAN), num(304.8)),
            ("10".into(), "0.1235".into(), "0".into(), "0".into(), "304.8".into())
        );
    }

    #[test]
    fn the_preview_draws_the_job_in_order() {
        let tool = ToolSettings { force: 0.3, speed_mm_s: 100.0, passes: 1, overcut_mm: 0.0, ..ToolSettings::default() };
        let job = CutJob {
            version: JOB_VERSION,
            mat: JobMat { id: "t".into(), size_mm: [304.8, 304.8], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: 304.8, y1: 304.8 } },
            operations: vec![Operation { kind: OpKind::Score, tool: tool.clone() }, Operation { kind: OpKind::Cut, tool }],
            paths: vec![
                Toolpath { op: 0, source: SourceRef::default(), closed: false, points: vec![[10.0, 80.0], [60.0, 80.0]] },
                Toolpath { op: 1, source: SourceRef::default(), closed: true, points: vec![[10.0, 10.0], [20.0, 10.0], [20.0, 20.0]] },
            ],
            warnings: vec![],
            track_enhancing: false,
        };
        let svg = to_svg(&job);
        assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="304.8mm" height="304.8mm" viewBox="0 0 304.8 304.8">"#));
        assert!(!svg.contains("cut-area"), "the whole mat is cut");
        assert!(svg.contains(r#"data-order="1" data-op="score""#) && svg.contains(r#"data-order="2" data-op="cut""#));
        assert!(svg.contains(r#"d="M10 10L20 10L20 20Z""#));
        assert!(svg.contains(r#"d="M0 0L10 80M60 80L10 10""#), "{svg}");
    }
}
