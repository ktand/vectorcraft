//! Nesting (C8.5): the pieces placed close together on the sheet by their true shapes, turned
//! where that helps, so less material is used. The sheet (the cut area, or the marks' for Print &
//! Cut) becomes a grid of small cells; each piece is drawn into cells from its cut outline, filled
//! (even-odd, so its holes stay free and a small piece can go in one). What is already on the sheet
//! is grown by the gap, so a piece whose cells meet none of it is at least the gap from everything.
//! The grid only ever covers more than a shape, never less, so the gap is never undercut; a piece
//! may end up a cell or two further off than it needs to.
//!
//! Largest first, each piece tries each turn allowed and goes where its bottom edge is highest on
//! the sheet (nearest the edge the machine takes first), leftmost among those: the material used
//! stays as short as it can. Objects that aren't nested stay where they are and are packed round.
//! The same pieces and settings always give the same result. Work is capped ([`MAX_WORK`]): a job
//! too large for it places what it can and says so.

use vectorcraft_cutjob::RectMm;

/// The cell size used unless the sheet is so large it would make too many cells, mm.
pub const CELL_MM: f64 = 0.5;
/// Most cells the sheet's grid has: past that the cells grow.
const MAX_CELLS: f64 = 4_000_000.0;
/// Most word comparisons one nesting makes.
pub const MAX_WORK: u64 = 3_000_000_000;
/// Most pieces nested at once.
pub const MAX_PIECES: usize = 2_000;

/// One outline of a piece, mm: closed (a shape) or open (a line).
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub closed: bool,
    pub points: Vec<[f64; 2]>,
}

/// What nesting is asked to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// The least distance between two pieces, and between a piece and what stays, mm.
    pub gap_mm: f64,
    /// Pieces may be turned by quarter turns.
    pub rotate: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { gap_mm: 2.0, rotate: true }
    }
}

/// Where a piece went: turned by `quarter_turns` (clockwise on the mat), then moved; `affine` maps
/// its points as they were to where they go (kurbo's order: x' = a·x + c·y + e, y' = b·x + d·y + f).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub quarter_turns: u8,
    pub affine: [f64; 6],
}

/// The result: each piece's placement (none: it didn't fit), how far down the sheet the nested
/// pieces reach (mm from the area's top), and whether the work ran out before every piece was tried.
#[derive(Clone, Debug, PartialEq)]
pub struct Nested {
    pub placed: Vec<Option<Placed>>,
    pub used_length_mm: f64,
    pub out_of_work: bool,
}

/// Bits in rows of 64-bit words: a cell is set where something is.
#[derive(Clone, Debug, PartialEq)]
struct Mask {
    w: usize,
    h: usize,
    words: usize,
    bits: Vec<u64>,
}

impl Mask {
    fn new(w: usize, h: usize) -> Mask {
        let words = w.div_ceil(64).max(1);
        Mask { w, h, words, bits: vec![0; words.saturating_mul(h)] }
    }

    fn set(&mut self, x: i64, y: i64) {
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else { return };
        if x >= self.w || y >= self.h {
            return;
        }
        if let Some(word) = self.bits.get_mut(y * self.words + x / 64) {
            *word |= 1 << (x % 64);
        }
    }

    fn row(&self, y: usize) -> &[u64] {
        self.bits.get(y * self.words..(y + 1) * self.words).unwrap_or_default()
    }

    fn count(&self) -> u64 {
        self.bits.iter().map(|w| u64::from(w.count_ones())).sum()
    }

    /// 64 bits of row `y` from bit `from` on (bits outside the mask are clear).
    fn bits_at(&self, y: usize, from: i64) -> u64 {
        let row = self.row(y);
        let word = |i: i64| usize::try_from(i).ok().and_then(|i| row.get(i)).copied().unwrap_or(0);
        let (q, r) = (from.div_euclid(64), from.rem_euclid(64));
        if r == 0 { word(q) } else { (word(q) >> r) | (word(q + 1) << (64 - r)) }
    }

    /// Whether `piece` placed with its top-left cell at (`x`, `y`) meets anything set here; adds
    /// the word comparisons made to `work`. Cells off this mask count as free.
    fn meets(&self, piece: &Mask, x: i64, y: i64, order: &[usize], work: &mut u64) -> bool {
        for &py in order {
            let Some(sy) = y.checked_add(py as i64).and_then(|v| usize::try_from(v).ok()).filter(|v| *v < self.h) else { continue };
            for (k, &p) in piece.row(py).iter().enumerate() {
                *work = work.saturating_add(1);
                if p != 0 && self.bits_at(sy, x + 64 * k as i64) & p != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// ORs `piece` in with its top-left cell at (`x`, `y`).
    fn add(&mut self, piece: &Mask, x: i64, y: i64) {
        for py in 0..piece.h {
            for px in 0..piece.w {
                let set = piece.row(py).get(px / 64).is_some_and(|w| w >> (px % 64) & 1 == 1);
                if set {
                    self.set(x + px as i64, y + py as i64);
                }
            }
        }
    }

    /// Grown by `g` mm in every direction, conservatively for cells `c` mm across: every point
    /// within `g` of a set cell's area lands in a set cell.
    fn dilated(&self, g: f64, c: f64) -> Mask {
        let r = (g / c).ceil().max(0.0) as usize + 1;
        let r = r.min(self.w.max(self.h));
        // Each row spread sideways by 0..=r cells.
        let mut spread: Vec<Vec<u64>> = vec![self.bits.clone()];
        for k in 1..=r {
            let Some(prev) = spread.get(k - 1) else { break };
            let mut next = prev.clone();
            for y in 0..self.h {
                for i in 0..self.words {
                    let at =
                        |j: i64| usize::try_from(j).ok().filter(|j| *j < self.words).and_then(|j| prev.get(y * self.words + j)).copied().unwrap_or(0);
                    let i = i as i64;
                    let left = (at(i) << 1) | (at(i - 1) >> 63);
                    let right = (at(i) >> 1) | (at(i + 1) << 63);
                    if let Some(w) = next.get_mut(y * self.words + i as usize) {
                        *w |= left | right;
                    }
                }
            }
            spread.push(next);
        }
        let mut out = self.clone();
        let limit = g / c;
        for dy in -(r as i64)..=(r as i64) {
            let ey = (dy.unsigned_abs() as f64 - 1.0).max(0.0);
            // The widest sideways reach this row offset allows.
            let reach = (0..=r).rev().find(|&dx| {
                let ex = (dx as f64 - 1.0).max(0.0);
                ex * ex + ey * ey <= limit * limit
            });
            let Some(reach) = reach else { continue };
            let Some(src) = spread.get(reach) else { continue };
            for y in 0..self.h {
                let Some(from) = (y as i64).checked_add(dy).and_then(|v| usize::try_from(v).ok()).filter(|v| *v < self.h) else { continue };
                for i in 0..self.words {
                    let v = src.get(from * self.words + i).copied().unwrap_or(0);
                    if let Some(w) = out.bits.get_mut(y * self.words + i) {
                        *w |= v;
                    }
                }
            }
        }
        // Bits past the row's end are never set.
        let tail = self.w % 64;
        if tail != 0 {
            for y in 0..out.h {
                if let Some(w) = out.bits.get_mut(y * out.words + out.words - 1) {
                    *w &= (1u64 << tail) - 1;
                }
            }
        }
        out
    }
}

/// `shapes` drawn into cells `c` mm across, `origin` (mm) at cell (0, 0), `w` × `h` cells: closed
/// shapes filled even-odd and every edge's cells marked (sampled finely enough to catch each cell
/// it crosses), open ones their line's cells.
fn draw(shapes: &[Shape], origin: [f64; 2], c: f64, w: usize, h: usize) -> Mask {
    let mut m = Mask::new(w, h);
    let cell = |v: f64, o: f64| ((v - o) / c).floor() as i64;
    for s in shapes {
        let n = s.points.len();
        let count = if s.closed { n } else { n.saturating_sub(1) };
        for k in 0..count {
            let (Some(a), Some(b)) = (s.points.get(k), s.points.get((k + 1) % n.max(1))) else { continue };
            let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            let steps = ((len / (c / 4.0)).ceil() as usize).clamp(1, 1_000_000);
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                m.set(cell(a[0] + t * (b[0] - a[0]), origin[0]), cell(a[1] + t * (b[1] - a[1]), origin[1]));
            }
        }
        if s.points.len() == 1
            && let Some(p) = s.points.first()
        {
            m.set(cell(p[0], origin[0]), cell(p[1], origin[1]));
        }
    }
    // Fill: each row's centre line, between the crossings of every closed shape (even-odd).
    for y in 0..h {
        let yc = origin[1] + (y as f64 + 0.5) * c;
        let mut xs: Vec<f64> = vec![];
        for s in shapes.iter().filter(|s| s.closed) {
            let n = s.points.len();
            for k in 0..n {
                let (Some(a), Some(b)) = (s.points.get(k), s.points.get((k + 1) % n)) else { continue };
                if (a[1] > yc) != (b[1] > yc) {
                    xs.push(a[0] + (yc - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks(2) {
            if let [x0, x1] = pair {
                let (from, to) = (cell(*x0, origin[0]), cell(*x1, origin[0]));
                for x in from..=to {
                    m.set(x, y as i64);
                }
            }
        }
    }
    m
}

/// What the points cover, mm.
fn bounds<'a>(points: impl Iterator<Item = &'a [f64; 2]>) -> Option<RectMm> {
    let mut r: Option<RectMm> = None;
    for q in points {
        if !(q[0].is_finite() && q[1].is_finite()) {
            return None;
        }
        r = Some(match r {
            None => RectMm { x0: q[0], y0: q[1], x1: q[0], y1: q[1] },
            Some(r) => RectMm { x0: r.x0.min(q[0]), y0: r.y0.min(q[1]), x1: r.x1.max(q[0]), y1: r.y1.max(q[1]) },
        });
    }
    r
}

/// The linear part of `q` quarter turns clockwise on the mat (+y down): (a, b, c, d).
fn turn(q: u8) -> [f64; 4] {
    match q % 4 {
        1 => [0.0, 1.0, -1.0, 0.0],
        2 => [-1.0, 0.0, 0.0, -1.0],
        3 => [0.0, -1.0, 1.0, 0.0],
        _ => [1.0, 0.0, 0.0, 1.0],
    }
}

fn apply(m: [f64; 4], p: [f64; 2]) -> [f64; 2] {
    [m[0] * p[0] + m[2] * p[1], m[1] * p[0] + m[3] * p[1]]
}

/// A piece turned one way: its own cells, tested against the sheet, and the same grown by the
/// gap, added to the sheet once it is placed.
struct Turned {
    q: u8,
    /// The outline's bounds' top-left, mm, after the turn: cell (0, 0) of `mask`.
    min: [f64; 2],
    mask: Mask,
    grown: Mask,
    /// Cells from `grown`'s top-left to `mask`'s.
    inset: i64,
    /// The mask's rows in the order to test them: fullest first, so a collision is found soon.
    order: Vec<usize>,
}

fn turned(shapes: &[Shape], q: u8, c: f64, gap: f64) -> Option<Turned> {
    let m = turn(q);
    let shapes: Vec<Shape> = shapes.iter().map(|s| Shape { closed: s.closed, points: s.points.iter().map(|p| apply(m, *p)).collect() }).collect();
    let b = bounds(shapes.iter().flat_map(|s| s.points.iter()))?;
    let w = usize::try_from(((b.x1 - b.x0) / c).floor() as i64 + 1).ok()?;
    let h = usize::try_from(((b.y1 - b.y0) / c).floor() as i64 + 1).ok()?;
    let inset = (gap / c).ceil() as i64 + 2;
    let pad = usize::try_from(2 * inset).ok()?;
    if (w + pad).saturating_mul(h + pad) > MAX_CELLS as usize {
        return None;
    }
    let mask = draw(&shapes, [b.x0, b.y0], c, w, h);
    let grown_at = [b.x0 - inset as f64 * c, b.y0 - inset as f64 * c];
    let grown = draw(&shapes, grown_at, c, w + pad, h + pad).dilated(gap, c);
    let mut order: Vec<usize> = (0..h).collect();
    order.sort_by_key(|&y| std::cmp::Reverse(mask.row(y).iter().map(|v| v.count_ones()).sum::<u32>()));
    Some(Turned { q, min: [b.x0, b.y0], mask, grown, inset, order })
}

/// Nest `pieces` (each a set of outlines, mm, where they are now) in `area`, round `fixed` (what
/// stays). Never panics; a piece with no finite outline, or larger than the area, isn't placed.
pub fn nest(pieces: &[Vec<Shape>], fixed: &[Shape], area: RectMm, opt: Options) -> Nested {
    let none = |out_of_work| Nested { placed: vec![None; pieces.len()], used_length_mm: 0.0, out_of_work };
    let (aw, ah) = (area.width(), area.height());
    if !(aw.is_finite() && ah.is_finite() && aw > 0.0 && ah > 0.0) || pieces.len() > MAX_PIECES {
        return none(false);
    }
    let c = CELL_MM.max((aw * ah / MAX_CELLS).sqrt());
    let g = if opt.gap_mm.is_finite() { opt.gap_mm.clamp(0.0, 100.0) } else { 2.0 };
    let (gw, gh) = ((aw / c).floor() as i64, (ah / c).floor() as i64);
    let (Ok(w), Ok(h)) = (usize::try_from(gw), usize::try_from(gh)) else { return none(false) };
    // What stays, grown by the gap.
    let mut sheet = draw(fixed, [area.x0, area.y0], c, w, h).dilated(g, c);
    let turns: &[u8] = if opt.rotate { &[0, 1, 2, 3] } else { &[0] };
    // Each piece's turns, and the order they are placed in: most cells first.
    let mut prepared: Vec<(usize, Vec<Turned>, u64)> = pieces
        .iter()
        .enumerate()
        .map(|(i, shapes)| {
            let ts: Vec<Turned> = turns.iter().filter_map(|&q| turned(shapes, q, c, g)).collect();
            let size = ts.first().map_or(0, |t| t.mask.count());
            (i, ts, size)
        })
        .collect();
    prepared.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    let mut placed = vec![None; pieces.len()];
    let mut work = 0u64;
    let mut out_of_work = false;
    let mut lowest = 0.0f64;
    for (i, ts, _) in &prepared {
        if work > MAX_WORK {
            out_of_work = true;
            break;
        }
        // Each turn's first fit, row by row from the top; the one whose bottom is highest wins.
        let mut best: Option<(i64, i64, usize)> = None;
        for (k, t) in ts.iter().enumerate() {
            let (cw, ch) = (t.mask.w as i64, t.mask.h as i64);
            if cw > gw || ch > gh {
                continue;
            }
            let found = 'search: {
                for y in 0..=(gh - ch) {
                    // A fit lower than the best so far can't win.
                    if best.is_some_and(|(_, by, bk)| y + ch > by + ts.get(bk).map_or(0, |b| b.mask.h as i64)) {
                        break 'search None;
                    }
                    for x in 0..=(gw - cw) {
                        if !sheet.meets(&t.mask, x, y, &t.order, &mut work) {
                            break 'search Some((x, y));
                        }
                        if work > MAX_WORK {
                            break 'search None;
                        }
                    }
                }
                None
            };
            if let Some((x, y)) = found {
                let better = match best {
                    None => true,
                    Some((bx, by, bk)) => {
                        let bh = ts.get(bk).map_or(0, |b| b.mask.h as i64);
                        (y + ch, x) < (by + bh, bx)
                    }
                };
                if better {
                    best = Some((x, y, k));
                }
            }
        }
        let Some((x, y, k)) = best else { continue };
        let Some(t) = ts.get(k) else { continue };
        sheet.add(&t.grown, x - t.inset, y - t.inset);
        let m = turn(t.q);
        let target = [area.x0 + x as f64 * c, area.y0 + y as f64 * c];
        let affine = [m[0], m[1], m[2], m[3], target[0] - t.min[0], target[1] - t.min[1]];
        lowest = lowest.max((y + t.mask.h as i64) as f64 * c);
        if let Some(slot) = placed.get_mut(*i) {
            *slot = Some(Placed { quarter_turns: t.q, affine });
        }
    }
    Nested { placed, used_length_mm: lowest, out_of_work }
}

/// A document nested: each piece moved (the transform, in document points, to give it) and the
/// ones left where they were because they didn't fit.
#[derive(Clone, Debug, PartialEq)]
pub struct DocNested {
    pub moves: Vec<(vectorcraft_doc::NodeId, vectorcraft_geom::Affine)>,
    pub unplaced: Vec<vectorcraft_doc::NodeId>,
    pub used_length_mm: f64,
    pub out_of_work: bool,
}

/// The objects nested when none are named: every top-level object on a visible, unlocked
/// top-level layer that isn't locked or hidden itself, the registration marks aside.
pub fn default_pieces(d: &vectorcraft_doc::Document) -> Vec<vectorcraft_doc::NodeId> {
    let marks = crate::marks::marks_layer(d);
    d.layers
        .iter()
        .filter(|l| l.visible && !l.locked && Some(l.id) != marks)
        .flat_map(|l| l.children().into_iter().flatten())
        .filter(|n| n.visible && !n.locked)
        .map(|n| n.id)
        .collect()
}

/// Nests `ids` (each moved as one piece, with all it holds) on `d`'s mat: in the cut area, or
/// between the registration marks for Print & Cut; the other objects that are cut stay where they
/// are and are packed round. A piece's shape is its cut outline, or its bounds when it has none
/// (art that is only printed). The transforms rotate and move, never scale.
pub fn nest_document(d: &vectorcraft_doc::Document, ids: &[vectorcraft_doc::NodeId], opt: Options) -> Result<DocNested, String> {
    use vectorcraft_geom::{Affine, PathEl, Point};
    let s = crate::settings(d);
    let placement = crate::placement::place(&s, d).ok_or("the cutting settings' artboard isn't in the document")?;
    let mut area = placement.mat.cut_area_mm;
    if s.print_and_cut {
        let m = crate::marks::layout(&s, d).ok_or("the artboard is too small for registration marks")?.area();
        let at = placement.artboard_mm;
        area = RectMm { x0: area.x0.max(at.x0 + m.x0), y0: area.y0.max(at.y0 + m.y0), x1: area.x1.min(at.x0 + m.x1), y1: area.y1.min(at.y0 + m.y1) };
    }
    let mut ids: Vec<vectorcraft_doc::NodeId> = ids.to_vec();
    ids.dedup();
    // One named inside another moves with it.
    let ancestry: Vec<Vec<vectorcraft_doc::NodeId>> = ids.iter().map(|id| d.ancestry(*id).unwrap_or_default()).collect();
    let inside_another = |k: usize| ancestry.get(k).is_some_and(|a| a.iter().rev().skip(1).any(|up| ids.contains(up)));
    let pieces: Vec<vectorcraft_doc::NodeId> = (0..ids.len()).filter(|k| !inside_another(*k)).filter_map(|k| ids.get(k).copied()).collect();
    if pieces.len() > MAX_PIECES {
        return Err(format!("at most {MAX_PIECES} pieces can be nested at once"));
    }
    // Every cut outline on the mat, in mm, with the piece it belongs to (none: it stays).
    let items = crate::select::select(d, &s);
    let (outlines, _) = crate::geometry::outlines(d, &items);
    let tolerance = s.tolerance_mm.clamp(0.001, 1.0);
    let piece_of = |node: u64| {
        let a = d.ancestry(vectorcraft_doc::NodeId(node)).unwrap_or_default();
        pieces.iter().position(|p| a.contains(p))
    };
    let mut shapes: Vec<Vec<Shape>> = vec![vec![]; pieces.len()];
    let mut fixed: Vec<Shape> = vec![];
    for o in &outlines {
        let on_mat = placement.doc_to_mat * o.path.clone();
        let mut points: Vec<[f64; 2]> = vec![];
        vectorcraft_geom::flatten(on_mat.elements().iter().copied(), tolerance, |el| {
            if let PathEl::MoveTo(p) | PathEl::LineTo(p) = el {
                points.push([p.x, p.y]);
            }
        });
        if points.iter().any(|p| !(p[0].is_finite() && p[1].is_finite())) {
            continue;
        }
        let shape = Shape { closed: o.closed, points };
        match piece_of(o.source.node) {
            Some(k) => {
                if let Some(v) = shapes.get_mut(k) {
                    v.push(shape);
                }
            }
            None => fixed.push(shape),
        }
    }
    // A piece with nothing cut: its bounds.
    let symbols = d.symbol_boxes();
    for (k, id) in pieces.iter().enumerate() {
        let Some(v) = shapes.get_mut(k) else { continue };
        if v.is_empty()
            && let Some(b) = d.node(*id).and_then(|n| n.visual_bounds_with(&symbols))
        {
            let corners = [Point::new(b.x0, b.y0), Point::new(b.x1, b.y0), Point::new(b.x1, b.y1), Point::new(b.x0, b.y1)];
            v.push(Shape { closed: true, points: corners.iter().map(|p| placement.doc_to_mat * *p).map(|p| [p.x, p.y]).collect() });
        }
    }
    let nested = nest(&shapes, &fixed, area, opt);
    let to_doc = placement.doc_to_mat.inverse();
    let mut moves = vec![];
    let mut unplaced = vec![];
    for (k, id) in pieces.iter().enumerate() {
        match nested.placed.get(k).copied().flatten() {
            Some(p) => moves.push((*id, to_doc * Affine::new(p.affine) * placement.doc_to_mat)),
            None => unplaced.push(*id),
        }
    }
    Ok(DocNested { moves, unplaced, used_length_mm: nested.used_length_mm, out_of_work: nested.out_of_work })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, w: f64, h: f64) -> Shape {
        Shape { closed: true, points: vec![[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]] }
    }

    fn moved(s: &Shape, p: &Placed) -> Shape {
        let a = p.affine;
        Shape { closed: s.closed, points: s.points.iter().map(|q| [a[0] * q[0] + a[2] * q[1] + a[4], a[1] * q[0] + a[3] * q[1] + a[5]]).collect() }
    }

    fn rect_of(s: &Shape) -> RectMm {
        bounds(s.points.iter()).unwrap()
    }

    const AREA: RectMm = RectMm { x0: 10.0, y0: 10.0, x1: 310.0, y1: 310.0 };

    /// Every placed outline on the area, and every two placed rectangles the gap apart.
    fn check_apart(rects: &[RectMm], gap: f64) {
        for r in rects {
            assert!(r.x0 >= AREA.x0 - 1e-9 && r.y0 >= AREA.y0 - 1e-9 && r.x1 <= AREA.x1 + 1e-9 && r.y1 <= AREA.y1 + 1e-9, "{r:?} off the area");
        }
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                let dx = (b.x0 - a.x1).max(a.x0 - b.x1).max(0.0);
                let dy = (b.y0 - a.y1).max(a.y0 - b.y1).max(0.0);
                assert!((dx * dx + dy * dy).sqrt() >= gap - 1e-9, "{a:?} and {b:?} closer than {gap}");
            }
        }
    }

    #[test]
    fn rectangles_pack_from_the_top_left_the_gap_apart() {
        let pieces: Vec<Vec<Shape>> = (0..6).map(|i| vec![rect(500.0 + 70.0 * i as f64, 500.0, 60.0, 40.0)]).collect();
        let n = nest(&pieces, &[], AREA, Options { gap_mm: 2.0, rotate: false });
        assert!(n.placed.iter().all(Option::is_some) && !n.out_of_work);
        let rects: Vec<RectMm> = pieces.iter().zip(&n.placed).map(|(p, at)| rect_of(&moved(&p[0], at.as_ref().unwrap()))).collect();
        check_apart(&rects, 2.0);
        assert!(rects.iter().any(|r| (r.x0 - AREA.x0).abs() < 1e-9 && (r.y0 - AREA.y0).abs() < 1e-9), "one at the top left: {rects:?}");
        // Four 60 mm wide in a 300 mm row with 2 mm gaps, then the rest below.
        // A cell or two of rounding on top of the gap.
        assert!(n.used_length_mm <= 40.0 * 2.0 + 2.0 + 4.0 * CELL_MM, "two rows: {}", n.used_length_mm);
    }

    /// A long piece fits only turned.
    #[test]
    fn a_piece_is_turned_when_that_is_what_fits() {
        let area = RectMm { x0: 0.0, y0: 0.0, x1: 50.0, y1: 300.0 };
        let piece = vec![rect(0.0, 0.0, 200.0, 20.0)];
        assert!(nest(std::slice::from_ref(&piece), &[], area, Options { rotate: false, ..Options::default() }).placed[0].is_none());
        let n = nest(std::slice::from_ref(&piece), &[], area, Options::default());
        let at = n.placed[0].unwrap();
        assert!(at.quarter_turns % 2 == 1, "{at:?}");
        let r = rect_of(&moved(&piece[0], &at));
        assert!((r.width() - 20.0).abs() < 1e-9 && (r.height() - 200.0).abs() < 1e-9 && r.x0 >= 0.0 && r.y1 <= 300.0, "{r:?}");
    }

    /// True shapes: a small square goes in a ring's hole; what stays is packed round.
    #[test]
    fn a_small_piece_goes_in_a_hole_and_round_what_stays() {
        let ring = vec![rect(0.0, 0.0, 100.0, 100.0), rect(20.0, 20.0, 60.0, 60.0)];
        let small = vec![rect(0.0, 0.0, 30.0, 30.0)];
        let area = RectMm { x0: 0.0, y0: 0.0, x1: 120.0, y1: 120.0 };
        let n = nest(&[ring.clone(), small.clone()], &[], area, Options { gap_mm: 2.0, rotate: false });
        let (ring_at, small_at) = (n.placed[0].unwrap(), n.placed[1].unwrap());
        let hole = rect_of(&moved(&ring[1], &ring_at));
        let s = rect_of(&moved(&small[0], &small_at));
        assert!(
            s.x0 >= hole.x0 + 2.0 - 1e-9 && s.y0 >= hole.y0 + 2.0 - 1e-9 && s.x1 <= hole.x1 - 2.0 + 1e-9 && s.y1 <= hole.y1 - 2.0 + 1e-9,
            "in the hole: {s:?} {hole:?}"
        );
        // A fixed square at the top left: the piece goes beside it, the gap away.
        let fixed = [rect(0.0, 0.0, 50.0, 50.0)];
        let n = nest(std::slice::from_ref(&small), &fixed, area, Options { gap_mm: 2.0, rotate: false });
        let s = rect_of(&moved(&small[0], &n.placed[0].unwrap()));
        assert!(s.x0 >= 52.0 - 1e-9 || s.y0 >= 52.0 - 1e-9, "round what stays: {s:?}");
    }

    #[test]
    fn what_cant_be_nested_isnt() {
        let n = nest(&[vec![rect(0.0, 0.0, 400.0, 400.0)], vec![]], &[], AREA, Options::default());
        assert_eq!(n.placed, [None, None], "too large, and nothing to place");
        let bad = Shape { closed: true, points: vec![[f64::NAN, 0.0], [1.0, 1.0], [0.0, 1.0]] };
        assert_eq!(nest(&[vec![bad]], &[], AREA, Options::default()).placed, [None]);
        let empty = RectMm { x0: 0.0, y0: 0.0, x1: 0.0, y1: 10.0 };
        assert_eq!(nest(&[vec![rect(0.0, 0.0, 1.0, 1.0)]], &[], empty, Options::default()).placed, [None]);
    }

    /// The grid never lets two pieces closer than the gap, whatever their shapes and turns.
    #[test]
    fn triangles_and_turns_keep_the_gap() {
        let tri = |s: f64| Shape { closed: true, points: vec![[0.0, 0.0], [s, 0.0], [0.0, s]] };
        let pieces: Vec<Vec<Shape>> = (0..12).map(|i| vec![tri(30.0 + 5.0 * i as f64)]).collect();
        let n = nest(&pieces, &[], AREA, Options { gap_mm: 3.0, rotate: true });
        let placed: Vec<Shape> = pieces.iter().zip(&n.placed).filter_map(|(p, at)| at.map(|at| moved(&p[0], &at))).collect();
        assert!(placed.len() >= 10, "most fit: {}", placed.len());
        // Point-to-segment distance between every two triangles.
        let seg = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
        };
        for (i, a) in placed.iter().enumerate() {
            for b in placed.iter().skip(i + 1) {
                for (x, y) in [(a, b), (b, a)] {
                    for p in &x.points {
                        for k in 0..3 {
                            let d = seg(*p, y.points[k], y.points[(k + 1) % 3]);
                            assert!(d >= 3.0 - 1e-6, "{d} < 3 between {x:?} and {y:?}");
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod scale {
    use super::*;

    /// Eighty round pieces of mixed sizes on a 12 × 12 in cut area: all placed, well inside the
    /// work cap, in reasonable time even in a test build.
    #[test]
    fn eighty_pieces_nest_quickly() {
        let circle = |r: f64| Shape {
            closed: true,
            points: (0..48)
                .map(|k| {
                    let a = k as f64 / 48.0 * std::f64::consts::TAU;
                    [r * a.cos(), r * a.sin()]
                })
                .collect(),
        };
        let pieces: Vec<Vec<Shape>> = (0..80).map(|i| vec![circle(5.0 + (i % 7) as f64 * 3.0)]).collect();
        let area = RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 };
        let started = std::time::Instant::now();
        let n = nest(&pieces, &[], area, Options::default());
        let took = started.elapsed();
        let placed = n.placed.iter().filter(|p| p.is_some()).count();
        eprintln!("80 circles: {placed} placed, {:.0} mm used, {took:?}", n.used_length_mm);
        assert_eq!(placed, 80);
        assert!(!n.out_of_work);
        assert!(took.as_secs_f64() < 20.0, "{took:?}");
    }
}

#[cfg(test)]
mod document {
    use vectorcraft_doc::{Appearance, Document, Node};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    /// Squares spread past the mat's edge: nested, every one is moved onto the mat, the job builds
    /// (none off the cut area) and each piece is one transform without scaling.
    #[test]
    fn a_documents_pieces_come_onto_the_mat() {
        let mut d = Document::new(1200.0, 400.0);
        let layer = d.layers[0].id;
        for k in 0..5 {
            let x = 50.0 + 220.0 * f64::from(k);
            let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, 50.0, x + 150.0, 150.0)), Appearance::default());
            d.insert(Some(layer), usize::MAX, sq).unwrap();
        }
        assert!(crate::build_job(&d).is_err(), "off the mat before");
        let ids = default_pieces(&d);
        assert_eq!(ids.len(), 5);
        let n = nest_document(&d, &ids, Options::default()).unwrap();
        assert!(n.unplaced.is_empty() && n.moves.len() == 5, "{n:?}");
        for (id, a) in &n.moves {
            let [a0, b, c, d0, ..] = a.as_coeffs();
            assert!(((a0 * d0 - b * c).abs() - 1.0).abs() < 1e-9, "no scaling");
            d.node_mut(*id).unwrap().transform(*a, false);
        }
        let job = crate::build_job(&d).unwrap();
        assert_eq!(job.paths.len(), 5, "all on the cut area");
    }
}
