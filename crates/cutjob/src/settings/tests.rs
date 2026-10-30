use proptest::prelude::*;
use serde_json::json;

use super::*;

fn read(v: Value) -> CuttingSettings {
    CuttingSettings::from_value(Some(&v))
}

#[test]
fn nothing_or_not_an_object_is_the_defaults() {
    let d = CuttingSettings::default();
    assert_eq!(CuttingSettings::from_value(None), d);
    for v in [json!(null), json!(5), json!("cut"), json!([1, 2]), json!({})] {
        assert_eq!(read(v.clone()), d, "{v}");
    }
}

#[test]
fn the_defaults_round_trip_as_documented() {
    let d = CuttingSettings::default();
    let v = d.to_value();
    assert_eq!(
        v,
        json!({
            "version": 1, "mat": "silhouette.12x12", "customMatMm": [304.8, 304.8], "orientation": "portrait",
            "artboard": 0, "offsetMm": [0.0, 0.0], "defaultOp": "cut", "toleranceMm": 0.025,
            "operations": {
                "cut": {"force": 0.3, "speedMmS": 100.0, "passes": 1, "overcutMm": 0.0, "depth": null, "accel": null, "cornerOvercutMm": 0.0, "bladeOffsetMm": null, "tool": null},
                "score": {"force": 0.1, "speedMmS": 100.0, "passes": 1, "overcutMm": 0.0, "depth": null, "accel": null, "cornerOvercutMm": 0.0, "bladeOffsetMm": null, "tool": null}
            },
            "ordering": "insideFirst",
            "clipToMat": false,
            "trackEnhancing": false,
            "grouping": "none",
            "printAndCut": false,
            "markInsetMm": 10.0,
            "weedBox": false,
            "weedMarginMm": 3.0,
            "weedLines": false,
            "weedLineSpacingMm": 0.0
        })
    );
    assert_eq!(read(v), d);
}

#[test]
fn every_setting_round_trips() {
    let s = CuttingSettings {
        version: 1,
        mat: "custom".into(),
        custom_mat_mm: [200.0, 450.5],
        orientation: Orientation::Landscape,
        artboard: 3,
        offset_mm: [-12.5, 40.0],
        default_op: Op::NoCut,
        tolerance_mm: 0.1,
        operations: Operations {
            cut: ToolSettings { force: 0.75, speed_mm_s: 50.0, passes: 3, overcut_mm: 1.5, ..ToolSettings::default() },
            score: ToolSettings { force: 0.05, speed_mm_s: 250.0, passes: 2, overcut_mm: 0.0, ..ToolSettings::default() },
            extra: Map::new(),
        },
        ordering: Ordering::Document,
        clip_to_mat: true,
        material: Some("Cardstock".into()),
        track_enhancing: true,
        grouping: Grouping::Color,
        print_and_cut: true,
        mark_inset_mm: 12.5,
        color_setups: [(
            "Gold".to_string(),
            crate::GroupSetup {
                material: Some("Vinyl".into()),
                cut: ToolSettings { passes: 2, ..ToolSettings::default() },
                score: ToolSettings::default(),
            },
        )]
        .into(),
        weed_box: true,
        weed_margin_mm: 5.5,
        weed_lines: true,
        weed_line_spacing_mm: 25.0,
        extra: Map::new(),
    };
    assert_eq!(read(s.to_value()), s);
    assert_eq!(s.mat_geometry(), MatGeometry::custom([200.0, 450.5]));
}

#[test]
fn damaged_values_are_defaults_and_numbers_are_clamped() {
    let s = read(json!({
        "mat": 7, "customMatMm": [1, 99999], "orientation": "sideways", "artboard": -2,
        "offsetMm": "here", "defaultOp": "perforate", "toleranceMm": 0,
        "operations": {"cut": {"force": 3, "speedMmS": "fast", "passes": 40, "overcutMm": -1}, "score": 5},
        "ordering": null
    }));
    let d = CuttingSettings::default();
    assert_eq!(s.mat, d.mat);
    assert_eq!(s.custom_mat_mm, [MIN_MAT, MAX_LENGTH]);
    assert_eq!(s.orientation, d.orientation);
    assert_eq!(s.artboard, 0);
    assert_eq!(s.offset_mm, d.offset_mm);
    assert_eq!(s.default_op, d.default_op);
    assert_eq!(s.tolerance_mm, 0.001);
    let cut = &s.operations.cut;
    assert_eq!((cut.force, cut.speed_mm_s, cut.passes, cut.overcut_mm), (1.0, d.operations.cut.speed_mm_s, 10, 0.0));
    assert_eq!(s.operations.score, d.operations.score);
    assert_eq!(s.ordering, d.ordering);
    let long = "m".repeat(MAX_MAT_ID + 1);
    assert_eq!(read(json!({"mat": long})).mat, d.mat);
    assert_eq!(read(json!({"mat": ""})).mat, d.mat);
}

/// What a newer version wrote stays: its version number, keys at every level, operations and a
/// mat id this version doesn't know (laid out on the default mat meanwhile).
#[test]
fn a_newer_version_keeps_what_it_wrote() {
    let v = json!({
        "version": 4, "mat": "brand.new", "nesting": {"gap": 2},
        "operations": {"perforate": {"force": 0.2}, "cut": {"force": 0.4, "blade": "deep"}}
    });
    let s = read(v);
    assert_eq!(s.version, 4);
    assert_eq!(s.mat, "brand.new");
    assert_eq!(s.mat_geometry(), MAT_PRESETS[0]);
    assert_eq!(s.operations.cut.force, 0.4);
    let out = s.to_value();
    assert_eq!(out["version"], json!(4));
    assert_eq!(out["mat"], json!("brand.new"));
    assert_eq!(out["nesting"], json!({"gap": 2}));
    assert_eq!(out["operations"]["perforate"], json!({"force": 0.2}));
    assert_eq!(out["operations"]["cut"]["blade"], json!("deep"));
    assert_eq!(read(out), s);
}

#[test]
fn an_older_version_number_reads_as_this_one() {
    assert_eq!(read(json!({"version": 0})).version, SETTINGS_VERSION);
}

#[test]
fn op_ids() {
    for op in Op::ALL {
        assert_eq!(Op::parse(op.id()), Some(op));
    }
    assert_eq!(Op::parse("inherit"), None);
    assert_eq!(Operations::default().get(Op::NoCut), None);
    assert_eq!(Operations::default().get(Op::Score).map(|t| t.force), Some(0.1));
}

fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        any::<f64>().prop_map(|f| serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)),
        prop_oneof![Just("cut".to_string()), Just("custom".to_string()), Just("landscape".to_string()), Just("document".to_string()), ".{0,80}"]
            .prop_map(Value::from),
    ];
    let key = prop_oneof![
        Just("version"),
        Just("mat"),
        Just("customMatMm"),
        Just("orientation"),
        Just("artboard"),
        Just("offsetMm"),
        Just("defaultOp"),
        Just("toleranceMm"),
        Just("operations"),
        Just("ordering"),
        Just("cut"),
        Just("score"),
        Just("force"),
        Just("speedMmS"),
        Just("passes"),
        Just("overcutMm"),
        Just("other"),
    ];
    leaf.prop_recursive(4, 64, 8, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            prop::collection::btree_map(key.clone().prop_map(str::to_string), inner, 0..8).prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

proptest! {
    /// Any JSON reads without panicking into settings in range, and what they write reads back
    /// the same.
    #[test]
    fn any_json_reads_in_range_and_round_trips(v in json_value()) {
        let s = CuttingSettings::from_value(Some(&v));
        prop_assert!((0.001..=1.0).contains(&s.tolerance_mm));
        prop_assert!(s.offset_mm.iter().all(|o| o.abs() <= MAX_LENGTH));
        prop_assert!(s.custom_mat_mm.iter().all(|m| (MIN_MAT..=MAX_LENGTH).contains(m)));
        for t in [&s.operations.cut, &s.operations.score] {
            prop_assert_eq!(t.problem(), None, "{:?}", t);
        }
        prop_assert!(s.version >= SETTINGS_VERSION);
        let again = CuttingSettings::from_value(Some(&s.to_value()));
        prop_assert_eq!(again, s);
    }
}
