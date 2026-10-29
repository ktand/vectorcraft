//! The cutting workspace's core (fork: `docs/cutting/ARCHITECTURE.md`): what a document says
//! about cutting, read from and written to the slots VectorCraft already keeps (§4), so the
//! document model and the native format are unchanged, and the cut job made from it.
//!
//! - [`ops`]: each object's operation, a foreign attribute (`cut:op`), inherited from its
//!   ancestors and otherwise the document's default.
//! - [`settings`] and [`set_settings`]: the document's [`CuttingSettings`], under
//!   `Document::unknown["cutting"]`.
//! - [`placement`]: where the settings' artboard lies on the mat, document points to mat mm.
//! - [`select`]: which objects a job cuts, and with which operation.
//! - [`geometry`]: the outlines each of them cuts along, as drawn.
//! - [`clip`]: those outlines cut to the clipping masks around them.
//! - [`weld`]: the shapes set to Cut Outline united, cut along the outline of the union.
//! - [`prepare`]: those outlines on the mat, in millimetres, flattened.
//! - [`group`]: which group each object is cut in, when the job is cut in groups.
//! - [`order`]: the order they are cut in.
//! - [`corners`]: sharp corners run past and back, for the operations that ask for it.
//! - [`marks`]: Print & Cut's registration marks, a locked layer of the document, and where they
//!   lie on the mat for the job.
//! - [`preview`]: a job as an SVG at the mat's size.
//! - [`build_job`]: all of it, a document to a validated [`CutJob`]. The document isn't changed:
//!   the job is made from it each time.
#![forbid(unsafe_code)]

pub mod clip;
pub mod corners;
pub mod geometry;
pub mod group;
pub mod marks;
pub mod ops;
pub mod order;
pub mod placement;
pub mod prepare;
pub mod preview;
pub mod select;
pub mod setups;
pub mod test_cut;
pub mod weed;
pub mod weld;

use vectorcraft_cutjob::{CutJob, CuttingSettings, DOCUMENT_KEY, JOB_VERSION, JobError, JobMat, OpKind, Operation, Toolpath};
use vectorcraft_doc::Document;

use crate::prepare::PrepError;

/// The document's cutting settings (the defaults when it has none, read leniently when damaged).
pub fn settings(d: &Document) -> CuttingSettings {
    CuttingSettings::from_value(d.unknown.get(DOCUMENT_KEY))
}

/// Keeps `s` as the document's cutting settings.
pub fn set_settings(d: &mut Document, s: &CuttingSettings) {
    d.unknown.insert(DOCUMENT_KEY.to_string(), s.to_value());
}

/// Why a document can't be made into a job.
#[derive(Clone, Debug, PartialEq)]
pub enum BuildError {
    /// The cutting settings name an artboard the document doesn't have.
    NoArtboard(u32),
    Prepare(PrepError),
    Invalid(JobError),
    /// Print & Cut can't be laid out (why).
    PrintAndCut(String),
    /// The test cut can't be made (why).
    TestCut(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::NoArtboard(i) => write!(f, "the cutting settings name artboard {i}, which the document doesn't have"),
            BuildError::Prepare(e) => e.fmt(f),
            BuildError::Invalid(e) => write!(f, "the job isn't valid: {e}"),
            BuildError::PrintAndCut(e) => e.fmt(f),
            BuildError::TestCut(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for BuildError {}

/// The cut job `d` makes with its cutting settings: what [`select`] picks, cut along its
/// [`geometry`], laid on the mat by [`prepare`] and put in [`order`], validated. The same document
/// and settings always make the same job.
pub fn build_job(d: &Document) -> Result<CutJob, BuildError> {
    let s = settings(d);
    let placement = placement::place(&s, d).ok_or(BuildError::NoArtboard(s.artboard))?;
    let items = select::select(d, &s);
    let (outlines, mut warnings) = geometry::outlines(d, &items);
    let (outlines, welding) = weld::weld(outlines);
    warnings.extend(welding);
    let (polylines, more) = prepare::prepare(&outlines, &placement, s.tolerance_mm, s.clip_to_mat).map_err(BuildError::Prepare)?;
    warnings.extend(more);
    let registration = if s.print_and_cut {
        let (r, near) = marks::registration(&s, d, &placement, &polylines).map_err(BuildError::PrintAndCut)?;
        warnings.extend(near);
        Some(r)
    } else {
        None
    };
    let area = placement.mat.cut_area_mm;
    // Each group in the order its first object comes, ordered by itself, the blade going on from
    // where the group before left it (one group without grouping).
    let mut names: Vec<String> = vec![];
    let mut group_of_node = std::collections::HashMap::new();
    for item in &items {
        let name = group::group_of(d, item.id, s.grouping);
        let index = names.iter().position(|n| *n == name).unwrap_or_else(|| {
            names.push(name);
            names.len() - 1
        });
        group_of_node.insert(item.id.0, index);
    }
    let mut buckets: Vec<Vec<prepare::Polyline>> = vec![vec![]; names.len().max(1)];
    for p in polylines {
        let g = group_of_node.get(&p.source.node).copied().unwrap_or(0);
        if let Some(b) = buckets.get_mut(g) {
            b.push(p);
        }
    }
    // Each group's own setup (C7.4), if it has one; else the document's.
    let setups: Vec<Option<vectorcraft_cutjob::GroupSetup>> = names.iter().map(|n| setups::setup_of(d, &s, n)).collect();
    // The paths in order, each with the group whose setup it is cut with (none: the document's).
    let mut ordered: Vec<(prepare::Polyline, Option<usize>)> = vec![];
    let mut groups = vec![];
    let mut at = [area.x0, area.y0];
    for (g, bucket) in buckets.into_iter().enumerate() {
        if bucket.is_empty() {
            continue;
        }
        let setup = setups.get(g).and_then(Option::as_ref);
        if s.grouping != vectorcraft_cutjob::Grouping::None {
            let (name, material) = (names.get(g).cloned().unwrap_or_default(), setup.and_then(|x| x.material.clone()));
            groups.push(vectorcraft_cutjob::JobGroup { name, start: ordered.len(), material });
        }
        let mut these = order::order(bucket, s.ordering, at);
        // Weeding after the group's own paths, within what may be cut (C8.2, C8.3).
        let may_cut = registration.map_or(area, |r| {
            let m = r.area();
            vectorcraft_cutjob::RectMm { x0: m.x0.max(area.x0), y0: m.y0.max(area.y0), x1: m.x1.min(area.x1), y1: m.y1.min(area.y1) }
        });
        let (weeding, said) = weed::weeding(&these, may_cut, &s);
        these.extend(weeding);
        warnings.extend(said);
        if let Some(last) = these.last().and_then(|p| if p.closed { p.points.first() } else { p.points.last() }) {
            at = *last;
        }
        let slot = setup.map(|_| g);
        ordered.extend(these.into_iter().map(|p| (p, slot)));
    }
    // One operation for each kind and setup the job uses, scored first.
    let mut keys: Vec<(OpKind, Option<usize>)> = ordered.iter().map(|(p, slot)| (p.op, *slot)).collect();
    keys.sort();
    keys.dedup();
    let operations: Vec<Operation> = keys
        .iter()
        .map(|&(kind, slot)| {
            let own = slot.and_then(|g| setups.get(g)).and_then(Option::as_ref);
            let tool = match (kind, own) {
                (OpKind::Cut, Some(x)) => x.cut.clone(),
                (OpKind::Score, Some(x)) => x.score.clone(),
                (OpKind::Cut, None) => s.operations.cut.clone(),
                (OpKind::Score, None) => s.operations.score.clone(),
            };
            Operation { kind, tool }
        })
        .collect();
    let paths = ordered
        .into_iter()
        .map(|(p, slot)| Toolpath {
            op: keys.iter().position(|k| *k == (p.op, slot)).unwrap_or(0),
            source: p.source,
            closed: p.closed,
            points: p.points,
        })
        .collect();
    let mut job = CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: placement.mat.id.to_string(), size_mm: placement.mat.size_mm, cut_area_mm: area },
        operations,
        paths,
        warnings,
        track_enhancing: s.track_enhancing,
        groups,
        registration,
    };
    corners::overcut_corners(&mut job);
    job.validate().map_err(BuildError::Invalid)?;
    Ok(job)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_cutjob::Orientation;

    use super::*;

    #[test]
    fn settings_live_in_the_documents_foreign_data() {
        let mut d = Document::new(100.0, 100.0);
        assert_eq!(settings(&d), CuttingSettings::default());
        let s = CuttingSettings { orientation: Orientation::Landscape, ..CuttingSettings::default() };
        set_settings(&mut d, &s);
        assert_eq!(settings(&d), s);
        assert_eq!(d.unknown[DOCUMENT_KEY]["orientation"], json!("landscape"));
        d.unknown.insert(DOCUMENT_KEY.into(), json!("damaged"));
        assert_eq!(settings(&d), CuttingSettings::default());
    }

    #[test]
    fn an_empty_document_makes_an_empty_job() {
        let job = build_job(&Document::new(100.0, 100.0)).unwrap();
        assert!(job.paths.is_empty() && job.operations.is_empty() && job.warnings.is_empty());
    }

    /// Grouped by layer: a group a layer, bottom first, each one's paths together.
    #[test]
    fn grouped_by_layer_the_layers_go_one_after_the_other() {
        use vectorcraft_doc::{Appearance, LayerColor, Node};
        use vectorcraft_geom::{Rect, shapes};
        let mut d = Document::new(500.0, 500.0);
        let bottom = d.layers[0].id;
        let top = Node::layer(d.alloc_id(), "Second", LayerColor::Preset(1));
        let top = d.insert(None, usize::MAX, top).unwrap();
        for (layer, x) in [(bottom, 10.0), (top, 20.0), (bottom, 300.0)] {
            let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, 10.0, x + 20.0, 30.0)), Appearance::default());
            d.insert(Some(layer), usize::MAX, sq).unwrap();
        }
        set_settings(&mut d, &CuttingSettings { grouping: vectorcraft_cutjob::Grouping::Layer, ..CuttingSettings::default() });
        let job = build_job(&d).unwrap();
        let names: Vec<(&str, usize)> = job.groups.iter().map(|g| (g.name.as_str(), g.start)).collect();
        let first = d.layers[0].name.clone().unwrap_or_default();
        assert_eq!(names, [(first.as_str(), 0), ("Second", 2)]);
        assert_eq!(job.paths.len(), 3);
        // Without grouping: one group, the squares in travel order.
        set_settings(&mut d, &CuttingSettings::default());
        assert!(build_job(&d).unwrap().groups.is_empty());
    }

    /// C7.4: a group with a setup of its own is cut with it, the others with the document's; the
    /// job names the material to load before it. By layer from the layer, by colour from the
    /// settings.
    #[test]
    fn each_group_is_cut_with_its_own_setup() {
        use vectorcraft_color::{Color, Paint};
        use vectorcraft_cutjob::{GroupSetup, Grouping, builtin_materials};
        use vectorcraft_doc::{Appearance, LayerColor, Node};
        use vectorcraft_geom::{Rect, shapes};
        let mut d = Document::new(500.0, 500.0);
        let bottom = d.layers[0].id;
        let gold = Node::layer(d.alloc_id(), "Gold", LayerColor::Preset(1));
        let gold = d.insert(None, usize::MAX, gold).unwrap();
        let red = Appearance::basic(Paint::None, Paint::solid(Color::rgb(1.0, 0.0, 0.0)), 1.0);
        for (layer, x) in [(bottom, 10.0), (gold, 100.0), (gold, 200.0)] {
            let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, 10.0, x + 20.0, 30.0)), red.clone());
            d.insert(Some(layer), usize::MAX, sq).unwrap();
        }
        let card = builtin_materials().into_iter().find(|m| m.name == "Cardstock").unwrap();
        setups::set_layer_setup(d.node_mut(gold).unwrap(), Some(&GroupSetup::of_material(&card)));
        let by_layer = CuttingSettings { grouping: Grouping::Layer, ..CuttingSettings::default() };
        set_settings(&mut d, &by_layer);
        let job = build_job(&d).unwrap();
        assert_eq!(job.operations.len(), 2, "the document's cut and Gold's: {:?}", job.operations);
        let tool = |i: usize| &job.operations[job.paths[i].op].tool;
        assert!(tool(0).same_setup(&by_layer.operations.cut), "the first layer: the document's");
        assert!(tool(1).same_setup(&card.cut) && tool(2).same_setup(&card.cut), "Gold's own");
        let materials: Vec<Option<&str>> = job.groups.iter().map(|g| g.material.as_deref()).collect();
        assert_eq!(materials, [None, Some("Cardstock")]);
        // Cut by colour, the layer's setup isn't used; the colour's is.
        let mut by_colour = CuttingSettings { grouping: Grouping::Color, ..CuttingSettings::default() };
        set_settings(&mut d, &by_colour);
        let job = build_job(&d).unwrap();
        assert_eq!(job.operations.len(), 1, "one colour, the document's setup");
        by_colour.color_setups.insert("#ff0000".into(), GroupSetup::of_material(&card));
        set_settings(&mut d, &by_colour);
        let job = build_job(&d).unwrap();
        assert!(job.operations.len() == 1 && job.operations[0].tool.same_setup(&card.cut));
        assert_eq!(job.groups[0].material.as_deref(), Some("Cardstock"));
    }

    /// C8.2, C8.3: weeding comes after each group's own paths, the box last, made by the job
    /// (node 0), with that group's cut.
    #[test]
    fn weeding_follows_each_group() {
        use vectorcraft_doc::{Appearance, LayerColor, Node};
        use vectorcraft_geom::{Rect, shapes};
        let mut d = Document::new(500.0, 500.0);
        let bottom = d.layers[0].id;
        let top = Node::layer(d.alloc_id(), "Second", LayerColor::Preset(1));
        let top = d.insert(None, usize::MAX, top).unwrap();
        for (layer, x) in [(bottom, 50.0), (top, 300.0)] {
            let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, 50.0, x + 60.0, 110.0)), Appearance::default());
            d.insert(Some(layer), usize::MAX, sq).unwrap();
        }
        let s = CuttingSettings { grouping: vectorcraft_cutjob::Grouping::Layer, weed_box: true, weed_lines: true, ..CuttingSettings::default() };
        set_settings(&mut d, &s);
        let job = build_job(&d).unwrap();
        // Each group: its square, four weed lines, its box.
        assert_eq!(job.paths.len(), 2 * 6, "{:?}", job.paths.iter().map(|p| p.source.node).collect::<Vec<_>>());
        assert_eq!(job.groups[1].start, 6);
        for g in [0, 6] {
            assert_ne!(job.paths[g].source.node, 0, "the design first");
            assert!(job.paths[g + 1..g + 6].iter().all(|p| p.source == weed::SOURCE));
            assert!(job.paths[g + 5].closed, "the box last");
        }
        set_settings(&mut d, &CuttingSettings { weed_box: false, weed_lines: false, ..s });
        assert_eq!(build_job(&d).unwrap().paths.len(), 2, "off: the design alone");
    }

    #[test]
    fn a_missing_artboard_is_an_error() {
        let mut d = Document::new(100.0, 100.0);
        set_settings(&mut d, &CuttingSettings { artboard: 3, ..CuttingSettings::default() });
        assert_eq!(build_job(&d), Err(BuildError::NoArtboard(3)));
    }
}
