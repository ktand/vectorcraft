//! Fork: the weeding map (C18.9): a page showing what to pull away after the cut. The sheet round
//! the design (its bounds and a margin) is shaded, the pieces that stay are left white, and their
//! holes (an O's counter) shaded again, as they are weeded too: one path of the sheet and every
//! closed cut, filled even-odd. Drawn as the job is cut, so a mirrored job's map is mirrored, as
//! the material on the mat is. C18.24: with a weed box the sheet is the box (it isn't a piece to
//! keep), and the weed lines are drawn on it, dashed, so the strips the waste comes away in show.

use vectorcraft_cutjob::{CutJob, OpKind, Toolpath};

/// Round the design, mm.
const MARGIN_MM: f64 = 5.0;

/// The map as an SVG at the size of the design and its margin, mm user units.
pub fn to_svg(job: &CutJob) -> String {
    let cut_ops: Vec<usize> = job.operations.iter().enumerate().filter(|(_, o)| o.kind == OpKind::Cut).map(|(i, _)| i).collect();
    let weeding = |p: &Toolpath| p.source == crate::weed::SOURCE;
    let cuts = || job.paths.iter().filter(|p| cut_ops.contains(&p.op));
    let closed: Vec<&Vec<[f64; 2]>> = cuts().filter(|p| p.closed && p.points.len() >= 3 && !weeding(p)).map(|p| &p.points).collect();
    let weed_box = cuts().find(|p| p.closed && weeding(p)).map(|p| &p.points);
    let weed_lines: Vec<&Vec<[f64; 2]>> = cuts().filter(|p| !p.closed && weeding(p) && p.points.len() >= 2).map(|p| &p.points).collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for q in closed.iter().flat_map(|p| p.iter()).filter(|q| q[0].is_finite() && q[1].is_finite()) {
        (x0, y0, x1, y1) = (x0.min(q[0]), y0.min(q[1]), x1.max(q[0]), y1.max(q[1]));
    }
    if !x0.is_finite() {
        (x0, y0, x1, y1) = (0.0, 0.0, 10.0, 10.0);
    }
    let (x0, y0, x1, y1) = match weed_box {
        // The weed box is the sheet's edge.
        Some(b) if !b.is_empty() && b.iter().all(|q| q[0].is_finite() && q[1].is_finite()) => {
            b.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |r, q| {
                (r.0.min(q[0]), r.1.min(q[1]), r.2.max(q[0]), r.3.max(q[1]))
            })
        }
        _ => (x0 - MARGIN_MM, y0 - MARGIN_MM, x1 + MARGIN_MM, y1 + MARGIN_MM),
    };
    let (w, h) = (x1 - x0, y1 - y0);
    let mut d = format!("M{:.3} {:.3}H{:.3}V{:.3}H{:.3}Z", 0.0, 0.0, w, h, 0.0);
    for p in &closed {
        for (i, q) in p.iter().enumerate() {
            d.push_str(&format!("{}{:.3} {:.3}", if i == 0 { "M" } else { "L" }, q[0] - x0, q[1] - y0));
        }
        d.push('Z');
    }
    // The weed lines: where the waste is cut into strips.
    let mut lines = String::new();
    for p in &weed_lines {
        for (i, q) in p.iter().enumerate() {
            lines.push_str(&format!("{}{:.3} {:.3}", if i == 0 { "M" } else { "L" }, q[0] - x0, q[1] - y0));
        }
    }
    let lines = if lines.is_empty() {
        String::new()
    } else {
        format!("<path d=\"{lines}\" fill=\"none\" stroke=\"#000\" stroke-width=\"0.4\" stroke-dasharray=\"2 1\"/>\n")
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.3}mm\" height=\"{h:.3}mm\" viewBox=\"0 0 {w:.3} {h:.3}\">\n\
         <title>Weeding map: pull away the grey, keep the white</title>\n\
         <path d=\"{d}\" fill=\"#c8c8c8\" fill-rule=\"evenodd\" stroke=\"#000\" stroke-width=\"0.2\"/>\n{lines}</svg>\n"
    )
}

#[cfg(test)]
mod tests {
    use vectorcraft_doc::{Appearance, Document, Node};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    #[test]
    fn the_map_shades_the_sheet_round_the_pieces() {
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        for r in [Rect::new(10.0, 10.0, 110.0, 110.0), Rect::new(40.0, 40.0, 80.0, 80.0)] {
            let sq = Node::path(d.alloc_id(), shapes::rectangle(r), Appearance::default());
            d.insert(Some(layer), usize::MAX, sq).unwrap();
        }
        let job = crate::build_job(&d).unwrap();
        let svg = to_svg(&job);
        assert!(svg.contains("fill-rule=\"evenodd\""));
        // The sheet and two closed cuts: three closed subpaths.
        assert_eq!(svg.matches('Z').count(), 3, "{svg}");
        assert!(svg.contains("width=\"45.278mm\""), "100 pt and two 5 mm margins: {svg}");
        // C18.24: with a weed box and smart lines, the box is the sheet (not a piece) and the lines
        // are drawn: a long design split into strips.
        let mut d = Document::new(800.0, 300.0);
        let layer = d.layers[0].id;
        for k in 0..6 {
            let x = 20.0 + 100.0 * f64::from(k);
            let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, 20.0, x + 60.0, 80.0)), Appearance::default());
            d.insert(Some(layer), usize::MAX, sq).unwrap();
        }
        let s = vectorcraft_cutjob::CuttingSettings { weed_box: true, weed_lines: true, ..vectorcraft_cutjob::CuttingSettings::default() };
        crate::set_settings(&mut d, &s);
        let svg = to_svg(&crate::build_job(&d).unwrap());
        assert_eq!(svg.matches('Z').count(), 1 + 6, "the sheet and six pieces, not the box: {svg}");
        assert!(svg.contains("stroke-dasharray"), "the weed lines: {svg}");
        // Nothing to cut: a small blank sheet, not a failure.
        assert!(to_svg(&crate::build_job(&Document::new(100.0, 100.0)).unwrap()).contains("<path"));
    }
}
