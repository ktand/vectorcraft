//! The material picker (C18.17): the Material fields open a menu with a filter at the top and the
//! materials under their groups (Vinyl, Paper, Card & Board…, the built-in ones last as Starting
//! Points), so a long imported list stays easy to find things in.

use serde_json::Value;

use crate::widgets;

/// A material as the picker lists it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub name: String,
    pub group: String,
    pub builtin: bool,
}

/// What was picked.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Picked {
    /// The choice listed before the materials (a group's "Document's setup").
    First,
    Material(String),
}

/// The materials of a `cutting.materials` answer.
pub(crate) fn entries(listed: &Value) -> Vec<Entry> {
    listed["materials"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let name = m["name"].as_str()?.to_string();
            let group = m["group"].as_str().unwrap_or(vectorcraft_cutjob::OTHER_GROUP).to_string();
            Some(Entry { name, group, builtin: m["builtin"].as_bool() == Some(true) })
        })
        .collect()
}

/// Where a group is listed: the suggested ones in their order, the user's own after them by name,
/// then Other, then the built-in Starting Points.
fn rank(group: &str) -> (usize, String) {
    use vectorcraft_cutjob::{BUILTIN_GROUP, GROUPS, OTHER_GROUP};
    let n = GROUPS.len();
    let at = match group {
        g if g == BUILTIN_GROUP => n + 2,
        g if g == OTHER_GROUP => n + 1,
        g => GROUPS.iter().position(|(s, _)| *s == g).unwrap_or(n),
    };
    (at, group.to_lowercase())
}

/// A group's name as shown: the ones VectorCraft names translated, the user's as they are.
pub(crate) fn group_label(group: &str) -> &str {
    use vectorcraft_cutjob::{BUILTIN_GROUP, GROUPS, OTHER_GROUP};
    if group == BUILTIN_GROUP || group == OTHER_GROUP || GROUPS.iter().any(|(g, _)| *g == group) { tl!(group) } else { group }
}

/// The entries matching `query` (in a name or its group's, any case; empty: all) under their
/// groups, in [`rank`] order; each group's materials by name.
pub(crate) fn grouped<'a>(entries: &'a [Entry], query: &str) -> Vec<(&'a str, Vec<&'a Entry>)> {
    let q = query.trim().to_lowercase();
    let hit = |e: &Entry| q.is_empty() || e.name.to_lowercase().contains(&q) || group_label(&e.group).to_lowercase().contains(&q);
    let mut out: Vec<(&str, Vec<&Entry>)> = vec![];
    for e in entries.iter().filter(|e| hit(e)) {
        match out.iter_mut().find(|(g, _)| *g == e.group) {
            Some((_, list)) => list.push(e),
            None => out.push((e.group.as_str(), vec![e])),
        }
    }
    out.sort_by_key(|(g, _)| rank(g));
    for (_, list) in &mut out {
        list.sort_by_key(|e| e.name.to_lowercase());
    }
    out
}

/// The menu's tallest list, points.
const MENU_H: f32 = 340.0;
/// The menu's narrowest, points: imported names are long.
const MENU_W: f32 = 240.0;

/// A Material field showing `shown`, `width` wide, whose menu filters and groups `entries`;
/// `first` (such as "Document's setup") is listed before them while nothing is typed.
pub(crate) fn picker(ui: &mut egui::Ui, id: impl std::hash::Hash, shown: &str, first: Option<&str>, entries: &[Entry], width: f32) -> Option<Picked> {
    let t = crate::theme::Tokens::get(ui.ctx());
    widgets::combo_menu(ui, id, shown, width, |menu| {
        menu.height(MENU_H).show_ui_with_header(
            |ui| {
                ui.set_min_width(MENU_W);
                let field = ui.scope_id().with("filter");
                if ui.memory(|m| m.focused().is_none()) {
                    ui.memory_mut(|m| m.request_focus(field.with("edit")));
                }
                widgets::search_field(ui, field, tl!("Filter materials"))
            },
            |ui, query| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                let mut picked = None;
                if let Some(first) = first.filter(|_| query.trim().is_empty()) {
                    if ui.add(egui::Button::selectable(shown == first, first)).clicked() {
                        picked = Some(Picked::First);
                    }
                    ui.separator();
                }
                let groups = grouped(entries, &query);
                if groups.is_empty() {
                    widgets::dim_label(ui, tl!("No material matches."));
                }
                for (group, list) in groups {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(group_label(group)).size(11.0).strong().color(t.text_dim));
                    for e in list {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            let row = ui.add(egui::Button::selectable(e.name == shown, e.name.as_str()));
                            let row = if e.builtin { row.on_hover_text(tl!("A starting point: make a test cut first.")) } else { row };
                            if row.clicked() {
                                picked = Some(Picked::Material(e.name.clone()));
                            }
                        });
                    }
                }
                if picked.is_some() {
                    ui.close();
                }
                picked
            },
        )
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(name: &str, group: &str, builtin: bool) -> Entry {
        Entry { name: name.into(), group: group.into(), builtin }
    }

    /// The groups in their order (the user's own after the suggested ones, Starting Points last),
    /// names sorted in each; the filter matches names and groups.
    #[test]
    fn materials_are_listed_under_their_groups() {
        let all = [
            entry("Vinyl", "Starting Points", true),
            entry("Copy Paper", "Paper", false),
            entry("Glossy Vinyl", "Vinyl", false),
            entry("Bag", "Shop", false),
            entry("Cotton", "Fabric", false),
            entry("Matte Vinyl", "Vinyl", false),
            entry("Gift Wrap", "Other", false),
        ];
        let order: Vec<(&str, Vec<&str>)> = grouped(&all, "").into_iter().map(|(g, l)| (g, l.iter().map(|e| e.name.as_str()).collect())).collect();
        assert_eq!(
            order,
            [
                ("Vinyl", vec!["Glossy Vinyl", "Matte Vinyl"]),
                ("Fabric", vec!["Cotton"]),
                ("Paper", vec!["Copy Paper"]),
                ("Shop", vec!["Bag"]),
                ("Other", vec!["Gift Wrap"]),
                ("Starting Points", vec!["Vinyl"]),
            ]
        );
        let vinyl: Vec<&str> = grouped(&all, " VINYL").into_iter().map(|(g, _)| g).collect();
        assert_eq!(vinyl, ["Vinyl", "Starting Points"]);
        let fabric: Vec<&str> = grouped(&all, "fabric").into_iter().flat_map(|(_, l)| l).map(|e| e.name.as_str()).collect();
        assert_eq!(fabric, ["Cotton"], "a group's name matches its materials");
        assert!(grouped(&all, "felt").is_empty());
        let listed = json!({"materials": [{"name": "A", "group": "Paper", "builtin": false}, {"name": 3}]});
        assert_eq!(entries(&listed), [entry("A", "Paper", false)]);
    }
}
