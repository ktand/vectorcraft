//! Lenient readers for untrusted JSON: a value that is missing, of the wrong type or not finite
//! gives the default; a finite number out of range is clamped into it.

use serde_json::{Map, Value};

/// `m[key]` as a number in `min..=max`.
pub(crate) fn num(m: &Map<String, Value>, key: &str, min: f64, max: f64, default: f64) -> f64 {
    match m.get(key).and_then(Value::as_f64) {
        Some(v) if v.is_finite() => v.clamp(min, max),
        _ => default,
    }
}

/// `m[key]` as a whole number in `min..=max` (a fractional number is rounded).
pub(crate) fn whole(m: &Map<String, Value>, key: &str, min: u32, max: u32, default: u32) -> u32 {
    let v = num(m, key, f64::from(min), f64::from(max), f64::from(default)).round();
    // In range and whole after the clamp and the rounding, so the conversion is exact.
    u32::try_from(v as i64).unwrap_or(default)
}

/// `m[key]` as a pair of numbers, each in `min..=max`; anything but an array of two numbers is
/// the default.
pub(crate) fn pair(m: &Map<String, Value>, key: &str, min: f64, max: f64, default: [f64; 2]) -> [f64; 2] {
    let Some([a, b]) = m.get(key).and_then(Value::as_array).map(Vec::as_slice) else { return default };
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) if a.is_finite() && b.is_finite() => [a.clamp(min, max), b.clamp(min, max)],
        _ => default,
    }
}

/// `m[key]` as one of `choices` by its id.
pub(crate) fn choice<T: Copy>(m: &Map<String, Value>, key: &str, choices: &[(&str, T)], default: T) -> T {
    let Some(s) = m.get(key).and_then(Value::as_str) else { return default };
    choices.iter().find(|(id, _)| *id == s).map_or(default, |(_, v)| *v)
}

/// `m[key]` as an object, if it is one.
pub(crate) fn object<'a>(m: &'a Map<String, Value>, key: &str) -> Option<&'a Map<String, Value>> {
    m.get(key).and_then(Value::as_object)
}

/// The entries of `m` whose keys aren't `known`: what a newer version wrote, kept for it.
pub(crate) fn unknown(m: &Map<String, Value>, known: &[&str]) -> Map<String, Value> {
    m.iter().filter(|(k, _)| !known.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn numbers_are_clamped_and_damage_is_the_default() {
        let m = map(json!({"a": 5.0, "b": -1, "c": "x", "d": 1e300, "e": null}));
        assert_eq!(num(&m, "a", 0.0, 1.0, 0.5), 1.0);
        assert_eq!(num(&m, "b", 0.0, 1.0, 0.5), 0.0);
        assert_eq!(num(&m, "c", 0.0, 1.0, 0.5), 0.5);
        assert_eq!(num(&m, "d", 0.0, 1.0, 0.5), 1.0);
        assert_eq!(num(&m, "e", 0.0, 1.0, 0.5), 0.5);
        assert_eq!(num(&m, "missing", 0.0, 1.0, 0.5), 0.5);
    }

    #[test]
    fn whole_numbers_round_and_clamp() {
        let m = map(json!({"a": 2.6, "b": -3, "c": 1e20, "d": true}));
        assert_eq!(whole(&m, "a", 1, 10, 1), 3);
        assert_eq!(whole(&m, "b", 1, 10, 1), 1);
        assert_eq!(whole(&m, "c", 1, 10, 1), 10);
        assert_eq!(whole(&m, "d", 1, 10, 4), 4);
    }

    #[test]
    fn pairs_need_two_numbers() {
        let m = map(json!({"a": [1, 2], "b": [1], "c": [1, "x"], "d": [1, 2, 3], "e": [-9, 9]}));
        let d = [7.0, 8.0];
        assert_eq!(pair(&m, "a", -5.0, 5.0, d), [1.0, 2.0]);
        assert_eq!(pair(&m, "b", -5.0, 5.0, d), d);
        assert_eq!(pair(&m, "c", -5.0, 5.0, d), d);
        assert_eq!(pair(&m, "d", -5.0, 5.0, d), d);
        assert_eq!(pair(&m, "e", -5.0, 5.0, d), [-5.0, 5.0]);
    }

    #[test]
    fn choices_and_unknown_keys() {
        let m = map(json!({"a": "two", "b": "three", "c": 2}));
        let choices = [("one", 1), ("two", 2)];
        assert_eq!(choice(&m, "a", &choices, 1), 2);
        assert_eq!(choice(&m, "b", &choices, 1), 1);
        assert_eq!(choice(&m, "c", &choices, 1), 1);
        assert_eq!(unknown(&m, &["a"]), map(json!({"b": "three", "c": 2})));
    }
}
