//! Fork (C4.11): every job checked against the protocol notes before it is sent. The machine
//! ignores a command it doesn't know without a word ([HW] 2026-11-04), so a mistake in what we
//! send would only show in the cut: [`check`] reads the job back with [`crate::decode`] (known
//! commands only, each ended) and checks what it asks: values in the ranges the notes give,
//! positions on the machine, a blade that cuts only inside the cut area the job sets, and that
//! area put back before the end. The encoder runs it on every job; a failure refuses the send.

use crate::decode::{Step, blade_paths, decode};
use crate::{FORCE, MAX_LENGTH_MM, SPEED};
use vectorcraft_cutter::ORIGIN_ADJUST_MM;

/// How far a position may round past an edge, mm (a unit and a bit).
const SLACK_MM: f64 = 0.06;

/// The mats the machine has a code for (protocol notes › Mats), 0 for none.
const MAT_CODES: [i64; 7] = [0, 1, 2, 3, 8, 9, 11];

/// Why a check failed, for the user: what was wrong and where.
fn fault(n: usize, what: &str) -> String {
    format!("the job didn't pass the check before sending (command {n}: {what}); nothing was sent")
}

/// Whether `v` is a whole step in `range`.
fn within(v: i64, (lo, hi): (u32, u32)) -> bool {
    (i64::from(lo)..=i64::from(hi)).contains(&v)
}

/// `bytes`, a job for a machine `width_mm` wide, against the protocol notes: an error saying
/// what is wrong, if anything.
pub fn check(bytes: &[u8], width_mm: f64) -> Result<(), String> {
    let steps = decode(bytes).map_err(|e| format!("the job didn't pass the check before sending: {e}; nothing was sent"))?;
    if steps.first() != Some(&Step::Reset) || steps.last() != Some(&Step::End) {
        return Err(fault(0, "it must start with a reset and end with SO0"));
    }
    let (across, down) = (width_mm + ORIGIN_ADJUST_MM + SLACK_MM, MAX_LENGTH_MM + ORIGIN_ADJUST_MM + SLACK_MM);
    let on_machine = |q: &[f64; 2]| q[0] >= -SLACK_MM && q[1] >= -SLACK_MM && q[0] <= across && q[1] <= down;
    let mut area_from: Option<[f64; 2]> = None;
    let mut area: Option<([f64; 2], [f64; 2])> = None;
    let in_area = |area: Option<([f64; 2], [f64; 2])>, q: &[f64; 2]| {
        area.is_none_or(|(lo, hi)| q[0] >= lo[0] - SLACK_MM && q[1] >= lo[1] - SLACK_MM && q[0] <= hi[0] + SLACK_MM && q[1] <= hi[1] + SLACK_MM)
    };
    for (n, s) in steps.iter().enumerate() {
        let ok = match s {
            Step::Reset | Step::End | Step::OffScale | Step::Home => Ok(()),
            Step::Mat(code) => MAT_CODES.contains(code).then_some(()).ok_or("a mat code the notes don't give"),
            Step::Orientation(o) => (*o == 0).then_some(()).ok_or("an orientation other than portrait"),
            Step::MarkSetting(code, v) => match (code, v) {
                (99, None) | (52, Some(2)) | (55, Some(1)) => Ok(()),
                (51 | 53, Some(v)) if (1..=2_000).contains(v) => Ok(()),
                _ => Err("a registration mark setting out of range"),
            },
            Step::MarkSearch { size, from } => {
                (size[0] > 0.0 && size[1] > 0.0 && on_machine(from) && on_machine(&[from[0] + size[0], from[1] + size[1]]))
                    .then_some(())
                    .ok_or("a mark search off the machine")
            }
            Step::TrackEnhancing(v) => (*v == 0).then_some(()).ok_or("a track enhancing value other than FY0"),
            Step::Holder(h) => (1..=2).contains(h).then_some(()).ok_or("no such tool holder"),
            Step::Force(f, h) => (within(*f, FORCE) && (1..=2).contains(h)).then_some(()).ok_or("a force step out of range"),
            Step::Speed(v, h) => (within(*v, SPEED) && (1..=2).contains(h)).then_some(()).ok_or("a speed step out of range"),
            Step::Offset(o, second, h) => {
                ((0..=40).contains(o) && *second == 1 && (1..=2).contains(h)).then_some(()).ok_or("a blade offset out of range")
            }
            Step::Depth(d, h) => ((1..=10).contains(d) && *h == 1).then_some(()).ok_or("a blade depth out of range or not for holder 1"),
            Step::Accel(a) => (1..=3).contains(a).then_some(()).ok_or("an acceleration out of range"),
            Step::LineType(t) => [0, 2, 3, 4].contains(t).then_some(()).ok_or("a line type the notes don't describe"),
            Step::Pitch(p) => (0.5..=100.0).contains(p).then_some(()).ok_or("a line type's pitch out of range"),
            Step::AreaFrom(q) => {
                area_from = Some(*q);
                on_machine(q).then_some(()).ok_or("a cut area off the machine")
            }
            Step::AreaTo(q) => match area_from.take() {
                Some(lo) if on_machine(q) && q[0] > lo[0] && q[1] > lo[1] => {
                    area = Some((lo, *q));
                    Ok(())
                }
                _ => Err("a cut area without its near corner, empty or off the machine"),
            },
            Step::Move { down, to } => {
                if !to.iter().all(on_machine) {
                    Err("a position off the machine")
                } else if *down && !to.iter().all(|q| in_area(area, q)) {
                    Err("a cut outside the cut area the job set")
                } else {
                    Ok(())
                }
            }
            Step::Bezier { from, a, b, to } => {
                let pts = [from, a, b, to];
                if !pts.iter().all(|q| on_machine(q)) {
                    Err("a curve off the machine")
                } else if !pts.iter().all(|q| in_area(area, q)) {
                    Err("a curve outside the cut area the job set")
                } else {
                    Ok(())
                }
            }
            Step::Circle { centre, radius, .. } => {
                let corners = [[centre[0] - radius[0], centre[1] - radius[1]], [centre[0] + radius[0], centre[1] + radius[1]]];
                if !corners.iter().all(on_machine) {
                    Err("a circle off the machine")
                } else if !corners.iter().all(|q| in_area(area, q)) {
                    Err("a circle outside the cut area the job set")
                } else {
                    Ok(())
                }
            }
        };
        ok.map_err(|what| fault(n, what))?;
    }
    // An area left set clips every later job until the machine is switched off: the job's last
    // one must be the machine's whole area again, from its origin.
    if let Some((lo, _)) = area
        && (lo[0] != 0.0 || lo[1] != 0.0)
    {
        return Err(fault(steps.len(), "the cut area isn't put back at the end"));
    }
    // Each curve starts where the blade is.
    blade_paths(&steps).map_err(|e| fault(steps.len(), &e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EOT, ESC, ETX};

    fn job(list: &[&str]) -> Vec<u8> {
        let mut b = vec![ESC, EOT];
        for c in list {
            b.extend(c.as_bytes());
            b.push(ETX);
        }
        b
    }

    const GOOD: [&str; 14] =
        ["TG1", "TB50,0", "\\0,0", "Z6096,6096", "J1", "FX17,1", "!10,1", "FC18,1,1", "M400,200", "D400,400", "@", "\\0,0", "Z60200,6296", "SO0"];

    #[test]
    fn a_good_job_passes() {
        assert_eq!(check(&job(&GOOD), 304.8), Ok(()));
    }

    /// Each kind of fault is refused, with what it was.
    #[test]
    fn each_kind_of_fault_is_refused() {
        let swap = |i: usize, with: &str| {
            let mut l: Vec<&str> = GOOD.to_vec();
            l[i] = with;
            l.iter().map(|s| s.to_string()).collect::<Vec<String>>()
        };
        let cases: Vec<(Vec<String>, &str)> = vec![
            (swap(1, "TB50,1"), "orientation"),
            (swap(0, "TG5"), "mat code"),
            (swap(5, "FX34,1"), "force"),
            (swap(6, "!0,1"), "speed"),
            (swap(4, "J3"), "holder"),
            (swap(7, "FC41,1,1"), "blade offset"),
            (swap(9, "D400,8000"), "off the machine"),
            (swap(3, "Z300,300"), "outside the cut area"),
            (swap(11, "\\20,20"), "put back"),
            (swap(9, "Q1"), "isn't one"),
            (swap(9, "BZ1,0,0,1,1,2,2,3,3,0"), "starts at"),
            (swap(9, "L7"), "line type"),
        ];
        for (list, said) in cases {
            let refs: Vec<&str> = list.iter().map(String::as_str).collect();
            let e = check(&job(&refs), 304.8).unwrap_err();
            assert!(e.contains(said), "{refs:?}: {e}");
        }
        // Not ended, not started with a reset, or not ended with SO0.
        let mut open = job(&GOOD);
        open.pop();
        assert!(check(&open, 304.8).unwrap_err().contains("isn't ended"));
        assert!(check(&job(&GOOD)[2..], 304.8).is_err());
        assert!(check(&job(&GOOD[..13]), 304.8).is_err());
    }
}
