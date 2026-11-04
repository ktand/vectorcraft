//! Fork: the job sheet (C18.8): a page to print beside the machine and the heat press, an A4 SVG
//! of text: the document, the mat, each group in cutting order with its material, its cut and
//! score settings and how the material is pressed (temperature, time, pressure, peel), and
//! whether it is cut mirrored.

use vectorcraft_cutjob::{CutJob, CuttingSettings, Material, ToolSettings};

/// The page, mm (A4 portrait).
const PAGE_MM: (f64, f64) = (210.0, 297.0);
/// One line's height and the margin, mm.
const LINE_MM: f64 = 6.0;
const MARGIN_MM: f64 = 15.0;

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn tool(t: &ToolSettings) -> String {
    format!("force {:.0}%, speed {:.0} mm/s, {} pass{}", t.force * 100.0, t.speed_mm_s, t.passes, if t.passes == 1 { "" } else { "es" })
}

/// The sheet's lines: `(text, bold)`.
pub fn lines(title: &str, s: &CuttingSettings, job: &CutJob, material_of: &dyn Fn(&str) -> Option<Material>) -> Vec<(String, bool)> {
    let mut out = vec![(format!("Job sheet: {title}"), true)];
    out.push((format!("Mat: {}{}", job.mat.id, if s.mirror { " · cut mirrored" } else { "" }), false));
    out.push((format!("{} paths", job.paths.len()), false));
    out.push((String::new(), false));
    let groups: Vec<(String, Option<String>)> = if job.groups.is_empty() {
        vec![(String::from("The whole job"), s.material.clone())]
    } else {
        job.groups.iter().map(|g| (g.name.clone(), g.material.clone().or_else(|| s.material.clone()))).collect()
    };
    for (i, (name, material)) in groups.iter().enumerate() {
        out.push((format!("{}. {name}", i + 1), true));
        let m = material.as_deref().and_then(material_of);
        match (&m, material) {
            (Some(m), _) => out.push((format!("Material: {}{}", m.name, if m.mirror { " (cut from the back)" } else { "" }), false)),
            (None, Some(n)) => out.push((format!("Material: {n}"), false)),
            (None, None) => out.push(("Material: not named".to_string(), false)),
        }
        out.push((format!("Cut: {}", tool(&s.operations.cut)), false));
        if let Some(p) = m.as_ref().and_then(|m| m.press) {
            out.push((format!("Press: {:.0} °C, {:.0} s, {} pressure, peel {}", p.temp_c, p.seconds, p.pressure.id(), p.peel.id()), false));
        }
        out.push((String::new(), false));
    }
    out
}

/// The sheet as an SVG page.
pub fn to_svg(title: &str, s: &CuttingSettings, job: &CutJob, material_of: &dyn Fn(&str) -> Option<Material>) -> String {
    let (w, h) = PAGE_MM;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\" font-family=\"sans-serif\" font-size=\"4\">\n"
    );
    let most = ((h - 2.0 * MARGIN_MM) / LINE_MM) as usize;
    for (i, (text, bold)) in lines(title, s, job, material_of).into_iter().take(most).enumerate() {
        let y = MARGIN_MM + (i as f64 + 1.0) * LINE_MM;
        let weight = if bold { " font-weight=\"bold\"" } else { "" };
        svg.push_str(&format!("<text x=\"{MARGIN_MM}\" y=\"{y:.1}\"{weight}>{}</text>\n", xml(&text)));
    }
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use vectorcraft_cutjob::{Peel, Press, Pressure};
    use vectorcraft_doc::{Appearance, Document, Node};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    #[test]
    fn the_sheet_lists_the_pressing() {
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(10.0, 10.0, 60.0, 60.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, sq).unwrap();
        let s = CuttingSettings { material: Some("Flex <HTV>".into()), ..CuttingSettings::default() };
        crate::set_settings(&mut d, &s);
        let job = crate::build_job(&d).unwrap();
        let press = Press { temp_c: 150.0, seconds: 15.0, pressure: Pressure::Medium, peel: Peel::Warm };
        let material_of = |n: &str| {
            (n == "Flex <HTV>").then(|| Material {
                press: Some(press),
                mirror: true,
                name: n.into(),
                ..vectorcraft_cutjob::builtin_materials()[0].clone()
            })
        };
        let text: Vec<String> = lines("Shirt", &s, &job, &material_of).into_iter().map(|(t, _)| t).collect();
        assert!(text.iter().any(|t| t == "Press: 150 °C, 15 s, medium pressure, peel warm"), "{text:?}");
        assert!(text.iter().any(|t| t.contains("(cut from the back)")));
        let svg = to_svg("Shirt", &s, &job, &material_of);
        assert!(svg.contains("Flex &lt;HTV&gt;") && svg.starts_with("<svg"));
    }
}
