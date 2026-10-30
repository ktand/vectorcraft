//! C3.9: random documents and cutting settings make a job or say why, never a panic. Coordinates
//! that aren't numbers or are huge, paths of a point or none, groups and clip groups inside each
//! other, compound paths, symbol instances placed anywhere, hidden objects, operations that
//! aren't one and settings of any shape. A job that comes back validates, and its simulation,
//! stats and preview are made without a panic too.
//!
//!
//! C8.7: the data added since reads leniently too: each layer's own setup (`cut:setup`, any text),
//! the settings' colour setups and weeding, and every random document also goes through the test
//! cut, the group listing and nesting (on one sheet and onto more). Nesting's packer gets shapes of
//! any coordinates on its own, and what it places keeps the gap.
//!
//! `FUZZ_COUNT=1 cargo test -p vectorcraft-cutting --test fuzz -- --nocapture` prints each case's
//! outcome, to check how many cases reach a job (about a fifth make one with paths; most of the
//! rest are documents with a huge coordinate, refused as off the mat).
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use proptest::prelude::*;
use serde_json::{Map, Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::{DOCUMENT_KEY, Simulation};
use vectorcraft_cutting::build_job;
use vectorcraft_cutting::ops::{NAME, NS, PREFIX};
use vectorcraft_doc::{Appearance, Document, ForeignAttribute, Node, NodeId, NodeKind, Symbol};
use vectorcraft_geom::{Affine, BezPath, PathData};

/// A coordinate, mostly on a mat-sized page, sometimes anything at all.
fn coord() -> impl Strategy<Value = f64> {
    prop_oneof![
        // On the 12 × 12 in mat (864 pt), with room for the curves' handles.
        24 => 10.0..800.0f64,
        2 => -50.0..900.0f64,
        1 => Just(f64::NAN),
        1 => Just(f64::INFINITY),
        1 => Just(-1e300),
        1 => any::<f64>(),
    ]
}

/// What an object's `cut:op` says, if anything.
fn op() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        4 => Just(None),
        1 => Just(Some("cut".to_string())),
        1 => Just(Some("score".to_string())),
        1 => Just(Some("none".to_string())),
        1 => Just(Some("outline".to_string())),
        1 => ".{0,12}".prop_map(Some),
    ]
}

/// The shape of an object, before ids are given out.
#[derive(Clone, Debug)]
enum Art {
    /// `stroke`: a black stroke that wide, if any (C11.4: what a stroke's edge goes round).
    Path {
        points: Vec<(f64, f64)>,
        closed: bool,
        curve: bool,
        stroke: Option<f64>,
    },
    Compound(Vec<Art>),
    Group {
        children: Vec<Art>,
        clip: bool,
    },
    Symbol {
        xf: [f64; 6],
    },
}

#[derive(Clone, Debug)]
struct Obj {
    art: Art,
    op: Option<String>,
    /// What its `cut:edge` says, if anything.
    edge: Option<String>,
    visible: bool,
}

/// A stroke width: none, ordinary, huge or not a number.
fn stroke() -> impl Strategy<Value = Option<f64>> {
    prop_oneof![
        3 => Just(None),
        3 => (0.0..40.0f64).prop_map(Some),
        1 => prop_oneof![Just(1e9), Just(f64::NAN), Just(f64::INFINITY), Just(-3.0)].prop_map(Some),
    ]
}

fn edge() -> impl Strategy<Value = Option<String>> {
    prop_oneof![3 => Just(None), 2 => Just(Some("stroke".to_string())), 1 => Just(Some("path".to_string())), 1 => ".{0,8}".prop_map(Some)]
}

fn path() -> impl Strategy<Value = Art> {
    (prop::collection::vec((coord(), coord()), 0..8), any::<bool>(), any::<bool>(), stroke()).prop_map(|(points, closed, curve, stroke)| Art::Path {
        points,
        closed,
        curve,
        stroke,
    })
}

fn obj() -> impl Strategy<Value = Obj> {
    let leaf = prop_oneof![
        6 => path(),
        1 => prop::array::uniform6(coord()).prop_map(|xf| Art::Symbol { xf }),
    ];
    let art = leaf.prop_recursive(3, 24, 5, |inner| {
        prop_oneof![
            prop::collection::vec(path(), 1..4).prop_map(Art::Compound),
            (prop::collection::vec(inner, 0..5), any::<bool>()).prop_map(|(children, clip)| Art::Group { children, clip }),
        ]
    });
    (art, op(), edge(), prop::bool::weighted(0.9)).prop_map(|(art, op, edge, visible)| Obj { art, op, edge, visible })
}

fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        (-1000i64..1000).prop_map(Value::from),
        coord().prop_map(|f| serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)),
        prop_oneof![
            Just("custom"),
            Just("landscape"),
            Just("document"),
            Just("silhouette.12x24"),
            Just("cut"),
            Just("none"),
            Just("layer"),
            Just("color"),
            Just("Cardstock"),
            Just(""),
        ]
        .prop_map(Value::from),
    ];
    let key = prop_oneof![
        Just("mat"),
        Just("customMatMm"),
        Just("orientation"),
        Just("artboard"),
        Just("offsetMm"),
        Just("defaultOp"),
        Just("toleranceMm"),
        Just("operations"),
        Just("ordering"),
        Just("clipToMat"),
        Just("cut"),
        Just("score"),
        Just("passes"),
        Just("overcutMm"),
        Just("grouping"),
        Just("colorSetups"),
        Just("material"),
        Just("printAndCut"),
        Just("markInsetMm"),
        Just("weedBox"),
        Just("weedMarginMm"),
        Just("weedLines"),
        Just("weedLineSpacingMm"),
        Just("bladeOffsetMm"),
        Just("depth"),
        Just("#000000"),
        Just("Gold"),
        Just("tool"),
        Just("rotary"),
        Just("pen"),
    ];
    leaf.prop_recursive(3, 32, 6, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..3).prop_map(Value::Array),
            prop::collection::btree_map(key.clone().prop_map(str::to_string), inner, 0..6)
                .prop_map(|m| Value::Object(m.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

fn bez(points: &[(f64, f64)], closed: bool, curve: bool) -> BezPath {
    let mut p = BezPath::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        match (i, curve) {
            (0, _) => p.move_to((x, y)),
            (_, true) => p.curve_to((x + 5.0, y - 5.0), (x - 5.0, y + 5.0), (x, y)),
            (_, false) => p.line_to((x, y)),
        }
    }
    if closed && !points.is_empty() {
        p.close_path();
    }
    p
}

fn build(d: &mut Document, art: &Art) -> Node {
    match art {
        Art::Path { points, closed, curve, stroke } => {
            let a = stroke.map_or_else(Appearance::default, |w| Appearance::basic(Paint::None, Paint::solid(Color::BLACK), w));
            Node::path(d.alloc_id(), PathData::from_bezpath(&bez(points, *closed, *curve)), a)
        }
        Art::Compound(parts) => {
            let children = parts.iter().map(|p| Arc::new(build(d, p))).collect();
            Node::new(d.alloc_id(), NodeKind::Compound { children, rule: Default::default() })
        }
        Art::Group { children, clip } => {
            let children = children.iter().map(|c| Arc::new(build(d, c))).collect();
            Node::new(d.alloc_id(), NodeKind::Group { children, clip: *clip })
        }
        Art::Symbol { xf } => Node::new(d.alloc_id(), NodeKind::SymbolInstance { symbol: "s".into(), xf: Affine::new(*xf) }),
    }
}

/// A layer beside the first: its name, and what its `cut:setup` says, if anything.
#[derive(Clone, Debug)]
struct Layer {
    name: String,
    setup: Option<String>,
}

fn layer() -> impl Strategy<Value = Layer> {
    let setup = prop_oneof![
        2 => Just(None),
        3 => json_value().prop_map(|v| Some(v.to_string())),
        1 => Just(Some(r#"{"material":"Vinyl","cut":{"force":0.5,"passes":2},"score":{}}"#.to_string())),
        1 => ".{0,40}".prop_map(Some),
    ];
    (prop_oneof![Just("Gold".to_string()), Just("Layer 1".to_string()), ".{0,10}"], setup).prop_map(|(name, setup)| Layer { name, setup })
}

fn document(objs: &[Obj], settings: &Value) -> Document {
    document_with(objs, &[], settings)
}

fn document_with(objs: &[Obj], layers: &[Layer], settings: &Value) -> Document {
    let mut d = Document::new(500.0, 500.0);
    let art = Node::path(d.alloc_id(), PathData::from_bezpath(&bez(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], true, false)), Appearance::default());
    d.symbols.push(Symbol { name: "s".into(), art: Arc::new(art) });
    let mut targets: Vec<NodeId> = vec![d.layers[0].id];
    for l in layers {
        let mut n = Node::layer(d.alloc_id(), &l.name, vectorcraft_doc::LayerColor::Preset(1));
        if let Some(v) = &l.setup {
            let name = vectorcraft_cutting::setups::NAME.to_string();
            n.edit_attrs(|a| a.foreign.push(ForeignAttribute { ns: NS.into(), prefix: PREFIX.into(), name, value: v.clone() }));
        }
        targets.push(d.insert(None, usize::MAX, n).unwrap());
    }
    for (k, o) in objs.iter().enumerate() {
        let layer = targets[k % targets.len()];
        let mut n = build(&mut d, &o.art);
        n.visible = o.visible;
        if let Some(v) = &o.op {
            n.edit_attrs(|a| a.foreign.push(ForeignAttribute { ns: NS.into(), prefix: PREFIX.into(), name: NAME.into(), value: v.clone() }));
        }
        if let Some(v) = &o.edge {
            let name = vectorcraft_cutting::edge::NAME.to_string();
            n.edit_attrs(|a| a.foreign.push(ForeignAttribute { ns: NS.into(), prefix: PREFIX.into(), name, value: v.clone() }));
        }
        d.insert(Some(layer), usize::MAX, n).unwrap();
    }
    d.unknown.insert(DOCUMENT_KEY.into(), settings.clone());
    d
}

proptest! {
    /// 256 cases by default; `PROPTEST_CASES=20000` for a longer run.
    #[test]
    fn any_document_makes_a_job_or_says_why(
        objs in prop::collection::vec(obj(), 0..8),
        layers in prop::collection::vec(layer(), 0..3),
        settings in prop_oneof![Just(json!({})), json_value(), json_value().prop_map(|v| json!({"grouping": "layer", "weedBox": true, "weedLines": true, "colorSetups": {"#000000": v}}))],
    ) {
        let d = document_with(&objs, &layers, &settings);
        // C8.7: the groups, the test cut at each corner, nesting: an answer or a reason, no panic.
        let s = vectorcraft_cutting::settings(&d);
        let groups = vectorcraft_cutting::setups::group_names(&d, &s);
        for g in &groups {
            let _ = vectorcraft_cutting::setups::setup_of(&d, &s, g);
        }
        for (_, corner) in vectorcraft_cutting::test_cut::Corner::CHOICES {
            if let Ok(job) = vectorcraft_cutting::test_cut::test_job(&d, corner, groups.first().map(String::as_str)) {
                prop_assert_eq!(job.validate(), Ok(()));
            }
        }
        let pieces = vectorcraft_cutting::nest::default_pieces(&d);
        if let Ok(n) = vectorcraft_cutting::nest::nest_document(&d, &pieces, vectorcraft_cutting::nest::Options::default()) {
            for (_, a) in &n.moves {
                prop_assert!(a.as_coeffs().iter().all(|v| v.is_finite()));
            }
        }
        let mut sheets = d.clone();
        let _ = vectorcraft_cutting::nest::nest_sheets(&mut sheets, &pieces, vectorcraft_cutting::nest::Options { gap_mm: 1.0, rotate: true });
        prop_assert!(sheets.artboards.len() <= d.artboards.len() + vectorcraft_cutting::nest::MAX_SHEETS);
        // C11.1: a contour round everything, holes or not: a path of closed lines or a reason.
        let all: Vec<NodeId> = d.layers.iter().map(|l| l.id).collect();
        for holes in [false, true] {
            if let Ok(c) = vectorcraft_cutting::contour::contour(&d, &all, vectorcraft_cutting::contour::Options { offset_mm: 2.0, holes }) {
                prop_assert!(c.subpaths.iter().all(|s| s.anchors.iter().all(|a| a.p.x.is_finite() && a.p.y.is_finite())));
            }
        }
        // C11.5: stencil bridges both ways: a finite path or a reason.
        use vectorcraft_cutting::bridges::{Direction, Options, bridged};
        for direction in [Direction::Vertical, Direction::Horizontal] {
            if let Ok(b) = bridged(&d, &all, Options { width_mm: 1.5, direction }) {
                prop_assert!(b.path.subpaths.iter().all(|s| s.anchors.iter().all(|a| a.p.x.is_finite() && a.p.y.is_finite())));
            }
        }
        match build_job(&d) {
            Ok(job) => {
                if std::env::var_os("FUZZ_COUNT").is_some() {
                    eprintln!("ok {}", job.paths.len());
                }
                prop_assert_eq!(job.validate(), Ok(()));
                let sim = Simulation::new(&job);
                prop_assert!(sim.total_mm.is_finite() && sim.total_seconds.is_finite());
                let _ = sim.at_fraction(0.5);
                let _ = job.stats();
                prop_assert!(vectorcraft_cutting::preview::to_svg(&job).ends_with("</svg>\n"));
            }
            Err(e) => {
                if std::env::var_os("FUZZ_COUNT").is_some() {
                    eprintln!("err {e}");
                }
                prop_assert!(!e.to_string().is_empty());
            }
        }
    }
}

/// A shape for the packer: a rectangle or any points, coordinates of any kind.
fn raw_shape() -> impl Strategy<Value = vectorcraft_cutting::nest::Shape> {
    use vectorcraft_cutting::nest::Shape;
    prop_oneof![
        4 => (0.0..400.0f64, 0.0..400.0f64, 0.0..120.0f64, 0.0..120.0f64)
            .prop_map(|(x, y, w, h)| Shape { closed: true, points: vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]] }),
        1 => (prop::collection::vec((coord(), coord()), 0..12), any::<bool>())
            .prop_map(|(p, closed)| Shape { closed, points: p.into_iter().map(|(x, y)| [x, y]).collect() }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// C8.7: the packer takes pieces of any shape, size and coordinates (none, a point, a line,
    /// not numbers, huge) without a panic, and the rectangles it places lie on the area the gap
    /// apart from each other.
    #[test]
    fn the_packer_takes_any_pieces(
        pieces in prop::collection::vec(prop::collection::vec(raw_shape(), 0..3), 0..14),
        fixed in prop::collection::vec(raw_shape(), 0..3),
        gap in prop_oneof![0.0..10.0f64, Just(f64::NAN), Just(-5.0), Just(1e9)],
        rotate in any::<bool>(),
    ) {
        use vectorcraft_cutjob::RectMm;
        use vectorcraft_cutting::nest::{Options, nest_within};
        let area = RectMm { x0: 5.0, y0: 5.0, x1: 305.0, y1: 305.0 };
        let n = nest_within(&pieces, &fixed, area, Options { gap_mm: gap, rotate }, 20_000_000);
        prop_assert_eq!(n.placed.len(), pieces.len());
        let used_gap = if gap.is_finite() { gap.clamp(0.0, 100.0) } else { 2.0 };
        // The placed pieces made only of rectangles, as rectangles where they went.
        let mut rects: Vec<RectMm> = vec![];
        for (shapes, at) in pieces.iter().zip(&n.placed) {
            let Some(at) = at else { continue };
            let a = at.affine;
            prop_assert!(a.iter().all(|v| v.is_finite()));
            let corners: Vec<[f64; 2]> = shapes.iter().flat_map(|s| s.points.iter()).map(|q| [a[0] * q[0] + a[2] * q[1] + a[4], a[1] * q[0] + a[3] * q[1] + a[5]]).collect();
            for q in &corners {
                prop_assert!(q[0] >= area.x0 - 1e-6 && q[1] >= area.y0 - 1e-6 && q[0] <= area.x1 + 1e-6 && q[1] <= area.y1 + 1e-6, "{:?} off the area", q);
            }
            if shapes.len() == 1 && shapes[0].points.len() == 4 && shapes[0].closed {
                let xs = corners.iter().map(|q| q[0]);
                let ys = corners.iter().map(|q| q[1]);
                rects.push(RectMm { x0: xs.clone().fold(f64::INFINITY, f64::min), x1: xs.fold(f64::NEG_INFINITY, f64::max), y0: ys.clone().fold(f64::INFINITY, f64::min), y1: ys.fold(f64::NEG_INFINITY, f64::max) });
            }
        }
        for (i, r) in rects.iter().enumerate() {
            for o in rects.iter().skip(i + 1) {
                let dx = (o.x0 - r.x1).max(r.x0 - o.x1).max(0.0);
                let dy = (o.y0 - r.y1).max(r.y0 - o.y1).max(0.0);
                prop_assert!((dx * dx + dy * dy).sqrt() >= used_gap - 1e-6, "{:?} and {:?} closer than {}", r, o, used_gap);
            }
        }
    }

    /// C8.7: a group's setup read from anything gives the same thing read again after it is
    /// written: the document keeps what it read.
    #[test]
    fn a_group_setup_reads_back_as_it_was_read(v in json_value()) {
        let defaults = vectorcraft_cutjob::Operations::default();
        if let Some(g) = vectorcraft_cutjob::GroupSetup::from_value(&v, &defaults) {
            prop_assert!(g.cut.problem().is_none() && g.score.problem().is_none());
            prop_assert_eq!(vectorcraft_cutjob::GroupSetup::from_value(&g.to_value(), &defaults), Some(g));
        }
        let mut m = Map::new();
        m.insert("colorSetups".into(), json!({"#000000": v.clone(), "": v.clone(), "x": v}));
        let s = vectorcraft_cutjob::CuttingSettings::from_value(Some(&Value::Object(m)));
        prop_assert_eq!(vectorcraft_cutjob::CuttingSettings::from_value(Some(&s.to_value())), s);
    }
}

/// A settings value that is a plain JSON object of one key is read like any other.
#[test]
fn settings_of_one_key_make_a_job() {
    let d = document(&[], &json!({"clipToMat": true}));
    assert!(build_job(&d).is_ok());
}
