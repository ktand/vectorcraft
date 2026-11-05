//! Materials exported from Silhouette Studio (C18.14): the user's own export of the maker's
//! software's material list (a CSV file), read into materials of the user's. Only the file the
//! user picks is read; no maker's material table is part of VectorCraft (the file's layout is in
//! `docs/cutting/protocol-silhouette.md` › Material lists).
//!
//! A material is a group row; its settings are the rows under it (by parent id), one per machine
//! and action. For the machine asked for: the first row that cuts (`cut`, else `kraft_cut`) with a
//! blade this driver cuts with gives the cut settings, the first `score` row the score settings.
//! Force and speed are the machine's own steps, which this driver sends as they are; passes,
//! acceleration and the end overcut are taken as they are. The file's depth is in millimetres
//! (the AutoBlade cuts at most 1 mm deep, the owner, 2026-11-05, and no AutoBlade row of their list
//! is deeper): the AutoBlade's is sent as 0.1 mm a step of its 1–10 ([`Depth::File`]); a blade set
//! by hand (Kraft, Deep-Cut, ratchet) keeps it as the depth to set it to, never sent
//! ([`Material::hand_depth_mm`]).

use vectorcraft_cutjob::{MAX_MATERIALS, MAX_NAME, Material, Operations, ToolSettings};

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

/// The blades this driver cuts with, by the file's tool names, and what an imported material's
/// name says when it isn't the automatic blade (whose depth is sent; the others' is set by hand).
fn blade(tool: &str) -> Option<Option<&'static str>> {
    match tool {
        "autoadjust" => Some(None),
        "adjustable" => Some(Some("Ratchet Blade")),
        "kraft" | "kraft2" => Some(Some("Kraft Blade")),
        "deepcut" => Some(Some("Deep-Cut Blade")),
        _ => None,
    }
}

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
        let cutting = ["cut", "kraft_cut"].iter().find_map(|a| mine.iter().find(|l| l.action == *a && blade(&l.tool).is_some()));
        let Some(cut_leaf) = cutting else {
            let mut kinds: Vec<&str> = mine.iter().map(|l| l.action.as_str()).collect();
            kinds.dedup();
            out.skipped.push(skip(&format!("only {}, which VectorCraft doesn't cut", kinds.join(", "))));
            continue;
        };
        let suffix = blade(&cut_leaf.tool).flatten();
        let Some(cut) = settings(cut_leaf.row, &c, &defaults.cut, depth, suffix.is_none()) else {
            out.skipped.push(skip("its cut has no force or speed"));
            continue;
        };
        let score = mine
            .iter()
            .find(|l| l.action == "score" && blade(&l.tool).is_some())
            .and_then(|l| settings(l.row, &c, &defaults.score, depth, blade(&l.tool).flatten().is_none()))
            .unwrap_or_else(|| defaults.score.clone());
        let full = match suffix {
            Some(s) => fit(&format!("{name} ({s})")),
            None => name.clone(),
        };
        if out.materials.iter().any(|m| m.name.eq_ignore_ascii_case(&full)) {
            out.skipped.push(skip("a second material of the same name"));
            continue;
        }
        if out.materials.len() >= MAX_MATERIALS {
            out.skipped.push(skip(&format!("more than {MAX_MATERIALS} materials")));
            continue;
        }
        // A blade set by hand: the depth to set it to.
        let hand_depth_mm =
            suffix.and_then(|_| number(cut_leaf.row, c.depth)).filter(|d| *d > 0.0).map(|d| d.min(vectorcraft_cutjob::MAX_HAND_DEPTH_MM));
        out.materials.push(Material {
            name: full,
            cut,
            score,
            builtin: false,
            mirror: false,
            price: None,
            roll_left_mm: None,
            press: None,
            hand_depth_mm,
        });
    }
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
            "Test Paper for M2,m2,20,8,autoadjust,0.4,1,1,0.1,M2,leaf,l3,g1,cut",
            "\"Board, thick\",desktop,15,5,adjustable,0.3,1,1,0.1,\"Board, thick\",group,g2,root,",
            "Board for M1,m1,40,0.3,kraft2,1.5,3,5,0.1,M1,leaf,l4,g2,kraft_cut",
            "Felt,desktop,15,5,adjustable,0.3,1,1,0.1,Felt,group,g3,root,",
            "Felt for M1,m1,20,5,rotary,0.3,1,1,0.1,M1,leaf,l5,g3,rotary_cut",
            "Foil,desktop,15,5,adjustable,0.3,1,1,0.1,Foil,group,g4,root,",
            "Foil for M2,m2,20,5,autoadjust,0.3,1,1,0.1,M2,leaf,l6,g4,cut",
        ];
        format!("{header}\r\n{}\r\n", rows.join("\r\n"))
    }

    #[test]
    fn a_machines_materials() {
        let got = import(&list(), "M1", Depth::Auto).unwrap();
        let names: Vec<&str> = got.materials.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Test Paper", "Board, thick (Kraft Blade)"]);
        let paper = &got.materials[0];
        // The machine's own steps, read back exactly.
        assert_eq!(crate::force_step(paper.cut.force), 12);
        assert_eq!(crate::speed_step(paper.cut.speed_mm_s), 4);
        assert_eq!((paper.cut.passes, paper.cut.accel, paper.cut.depth), (2, Some(3), None));
        assert!((paper.cut.overcut_mm - 0.2).abs() < 1e-9);
        assert_eq!(crate::force_step(paper.score.force), 6);
        assert!(!paper.builtin && paper.cut.tool.is_none());
        // A speed under a step, an acceleration over ours: brought into range.
        let board = &got.materials[1];
        assert_eq!((crate::force_step(board.cut.force), crate::speed_step(board.cut.speed_mm_s)), (33, 1));
        assert_eq!((board.cut.passes, board.cut.accel), (3, Some(3)));
        // No score row: the usual score settings.
        assert_eq!(board.score, Operations::default().score);
        assert_eq!(got.machines, ["m1", "m2"]);
        let skipped: Vec<(&str, &str)> = got.skipped.iter().map(|s| (s.name.as_str(), s.reason.as_str())).collect();
        assert_eq!(skipped, [("Felt", "only rotary_cut, which VectorCraft doesn't cut"), ("Foil", "no settings for m1")]);
    }

    #[test]
    fn depth_from_the_file_only_for_the_automatic_blade() {
        let got = import(&list(), "m1", Depth::File).unwrap();
        assert_eq!(got.materials[0].cut.depth, Some(2));
        assert_eq!(got.materials[0].score.depth, Some(1));
        assert_eq!(got.materials[1].cut.depth, None);
        // The Kraft Blade's is kept as the depth to set it to, not sent.
        assert_eq!((got.materials[0].hand_depth_mm, got.materials[1].hand_depth_mm), (None, Some(1.5)));
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
