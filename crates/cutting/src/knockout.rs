//! Fork: knockout for layered vinyl (C18.1). When the job is cut in groups (by colour or by
//! layer) and knockout is on, each object's closed cut is cut back to what shows of it: the
//! objects of other groups painted above it are taken out, shrunk by the overlap, so the lower
//! colour still runs that far under the edges of the one above and no gap shows. Stacked heat
//! transfer vinyl then has one layer, not several, almost everywhere. The document is unchanged:
//! only the job's paths are.

use std::collections::HashMap;

use vectorcraft_cutjob::{CuttingSettings, Grouping, JobWarning, OpKind, SourceRef};
use vectorcraft_doc::Document;
use vectorcraft_geom::{BezPath, FillRule, PathData, PathEl, Shape};
use vectorcraft_pathops::{BoolOp, DEFAULT_PRECISION, Join, offset_path, try_boolean, try_unite_all};

use crate::geometry::{Outline, subpaths};
use crate::group::group_of;
use crate::placement::MM_PER_PT;
use crate::select::Item;

/// `outlines` with each object's closed cut taken back to what shows of it, when knockout is on
/// and the job is grouped; what that couldn't do.
pub fn knockout(d: &Document, s: &CuttingSettings, items: &[Item], outlines: Vec<Outline>) -> (Vec<Outline>, Vec<JobWarning>) {
    if !s.knockout || s.grouping == Grouping::None {
        return (outlines, vec![]);
    }
    let order: HashMap<u64, usize> = items.iter().enumerate().map(|(i, it)| (it.id.0, i)).collect();
    let group: HashMap<u64, String> = items.iter().map(|it| (it.id.0, group_of(d, it.id, s.grouping))).collect();
    // Each object's closed cut, as one even-odd region, in paint order (bottom first).
    let mut regions: Vec<(u64, BezPath)> = vec![];
    let mut rest = vec![];
    for o in outlines {
        if !(o.closed && o.op == OpKind::Cut) {
            rest.push(o);
            continue;
        }
        match regions.iter_mut().find(|(n, _)| *n == o.source.node) {
            Some((_, p)) => p.extend(o.path.elements().iter().copied().chain([PathEl::ClosePath])),
            None => {
                let mut p = o.path.clone();
                p.close_path();
                regions.push((o.source.node, p));
            }
        }
    }
    regions.sort_by_key(|(n, _)| order.get(n).copied().unwrap_or(usize::MAX));
    let overlap = s.knockout_overlap_mm / MM_PER_PT;
    let mut warnings = vec![];
    let mut out = vec![];
    for (i, (node, region)) in regions.iter().enumerate() {
        let mine = group.get(node);
        let above: Vec<PathData> = regions[i + 1..]
            .iter()
            .filter(|(n, p)| group.get(n) != mine && p.bounding_box().intersect(region.bounding_box()).area() > 0.0)
            .map(|(_, p)| PathData::from_bezpath(p))
            .collect();
        let own = PathData::from_bezpath(region);
        let kept = if above.is_empty() {
            Ok(own.clone())
        } else {
            let inputs: Vec<(&PathData, FillRule)> = above.iter().map(|p| (p, FillRule::EvenOdd)).collect();
            try_unite_all(&inputs)
                .map(|cover| if overlap > 0.0 { offset_path(&cover, -overlap, Join::Miter, 4.0) } else { cover })
                .and_then(|cover| try_boolean(&own, FillRule::EvenOdd, &cover, FillRule::NonZero, BoolOp::Difference, DEFAULT_PRECISION))
        };
        let kept = match kept {
            Ok(k) => k,
            Err(_) => {
                warnings.push(JobWarning {
                    source: Some(SourceRef { node: *node, subpath: 0 }),
                    message: "the colours above this object couldn't be taken out of it: it is cut whole".into(),
                });
                own
            }
        };
        let pieces = subpaths(&kept.to_bezpath());
        if pieces.is_empty() {
            warnings.push(JobWarning {
                source: Some(SourceRef { node: *node, subpath: 0 }),
                message: "this object is wholly covered by the colours above it: it isn't cut (knockout)".into(),
            });
        }
        for (k, (path, _)) in pieces.into_iter().enumerate() {
            out.push(Outline {
                op: OpKind::Cut,
                outline: false,
                source: SourceRef { node: *node, subpath: u32::try_from(k).unwrap_or(u32::MAX) },
                closed: true,
                path,
            });
        }
    }
    out.extend(rest);
    (out, warnings)
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::{Color, Paint};
    use vectorcraft_doc::{Appearance, Node};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    /// Two squares of two colours, the red one over the blue: red is cut whole, blue is cut back
    /// to what shows plus the overlap under red.
    fn two_squares(overlap_mm: f64) -> (Document, CuttingSettings) {
        let mut d = Document::new(500.0, 500.0);
        let layer = d.layers[0].id;
        // Red is taller than blue, so only its left edge crosses blue.
        let blue = (Rect::new(0.0, 0.0, 100.0, 100.0), Color::Rgb { r: 0.0, g: 0.0, b: 1.0 });
        let red = (Rect::new(50.0, -50.0, 150.0, 150.0), Color::Rgb { r: 1.0, g: 0.0, b: 0.0 });
        for (r, c) in [blue, red] {
            let sq = Node::path(d.alloc_id(), shapes::rectangle(r), Appearance::basic(Paint::solid(c), Paint::None, 0.0));
            d.insert(Some(layer), usize::MAX, sq).unwrap();
        }
        let s = CuttingSettings { grouping: Grouping::Color, knockout: true, knockout_overlap_mm: overlap_mm, ..CuttingSettings::default() };
        crate::set_settings(&mut d, &s);
        (d, s)
    }

    fn widths(d: &Document, s: &CuttingSettings) -> Vec<f64> {
        let items = crate::select::select(d, s);
        let (outlines, _) = crate::geometry::outlines(d, &items);
        let (out, warnings) = knockout(d, s, &items, outlines);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut w: Vec<f64> = out.iter().map(|o| o.path.bounding_box().width()).collect();
        w.sort_by(f64::total_cmp);
        w
    }

    #[test]
    fn the_lower_colour_is_cut_back_to_the_overlap() {
        let (d, s) = two_squares(0.0);
        let w = widths(&d, &s);
        assert!((w[0] - 50.0).abs() < 0.01 && (w[1] - 100.0).abs() < 0.01, "{w:?}");
        let (d, s) = two_squares(2.0);
        let w = widths(&d, &s);
        let extra = 2.0 / MM_PER_PT;
        assert!((w[0] - (50.0 + extra)).abs() < 0.05, "blue runs 2 mm under red: {w:?}");
    }

    /// Off, or without groups, nothing changes; a colour wholly hidden is left out with a warning.
    #[test]
    fn off_or_hidden() {
        let (d, s) = two_squares(0.0);
        let off = CuttingSettings { knockout: false, ..s.clone() };
        assert_eq!(widths(&d, &off), vec![100.0, 100.0]);
        let ungrouped = CuttingSettings { grouping: Grouping::None, ..s.clone() };
        assert_eq!(widths(&d, &ungrouped), vec![100.0, 100.0]);
        let mut d = Document::new(500.0, 500.0);
        let layer = d.layers[0].id;
        for (r, c) in [
            (Rect::new(10.0, 10.0, 20.0, 20.0), Color::Rgb { r: 0.0, g: 0.0, b: 1.0 }),
            (Rect::new(0.0, 0.0, 100.0, 100.0), Color::Rgb { r: 1.0, g: 0.0, b: 0.0 }),
        ] {
            let sq = Node::path(d.alloc_id(), shapes::rectangle(r), Appearance::basic(Paint::solid(c), Paint::None, 0.0));
            d.insert(Some(layer), usize::MAX, sq).unwrap();
        }
        crate::set_settings(&mut d, &s);
        let items = crate::select::select(&d, &s);
        let (outlines, _) = crate::geometry::outlines(&d, &items);
        let (out, warnings) = knockout(&d, &s, &items, outlines);
        assert_eq!(out.len(), 1);
        assert!(warnings[0].message.contains("wholly covered"));
    }
}
