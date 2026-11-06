//! Fork: material calibration (C18.23): a row of test cuts with one setting a step apart from
//! cell to cell (force, speed, the AutoBlade's depth, overcut, corner overcut), the rest as the
//! document's cut has them. The user cuts it, picks the cell that came out best, and that value
//! goes into the document's cut, and into the material when it is one of theirs.

use serde_json::{Value, json};
use vectorcraft_cutjob::ToolSettings;
use vectorcraft_cutter_silhouette as silhouette;
use vectorcraft_cutting::calibrate::{MAX_CELLS, calibration_job};
use vectorcraft_cutting::test_cut::Corner;

use super::devices::{machine, start};
use super::{CommandSpec, Result, Session, bad, cmd, has_doc, str_param};

/// What a row can vary: its id, the cells' unit, and the values a cell may take.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Setting {
    /// The machine's force steps.
    Force,
    /// The machine's speed steps.
    Speed,
    /// The AutoBlade's depth, 1–10.
    Depth,
    /// How far a closed shape's cut runs on past its start, mm.
    Overcut,
    /// How far the blade runs past each sharp corner, mm.
    Corners,
}

const SETTINGS: [(&str, Setting); 5] =
    [("force", Setting::Force), ("speed", Setting::Speed), ("depth", Setting::Depth), ("overcut", Setting::Overcut), ("corners", Setting::Corners)];

impl Setting {
    fn parse(p: &Value, cmd: &str) -> Result<Setting> {
        let id = str_param(p, "setting").ok_or_else(|| bad(cmd, "setting is required: force, speed, depth, overcut or corners"))?;
        SETTINGS
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, s)| *s)
            .ok_or_else(|| bad(cmd, format!("setting is force, speed, depth, overcut or corners, not {id:?}")))
    }

    /// The row's values, centred on the cut's own where the range allows.
    fn values(self, cut: &ToolSettings) -> Vec<f64> {
        // `n` whole steps round `at`, kept within `lo..=hi`.
        let steps = |at: u32, lo: u32, hi: u32, n: u32| -> Vec<f64> {
            let n = n.min(hi - lo + 1);
            let start = at.saturating_sub(n / 2).clamp(lo, hi + 1 - n);
            (start..start + n).map(f64::from).collect()
        };
        match self {
            Setting::Force => steps(silhouette::force_step(cut.force), silhouette::FORCE.0, silhouette::FORCE.1, 7),
            Setting::Speed => steps(silhouette::speed_step(cut.speed_mm_s), silhouette::SPEED.0, silhouette::SPEED.1, 5),
            Setting::Depth => steps(u32::from(cut.depth.unwrap_or(2)), 1, 10, 5),
            Setting::Overcut => vec![0.0, 0.25, 0.5, 0.75, 1.0],
            Setting::Corners => vec![0.0, 0.2, 0.4, 0.6, 0.8],
        }
    }

    /// `cut` with this setting at `v`.
    fn with(self, cut: &ToolSettings, v: f64) -> ToolSettings {
        let mut t = cut.clone();
        match self {
            Setting::Force => t.force = silhouette::force_from_step(v as u32),
            Setting::Speed => t.speed_mm_s = silhouette::speed_from_step(v as u32),
            Setting::Depth => t.depth = Some(v as u8),
            Setting::Overcut => t.overcut_mm = v,
            Setting::Corners => t.corner_overcut_mm = v,
        }
        t
    }

    /// The `cutting.settings.set` key the value goes to, and the value as it takes it.
    fn set(self, cut: &ToolSettings, v: f64) -> (&'static str, Value) {
        let t = self.with(cut, v);
        match self {
            Setting::Force => ("force", json!(t.force)),
            Setting::Speed => ("speedMmS", json!(t.speed_mm_s)),
            Setting::Depth => ("depth", json!(t.depth)),
            Setting::Overcut => ("overcutMm", json!(t.overcut_mm)),
            Setting::Corners => ("cornerOvercutMm", json!(t.corner_overcut_mm)),
        }
    }

    fn label(self, v: f64) -> String {
        match self {
            Setting::Overcut | Setting::Corners => format!("{v} mm"),
            _ => format!("{v}"),
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "cutting.calibrate.cells",
            "Calibration Cells",
            [],
            None,
            "{setting: force|speed|depth|overcut|corners} the calibration row for that setting (C18.23): each cell's number (from the row's start, marked by a short line), its value (force and speed in the machine's steps, depth the AutoBlade's 1–10, overcut and corners in mm) and label, centred on the document's cut where the range allows; the rest of each cell's settings the document's cut → {setting, cells: [{n, value, label}], current: the document's value}",
            has_doc,
            cells
        ),
        cmd!(
            query "cutting.calibrate.job",
            "Calibration Job",
            [],
            None,
            "{setting, corner?: topLeft|topRight|bottomLeft|bottomRight (default topLeft)} the calibration row as a job: a test cut's square and triangle for each cell, side by side from that corner, each with its own setting, and a short mark beside the first → {job, cells}",
            has_doc,
            job_cmd
        ),
        cmd!(
            query "cutting.calibrate",
            "Cut Calibration Row",
            [],
            None,
            "{setting, corner?, device?} cut the calibration row (as cutting.calibrate.job), in the background as cutting.test does → as cutting.send",
            has_doc,
            cut
        ),
        cmd!(
            "cutting.calibrate.choose",
            "Use Calibration Result",
            [],
            None,
            "{setting, cell: the best cell's number} that cell's value into the document's cut, one undo step; and, when the material chosen is one of the user's, saved into it (cutting.material.save) → {setting, value, label, savedTo: the material's name or null}",
            has_doc,
            choose
        ),
    ]
}

fn row(s: &Session, setting: Setting, cmd: &str) -> Result<(ToolSettings, Vec<f64>)> {
    let settings = vectorcraft_cutting::settings(&s.doc()?.doc);
    let cut = settings.operations.cut;
    if setting == Setting::Depth && cut.tool.is_some() {
        return Err(bad(cmd, "the blade depth is the AutoBlade's: choose the AutoBlade for the cut first"));
    }
    let values = setting.values(&cut);
    if values.is_empty() || values.len() > MAX_CELLS {
        return Err(bad(cmd, "the calibration row couldn't be made"));
    }
    Ok((cut, values))
}

fn cells_json(setting: Setting, values: &[f64]) -> Value {
    json!(values.iter().enumerate().map(|(k, v)| json!({"n": k + 1, "value": v, "label": setting.label(*v)})).collect::<Vec<_>>())
}

fn current(setting: Setting, cut: &ToolSettings) -> Value {
    match setting {
        Setting::Force => json!(silhouette::force_step(cut.force)),
        Setting::Speed => json!(silhouette::speed_step(cut.speed_mm_s)),
        Setting::Depth => json!(cut.depth),
        Setting::Overcut => json!(cut.overcut_mm),
        Setting::Corners => json!(cut.corner_overcut_mm),
    }
}

fn cells(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.calibrate.cells";
    let setting = Setting::parse(p, CMD)?;
    let (cut, values) = row(s, setting, CMD)?;
    Ok(json!({"setting": str_param(p, "setting"), "cells": cells_json(setting, &values), "current": current(setting, &cut)}))
}

fn the_job(s: &Session, p: &Value, cmd: &str) -> Result<(vectorcraft_cutjob::CutJob, Value)> {
    let setting = Setting::parse(p, cmd)?;
    let corner = match str_param(p, "corner") {
        None => Corner::default(),
        Some(id) => Corner::parse(id).ok_or_else(|| bad(cmd, format!("corner must be topLeft, topRight, bottomLeft or bottomRight, not {id:?}")))?,
    };
    let (cut, values) = row(s, setting, cmd)?;
    let tools: Vec<ToolSettings> = values.iter().map(|v| setting.with(&cut, *v)).collect();
    let job = calibration_job(&s.doc()?.doc, &tools, corner).map_err(|e| bad(cmd, e.to_string()))?;
    Ok((job, cells_json(setting, &values)))
}

fn job_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let (job, cells) = the_job(s, p, "cutting.calibrate.job")?;
    Ok(json!({"job": job.to_value(), "cells": cells}))
}

fn cut(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.calibrate";
    machine(p, CMD)?;
    let (job, _) = the_job(s, p, CMD)?;
    start(s, &job, p, CMD, vec![], true)
}

fn choose(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cutting.calibrate.choose";
    let setting = Setting::parse(p, CMD)?;
    let (cut, values) = row(s, setting, CMD)?;
    let n = p.get("cell").and_then(Value::as_u64).ok_or_else(|| bad(CMD, "cell is required: the best cell's number"))?;
    let v = usize::try_from(n)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|k| values.get(k).copied())
        .ok_or_else(|| bad(CMD, format!("cell is 1 to {}", values.len())))?;
    let (key, value) = setting.set(&cut, v);
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {key: value}}}))?;
    // The material chosen, if it is one of the user's, keeps the result.
    let listed = s.execute("cutting.materials", &json!({}))?;
    let own = listed["current"]
        .as_str()
        .filter(|c| listed["materials"].as_array().is_some_and(|all| all.iter().any(|m| m["name"].as_str() == Some(c) && m["builtin"] == false)));
    let saved = match own {
        Some(name) => {
            let name = name.to_string();
            s.execute("cutting.material.save", &json!({"name": name}))?;
            Some(name)
        }
        None => None,
    };
    Ok(json!({"setting": str_param(p, "setting"), "value": v, "label": setting.label(v), "savedTo": saved}))
}
