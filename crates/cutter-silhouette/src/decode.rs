//! Fork (C4.10): reading back what the encoder sends, from the protocol notes
//! (`docs/cutting/protocol-silhouette.md`) only. [`decode`] turns a byte stream into the steps it
//! asks of the machine, and refuses anything the notes don't describe; [`blade_paths`] turns the
//! steps into where the blade cuts, in mat mm. Tests decode every job they encode and compare it
//! with the job, and the pre-send check (C4.11) decodes each job before it goes.
//!
//! Positions are as the machine takes them: whole units, 20 a millimetre, from its origin, down
//! first; here they are given back as mat mm across and down from that origin.

use crate::{EOT, ESC, ETX, UNITS_PER_MM};

/// The most numbers one command may carry (a multi-point `D` included).
const MAX_NUMBERS: usize = 2_000;
/// The most commands one stream may hold.
const MAX_COMMANDS: usize = 4_000_000;
/// The longest command, bytes.
const MAX_COMMAND_BYTES: usize = 32_768;

/// One thing the stream asks of the machine. Positions are mat mm, `[across, down]`, from the
/// machine's origin; tool settings are the machine's own steps.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// ESC EOT: start again.
    Reset,
    /// `TG`: the mat's code (0: none).
    Mat(i64),
    /// `TB50`: the orientation (0: portrait).
    Orientation(i64),
    /// `TB99`, `TB52`, `TB51`, `TB53`, `TB55`: a registration mark setting (the number after
    /// `TB`, then its value, if any).
    MarkSetting(i64, Option<i64>),
    /// `TB123`: look for the marks over `size` from `from`.
    MarkSearch { size: [f64; 2], from: [f64; 2] },
    /// `FY`: track enhancing.
    TrackEnhancing(i64),
    /// `J`: the tool holder used.
    Holder(i64),
    /// `FX`: force step, holder.
    Force(i64, i64),
    /// `!`: speed step, holder.
    Speed(i64, i64),
    /// `FC`: blade offset (units), the second value, holder.
    Offset(i64, i64, i64),
    /// `TF`: AutoBlade depth, holder.
    Depth(i64, i64),
    /// `TJ`: acceleration.
    Accel(i64),
    /// `\`: the cut area's near corner.
    AreaFrom([f64; 2]),
    /// `Z`: the cut area's far corner.
    AreaTo([f64; 2]),
    /// `M` (blade up) or `D` (blade down) to each point in turn.
    Move { down: bool, to: Vec<[f64; 2]> },
    /// `BZ1`: a cubic from `from` (where the blade must be) through `a` and `b` to `to`, blade down.
    Bezier { from: [f64; 2], a: [f64; 2], b: [f64; 2], to: [f64; 2] },
    /// `W`: round `centre` with `radius` (across, down), from `start` to `end` degrees (0: down
    /// the mat), blade down.
    Circle { centre: [f64; 2], radius: [f64; 2], start: f64, end: f64 },
    /// `L`: the line type (0: solid).
    LineType(i64),
    /// `B`: the line type's pitch, mm.
    Pitch(f64),
    /// `@`: asked whether anything was outside the cut area.
    OffScale,
    /// `H`: the head to the machine's own home (C12.6), the origin left where it is.
    Home,
    /// `SO0`: the end of the job.
    End,
}

/// Why a stream can't be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The last command has no ETX.
    Unterminated,
    /// Command `n` (from 0) isn't one the notes describe, or its values aren't what it takes.
    Unknown(usize, String),
    /// Too much to read.
    TooLong,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Unterminated => write!(f, "the last command isn't ended"),
            DecodeError::Unknown(n, text) => write!(f, "command {n} ({text:?}) isn't one the machine is known to take"),
            DecodeError::TooLong => write!(f, "too much to read"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Units as mm.
fn mm(units: i64) -> f64 {
    units as f64 / UNITS_PER_MM
}

/// A machine pair (down, across, units) as mat mm `[across, down]`.
fn point(down: i64, across: i64) -> [f64; 2] {
    [mm(across), mm(down)]
}

/// `text`'s comma-separated whole numbers, if that is all it is.
fn numbers(text: &str) -> Option<Vec<i64>> {
    if text.is_empty() {
        return Some(vec![]);
    }
    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() > MAX_NUMBERS {
        return None;
    }
    parts
        .into_iter()
        .map(|p| {
            let digits = p.strip_prefix('-').unwrap_or(p);
            (!digits.is_empty() && digits.len() <= 9 && digits.bytes().all(|b| b.is_ascii_digit())).then(|| p.parse().ok()).flatten()
        })
        .collect()
}

/// The pairs `n` holds as points, if it holds whole pairs.
fn points(n: &[i64]) -> Option<Vec<[f64; 2]>> {
    (!n.is_empty() && n.len().is_multiple_of(2)).then(|| {
        n.chunks(2)
            .filter_map(|c| match c {
                [d, a] => Some(point(*d, *a)),
                _ => None,
            })
            .collect()
    })
}

/// One ETX-ended command's text as a step.
fn step(text: &str) -> Option<Step> {
    // Longer names first: `BZ` before `B`, `TB` before `T…`.
    let names = ["TB", "TG", "TF", "TJ", "FX", "FC", "FY", "BZ", "SO", "!", "J", "M", "D", "W", "L", "B", "\\", "Z", "@", "H"];
    let name = names.into_iter().find(|n| text.starts_with(n))?;
    let n = numbers(text.get(name.len()..)?)?;
    let one = |n: &[i64]| match n {
        [a] => Some(*a),
        _ => None,
    };
    let two = |n: &[i64]| match n {
        [a, b] => Some((*a, *b)),
        _ => None,
    };
    Some(match name {
        "TB" => match n.as_slice() {
            [50, o] => Step::Orientation(*o),
            [123, h, w, top, left] => Step::MarkSearch { size: point(*h, *w), from: point(*top, *left) },
            [code @ (99 | 51 | 52 | 53 | 55)] => Step::MarkSetting(*code, None),
            [code @ (99 | 51 | 52 | 53 | 55), v] => Step::MarkSetting(*code, Some(*v)),
            _ => return None,
        },
        "TG" => Step::Mat(one(&n)?),
        "TF" => two(&n).map(|(d, h)| Step::Depth(d, h))?,
        "TJ" => Step::Accel(one(&n)?),
        "FX" => two(&n).map(|(f, h)| Step::Force(f, h))?,
        "FC" => match n.as_slice() {
            [o, s, h] => Step::Offset(*o, *s, *h),
            _ => return None,
        },
        "FY" => Step::TrackEnhancing(one(&n)?),
        "BZ" => match n.as_slice() {
            [1, fd, fa, ad, aa, bd, ba, td, ta, 0] => {
                Step::Bezier { from: point(*fd, *fa), a: point(*ad, *aa), b: point(*bd, *ba), to: point(*td, *ta) }
            }
            _ => return None,
        },
        "SO" => match n.as_slice() {
            [0] => Step::End,
            _ => return None,
        },
        "!" => two(&n).map(|(s, h)| Step::Speed(s, h))?,
        "J" => Step::Holder(one(&n)?),
        "M" | "D" => Step::Move { down: name == "D", to: points(&n)? },
        "W" => match n.as_slice() {
            [cd, ca, rd, ra, s, e] if *rd > 0 && *ra > 0 => {
                Step::Circle { centre: point(*cd, *ca), radius: [mm(*ra), mm(*rd)], start: *s as f64 / 10.0, end: *e as f64 / 10.0 }
            }
            _ => return None,
        },
        "L" => Step::LineType(one(&n)?),
        "B" => Step::Pitch(mm(one(&n)?)),
        "\\" => two(&n).map(|(d, a)| Step::AreaFrom(point(d, a)))?,
        "Z" => two(&n).map(|(d, a)| Step::AreaTo(point(d, a)))?,
        "@" if n.is_empty() => Step::OffScale,
        "H" if n.is_empty() => Step::Home,
        _ => return None,
    })
}

/// `bytes` as the steps they ask of the machine, in order: ESC EOT, then ETX-ended ASCII
/// commands, each one the protocol notes describe with the values it takes.
pub fn decode(bytes: &[u8]) -> Result<Vec<Step>, DecodeError> {
    let mut out = vec![];
    let mut rest = bytes;
    while !rest.is_empty() {
        if out.len() >= MAX_COMMANDS {
            return Err(DecodeError::TooLong);
        }
        if let Some(after) = rest.strip_prefix(&[ESC, EOT]) {
            out.push(Step::Reset);
            rest = after;
            continue;
        }
        let end = rest.iter().position(|b| *b == ETX).ok_or(DecodeError::Unterminated)?;
        let (command, after) = rest.split_at(end);
        rest = after.get(1..).unwrap_or_default();
        let shown = || String::from_utf8_lossy(command.get(..command.len().min(40)).unwrap_or_default()).to_string();
        if command.len() > MAX_COMMAND_BYTES {
            return Err(DecodeError::Unknown(out.len(), shown()));
        }
        let text = std::str::from_utf8(command).ok().filter(|t| t.is_ascii());
        match text.and_then(step) {
            Some(s) => out.push(s),
            None => return Err(DecodeError::Unknown(out.len(), shown())),
        }
    }
    Ok(out)
}

/// The share of each pitch cut for line type `t`, as the notes read the manual (1/2, 25/32,
/// 1/8: [HW] 2026-11-04); 1 for solid.
fn share(t: i64) -> Option<f64> {
    match t {
        0 => Some(1.0),
        3 => Some(0.5),
        4 => Some(25.0 / 32.0),
        2 => Some(0.125),
        _ => None,
    }
}

/// Points along a circle step, mat mm: every 2°, and its end.
fn arc(centre: [f64; 2], radius: [f64; 2], start: f64, end: f64) -> Vec<[f64; 2]> {
    let at = |deg: f64| {
        let t = deg.to_radians();
        // Angle 0 is down the mat; the machine's first axis (down) is the cosine.
        [centre[0] + radius[0] * t.sin(), centre[1] + radius[1] * t.cos()]
    };
    let span = (end - start).abs().min(3_600.0);
    let n = ((span / 2.0).ceil() as usize).max(1);
    (1..=n).map(|i| at(start + (end - start) * i as f64 / n as f64)).collect()
}

/// Points along a cubic, mat mm: sixteen, the last its end.
fn cubic(p0: [f64; 2], a: [f64; 2], b: [f64; 2], q: [f64; 2]) -> Vec<[f64; 2]> {
    (1..=16)
        .map(|i| {
            let t = f64::from(i) / 16.0;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            [0, 1].map(|k| w[0] * p0[k] + w[1] * a[k] + w[2] * b[k] + w[3] * q[k])
        })
        .collect()
}

/// `run` (a blade-down polyline, mat mm) as the dashes of `share` of each `pitch` mm, from a cut
/// at its start. Where the machine's dashes really start is hardware checks step 20.
fn dash(run: &[[f64; 2]], pitch: f64, share: f64) -> Vec<Vec<[f64; 2]>> {
    if share >= 1.0 || !(pitch.is_finite() && pitch > 0.0) {
        return vec![run.to_vec()];
    }
    let (on, off) = (share * pitch, (1.0 - share) * pitch);
    let mut out = vec![];
    let mut current: Vec<[f64; 2]> = run.first().map(|p| vec![*p]).unwrap_or_default();
    let (mut cutting, mut left) = (true, on);
    for w in run.windows(2) {
        let (mut from, to) = (w[0], w[1]);
        let mut len = (to[0] - from[0]).hypot(to[1] - from[1]);
        while len > 1e-12 {
            let step = left.min(len);
            let t = step / len;
            let at = [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t];
            if cutting {
                current.push(at);
            }
            left -= step;
            len -= step;
            from = at;
            if left <= 1e-12 {
                if cutting {
                    out.push(std::mem::take(&mut current));
                } else {
                    current = vec![at];
                }
                cutting = !cutting;
                left = if cutting { on } else { off };
            }
        }
    }
    if cutting && current.len() > 1 {
        out.push(current);
    }
    out
}

/// Where the blade cuts, as `steps` say: each run of it down, a polyline in mat mm from the
/// machine's origin, curves as points along them, under a line type its dashes; the start is
/// the origin. A curve that doesn't start where the blade is is an error.
pub fn blade_paths(steps: &[Step]) -> Result<Vec<Vec<[f64; 2]>>, String> {
    let mut out = vec![];
    let mut run: Vec<[f64; 2]> = vec![];
    let mut at = [0.0, 0.0];
    let (mut line, mut pitch) = (0, 0.0);
    let flush = |run: &mut Vec<[f64; 2]>, out: &mut Vec<Vec<[f64; 2]>>, line: i64, pitch: f64| {
        if run.len() > 1 {
            out.extend(dash(run, pitch, share(line).unwrap_or(1.0)));
        }
        run.clear();
    };
    for s in steps {
        match s {
            Step::Move { down: false, to } => {
                flush(&mut run, &mut out, line, pitch);
                at = to.last().copied().unwrap_or(at);
            }
            Step::Move { down: true, to } => {
                if run.is_empty() {
                    run.push(at);
                }
                run.extend(to);
                at = to.last().copied().unwrap_or(at);
            }
            Step::Bezier { from, a, b, to } => {
                if (from[0] - at[0]).hypot(from[1] - at[1]) > 1e-9 {
                    return Err(format!("a curve starts at {from:?}, the blade at {at:?}"));
                }
                if run.is_empty() {
                    run.push(at);
                }
                run.extend(cubic(*from, *a, *b, *to));
                at = *to;
            }
            Step::Circle { centre, radius, start, end } => {
                let points = arc(*centre, *radius, *start, *end);
                if run.is_empty() {
                    run.push(at);
                }
                at = points.last().copied().unwrap_or(at);
                run.extend(points);
            }
            Step::LineType(t) => {
                flush(&mut run, &mut out, line, pitch);
                share(*t).ok_or_else(|| format!("line type {t} isn't one the notes describe"))?;
                line = *t;
            }
            Step::Pitch(p) => {
                flush(&mut run, &mut out, line, pitch);
                pitch = *p;
            }
            // Settings, the area and the marks don't move the blade; a reset or the end of the
            // job lifts it.
            Step::Reset | Step::End => flush(&mut run, &mut out, line, pitch),
            // Home is the machine's own, not a position from the origin: only lifts the blade.
            Step::Home => flush(&mut run, &mut out, line, pitch),
            _ => {}
        }
    }
    flush(&mut run, &mut out, line, pitch);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds(list: &[&str]) -> Vec<u8> {
        let mut b = vec![ESC, EOT];
        for c in list {
            b.extend(c.as_bytes());
            b.push(ETX);
        }
        b
    }

    #[test]
    fn the_commands_read_back() {
        let s = decode(&cmds(&[
            "TG1",
            "TB50,0",
            "\\0,0",
            "Z6096,6096",
            "J1",
            "FX17,1",
            "!10,1",
            "FC18,1,1",
            "M400,200",
            "D400,400,600,400",
            "L3",
            "B80",
            "@",
            "SO0",
        ]))
        .unwrap();
        assert_eq!(
            s,
            [
                Step::Reset,
                Step::Mat(1),
                Step::Orientation(0),
                Step::AreaFrom([0.0, 0.0]),
                Step::AreaTo([304.8, 304.8]),
                Step::Holder(1),
                Step::Force(17, 1),
                Step::Speed(10, 1),
                Step::Offset(18, 1, 1),
                Step::Move { down: false, to: vec![[10.0, 20.0]] },
                Step::Move { down: true, to: vec![[20.0, 20.0], [20.0, 30.0]] },
                Step::LineType(3),
                Step::Pitch(4.0),
                Step::OffScale,
                Step::End,
            ]
        );
    }

    #[test]
    fn what_the_notes_dont_describe_is_refused() {
        for bad in [
            &b"TG1"[..],
            b"Q1\x03",
            b"M1\x03",
            b"M1,2,3\x03",
            b"Ma,b\x03",
            b"M1,,2\x03",
            b"BZ1,1,2\x03",
            b"W0,0,0,0,0,3600\x03",
            b"\xFF\x03",
            b"TB77\x03",
            b"@1\x03",
        ] {
            assert!(decode(bad).is_err(), "{:?}", String::from_utf8_lossy(bad));
        }
        assert_eq!(decode(b"TG1"), Err(DecodeError::Unterminated));
        assert_eq!(decode(b"H\x03"), Ok(vec![Step::Home]));
        assert!(decode(b"H1\x03").is_err());
        assert_eq!(decode(b""), Ok(vec![]));
    }

    #[test]
    fn the_blade_runs_and_their_dashes() {
        let s = decode(&cmds(&["M0,0", "D0,200", "M200,0", "L3", "B40", "D200,200", "L0", "D400,200", "SO0"])).unwrap();
        let runs = blade_paths(&s).unwrap();
        // A 10 mm line; then 10 mm in 1 mm dashes of 2 mm pitch (five); then solid again, 10 mm down.
        assert_eq!(runs.len(), 1 + 5 + 1, "{runs:?}");
        assert_eq!(runs[0], [[0.0, 0.0], [10.0, 0.0]]);
        let first = &runs[1];
        assert!((first[1][0] - 1.0).abs() < 1e-9 && first[0] == [0.0, 10.0], "{first:?}");
        assert_eq!(*runs.last().unwrap(), [[10.0, 10.0], [10.0, 20.0]]);
        // A curve must start where the blade is.
        assert!(blade_paths(&decode(&cmds(&["M0,0", "BZ1,20,20,40,40,60,60,80,80,0"])).unwrap()).is_err());
        // A circle ends where it began.
        let c = blade_paths(&decode(&cmds(&["M300,200", "W200,200,100,100,0,3600"])).unwrap()).unwrap();
        let (s0, e) = (c[0][0], *c[0].last().unwrap());
        assert!((s0[0] - e[0]).abs() < 1e-9 && (s0[1] - e[1]).abs() < 1e-9, "{s0:?} {e:?}");
    }
}
