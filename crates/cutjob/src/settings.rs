//! A document's cutting settings, kept as JSON under [`DOCUMENT_KEY`] in the document's foreign
//! data (ARCHITECTURE.md › 4.1).

use serde_json::{Map, Value, json};

use crate::mat::{CUSTOM, MAT_PRESETS, MatGeometry};
use crate::read::{choice, flag, num, object, pair, unknown, whole};

/// `Document::unknown` key the settings live under.
pub const DOCUMENT_KEY: &str = "cutting";

/// The settings schema this version writes. A file from a newer version keeps its number.
pub const SETTINGS_VERSION: u32 = 1;

/// Longest mat id read; a longer one is damage.
const MAX_MAT_ID: usize = 64;
/// Most artboards a setting can point at.
const MAX_ARTBOARD: u32 = 9_999;
/// Largest custom mat side and offset, mm (3 m).
const MAX_LENGTH: f64 = 3_000.0;
/// Smallest custom mat side, mm.
const MIN_MAT: f64 = 10.0;

/// What happens to an object when the document is cut.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Op {
    /// Cut through.
    #[default]
    Cut,
    /// Score: cut part way, to fold along.
    Score,
    /// Left out of the job.
    NoCut,
    /// Cut along the outline of all the shapes set to it, united: where they overlap, the lines
    /// inside aren't cut (C3.10).
    Outline,
}

impl Op {
    pub const ALL: [Op; 4] = [Op::Cut, Op::Score, Op::NoCut, Op::Outline];

    /// The id files and commands use.
    pub fn id(self) -> &'static str {
        match self {
            Op::Cut => "cut",
            Op::Score => "score",
            Op::NoCut => "none",
            Op::Outline => "outline",
        }
    }

    /// The operation of `id`.
    pub fn parse(id: &str) -> Option<Op> {
        Op::ALL.into_iter().find(|o| o.id() == id)
    }

    fn choices() -> [(&'static str, Op); 4] {
        Op::ALL.map(|o| (o.id(), o))
    }
}

/// How the artboard lies on the mat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    /// The artboard's top along the mat's top (the edge the machine feeds first).
    #[default]
    Portrait,
    /// The artboard turned a quarter turn.
    Landscape,
}

impl Orientation {
    pub fn id(self) -> &'static str {
        match self {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    }

    const CHOICES: [(&'static str, Orientation); 2] = [("portrait", Orientation::Portrait), ("landscape", Orientation::Landscape)];
}

/// The order paths are cut in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ordering {
    /// A path inside another closed path is cut first, so the piece doesn't shift; shortest travel
    /// otherwise.
    #[default]
    InsideFirst,
    /// The document's paint order, bottom to top.
    Document,
}

impl Ordering {
    pub fn id(self) -> &'static str {
        match self {
            Ordering::InsideFirst => "insideFirst",
            Ordering::Document => "document",
        }
    }

    const CHOICES: [(&'static str, Ordering); 2] = [("insideFirst", Ordering::InsideFirst), ("document", Ordering::Document)];
}

/// How one operation drives the tool, machine-neutral: a driver maps these to its own steps.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolSettings {
    /// 0–1 of the machine's force range.
    pub force: f64,
    /// Requested speed, mm/s (a driver clamps it to what the machine can do).
    pub speed_mm_s: f64,
    /// Times each path is cut, 1–10.
    pub passes: u8,
    /// How far a closed path's cut runs on past its start, mm (0–5).
    pub overcut_mm: f64,
    /// Keys this version doesn't know, kept.
    pub extra: Map<String, Value>,
}

impl ToolSettings {
    const KEYS: [&'static str; 4] = ["force", "speedMmS", "passes", "overcutMm"];

    /// The ranges settings are read into and a job is checked against ([`crate::CutJob::validate`]).
    pub const FORCE: (f64, f64) = (0.0, 1.0);
    pub const SPEED_MM_S: (f64, f64) = (1.0, 2_000.0);
    pub const PASSES: (u8, u8) = (1, 10);
    pub const OVERCUT_MM: (f64, f64) = (0.0, 5.0);

    fn with_force(force: f64) -> Self {
        ToolSettings { force, speed_mm_s: 100.0, passes: 1, overcut_mm: 0.0, extra: Map::new() }
    }

    fn read(m: Option<&Map<String, Value>>, default: ToolSettings) -> Self {
        let Some(m) = m else { return default };
        let passes = whole(m, "passes", u32::from(Self::PASSES.0), u32::from(Self::PASSES.1), u32::from(default.passes));
        ToolSettings {
            force: num(m, "force", Self::FORCE.0, Self::FORCE.1, default.force),
            speed_mm_s: num(m, "speedMmS", Self::SPEED_MM_S.0, Self::SPEED_MM_S.1, default.speed_mm_s),
            passes: u8::try_from(passes).unwrap_or(default.passes),
            overcut_mm: num(m, "overcutMm", Self::OVERCUT_MM.0, Self::OVERCUT_MM.1, default.overcut_mm),
            extra: unknown(m, &Self::KEYS),
        }
    }

    /// What in these settings is out of range or not finite, if anything.
    pub fn problem(&self) -> Option<&'static str> {
        let within = |v: f64, (lo, hi): (f64, f64)| v.is_finite() && (lo..=hi).contains(&v);
        if !within(self.force, Self::FORCE) {
            Some("force")
        } else if !within(self.speed_mm_s, Self::SPEED_MM_S) {
            Some("speedMmS")
        } else if !(Self::PASSES.0..=Self::PASSES.1).contains(&self.passes) {
            Some("passes")
        } else if !within(self.overcut_mm, Self::OVERCUT_MM) {
            Some("overcutMm")
        } else {
            None
        }
    }

    fn to_value(&self) -> Value {
        let mut m = self.extra.clone();
        m.insert("force".into(), json!(self.force));
        m.insert("speedMmS".into(), json!(self.speed_mm_s));
        m.insert("passes".into(), json!(self.passes));
        m.insert("overcutMm".into(), json!(self.overcut_mm));
        Value::Object(m)
    }
}

/// The tool settings of each operation.
#[derive(Clone, Debug, PartialEq)]
pub struct Operations {
    pub cut: ToolSettings,
    pub score: ToolSettings,
    /// Operations this version doesn't know, kept.
    pub extra: Map<String, Value>,
}

impl Default for Operations {
    fn default() -> Self {
        Operations { cut: ToolSettings::with_force(0.3), score: ToolSettings::with_force(0.1), extra: Map::new() }
    }
}

impl Operations {
    /// The tool settings of `op` (none for [`Op::NoCut`]; Cut's for [`Op::Outline`], which cuts).
    pub fn get(&self, op: Op) -> Option<&ToolSettings> {
        match op {
            Op::Cut => Some(&self.cut),
            Op::Score => Some(&self.score),
            Op::NoCut => None,
            Op::Outline => Some(&self.cut),
        }
    }
}

/// What a document keeps for cutting (ARCHITECTURE.md › 4.1).
#[derive(Clone, Debug, PartialEq)]
pub struct CuttingSettings {
    /// The schema version read (at least [`SETTINGS_VERSION`]).
    pub version: u32,
    /// The mat: a [`MAT_PRESETS`] id or `"custom"`. An id this version doesn't know is kept, and laid
    /// out on the default mat ([`CuttingSettings::mat_geometry`]).
    pub mat: String,
    /// The custom mat's size, mm (used when `mat` is `"custom"`).
    pub custom_mat_mm: [f64; 2],
    pub orientation: Orientation,
    /// Index of the artboard laid on the mat.
    pub artboard: u32,
    /// Where the artboard's top-left corner goes, from the cut area's top-left, mm.
    pub offset_mm: [f64; 2],
    /// The operation of objects that, with their ancestors, set none.
    pub default_op: Op,
    /// Curves are flattened to within this distance, mm (0.001–1).
    pub tolerance_mm: f64,
    pub operations: Operations,
    pub ordering: Ordering,
    /// Paths that leave the cut area are cut up to its edge (with a warning) instead of stopping
    /// the job.
    pub clip_to_mat: bool,
    /// Keys this version doesn't know, kept.
    pub extra: Map<String, Value>,
}

impl Default for CuttingSettings {
    fn default() -> Self {
        CuttingSettings {
            version: SETTINGS_VERSION,
            mat: default_mat().id.to_string(),
            custom_mat_mm: [304.8, 304.8],
            orientation: Orientation::Portrait,
            artboard: 0,
            offset_mm: [0.0, 0.0],
            default_op: Op::Cut,
            tolerance_mm: 0.025,
            operations: Operations::default(),
            ordering: Ordering::InsideFirst,
            clip_to_mat: false,
            extra: Map::new(),
        }
    }
}

/// The first preset: the default mat.
fn default_mat() -> &'static MatGeometry {
    &MAT_PRESETS[0]
}

impl CuttingSettings {
    const KEYS: [&'static str; 11] =
        ["version", "mat", "customMatMm", "orientation", "artboard", "offsetMm", "defaultOp", "toleranceMm", "operations", "ordering", "clipToMat"];

    /// The settings in `v` (a document's `unknown["cutting"]`, from a file): anything that isn't an
    /// object gives the defaults. Never fails.
    pub fn from_value(v: Option<&Value>) -> Self {
        let d = CuttingSettings::default();
        let Some(m) = v.and_then(Value::as_object) else { return d };
        let ops = object(m, "operations");
        CuttingSettings {
            version: whole(m, "version", SETTINGS_VERSION, u32::MAX, SETTINGS_VERSION),
            mat: m.get("mat").and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= MAX_MAT_ID).map_or(d.mat, str::to_string),
            custom_mat_mm: pair(m, "customMatMm", MIN_MAT, MAX_LENGTH, d.custom_mat_mm),
            orientation: choice(m, "orientation", &Orientation::CHOICES, d.orientation),
            artboard: whole(m, "artboard", 0, MAX_ARTBOARD, d.artboard),
            offset_mm: pair(m, "offsetMm", -MAX_LENGTH, MAX_LENGTH, d.offset_mm),
            default_op: choice(m, "defaultOp", &Op::choices(), d.default_op),
            tolerance_mm: num(m, "toleranceMm", 0.001, 1.0, d.tolerance_mm),
            operations: Operations {
                cut: ToolSettings::read(ops.and_then(|o| object(o, "cut")), d.operations.cut),
                score: ToolSettings::read(ops.and_then(|o| object(o, "score")), d.operations.score),
                extra: ops.map(|o| unknown(o, &["cut", "score"])).unwrap_or_default(),
            },
            ordering: choice(m, "ordering", &Ordering::CHOICES, d.ordering),
            clip_to_mat: flag(m, "clipToMat", d.clip_to_mat),
            extra: unknown(m, &Self::KEYS),
        }
    }

    /// The settings as the document keeps them.
    pub fn to_value(&self) -> Value {
        let mut ops = self.operations.extra.clone();
        ops.insert("cut".into(), self.operations.cut.to_value());
        ops.insert("score".into(), self.operations.score.to_value());
        let mut m = self.extra.clone();
        m.insert("version".into(), json!(self.version.max(SETTINGS_VERSION)));
        m.insert("mat".into(), json!(self.mat));
        m.insert("customMatMm".into(), json!(self.custom_mat_mm));
        m.insert("orientation".into(), json!(self.orientation.id()));
        m.insert("artboard".into(), json!(self.artboard));
        m.insert("offsetMm".into(), json!(self.offset_mm));
        m.insert("defaultOp".into(), json!(self.default_op.id()));
        m.insert("toleranceMm".into(), json!(self.tolerance_mm));
        m.insert("operations".into(), Value::Object(ops));
        m.insert("ordering".into(), json!(self.ordering.id()));
        m.insert("clipToMat".into(), json!(self.clip_to_mat));
        Value::Object(m)
    }

    /// The mat the job is laid out on: the preset `mat` names, the custom size, or the default
    /// mat for an id this version doesn't know.
    pub fn mat_geometry(&self) -> MatGeometry {
        if self.mat == CUSTOM {
            return MatGeometry::custom(self.custom_mat_mm);
        }
        MatGeometry::preset(&self.mat).unwrap_or_else(default_mat).clone()
    }
}

#[cfg(test)]
mod tests;
