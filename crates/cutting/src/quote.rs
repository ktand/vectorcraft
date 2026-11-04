//! Fork: the job's quote (C18.3): how long it takes (the simulator's time), and for each group
//! (the whole job without groups) its material, what of it the job uses (a sheet on a mat; on a
//! roll, the length to the end of the group's paths and 10 mm more), and what that costs where the
//! material has a price. Prices are the user's, in their own currency.

use serde_json::{Value, json};
use vectorcraft_cutjob::{CutJob, CuttingSettings, Price, ROLL, Simulation};

/// Past the end of a group's paths, a roll is cut this much further, mm.
pub const ROLL_MARGIN_MM: f64 = 10.0;

/// One group's share.
#[derive(Clone, Debug, PartialEq)]
pub struct Share {
    /// The group's name (none without groups).
    pub name: Option<String>,
    /// The material it is cut from: the group's own, else the document's.
    pub material: Option<String>,
    /// Sheets (on a mat: one), or metres of a roll.
    pub sheets: f64,
    pub metres: f64,
    /// None: its material has no price.
    pub cost: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quote {
    pub seconds: f64,
    pub shares: Vec<Share>,
    /// The whole job's cost; none when a group's material has no price.
    pub cost: Option<f64>,
}

impl Quote {
    pub fn to_value(&self) -> Value {
        let groups: Vec<Value> = self
            .shares
            .iter()
            .map(|g| json!({"name": g.name, "material": g.material, "sheets": g.sheets, "metres": g.metres, "cost": g.cost}))
            .collect();
        json!({"seconds": self.seconds, "groups": groups, "cost": self.cost})
    }
}

/// `job`'s quote, its materials' prices from `price_of` (by name).
pub fn quote(s: &CuttingSettings, job: &CutJob, price_of: &dyn Fn(&str) -> Option<Price>) -> Quote {
    let seconds = Simulation::new(job).total_seconds;
    let roll = s.mat == ROLL;
    let mut spans: Vec<(Option<String>, Option<String>, usize, usize)> = vec![];
    if job.groups.is_empty() {
        spans.push((None, s.material.clone(), 0, job.paths.len()));
    } else {
        for (i, g) in job.groups.iter().enumerate() {
            let end = job.groups.get(i + 1).map_or(job.paths.len(), |n| n.start);
            spans.push((Some(g.name.clone()), g.material.clone().or_else(|| s.material.clone()), g.start, end));
        }
    }
    let mut total = Some(0.0);
    let shares: Vec<Share> = spans
        .into_iter()
        .map(|(name, material, a, b)| {
            let paths = job.paths.get(a..b).unwrap_or_default();
            let end = paths.iter().flat_map(|p| &p.points).map(|q| q[1]).filter(|y| y.is_finite()).fold(0.0f64, f64::max);
            let (sheets, metres) = if roll { (0.0, (end + ROLL_MARGIN_MM) / 1000.0) } else { (1.0, 0.0) };
            let cost = material.as_deref().and_then(price_of).map(|p| p.cost(sheets, metres));
            total = total.zip(cost).map(|(t, c)| t + c);
            Share { name, material, sheets, metres, cost }
        })
        .collect();
    let cost = if shares.is_empty() { None } else { total };
    Quote { seconds, shares, cost }
}

#[cfg(test)]
mod tests {
    use vectorcraft_doc::{Appearance, Document, Node};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    fn job_of(roll: bool) -> (CuttingSettings, CutJob) {
        let mut d = Document::new(500.0, 500.0);
        let layer = d.layers[0].id;
        let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(10.0, 10.0, 110.0, 290.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, sq).unwrap();
        let mut s = CuttingSettings { material: Some("Mine".into()), ..CuttingSettings::default() };
        if roll {
            s.mat = ROLL.to_string();
            s.custom_mat_mm = [300.0, 1000.0];
        }
        crate::set_settings(&mut d, &s);
        (s, crate::build_job(&d).unwrap())
    }

    #[test]
    fn a_sheet_and_a_roll_are_priced() {
        let price = |per_metre| move |name: &str| (name == "Mine").then_some(Price { amount: 2.0, per_metre, waste_pct: 50.0 });
        let (s, job) = job_of(false);
        let q = quote(&s, &job, &price(false));
        assert!(q.seconds > 0.0);
        assert_eq!((q.shares.len(), q.cost), (1, Some(2.0)), "one sheet");
        let (s, job) = job_of(true);
        let q = quote(&s, &job, &price(true));
        let m = q.shares[0].metres;
        // The square's bottom edge at 290 pt (about 102 mm down), 10 mm more, and the waste.
        assert!(m > 0.09 && m < 0.2, "{m}");
        assert!((q.cost.unwrap() - 2.0 * m * 1.5).abs() < 1e-9);
        let none = quote(&s, &job, &|_: &str| None);
        assert_eq!(none.cost, None, "no price, no cost");
    }
}
