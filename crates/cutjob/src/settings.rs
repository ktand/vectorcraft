//! A document's cutting settings, kept as JSON under [`DOCUMENT_KEY`] in the document's foreign
//! data (ARCHITECTURE.md › 4.1).

use serde_json::{Map, Value, json};

use crate::mat::{CUSTOM, MAT_PRESETS, MatGeometry, ROLL};
use crate::read::{choice, flag, num, object, pair, quad, unknown, whole};

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
    /// Cut in dashes, so the sheet tears along the line (C14.4): the stroke's dash pattern, else
    /// the settings' perforation lengths.
    Perforate,
}

impl Op {
    pub const ALL: [Op; 5] = [Op::Cut, Op::Score, Op::NoCut, Op::Outline, Op::Perforate];

    /// The id files and commands use.
    pub fn id(self) -> &'static str {
        match self {
            Op::Cut => "cut",
            Op::Score => "score",
            Op::NoCut => "none",
            Op::Outline => "outline",
            Op::Perforate => "perforate",
        }
    }

    /// The operation of `id`.
    pub fn parse(id: &str) -> Option<Op> {
        Op::ALL.into_iter().find(|o| o.id() == id)
    }

    fn choices() -> [(&'static str, Op); 5] {
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
    /// Fewest roller moves (C14.6): the mat fed in bands, each cut before the next as with
    /// [`Ordering::InsideFirst`], so the material goes one way; inner paths still first.
    Rollers,
    /// The document's paint order, bottom to top.
    Document,
}

impl Ordering {
    pub fn id(self) -> &'static str {
        match self {
            Ordering::InsideFirst => "insideFirst",
            Ordering::Rollers => "rollers",
            Ordering::Document => "document",
        }
    }

    pub const ALL: [Ordering; 3] = [Ordering::InsideFirst, Ordering::Rollers, Ordering::Document];
    const CHOICES: [(&'static str, Ordering); 3] =
        [("insideFirst", Ordering::InsideFirst), ("rollers", Ordering::Rollers), ("document", Ordering::Document)];
}

/// `m[key]` as a whole step in `range`, clamped; `null` is none; missing or damaged is `default`.
fn optional_step(m: &Map<String, Value>, key: &str, (lo, hi): (u8, u8), default: Option<u8>) -> Option<u8> {
    match m.get(key) {
        Some(Value::Null) => None,
        Some(v) => v.as_f64().filter(|f| f.is_finite()).map_or(default, |f| u8::try_from(f.round().clamp(f64::from(lo), f64::from(hi)) as i64).ok()),
        None => default,
    }
}

/// How a job is split into groups cut one after the other, with a pause between (C5.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grouping {
    /// One group: everything in one go.
    #[default]
    None,
    /// A group for each top-level layer, bottom first.
    Layer,
    /// A group for each line colour (an object's stroke, else its fill), in the order they come.
    Color,
    /// A group for each fill colour (an object's fill, else its stroke), in the order they come
    /// (C14.3).
    FillColor,
}

/// One group's switches (C14.3), kept by the group's name: whether it is cut, and whether the
/// machine pauses before it (the first group's pause is never made).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupOptions {
    pub off: bool,
    pub pause: bool,
}

impl Default for GroupOptions {
    fn default() -> Self {
        GroupOptions { off: false, pause: true }
    }
}

impl Grouping {
    pub fn id(self) -> &'static str {
        match self {
            Grouping::None => "none",
            Grouping::Layer => "layer",
            Grouping::Color => "color",
            Grouping::FillColor => "fillColor",
        }
    }

    /// Whether the groups are colours (their setups kept in the settings' `colorSetups`).
    pub fn by_colour(self) -> bool {
        matches!(self, Grouping::Color | Grouping::FillColor)
    }

    const CHOICES: [(&'static str, Grouping); 4] =
        [("none", Grouping::None), ("layer", Grouping::Layer), ("color", Grouping::Color), ("fillColor", Grouping::FillColor)];
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
    /// An automatic blade's depth, 1–10 (`None`: the machine's own setting is left alone).
    pub depth: Option<u8>,
    /// Acceleration, 1 (gentle) to 3 (`None`: the machine's own).
    pub accel: Option<u8>,
    /// Line segment overcut: at a sharp corner the blade runs on past it by this, mm, and back,
    /// so the corner is cut clean (0: off; 0–2).
    pub corner_overcut_mm: f64,
    /// How far the blade's tip trails its axis, mm (0–2; 0: a pen, which doesn't swivel), for a
    /// machine that turns the blade round corners itself (`None`: the driver's usual blade).
    pub blade_offset_mm: Option<f64>,
    /// The tool it is cut with, a driver's id for it (C10.2; `None`: the driver's usual tool). The
    /// driver says which holder it goes in. It is the machine's, not the material's: it isn't part
    /// of [`ToolSettings::same_setup`], and a material leaves it as it is.
    pub tool: Option<String>,
    /// Keys this version doesn't know, kept.
    pub extra: Map<String, Value>,
}

impl Default for ToolSettings {
    /// The cutting operation's defaults.
    fn default() -> Self {
        ToolSettings::with_force(0.3)
    }
}

impl ToolSettings {
    const KEYS: [&'static str; 9] = ["force", "speedMmS", "passes", "overcutMm", "depth", "accel", "cornerOvercutMm", "bladeOffsetMm", "tool"];
    /// The longest tool id read.
    pub const MAX_TOOL_ID: usize = 32;

    /// The ranges settings are read into and a job is checked against ([`crate::CutJob::validate`]).
    pub const FORCE: (f64, f64) = (0.0, 1.0);
    pub const SPEED_MM_S: (f64, f64) = (1.0, 2_000.0);
    pub const PASSES: (u8, u8) = (1, 10);
    pub const OVERCUT_MM: (f64, f64) = (0.0, 5.0);
    pub const DEPTH: (u8, u8) = (1, 10);
    pub const ACCEL: (u8, u8) = (1, 3);
    pub const CORNER_OVERCUT_MM: (f64, f64) = (0.0, 2.0);
    pub const BLADE_OFFSET_MM: (f64, f64) = (0.0, 2.0);

    fn with_force(force: f64) -> Self {
        ToolSettings {
            force,
            speed_mm_s: 100.0,
            passes: 1,
            overcut_mm: 0.0,
            depth: None,
            accel: None,
            corner_overcut_mm: 0.0,
            blade_offset_mm: None,
            tool: None,
            extra: Map::new(),
        }
    }

    /// Settings read leniently from `m` (as the document's are), else `default`.
    pub(crate) fn read_lenient(m: Option<&Map<String, Value>>, default: ToolSettings) -> Self {
        Self::read(m, default)
    }

    fn read(m: Option<&Map<String, Value>>, default: ToolSettings) -> Self {
        let Some(m) = m else { return default };
        let passes = whole(m, "passes", u32::from(Self::PASSES.0), u32::from(Self::PASSES.1), u32::from(default.passes));
        ToolSettings {
            force: num(m, "force", Self::FORCE.0, Self::FORCE.1, default.force),
            speed_mm_s: num(m, "speedMmS", Self::SPEED_MM_S.0, Self::SPEED_MM_S.1, default.speed_mm_s),
            passes: u8::try_from(passes).unwrap_or(default.passes),
            overcut_mm: num(m, "overcutMm", Self::OVERCUT_MM.0, Self::OVERCUT_MM.1, default.overcut_mm),
            depth: optional_step(m, "depth", Self::DEPTH, default.depth),
            accel: optional_step(m, "accel", Self::ACCEL, default.accel),
            corner_overcut_mm: num(m, "cornerOvercutMm", Self::CORNER_OVERCUT_MM.0, Self::CORNER_OVERCUT_MM.1, default.corner_overcut_mm),
            blade_offset_mm: match m.get("bladeOffsetMm") {
                Some(Value::Null) => None,
                Some(v) => v
                    .as_f64()
                    .filter(|f| f.is_finite())
                    .map_or(default.blade_offset_mm, |f| Some(f.clamp(Self::BLADE_OFFSET_MM.0, Self::BLADE_OFFSET_MM.1))),
                None => default.blade_offset_mm,
            },
            tool: match m.get("tool") {
                Some(Value::Null) => None,
                Some(Value::String(id)) if !id.trim().is_empty() && id.len() <= Self::MAX_TOOL_ID => Some(id.trim().to_string()),
                _ => default.tool,
            },
            extra: unknown(m, &Self::KEYS),
        }
    }

    /// The same tool setup as `other` (numbers to within rounding; unknown keys and the tool aside).
    pub fn same_setup(&self, other: &ToolSettings) -> bool {
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        near(self.force, other.force)
            && near(self.speed_mm_s, other.speed_mm_s)
            && self.passes == other.passes
            && near(self.overcut_mm, other.overcut_mm)
            && self.depth == other.depth
            && self.accel == other.accel
            && near(self.corner_overcut_mm, other.corner_overcut_mm)
            && match (self.blade_offset_mm, other.blade_offset_mm) {
                (Some(a), Some(b)) => near(a, b),
                (a, b) => a.is_none() && b.is_none(),
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
        } else if self.depth.is_some_and(|d| !(Self::DEPTH.0..=Self::DEPTH.1).contains(&d)) {
            Some("depth")
        } else if self.accel.is_some_and(|a| !(Self::ACCEL.0..=Self::ACCEL.1).contains(&a)) {
            Some("accel")
        } else if !within(self.corner_overcut_mm, Self::CORNER_OVERCUT_MM) {
            Some("cornerOvercutMm")
        } else if self.blade_offset_mm.is_some_and(|b| !within(b, Self::BLADE_OFFSET_MM)) {
            Some("bladeOffsetMm")
        } else {
            None
        }
    }

    /// The settings as JSON, as the document and the materials file keep them.
    pub fn to_value(&self) -> Value {
        let mut m = self.extra.clone();
        m.insert("force".into(), json!(self.force));
        m.insert("speedMmS".into(), json!(self.speed_mm_s));
        m.insert("passes".into(), json!(self.passes));
        m.insert("overcutMm".into(), json!(self.overcut_mm));
        m.insert("depth".into(), json!(self.depth));
        m.insert("accel".into(), json!(self.accel));
        m.insert("cornerOvercutMm".into(), json!(self.corner_overcut_mm));
        m.insert("bladeOffsetMm".into(), json!(self.blade_offset_mm));
        m.insert("tool".into(), json!(self.tool));
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
            Op::Outline | Op::Perforate => Some(&self.cut),
        }
    }
}

/// Most colours with a setup of their own a document keeps.
pub const MAX_GROUP_SETUPS: usize = 256;
/// Longest group name a setup is kept under.
pub const MAX_GROUP_NAME: usize = 256;

/// What a document keeps for cutting (ARCHITECTURE.md › 4.1).
#[derive(Clone, Debug, PartialEq)]
pub struct CuttingSettings {
    /// The schema version read (at least [`SETTINGS_VERSION`]).
    pub version: u32,
    /// The mat: a [`MAT_PRESETS`] id or `"custom"`. An id this version doesn't know is kept, and laid
    /// out on the default mat ([`CuttingSettings::mat_geometry`]).
    pub mat: String,
    /// The custom mat's size, mm (used when `mat` is `"custom"`), or the roll's width and the
    /// length cut from it (when `mat` is `"roll"`, C17.1).
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
    /// Cut mirrored, left to right (C14.7), for material cut from the back, such as heat transfer
    /// vinyl; the art itself isn't changed. Not with Print & Cut.
    pub mirror: bool,
    /// The material the tool settings were last set from (C5.1), by name; none when set by hand.
    pub material: Option<String>,
    /// Track enhancing: the machine grips the material harder as it feeds (C5.2).
    pub track_enhancing: bool,
    /// Cut in groups, with a pause before each after the first (C5.3).
    pub grouping: Grouping,
    /// Print & Cut (C6.1): the artboard is printed with registration marks, and the machine
    /// finds them and cuts from them.
    pub print_and_cut: bool,
    /// How far in from the artboard's edges the marks go, mm ([`crate::marks::INSET_MM`]).
    pub mark_inset_mm: f64,
    /// Each edge's own inset (left, top, right, bottom), mm, in place of [`Self::mark_inset_mm`]
    /// (C12.1).
    pub mark_insets_mm: Option<[f64; 4]>,
    /// The L marks' arms and the width the marks are drawn, mm ([`crate::marks::LENGTH_MM`],
    /// [`crate::marks::LINE_MM`]).
    pub mark_length_mm: f64,
    pub mark_line_mm: f64,
    /// How far the print bleed runs past the art, mm (C12.3; `vectorcraft_cutting::bleed`).
    pub bleed_mm: f64,
    /// Perforating (C14.4): the cuts' and the gaps' length, mm, for an object without a dashed
    /// stroke of its own.
    pub perforate_cut_mm: f64,
    pub perforate_gap_mm: f64,
    /// Cut by colour (C7.4): each colour's own setup, by the name its group has (a swatch's name
    /// or `#rrggbb`); colours without one are cut with [`Self::operations`]. A layer's own setup is
    /// on the layer (`vectorcraft_cutting::setups`).
    pub color_setups: std::collections::BTreeMap<String, crate::GroupSetup>,
    /// Each group's switches by its name (C14.3); a group not listed is on and paused before.
    pub group_options: std::collections::BTreeMap<String, GroupOptions>,
    /// Weeding (C8.2): a box cut round the design (each group's) this far from it, mm.
    pub weed_box: bool,
    pub weed_margin_mm: f64,
    /// Weeding (C8.3): lines from the box in to the design; one from the middle of each side
    /// (spacing 0), or one every this many mm.
    pub weed_lines: bool,
    pub weed_line_spacing_mm: f64,
    /// Knockout for layered vinyl (C18.1): in a grouped job, each object cut back to what shows
    /// of it under the other groups painted above, running this far under their edges, mm.
    pub knockout: bool,
    pub knockout_overlap_mm: f64,
    /// Slower for small details (C18.7): paths no larger than `detail_size_mm` either way are cut
    /// at no more than `detail_speed_mm_s`, so fine vinyl lifts less; the rest at full speed.
    pub detail_slow: bool,
    pub detail_speed_mm_s: f64,
    pub detail_size_mm: f64,
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
            mirror: false,
            material: None,
            track_enhancing: false,
            grouping: Grouping::None,
            print_and_cut: false,
            mark_inset_mm: crate::marks::DEFAULT_INSET_MM,
            mark_insets_mm: None,
            mark_length_mm: crate::marks::DEFAULT_LENGTH_MM,
            mark_line_mm: crate::marks::DEFAULT_LINE_MM,
            bleed_mm: Self::DEFAULT_BLEED_MM,
            perforate_cut_mm: 3.0,
            perforate_gap_mm: 1.0,
            color_setups: Default::default(),
            group_options: Default::default(),
            weed_box: false,
            weed_margin_mm: 3.0,
            weed_lines: false,
            weed_line_spacing_mm: 0.0,
            knockout: false,
            knockout_overlap_mm: 0.5,
            detail_slow: false,
            detail_speed_mm_s: 30.0,
            detail_size_mm: 10.0,
            extra: Map::new(),
        }
    }
}

/// The first preset: the default mat.
fn default_mat() -> &'static MatGeometry {
    &MAT_PRESETS[0]
}

impl CuttingSettings {
    const KEYS: [&'static str; 34] = [
        "version",
        "mat",
        "customMatMm",
        "orientation",
        "artboard",
        "offsetMm",
        "defaultOp",
        "toleranceMm",
        "operations",
        "ordering",
        "clipToMat",
        "mirror",
        "material",
        "trackEnhancing",
        "grouping",
        "printAndCut",
        "markInsetMm",
        "markInsetsMm",
        "markLengthMm",
        "markLineMm",
        "bleedMm",
        "perforateCutMm",
        "perforateGapMm",
        "colorSetups",
        "groupOptions",
        "weedBox",
        "weedMarginMm",
        "weedLines",
        "weedLineSpacingMm",
        "knockout",
        "knockoutOverlapMm",
        "detailSlow",
        "detailSpeedMmS",
        "detailSizeMm",
    ];

    /// The weed box's margin and the weed lines' spacing (0: one from each side's middle), mm.
    pub const WEED_MARGIN_MM: (f64, f64) = (0.0, 50.0);
    pub const WEED_LINE_SPACING_MM: (f64, f64) = (0.0, 200.0);
    /// The print bleed's distance, mm: the range, and the usual (C12.3).
    pub const BLEED_MM: (f64, f64) = (0.1, 5.0);
    /// How far a lower colour runs under the one above it, mm (C18.1).
    pub const KNOCKOUT_MM: (f64, f64) = (0.0, 3.0);
    /// What counts as a small detail, mm across at most (C18.7).
    pub const DETAIL_SIZE_MM: (f64, f64) = (1.0, 100.0);
    pub const DEFAULT_BLEED_MM: f64 = 1.27;
    /// A perforation's cut and gap lengths, mm: the range (C14.4).
    pub const PERFORATE_MM: (f64, f64) = (0.2, 50.0);

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
            mirror: flag(m, "mirror", d.mirror),
            track_enhancing: flag(m, "trackEnhancing", d.track_enhancing),
            grouping: choice(m, "grouping", &Grouping::CHOICES, d.grouping),
            print_and_cut: flag(m, "printAndCut", d.print_and_cut),
            mark_inset_mm: num(m, "markInsetMm", crate::marks::READ_INSET_MM.0, crate::marks::READ_INSET_MM.1, d.mark_inset_mm),
            mark_insets_mm: quad(m, "markInsetsMm", crate::marks::READ_INSET_MM.0, crate::marks::READ_INSET_MM.1),
            mark_length_mm: num(m, "markLengthMm", crate::marks::LENGTH_MM.0, crate::marks::LENGTH_MM.1, d.mark_length_mm),
            mark_line_mm: num(m, "markLineMm", crate::marks::LINE_MM.0, crate::marks::LINE_MM.1, d.mark_line_mm),
            bleed_mm: num(m, "bleedMm", Self::BLEED_MM.0, Self::BLEED_MM.1, d.bleed_mm),
            perforate_cut_mm: num(m, "perforateCutMm", Self::PERFORATE_MM.0, Self::PERFORATE_MM.1, d.perforate_cut_mm),
            perforate_gap_mm: num(m, "perforateGapMm", Self::PERFORATE_MM.0, Self::PERFORATE_MM.1, d.perforate_gap_mm),
            material: m.get("material").and_then(Value::as_str).and_then(crate::material::valid_name),
            color_setups: object(m, "colorSetups")
                .map(|c| {
                    let defaults = Operations::default();
                    c.iter()
                        .filter(|(k, _)| !k.is_empty() && k.chars().count() <= MAX_GROUP_NAME)
                        .filter_map(|(k, v)| Some((k.clone(), crate::GroupSetup::from_value(v, &defaults)?)))
                        .take(MAX_GROUP_SETUPS)
                        .collect()
                })
                .unwrap_or_default(),
            group_options: object(m, "groupOptions")
                .map(|g| {
                    g.iter()
                        .filter(|(k, _)| !k.is_empty() && k.chars().count() <= MAX_GROUP_NAME)
                        .filter_map(|(k, v)| {
                            let o = v.as_object()?;
                            let d = GroupOptions::default();
                            Some((k.clone(), GroupOptions { off: flag(o, "off", d.off), pause: flag(o, "pause", d.pause) }))
                        })
                        .filter(|(_, o)| *o != GroupOptions::default())
                        .take(MAX_GROUP_SETUPS)
                        .collect()
                })
                .unwrap_or_default(),
            weed_box: flag(m, "weedBox", d.weed_box),
            weed_margin_mm: num(m, "weedMarginMm", Self::WEED_MARGIN_MM.0, Self::WEED_MARGIN_MM.1, d.weed_margin_mm),
            weed_lines: flag(m, "weedLines", d.weed_lines),
            weed_line_spacing_mm: num(m, "weedLineSpacingMm", Self::WEED_LINE_SPACING_MM.0, Self::WEED_LINE_SPACING_MM.1, d.weed_line_spacing_mm),
            knockout: flag(m, "knockout", d.knockout),
            knockout_overlap_mm: num(m, "knockoutOverlapMm", Self::KNOCKOUT_MM.0, Self::KNOCKOUT_MM.1, d.knockout_overlap_mm),
            detail_slow: flag(m, "detailSlow", d.detail_slow),
            detail_speed_mm_s: num(m, "detailSpeedMmS", ToolSettings::SPEED_MM_S.0, ToolSettings::SPEED_MM_S.1, d.detail_speed_mm_s),
            detail_size_mm: num(m, "detailSizeMm", Self::DETAIL_SIZE_MM.0, Self::DETAIL_SIZE_MM.1, d.detail_size_mm),
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
        m.insert("mirror".into(), json!(self.mirror));
        m.insert("trackEnhancing".into(), json!(self.track_enhancing));
        m.insert("grouping".into(), json!(self.grouping.id()));
        m.insert("printAndCut".into(), json!(self.print_and_cut));
        m.insert("markInsetMm".into(), json!(self.mark_inset_mm));
        if let Some(i) = self.mark_insets_mm {
            m.insert("markInsetsMm".into(), json!(i));
        }
        m.insert("markLengthMm".into(), json!(self.mark_length_mm));
        m.insert("markLineMm".into(), json!(self.mark_line_mm));
        m.insert("bleedMm".into(), json!(self.bleed_mm));
        m.insert("perforateCutMm".into(), json!(self.perforate_cut_mm));
        m.insert("perforateGapMm".into(), json!(self.perforate_gap_mm));
        if let Some(name) = &self.material {
            m.insert("material".into(), json!(name));
        }
        m.insert("weedBox".into(), json!(self.weed_box));
        m.insert("weedMarginMm".into(), json!(self.weed_margin_mm));
        m.insert("weedLines".into(), json!(self.weed_lines));
        m.insert("weedLineSpacingMm".into(), json!(self.weed_line_spacing_mm));
        m.insert("knockout".into(), json!(self.knockout));
        m.insert("knockoutOverlapMm".into(), json!(self.knockout_overlap_mm));
        m.insert("detailSlow".into(), json!(self.detail_slow));
        m.insert("detailSpeedMmS".into(), json!(self.detail_speed_mm_s));
        m.insert("detailSizeMm".into(), json!(self.detail_size_mm));
        if !self.group_options.is_empty() {
            let g: Map<String, Value> = self.group_options.iter().map(|(k, o)| (k.clone(), json!({"off": o.off, "pause": o.pause}))).collect();
            m.insert("groupOptions".into(), Value::Object(g));
        }
        if !self.color_setups.is_empty() {
            let c: Map<String, Value> = self.color_setups.iter().map(|(k, v)| (k.clone(), v.to_value())).collect();
            m.insert("colorSetups".into(), Value::Object(c));
        }
        Value::Object(m)
    }

    /// Group `name`'s switches (the usual ones when it has none of its own).
    pub fn group_options(&self, name: &str) -> GroupOptions {
        self.group_options.get(name).copied().unwrap_or_default()
    }

    /// How the registration marks are drawn: each edge's own inset if set, else the one inset.
    pub fn mark_style(&self) -> crate::marks::MarkStyle {
        crate::marks::MarkStyle {
            insets_mm: self.mark_insets_mm.unwrap_or([self.mark_inset_mm; 4]),
            length_mm: self.mark_length_mm,
            line_mm: self.mark_line_mm,
        }
    }

    /// The mat the job is laid out on: the preset `mat` names, the custom size, or the default
    /// mat for an id this version doesn't know.
    pub fn mat_geometry(&self) -> MatGeometry {
        if self.mat == CUSTOM {
            return MatGeometry::custom(self.custom_mat_mm);
        }
        if self.mat == ROLL {
            return MatGeometry::roll(self.custom_mat_mm);
        }
        MatGeometry::preset(&self.mat).unwrap_or_else(default_mat).clone()
    }
}

#[cfg(test)]
mod tests;
