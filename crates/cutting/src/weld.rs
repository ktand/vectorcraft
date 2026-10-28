//! Cut Outline (C3.10): the closed shapes of every object set to Cut Outline are united, and the
//! job cuts only the outline of the union: where they overlap, the lines inside aren't cut, and
//! holes that stay holes (an O's counter) are. Each object counts as its own subpaths filled
//! even-odd, so its holes stay holes; shapes that don't touch stay separate pieces. Open paths
//! have no inside to unite and are cut as they are, with a warning.

use vectorcraft_cutjob::{JobWarning, OpKind, SourceRef};
use vectorcraft_geom::{BezPath, FillRule, PathData, Rect, Shape};
use vectorcraft_pathops::try_unite_all;

use crate::geometry::{Outline, subpaths};

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && a.x1 >= b.x0 && a.y0 <= b.y1 && a.y1 >= b.y0
}

/// `outlines` with those set to Cut Outline united; what that left out.
pub fn weld(outlines: Vec<Outline>) -> (Vec<Outline>, Vec<JobWarning>) {
    let mut warnings = vec![];
    let mut out = vec![];
    // Each object's closed outlines, in the order they came, as one even-odd region.
    let mut objects: Vec<(u64, BezPath)> = vec![];
    let mut open_warned: Vec<u64> = vec![];
    for o in outlines {
        if !(o.outline && o.op == OpKind::Cut) {
            out.push(o);
            continue;
        }
        if !o.closed {
            if !open_warned.contains(&o.source.node) {
                open_warned.push(o.source.node);
                warnings.push(JobWarning {
                    source: Some(SourceRef { subpath: 0, ..o.source }),
                    message: "an open path has no outline to cut: it is cut along its line".into(),
                });
            }
            out.push(o);
            continue;
        }
        match objects.iter_mut().find(|(n, _)| *n == o.source.node) {
            Some((_, p)) => p.extend(o.path.elements().iter().copied().chain([vectorcraft_geom::PathEl::ClosePath])),
            None => {
                let mut p = o.path.clone();
                p.close_path();
                objects.push((o.source.node, p));
            }
        }
    }
    if objects.is_empty() {
        return (out, warnings);
    }
    let regions: Vec<PathData> = objects.iter().map(|(_, p)| PathData::from_bezpath(p)).collect();
    let inputs: Vec<(&PathData, FillRule)> = regions.iter().map(|r| (r, FillRule::EvenOdd)).collect();
    match try_unite_all(&inputs) {
        Ok(united) => {
            let boxes: Vec<(u64, Rect)> = objects.iter().map(|(n, p)| (*n, p.bounding_box())).collect();
            let mut index = std::collections::HashMap::<u64, u32>::new();
            let mut welded = vec![];
            for (path, _) in subpaths(&united.to_bezpath()) {
                let b = path.bounding_box();
                let node = boxes.iter().find(|(_, ob)| overlaps(*ob, b)).or(boxes.first()).map_or(0, |(n, _)| *n);
                let i = index.entry(node).or_insert(0);
                welded.push(Outline { op: OpKind::Cut, outline: true, source: SourceRef { node, subpath: *i }, closed: true, path });
                *i += 1;
            }
            welded.extend(out);
            (welded, warnings)
        }
        Err(_) => {
            // Each object cut along all its lines, as plain Cut would.
            warnings.push(JobWarning {
                source: None,
                message: "the shapes set to Cut Outline couldn't be united: each is cut along all its lines".into(),
            });
            for (node, p) in objects {
                for (i, (path, _)) in subpaths(&p).into_iter().enumerate() {
                    out.push(Outline {
                        op: OpKind::Cut,
                        outline: true,
                        source: SourceRef { node, subpath: u32::try_from(i).unwrap_or(u32::MAX) },
                        closed: true,
                        path,
                    });
                }
            }
            (out, warnings)
        }
    }
}

#[cfg(test)]
mod tests;
