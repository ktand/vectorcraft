//! C18.14: material lists read from any text: never a panic; every material imported within the
//! settings' ranges, named, at most the most kept.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use proptest::prelude::*;
use vectorcraft_cutjob::{MAX_MATERIALS, MAX_NAME};
use vectorcraft_cutter_silhouette::studio::{Depth, import};

const HEADER: &str = "full_name,cutter_class_id,(force.pressure),(speed),(tool.type),(blade_depth),(nth_pass),(force.acceleration),(overcut.end_point_length),name_resolved,category,uuid,parent_uuid,action";

/// A cell: a number (any, some out of range), a word the file uses, or anything.
fn cell() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<f64>().prop_map(|v| v.to_string()),
        (-50i32..100).prop_map(|v| v.to_string()),
        prop::sample::select(vec![
            "group",
            "leaf",
            "cut",
            "kraft_cut",
            "score",
            "autoadjust",
            "kraft2",
            "deepcut",
            "rotary",
            "cameo4",
            "g1",
            "g2",
            "\"a,b\"",
            ""
        ])
        .prop_map(str::to_string),
        "[ -~]{0,12}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn any_list_reads_safely(rows in prop::collection::vec(prop::collection::vec(cell(), 0..16), 0..40), file_depth in any::<bool>()) {
        let text = std::iter::once(HEADER.to_string()).chain(rows.iter().map(|r| r.join(","))).collect::<Vec<_>>().join("\n");
        let depth = if file_depth { Depth::File } else { Depth::Auto };
        if let Ok(got) = import(&text, "cameo4", depth) {
            prop_assert!(got.materials.len() <= MAX_MATERIALS);
            for m in &got.materials {
                prop_assert!(!m.name.is_empty() && m.name.chars().count() <= MAX_NAME);
            prop_assert!(m.hand_depth_mm.is_none_or(|d| d > 0.0 && d <= vectorcraft_cutjob::MAX_HAND_DEPTH_MM));
                for t in [&m.cut, &m.score] {
                    prop_assert!((0.0..=1.0).contains(&t.force) && t.speed_mm_s.is_finite() && t.speed_mm_s > 0.0);
                    prop_assert!((1..=10).contains(&t.passes) && (0.0..=5.0).contains(&t.overcut_mm));
                    prop_assert!(t.depth.is_none_or(|d| (1..=10).contains(&d)) && t.accel.is_none_or(|a| (1..=3).contains(&a)));
                }
            }
        }
    }

    #[test]
    fn any_text_reads_safely(text in "(?s).{0,400}") {
        let _ = import(&text, "cameo4", Depth::File);
    }
}
