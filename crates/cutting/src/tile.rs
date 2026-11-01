//! Fork: tiling (C14.7). A design larger than the mat is cut in tiles: artboards the size of what
//! the mat can cut, laid over the art row by row from its top-left, each overlapping the next by
//! the overlap asked for, so the pieces can be joined. Each tile is its own sheet: the cutting
//! settings are set to the first and Clip to Mat turned on, so each cuts what lies on it up to
//! its edges; the user cuts them one after another by choosing the artboard.

use vectorcraft_cutjob::{CuttingSettings, Orientation};
use vectorcraft_doc::{Artboard, Document};
use vectorcraft_geom::Rect;

use crate::placement::MM_PER_PT;

/// Most tiles one tiling makes.
pub const MAX_TILES: usize = 400;
/// The overlap between neighbouring tiles, mm: the range and the usual.
pub const OVERLAP_MM: (f64, f64) = (0.0, 50.0);
pub const DEFAULT_OVERLAP_MM: f64 = 5.0;

/// The tiles covering `region` (points), each `tile` (width, height, points), each the next along
/// `tile` less `overlap` from the one before: row by row from the top-left.
pub fn layout(region: Rect, tile: (f64, f64), overlap: f64) -> Result<Vec<Rect>, String> {
    let (w, h) = tile;
    let finite = [region.x0, region.y0, region.x1, region.y1, w, h, overlap].iter().all(|v| v.is_finite());
    if !finite || w <= 0.0 || h <= 0.0 || overlap < 0.0 {
        return Err("the tiles' size and overlap must be numbers above 0".into());
    }
    let (step_x, step_y) = (w - overlap, h - overlap);
    if step_x <= 0.0 || step_y <= 0.0 {
        return Err("the overlap must be smaller than the mat".into());
    }
    // How many tiles reach across `len`: one, then one more for each step past the first.
    let count = |len: f64, size: f64, step: f64| -> Result<usize, String> {
        let more = ((len - size) / step).ceil().max(0.0);
        if more >= MAX_TILES as f64 {
            return Err(format!("that would be more than {MAX_TILES} tiles"));
        }
        // Whole and below MAX_TILES: the conversion is exact.
        Ok(more as usize + 1)
    };
    let (cols, rows) = (count(region.width(), w, step_x)?, count(region.height(), h, step_y)?);
    if cols.saturating_mul(rows) > MAX_TILES {
        return Err(format!("that would be more than {MAX_TILES} tiles"));
    }
    let mut out = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = (region.x0 + c as f64 * step_x, region.y0 + r as f64 * step_y);
            out.push(Rect::new(x, y, x + w, y + h));
        }
    }
    Ok(out)
}

/// What the mat can cut, as an artboard's width and height in points: the cut area less the
/// offset, turned for landscape (the artboard's width along the feed).
pub fn tile_size(s: &CuttingSettings) -> (f64, f64) {
    let a = s.mat_geometry().cut_area_mm;
    let across = (a.x1 - a.x0 - s.offset_mm[0].max(0.0)) / MM_PER_PT;
    let along = (a.y1 - a.y0 - s.offset_mm[1].max(0.0)) / MM_PER_PT;
    match s.orientation {
        Orientation::Portrait => (across, along),
        Orientation::Landscape => (along, across),
    }
}

/// What [`add_tiles`] made.
#[derive(Clone, Debug, PartialEq)]
pub struct Tiled {
    /// The new artboards' indices, in cutting order.
    pub artboards: Vec<usize>,
    pub rows: usize,
    pub cols: usize,
}

/// `region` (points) cut in tiles `overlap_mm` apart where they meet: added as artboards Tile 1,
/// Tile 2… (the first artboard's other properties), the cutting settings set to the first and to
/// clip to the mat. An error, changing nothing, when the region fits the mat as it is.
pub fn add_tiles(d: &mut Document, region: Rect, overlap_mm: f64) -> Result<Tiled, String> {
    if !(OVERLAP_MM.0..=OVERLAP_MM.1).contains(&overlap_mm) {
        return Err(format!("the overlap is {} to {} mm", OVERLAP_MM.0, OVERLAP_MM.1));
    }
    let mut s = crate::settings(d);
    let (w, h) = tile_size(&s);
    if region.width() <= w && region.height() <= h {
        return Err("the art fits on the mat as it is: no tiles needed".into());
    }
    let rects = layout(region, (w, h), overlap_mm / MM_PER_PT)?;
    let cols = rects.iter().filter(|r| r.y0 == region.y0).count().max(1);
    let like = d.artboards.first().cloned().ok_or("the document has no artboard")?;
    let mut artboards = Vec::with_capacity(rects.len());
    for (k, rect) in rects.iter().enumerate() {
        let id = d.next_artboard_id();
        d.artboards.push(Artboard { id, name: format!("Tile {}", k + 1), rect: *rect, locked: false, ..like.clone() });
        artboards.push(d.artboards.len().saturating_sub(1));
    }
    if let Some(first) = artboards.first().and_then(|i| u32::try_from(*i).ok()) {
        s.artboard = first;
    }
    s.clip_to_mat = true;
    crate::set_settings(d, &s);
    Ok(Tiled { rows: rects.len() / cols, cols, artboards })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 250 × 100 in tiles 100 × 100 overlapping by 10: three across (0, 90, 180), one down.
    #[test]
    fn tiles_cover_the_region_overlapping() {
        let t = layout(Rect::new(0.0, 0.0, 250.0, 100.0), (100.0, 100.0), 10.0).unwrap();
        assert_eq!(t.len(), 3);
        assert_eq!(t.iter().map(|r| r.x0).collect::<Vec<_>>(), [0.0, 90.0, 180.0]);
        assert!(t.last().unwrap().x1 >= 250.0);
        assert_eq!(layout(Rect::new(0.0, 0.0, 50.0, 50.0), (100.0, 100.0), 0.0).unwrap().len(), 1);
        assert!(layout(Rect::new(0.0, 0.0, 250.0, 100.0), (100.0, 100.0), 100.0).is_err(), "overlap as large as a tile");
        assert!(layout(Rect::new(0.0, 0.0, 1e9, 1e9), (100.0, 100.0), 0.0).is_err(), "too many");
        assert!(layout(Rect::new(0.0, 0.0, f64::NAN, 1.0), (100.0, 100.0), 0.0).is_err());
    }

    /// A 600 × 400 mm artboard on the 12 × 12 in mat: tiles the cut area's size, the settings on
    /// the first with Clip to Mat, each tile's job cut up to its edge.
    #[test]
    fn a_large_artboard_is_tiled_onto_mats() {
        let mut d = Document::new(600.0 / MM_PER_PT, 400.0 / MM_PER_PT);
        let layer = d.layers[0].id;
        let line = vectorcraft_geom::shapes::rectangle(Rect::new(10.0, 10.0, 590.0 / MM_PER_PT, 390.0 / MM_PER_PT));
        let n = vectorcraft_doc::Node::path(d.alloc_id(), line, vectorcraft_doc::Appearance::default());
        d.insert(Some(layer), usize::MAX, n).unwrap();
        let region = d.artboards[0].rect;
        let tiled = add_tiles(&mut d, region, 5.0).unwrap();
        let (w, h) = tile_size(&crate::settings(&d));
        let expect_cols = ((600.0 - w * MM_PER_PT) / (w * MM_PER_PT - 5.0)).ceil() as usize + 1;
        assert_eq!(tiled.cols, expect_cols);
        assert_eq!(tiled.artboards.len(), tiled.rows * tiled.cols);
        assert_eq!(d.artboards[tiled.artboards[0]].name, "Tile 1");
        assert!((d.artboards[tiled.artboards[0]].rect.width() - w).abs() < 1e-9 && (d.artboards[tiled.artboards[0]].rect.height() - h).abs() < 1e-9);
        let s = crate::settings(&d);
        assert!(s.clip_to_mat && s.artboard as usize == tiled.artboards[0]);
        let job = crate::build_job(&d).unwrap();
        assert!(!job.paths.is_empty());
        let a = job.mat.cut_area_mm;
        assert!(job.paths.iter().flat_map(|p| &p.points).all(|q| q[0] >= a.x0 - 1e-6 && q[0] <= a.x1 + 1e-6 && q[1] <= a.y1 + 1e-6));
        // Small enough already: nothing made.
        let before = d.artboards.len();
        assert!(add_tiles(&mut d, Rect::new(0.0, 0.0, 50.0, 50.0), 5.0).is_err());
        assert!(add_tiles(&mut d, region, 80.0).is_err(), "overlap out of range");
        assert_eq!(d.artboards.len(), before);
    }
}
