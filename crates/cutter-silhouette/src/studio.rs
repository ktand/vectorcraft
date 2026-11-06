//! Materials exported from Silhouette Studio (C18.14): the user's own export of the maker's
//! software's material list (a CSV file), read into materials of the user's. Only the file the
//! user picks is read; no maker's material table is part of VectorCraft (the file's layout is in
//! `docs/cutting/protocol-silhouette.md` › Material lists).
//!
//! A material is a group row; its settings are the rows under it (by parent id), one per machine
//! and action. For the machine asked for, each tool this driver knows takes the first row for it
//! (C18.15, [`use_of`]): the AutoBlade's `cut` row gives the cut settings and its `score` row the
//! score settings; the Deep-Cut, ratchet, Kraft and rotary blades', the pen's (`sketch`) and the
//! punch's are kept as the material's settings for that tool. A material with no AutoBlade cut
//! has its first other tool's settings as its cut (for that tool, [`Material::cut_tool`]). Actions
//! VectorCraft doesn't do (stipple, pop-out cards, embossing…) are listed as not used.
//! Force and speed are the machine's own steps, which this driver sends as they are; passes,
//! acceleration and the end overcut are taken as they are. The file's depth is in millimetres
//! (the AutoBlade cuts at most 1 mm deep, the owner, 2026-11-05, and no AutoBlade row of their list
//! is deeper): the AutoBlade's is sent as 0.1 mm a step of its 1–10 ([`Depth::File`]); a blade set
//! by hand (Kraft, Deep-Cut, ratchet) keeps it as the depth to set it to, never sent
//! ([`Material::hand_depth_mm`]).

use std::collections::BTreeMap;

use vectorcraft_cutjob::{ForTool, MAX_HAND_DEPTH_MM, MAX_MATERIALS, MAX_NAME, Material, Operations, ToolSettings};

use crate::{force_from_step, speed_from_step};

/// The largest file read, bytes.
pub const MAX_BYTES: usize = 8 << 20;
/// Most rows read.
const MAX_ROWS: usize = 50_000;
/// Most fields a row has.
const MAX_FIELDS: usize = 256;

/// The machine a list's rows are for unless another is asked for.
pub const DEFAULT_MACHINE: &str = "cameo4";

/// Where an imported material's blade depth comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// Left to the machine (the setting unset).
    Auto,
    /// The file's depth for the automatic blade, mm, as its 1–10 (0.1 mm a step).
    File,
}

/// A material not imported, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Skipped {
    pub name: String,
    pub reason: String,
}

/// What a list gives for one machine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Imported {
    pub materials: Vec<Material>,
    pub skipped: Vec<Skipped>,
    /// The machines the list has rows for, in the order first seen.
    pub machines: Vec<String>,
    /// The machine's actions the list has that weren't used (the file's names, sorted).
    pub unused: Vec<String>,
}

/// The CSV rows of `text`: fields split at commas, quoted fields with `""` for a quote; at most
/// [`MAX_ROWS`] rows of [`MAX_FIELDS`] fields (the rest of a longer row dropped).
fn rows(text: &str) -> Vec<Vec<String>> {
    let mut out = vec![];
    let mut row: Vec<String> = vec![];
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let end_field = |row: &mut Vec<String>, field: &mut String| {
        if row.len() < MAX_FIELDS {
            row.push(std::mem::take(field));
        } else {
            field.clear();
        }
    };
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ',' => end_field(&mut row, &mut field),
            '\r' => {}
            '\n' => {
                end_field(&mut row, &mut field);
                out.push(std::mem::take(&mut row));
                if out.len() >= MAX_ROWS {
                    return out;
                }
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        end_field(&mut row, &mut field);
        out.push(row);
    }
    out
}

/// The columns read, by their names in the file's first row.
struct Columns {
    full_name: usize,
    class: usize,
    force: usize,
    speed: usize,
    tool: usize,
    category: usize,
    uuid: usize,
    parent: usize,
    action: usize,
    depth: Option<usize>,
    passes: Option<usize>,
    accel: Option<usize>,
    overcut: Option<usize>,
    resolved: Option<usize>,
}

impl Columns {
    fn find(header: &[String]) -> Result<Columns, String> {
        let at = |name: &str| header.iter().position(|h| h.trim().eq_ignore_ascii_case(name));
        let need = |name: &str| at(name).ok_or_else(|| format!("not a material list exported from Silhouette Studio: it has no {name} column"));
        Ok(Columns {
            full_name: need("full_name")?,
            class: need("cutter_class_id")?,
            force: need("(force.pressure)")?,
            speed: need("(speed)")?,
            tool: need("(tool.type)")?,
            category: need("category")?,
            uuid: need("uuid")?,
            parent: need("parent_uuid")?,
            action: need("action")?,
            depth: at("(blade_depth)"),
            passes: at("(nth_pass)"),
            accel: at("(force.acceleration)"),
            overcut: at("(overcut.end_point_length)"),
            resolved: at("name_resolved"),
        })
    }
}

/// A settings row of one material.
struct Leaf<'a> {
    row: &'a [String],
    action: String,
    tool: String,
}

/// What a settings row is used for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    /// The AutoBlade's cut.
    Cut,
    /// The AutoBlade's score.
    Score,
    /// The settings for these tools of this driver ([`crate::TOOLS`] ids), and whether the blade's
    /// depth is set by hand (kept as the depth to set it to).
    Tools(&'static [&'static str], bool),
}

/// The use of a row with `action` and `tool` (the file's names), none when VectorCraft doesn't
/// do it. The file doesn't say which Kraft blade (2 or 3 mm) a row is for: both take it.
fn use_of(action: &str, tool: &str) -> Option<Use> {
    Some(match (action, tool) {
        ("cut", "autoadjust") => Use::Cut,
        ("score", "autoadjust") => Use::Score,
        ("cut", "deepcut") => Use::Tools(&["deepcut"], true),
        ("cut", "adjustable") => Use::Tools(&["ratchet"], true),
        ("kraft_cut", "kraft" | "kraft2") => Use::Tools(&["kraft3", "kraft2"], true),
        ("rotary_cut", "rotary") => Use::Tools(&["rotary"], false),
        ("sketch", "pen" | "pen2") => Use::Tools(&["pen"], false),
        ("punch", "puncher") => Use::Tools(&["punch"], false),
        _ => return None,
    })
}

/// Which tool's settings become a material's cut when it has no AutoBlade cut, first first.
const MAIN_TOOL: [&str; 7] = ["deepcut", "ratchet", "kraft3", "kraft2", "rotary", "pen", "punch"];

fn cell(row: &[String], i: usize) -> &str {
    row.get(i).map_or("", |s| s.trim())
}

fn number(row: &[String], i: Option<usize>) -> Option<f64> {
    i.and_then(|i| cell(row, i).parse::<f64>().ok()).filter(|v| v.is_finite())
}

/// A step the file gives as a number: rounded, at least 1, at most `max` (`None` when not a number).
fn step(row: &[String], i: Option<usize>, max: u32) -> Option<u32> {
    // Clamped before the cast, so it is exact.
    number(row, i).map(|v| v.round().clamp(1.0, f64::from(max)) as u32)
}

/// `row`'s settings over `base` (`None` when its force or speed isn't a number).
fn settings(row: &[String], c: &Columns, base: &ToolSettings, depth: Depth, auto_blade: bool) -> Option<ToolSettings> {
    let force = step(row, Some(c.force), crate::FORCE.1)?;
    let speed = step(row, Some(c.speed), crate::SPEED.1)?;
    let mut t = ToolSettings { force: force_from_step(force), speed_mm_s: speed_from_step(speed), tool: None, ..base.clone() };
    if let Some(p) = step(row, c.passes, 10) {
        // At most 10 after the clamp.
        t.passes = p as u8;
    }
    // The file has 1–5 for some tools; ours is 1–3.
    t.accel = step(row, c.accel, 3).map(|a| a as u8).or(t.accel);
    if let Some(o) = number(row, c.overcut) {
        t.overcut_mm = o.clamp(0.0, 5.0);
    }
    t.depth = match depth {
        Depth::File if auto_blade => number(row, c.depth).map(|d| (d * 10.0).round().clamp(1.0, 10.0) as u8),
        _ => None,
    };
    Some(t)
}

/// A name a material may have, cut to [`MAX_NAME`] characters.
fn fit(name: &str) -> String {
    name.trim().chars().take(MAX_NAME).collect::<String>().trim().to_string()
}

/// The materials in `text` (an exported material list) for machine `machine` (the file's
/// cutter class, such as `cameo4`, any case), at most [`MAX_MATERIALS`]; the rest skipped with a
/// reason. `Err` when the file isn't such a list.
pub fn import(text: &str, machine: &str, depth: Depth) -> Result<Imported, String> {
    if text.len() > MAX_BYTES {
        return Err(format!("larger than {} MB", MAX_BYTES >> 20));
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let all = rows(text);
    let (header, body) = all.split_first().ok_or("the file is empty")?;
    let c = Columns::find(header)?;
    let machine = machine.trim().to_ascii_lowercase();
    let mut out = Imported::default();
    let mut groups: Vec<(&str, String)> = vec![];
    let mut leaves: Vec<(&str, Leaf)> = vec![];
    for row in body {
        let category = cell(row, c.category);
        if category.eq_ignore_ascii_case("group") {
            let name = c.resolved.map(|i| cell(row, i)).filter(|n| !n.is_empty()).unwrap_or_else(|| cell(row, c.full_name));
            groups.push((cell(row, c.uuid), fit(name)));
        } else if category.eq_ignore_ascii_case("leaf") {
            let class = cell(row, c.class).to_ascii_lowercase();
            if !class.is_empty() && !out.machines.contains(&class) {
                out.machines.push(class.clone());
            }
            if class == machine {
                let leaf = Leaf { row, action: cell(row, c.action).to_ascii_lowercase(), tool: cell(row, c.tool).to_ascii_lowercase() };
                leaves.push((cell(row, c.parent), leaf));
            }
        }
    }
    if groups.is_empty() {
        return Err("not a material list exported from Silhouette Studio: it has no materials".into());
    }
    let defaults = Operations::default();
    let mut unused: Vec<String> = vec![];
    for (uuid, name) in groups {
        let skip = |reason: &str| Skipped { name: name.clone(), reason: reason.into() };
        if name.is_empty() {
            continue;
        }
        let mine: Vec<&Leaf> = leaves.iter().filter(|(p, _)| p.eq_ignore_ascii_case(uuid)).map(|(_, l)| l).collect();
        if mine.is_empty() {
            out.skipped.push(skip(&format!("no settings for {machine}")));
            continue;
        }
        let mut cut = None;
        let mut score = None;
        let mut tools: BTreeMap<String, ForTool> = BTreeMap::new();
        let mut usable = false;
        for l in &mine {
            let Some(u) = use_of(&l.action, &l.tool) else {
                if !l.action.is_empty() && !unused.contains(&l.action) {
                    unused.push(l.action.clone());
                }
                continue;
            };
            usable = true;
            match u {
                Use::Cut if cut.is_none() => cut = settings(l.row, &c, &defaults.cut, depth, true),
                Use::Score if score.is_none() => score = settings(l.row, &c, &defaults.score, depth, true),
                Use::Tools(ids, by_hand) => {
                    let Some(t) = settings(l.row, &c, &defaults.cut, depth, false) else { continue };
                    let hand_depth_mm = number(l.row, c.depth).filter(|d| by_hand && *d > 0.0).map(|d| d.min(MAX_HAND_DEPTH_MM));
                    for id in ids {
                        tools.entry((*id).to_string()).or_insert_with(|| ForTool { settings: t.clone(), hand_depth_mm });
                    }
                }
                _ => {}
            }
        }
        let (cut, cut_tool, hand_depth_mm) = match cut {
            Some(cut) => (cut, None, None),
            None => {
                let Some(main) = MAIN_TOOL.iter().find_map(|id| tools.remove_entry(*id)) else {
                    let mut kinds: Vec<&str> = mine.iter().map(|l| l.action.as_str()).filter(|a| !a.is_empty()).collect();
                    kinds.sort_unstable();
                    kinds.dedup();
                    let reason = if usable || kinds.is_empty() {
                        "no cut, sketch or punch settings with a force and speed".to_string()
                    } else {
                        format!("only {}, which VectorCraft doesn't do", kinds.join(", "))
                    };
                    out.skipped.push(skip(&reason));
                    continue;
                };
                (main.1.settings, Some(main.0), main.1.hand_depth_mm)
            }
        };
        if out.materials.iter().any(|m| m.name.eq_ignore_ascii_case(&name)) {
            out.skipped.push(skip("a second material of the same name"));
            continue;
        }
        if out.materials.len() >= MAX_MATERIALS {
            out.skipped.push(skip(&format!("more than {MAX_MATERIALS} materials")));
            continue;
        }
        out.materials.push(Material {
            name: name.clone(),
            cut,
            score: score.unwrap_or_else(|| defaults.score.clone()),
            builtin: false,
            mirror: false,
            price: None,
            roll_left_mm: None,
            press: None,
            hand_depth_mm,
            cut_tool,
            tools,
        });
    }
    unused.sort_unstable();
    out.unused = unused;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A list of our own in the layout described in the protocol notes (made-up materials and values).
    fn list() -> String {
        let header = "full_name,cutter_class_id,(force.pressure),(speed),(tool.type),(blade_depth),(nth_pass),(force.acceleration),(overcut.end_point_length),name_resolved,category,uuid,parent_uuid,action";
        let rows = [
            "(File Type ID),Full Name,Force,Speed,Tool,Depth,Passes,Accel,Overcut,Name,Category,ID,Parent ID,Action",
            "Test Paper,desktop,15,5,adjustable,0.3,1,1,0.1,Test Paper,group,g1,root,",
            "Test Paper for M1,m1,12,4,autoadjust,0.2,2,3,0.2,M1,leaf,l1,g1,cut",
            "Test Paper for M1,m1,6,5,autoadjust,0.1,1,3,0.1,M1,leaf,l2,g1,score",
            "Test Paper for M1,m1,8,10,pen2,0.3,1,1,0,M1,leaf,l7,g1,sketch",
            "Test Paper for M1,m1,9,10,pen2,0.3,1,1,0,M1,leaf,l8,g1,stipple",
            "Test Paper for M1,m1,30,3,puncher,0.2,1,1,0,M1,leaf,l9,g1,punch",
            "Test Paper for M1,m1,25,3,deepcut,1.2,1,1,0,M1,leaf,l10,g1,cut",
            "Test Paper for M2,m2,20,8,autoadjust,0.4,1,1,0.1,M2,leaf,l3,g1,cut",
            "\"Board, thick\",desktop,15,5,adjustable,0.3,1,1,0.1,\"Board, thick\",group,g2,root,",
            "Board for M1,m1,40,0.3,kraft2,1.5,3,5,0.1,M1,leaf,l4,g2,kraft_cut",
            "Felt,desktop,15,5,adjustable,0.3,1,1,0.1,Felt,group,g3,root,",
            "Felt for M1,m1,20,5,autoadjust,0.3,1,1,0.1,M1,leaf,l5,g3,rotary_cut",
            "Felt for M1,m1,20,5,embosser,0.3,1,1,0.1,M1,leaf,l11,g3,emboss",
            "Cloth,desktop,15,5,adjustable,0.3,1,1,0.1,Cloth,group,g5,root,",
            "Cloth for M1,m1,14,6,rotary,0.5,1,1,0.1,M1,leaf,l12,g5,rotary_cut",
            "Foil,desktop,15,5,adjustable,0.3,1,1,0.1,Foil,group,g4,root,",
            "Foil for M2,m2,20,5,autoadjust,0.3,1,1,0.1,M2,leaf,l6,g4,cut",
        ];
        format!("{header}\r\n{}\r\n", rows.join("\r\n"))
    }

    #[test]
    fn a_machines_materials() {
        let got = import(&list(), "M1", Depth::Auto).unwrap();
        let names: Vec<&str> = got.materials.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Test Paper", "Board, thick", "Cloth"]);
        let paper = &got.materials[0];
        // The machine's own steps, read back exactly.
        assert_eq!(crate::force_step(paper.cut.force), 12);
        assert_eq!(crate::speed_step(paper.cut.speed_mm_s), 4);
        assert_eq!((paper.cut.passes, paper.cut.accel, paper.cut.depth), (2, Some(3), None));
        assert!((paper.cut.overcut_mm - 0.2).abs() < 1e-9);
        assert_eq!(crate::force_step(paper.score.force), 6);
        assert!(!paper.builtin && paper.cut.tool.is_none() && paper.cut_tool.is_none());
        // C18.15: the pen's sketch, the punch and the Deep-Cut blade's cut kept for those tools.
        let ids: Vec<&str> = paper.tools.keys().map(String::as_str).collect();
        assert_eq!(ids, ["deepcut", "pen", "punch"]);
        assert_eq!(crate::force_step(paper.tools["pen"].settings.force), 8);
        assert_eq!(crate::force_step(paper.tools["punch"].settings.force), 30);
        assert_eq!((paper.tools["pen"].hand_depth_mm, paper.tools["deepcut"].hand_depth_mm), (None, Some(1.2)));
        // A speed under a step, an acceleration over ours: brought into range. Only a Kraft cut: it
        // is the material's cut, for the 3 mm Kraft blade, and the 2 mm one's too.
        let board = &got.materials[1];
        assert_eq!(board.cut_tool.as_deref(), Some("kraft3"));
        assert_eq!(board.tools.keys().collect::<Vec<_>>(), ["kraft2"]);
        assert_eq!(board.tools["kraft2"].settings, board.cut);
        assert_eq!(got.materials[2].cut_tool.as_deref(), Some("rotary"));
        assert_eq!((crate::force_step(board.cut.force), crate::speed_step(board.cut.speed_mm_s)), (33, 1));
        assert_eq!((board.cut.passes, board.cut.accel), (3, Some(3)));
        // No score row: the usual score settings.
        assert_eq!(board.score, Operations::default().score);
        assert_eq!(got.machines, ["m1", "m2"]);
        let skipped: Vec<(&str, &str)> = got.skipped.iter().map(|s| (s.name.as_str(), s.reason.as_str())).collect();
        assert_eq!(skipped, [("Felt", "only emboss, rotary_cut, which VectorCraft doesn't do"), ("Foil", "no settings for m1")]);
        assert_eq!(got.unused, ["emboss", "rotary_cut", "stipple"]);
    }

    #[test]
    fn depth_from_the_file_only_for_the_automatic_blade() {
        let got = import(&list(), "m1", Depth::File).unwrap();
        assert_eq!(got.materials[0].cut.depth, Some(2));
        assert_eq!(got.materials[0].score.depth, Some(1));
        assert_eq!(got.materials[1].cut.depth, None);
        assert_eq!(got.materials[0].tools["deepcut"].settings.depth, None);
        // The Kraft Blade's is kept as the depth to set it to, not sent.
        assert_eq!((got.materials[0].hand_depth_mm, got.materials[1].hand_depth_mm), (None, Some(1.5)));
        assert_eq!(got.materials[1].tools["kraft2"].hand_depth_mm, Some(1.5));
        // Not for a rotary blade.
        assert_eq!(got.materials[2].hand_depth_mm, None);
    }

    #[test]
    fn another_machine() {
        let got = import(&list(), "m2", Depth::Auto).unwrap();
        let names: Vec<&str> = got.materials.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Test Paper", "Foil"]);
        assert_eq!(crate::force_step(got.materials[0].cut.force), 20);
    }

    #[test]
    fn not_a_list() {
        assert!(import("", "m1", Depth::Auto).is_err());
        assert!(import("a,b,c\n1,2,3\n", "m1", Depth::Auto).unwrap_err().contains("no full_name column"));
        let header_only = list().lines().next().unwrap().to_string();
        assert!(import(&header_only, "m1", Depth::Auto).unwrap_err().contains("no materials"));
        assert!(import(&"x".repeat(MAX_BYTES + 1), "m1", Depth::Auto).is_err());
    }

    #[test]
    fn quoted_fields_and_a_byte_order_mark() {
        let r = rows("\u{feff}a,\"b,\"\"c\"\"\"\nd");
        assert_eq!(r, vec![vec!["\u{feff}a".to_string(), "b,\"c\"".into()], vec!["d".into()]]);
        assert!(import(&format!("\u{feff}{}", list()), "m1", Depth::Auto).is_ok());
    }

    #[test]
    fn garbage_never_panics() {
        for text in [
            "\"",
            ",,,\n\n\"\"\"",
            "full_name,cutter_class_id,(force.pressure),(speed),(tool.type),category,uuid,parent_uuid,action\n,,,,,group,,,\n,,x,y,,leaf,,,cut",
        ] {
            let _ = import(text, "", Depth::File);
        }
    }
}
