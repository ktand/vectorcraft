//! C3.9: random documents and cutting settings make a job or say why, never a panic. Coordinates
//! that aren't numbers or are huge, paths of a point or none, groups and clip groups inside each
//! other, compound paths, symbol instances placed anywhere, hidden objects, operations that
//! aren't one and settings of any shape. A job that comes back validates, and its simulation,
//! stats and preview are made without a panic too.
//!
//! `FUZZ_COUNT=1 cargo test -p vectorcraft-cutting --test fuzz -- --nocapture` prints each case's
//! outcome, to check how many cases reach a job (about a fifth make one with paths; most of the
//! rest are documents with a huge coordinate, refused as off the mat).
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use proptest::prelude::*;
use serde_json::{Map, Value, json};
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
    Path { points: Vec<(f64, f64)>, closed: bool, curve: bool },
    Compound(Vec<Art>),
    Group { children: Vec<Art>, clip: bool },
    Symbol { xf: [f64; 6] },
}

#[derive(Clone, Debug)]
struct Obj {
    art: Art,
    op: Option<String>,
    visible: bool,
}

fn path() -> impl Strategy<Value = Art> {
    (prop::collection::vec((coord(), coord()), 0..8), any::<bool>(), any::<bool>()).prop_map(|(points, closed, curve)| Art::Path {
        points,
        closed,
        curve,
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
    (art, op(), prop::bool::weighted(0.9)).prop_map(|(art, op, visible)| Obj { art, op, visible })
}

fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        (-1000i64..1000).prop_map(Value::from),
        coord().prop_map(|f| serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)),
        prop_oneof![Just("custom"), Just("landscape"), Just("document"), Just("silhouette.12x24"), Just("cut"), Just("none")].prop_map(Value::from),
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
        Art::Path { points, closed, curve } => Node::path(d.alloc_id(), PathData::from_bezpath(&bez(points, *closed, *curve)), Appearance::default()),
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

fn document(objs: &[Obj], settings: &Value) -> Document {
    let mut d = Document::new(500.0, 500.0);
    let art = Node::path(d.alloc_id(), PathData::from_bezpath(&bez(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], true, false)), Appearance::default());
    d.symbols.push(Symbol { name: "s".into(), art: Arc::new(art) });
    let layer: NodeId = d.layers[0].id;
    for o in objs {
        let mut n = build(&mut d, &o.art);
        n.visible = o.visible;
        if let Some(v) = &o.op {
            n.edit_attrs(|a| a.foreign.push(ForeignAttribute { ns: NS.into(), prefix: PREFIX.into(), name: NAME.into(), value: v.clone() }));
        }
        d.insert(Some(layer), usize::MAX, n).unwrap();
    }
    d.unknown.insert(DOCUMENT_KEY.into(), settings.clone());
    d
}

proptest! {
    /// 256 cases by default; `PROPTEST_CASES=20000` for a longer run.
    #[test]
    fn any_document_makes_a_job_or_says_why(objs in prop::collection::vec(obj(), 0..8), settings in prop_oneof![Just(json!({})), json_value()]) {
        let d = document(&objs, &settings);
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

/// A settings value that is a plain JSON object of one key is read like any other.
#[test]
fn settings_of_one_key_make_a_job() {
    let d = document(&[], &json!({"clipToMat": true}));
    assert!(build_job(&d).is_ok());
}
