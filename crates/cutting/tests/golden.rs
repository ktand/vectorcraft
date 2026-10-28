//! Golden jobs: hand-written SVGs (`tests/golden/*.svg`) made into cut jobs, compared with the
//! jobs kept next to them (`*.job.json`). The same file always makes the same job. To accept a
//! change in the jobs, run with `UPDATE_GOLDEN=1` and review the diff.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::Value;
use vectorcraft_cutting::build_job;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden")
}

fn job_of(svg: &str) -> Value {
    let d = vectorcraft_svg::import(svg).unwrap();
    let job = build_job(&d).unwrap();
    let mut v = job.to_value();
    v["stats"] = job.stats().to_value();
    // Node ids depend on the importer's numbering, not on the job: leave them out.
    for p in v["paths"].as_array_mut().into_iter().flatten() {
        p["source"]["node"] = Value::Null;
    }
    for w in v["warnings"].as_array_mut().into_iter().flatten() {
        w["source"] = Value::Null;
    }
    round(&mut v);
    v
}

/// Numbers rounded to 1e-6 mm, so that the files don't change with the last bits of a float.
fn round(v: &mut Value) {
    match v {
        Value::Number(n) if !n.is_i64() && !n.is_u64() => {
            if let Some(f) = n.as_f64() {
                *v = serde_json::json!((f * 1e6).round() / 1e6);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(round),
        Value::Object(m) => m.values_mut().for_each(round),
        _ => {}
    }
}

#[test]
fn golden_jobs() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut inputs: Vec<PathBuf> =
        std::fs::read_dir(dir()).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "svg")).collect();
    inputs.sort();
    assert!(!inputs.is_empty());
    for svg in inputs {
        let text = std::fs::read_to_string(&svg).unwrap();
        let got = job_of(&text);
        assert_eq!(got, job_of(&text), "{}: the same file makes the same job", svg.display());
        let want = svg.with_extension("job.json");
        if update {
            std::fs::write(&want, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
            continue;
        }
        let want_text = std::fs::read_to_string(&want).unwrap_or_else(|_| panic!("{} is missing: run with UPDATE_GOLDEN=1", want.display()));
        let want: Value = serde_json::from_str(&want_text).unwrap();
        assert_eq!(got, want, "{}: the job changed (UPDATE_GOLDEN=1 accepts it)", svg.display());
    }
}
