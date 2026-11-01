//! Fork: perforating (C14.4). An object set to Perforate is cut in dashes, so the sheet tears
//! along the line: its stroke's own dash pattern when it has one (as the maker's software cuts a
//! dashed line), else the document's cut and gap lengths. The dashes are made from the job's
//! flattened lines, in mm on the mat, and cut as open lines.

/// Most dashes one line is cut in.
const MAX_DASHES: usize = 200_000;

/// The lengths `pattern` (dash, gap, dash, gap…, mm) gives, as dash–gap pairs: an odd pattern is
/// taken twice, as SVG and PDF do; `None` for no usable pattern (nothing positive, or a value
/// that isn't a number).
pub fn pairs(pattern: &[f64]) -> Option<Vec<(f64, f64)>> {
    if pattern.is_empty() || pattern.iter().any(|v| !(v.is_finite() && *v >= 0.0)) || pattern.iter().all(|v| *v <= 0.0) {
        return None;
    }
    let full: Vec<f64> = if pattern.len() % 2 == 1 { pattern.iter().chain(pattern).copied().collect() } else { pattern.to_vec() };
    let out: Vec<(f64, f64)> = full
        .chunks(2)
        .filter_map(|c| match c {
            [d, g] => Some((*d, *g)),
            _ => None,
        })
        .collect();
    // A pattern of gaps only cuts nothing; one with no gaps cuts the line whole.
    out.iter().any(|(d, _)| *d > 0.0).then_some(out)
}

/// `points` (closed: back to the first at the end) cut into dashes as `pattern` says, each an open
/// line; the line whole when the pattern has no gap.
pub fn dashes(points: &[[f64; 2]], closed: bool, pattern: &[(f64, f64)]) -> Vec<Vec<[f64; 2]>> {
    let mut pts: Vec<[f64; 2]> = points.to_vec();
    if closed && let Some(first) = points.first() {
        pts.push(*first);
    }
    if pts.len() < 2 || pattern.iter().all(|(_, g)| *g <= 0.0) {
        return vec![pts];
    }
    let mut out = vec![];
    let mut current: Vec<[f64; 2]> = vec![];
    // Where in the pattern the line is: which pair, whether in its dash, and what is left of it.
    let (mut k, mut down, mut left) = (0usize, true, pattern.first().map_or(0.0, |p| p.0));
    if let Some(p) = pts.first() {
        current.push(*p);
    }
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        let mut at = 0.0;
        while len - at > 1e-12 {
            let step = left.min(len - at);
            at += step;
            left -= step;
            let t = at / len;
            let q = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if down {
                current.push(q);
            }
            if left <= 1e-12 {
                // The end of a dash or a gap: the next part of the pattern.
                if down {
                    if current.len() >= 2 {
                        out.push(std::mem::take(&mut current));
                    }
                    current.clear();
                    if out.len() >= MAX_DASHES {
                        return out;
                    }
                    left = pattern.get(k).map_or(0.0, |p| p.1);
                    down = false;
                } else {
                    k = (k + 1) % pattern.len();
                    left = pattern.get(k).map_or(0.0, |p| p.0);
                    down = true;
                    current.push(q);
                }
            }
        }
    }
    if current.len() >= 2 {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn length(line: &[[f64; 2]]) -> f64 {
        line.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum()
    }

    /// A 20 mm line, 3 mm cuts with 1 mm gaps: five dashes, the last 3 mm too (20 = 5 × 4).
    #[test]
    fn a_line_is_cut_in_dashes() {
        let d = dashes(&[[0.0, 0.0], [20.0, 0.0]], false, &[(3.0, 1.0)]);
        assert_eq!(d.len(), 5, "{d:?}");
        assert!(d.iter().all(|l| (length(l) - 3.0).abs() < 1e-9), "{d:?}");
        assert!((d[1][0][0] - 4.0).abs() < 1e-9, "a gap before the second");
    }

    /// Round a 10 mm square's corners: the dashes go on round them, 40 mm in 10 dashes.
    #[test]
    fn dashes_go_round_corners_and_close() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let d = dashes(&sq, true, &[(3.0, 1.0)]);
        assert_eq!(d.len(), 10);
        let cut: f64 = d.iter().map(|l| length(l)).sum();
        assert!((cut - 30.0).abs() < 1e-9, "{cut}");
        assert!(d.iter().any(|l| l.len() == 3), "one turns a corner");
    }

    #[test]
    fn patterns_are_read_as_svg_does() {
        assert_eq!(pairs(&[2.0]), Some(vec![(2.0, 2.0)]), "odd: taken twice");
        assert_eq!(pairs(&[3.0, 1.0, 1.0]), Some(vec![(3.0, 1.0), (1.0, 3.0), (1.0, 1.0)]));
        assert_eq!(pairs(&[]), None);
        assert_eq!(pairs(&[0.0, 0.0]), None);
        assert_eq!(pairs(&[f64::NAN, 1.0]), None);
        assert_eq!(pairs(&[0.0, 2.0]), None, "gaps only");
        assert_eq!(dashes(&[[0.0, 0.0], [5.0, 0.0]], false, &[(3.0, 0.0)]).len(), 1, "no gap: whole");
    }
}
