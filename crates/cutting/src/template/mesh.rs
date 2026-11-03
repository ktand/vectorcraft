//! Fork: reading a 3D model for papercraft (C15.6): Wavefront OBJ (text) and STL (text or binary),
//! read from their public descriptions. Only the corners and the faces are read; normals,
//! textures and materials are ignored. Files are untrusted: sizes and counts are capped, numbers
//! must be finite, and anything malformed is an error, never a panic.

use super::unfold::{Solid, V3};

/// At most this many corners and faces are read.
pub(super) const MAX_VERTS: usize = 200_000;
pub(super) const MAX_TRIANGLES: usize = 200_000;
/// The largest file read, bytes.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;

/// A model's corners and its faces as triangles (a polygon fanned from its first corner).
pub(super) fn read(name: &str, bytes: &[u8]) -> Result<Solid, String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("the model is larger than {} MB", MAX_BYTES / (1024 * 1024)));
    }
    let lower = name.to_ascii_lowercase();
    let solid = if lower.ends_with(".obj") {
        obj(bytes)?
    } else if lower.ends_with(".stl") {
        stl(bytes)?
    } else {
        return Err("a 3D model is read from an .obj or an .stl file".to_string());
    };
    if solid.faces.is_empty() {
        return Err("the model has no faces".to_string());
    }
    Ok(solid)
}

fn finite(v: V3) -> Result<V3, String> {
    if v.iter().all(|c| c.is_finite() && c.abs() < 1e12) { Ok(v) } else { Err("the model has a corner that isn't a finite number".to_string()) }
}

/// OBJ: `v x y z` corners, `f a b c …` faces (1-based, or negative from the end; `a/t/n` forms
/// read for the corner only).
fn obj(bytes: &[u8]) -> Result<Solid, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "the OBJ file isn't text".to_string())?;
    let mut verts: Vec<V3> = vec![];
    let mut faces: Vec<Vec<usize>> = vec![];
    for (n, line) in text.lines().enumerate() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                if verts.len() >= MAX_VERTS {
                    return Err(format!("the model has more than {MAX_VERTS} corners"));
                }
                let mut c = [0.0; 3];
                for slot in &mut c {
                    *slot = it.next().and_then(|s| s.parse::<f64>().ok()).ok_or_else(|| format!("line {}: a corner needs x, y and z", n + 1))?;
                }
                verts.push(finite(c)?);
            }
            Some("f") => {
                let mut f = vec![];
                for tok in it {
                    let first = tok.split('/').next().unwrap_or_default();
                    let i: i64 = first.parse().map_err(|_| format!("line {}: a face's corner isn't a number", n + 1))?;
                    let idx = if i > 0 {
                        usize::try_from(i - 1).ok()
                    } else if i < 0 {
                        verts.len().checked_sub(usize::try_from(i.unsigned_abs()).unwrap_or(usize::MAX))
                    } else {
                        None
                    };
                    let idx = idx.filter(|&k| k < verts.len()).ok_or_else(|| format!("line {}: a face names a corner that isn't there", n + 1))?;
                    if f.len() >= 64 {
                        return Err(format!("line {}: a face has more than 64 corners", n + 1));
                    }
                    f.push(idx);
                }
                if f.len() < 3 {
                    return Err(format!("line {}: a face needs three corners", n + 1));
                }
                for k in 1..f.len() - 1 {
                    if faces.len() >= MAX_TRIANGLES {
                        return Err(format!("the model has more than {MAX_TRIANGLES} triangles"));
                    }
                    faces.push(vec![f[0], f[k], f[k + 1]]);
                }
            }
            _ => {}
        }
    }
    Ok(Solid { verts, faces })
}

/// STL: binary (an 80-byte header, a count, 50 bytes a triangle) unless it starts with `solid`
/// and reads as text (`vertex x y z` lines, three to a facet). Corners at the same place are
/// joined, so the faces share their edges.
fn stl(bytes: &[u8]) -> Result<Solid, String> {
    let mut tris: Vec<[V3; 3]> = vec![];
    let text = bytes.starts_with(b"solid").then(|| std::str::from_utf8(bytes).ok()).flatten();
    match text.filter(|t| t.contains("vertex")) {
        Some(t) => {
            let mut corner: Vec<V3> = vec![];
            for line in t.lines() {
                let mut it = line.split_whitespace();
                if it.next() == Some("vertex") {
                    let mut c = [0.0; 3];
                    for slot in &mut c {
                        *slot = it.next().and_then(|s| s.parse::<f64>().ok()).ok_or("a vertex needs x, y and z")?;
                    }
                    corner.push(finite(c)?);
                    if let [a, b, c] = corner.as_slice() {
                        if tris.len() >= MAX_TRIANGLES {
                            return Err(format!("the model has more than {MAX_TRIANGLES} triangles"));
                        }
                        tris.push([*a, *b, *c]);
                        corner.clear();
                    }
                }
            }
        }
        None => {
            let count = bytes.get(80..84).ok_or("the STL file is too short")?;
            let n = u32::from_le_bytes([count[0], count[1], count[2], count[3]]) as usize;
            if n > MAX_TRIANGLES {
                return Err(format!("the model has more than {MAX_TRIANGLES} triangles"));
            }
            let f = |b: &[u8], at: usize| -> Option<f64> {
                let s = b.get(at..at + 4)?;
                Some(f64::from(f32::from_le_bytes([s[0], s[1], s[2], s[3]])))
            };
            for i in 0..n {
                let base = 84 + i * 50;
                let rec = bytes.get(base..base + 50).ok_or("the STL file ends before its last triangle")?;
                let mut t = [[0.0; 3]; 3];
                for (k, c) in t.iter_mut().enumerate() {
                    for (j, slot) in c.iter_mut().enumerate() {
                        *slot = f(rec, 12 + k * 12 + j * 4).ok_or("the STL file ends early")?;
                    }
                    *c = finite(*c)?;
                }
                tris.push(t);
            }
        }
    }
    // Join corners at the same place (to a millionth of the model's size).
    let span = tris.iter().flatten().flatten().fold(0.0f64, |m, c| m.max(c.abs())).max(1e-9);
    let q = |c: f64| (c / span * 1e6).round() as i64;
    let mut index: std::collections::HashMap<[i64; 3], usize> = std::collections::HashMap::new();
    let mut verts: Vec<V3> = vec![];
    let mut faces = vec![];
    for t in &tris {
        let mut f = Vec::with_capacity(3);
        for c in t {
            let key = [q(c[0]), q(c[1]), q(c[2])];
            let i = *index.entry(key).or_insert_with(|| {
                verts.push(*c);
                verts.len() - 1
            });
            f.push(i);
        }
        // A triangle with two corners in one place has no area: dropped.
        if f[0] != f[1] && f[1] != f[2] && f[0] != f[2] {
            faces.push(f);
        }
    }
    Ok(Solid { verts, faces })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE_OBJ: &str = "# a cube\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv 0 0 1\nv 1 0 1\nv 1 1 1\nv 0 1 1\n\
        f 1 4 3 2\nf 5 6 7 8\nf 1 2 6 5\nf 2 3 7 6\nf 3 4 8 7\nf 4 1 5 8\n";

    #[test]
    fn an_obj_cube_is_read_as_triangles() {
        let s = read("cube.OBJ", CUBE_OBJ.as_bytes()).unwrap();
        assert_eq!((s.verts.len(), s.faces.len()), (8, 12));
        let neg = read("t.obj", b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3/1/1 -2//2 -1\n").unwrap();
        assert_eq!(neg.faces, vec![vec![0, 1, 2]]);
    }

    #[test]
    fn stl_text_and_binary_are_read_and_joined() {
        let text = "solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\n\
            facet normal 0 0 1\nouter loop\nvertex 1 0 0\nvertex 1 1 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid t\n";
        let s = read("t.stl", text.as_bytes()).unwrap();
        assert_eq!((s.verts.len(), s.faces.len()), (4, 2), "the shared corners joined");
        let mut bin = vec![0u8; 80];
        bin.extend(1u32.to_le_bytes());
        bin.extend([0u8; 12]);
        for c in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for x in c {
                bin.extend(x.to_le_bytes());
            }
        }
        bin.extend([0u8; 2]);
        let s = read("b.stl", &bin).unwrap();
        assert_eq!((s.verts.len(), s.faces.len()), (3, 1));
    }

    #[test]
    fn malformed_models_are_refused() {
        for (name, bytes) in [
            ("a.obj", &b"v 0 0\n"[..]),
            ("a.obj", b"v 0 0 0\nf 1 2 3\n"),
            ("a.obj", b"v 0 0 nan\n"),
            ("a.obj", b"v 0 0 1e400\n"),
            ("a.obj", b"f 0 0 0\n"),
            ("a.obj", b"\xff\xfe"),
            ("a.stl", b"short"),
            ("a.stl", &[0u8; 84]),
            ("a.ply", b"ply"),
        ] {
            assert!(read(name, bytes).is_err(), "{name} {bytes:?}");
        }
        // A count larger than the file.
        let mut bin = vec![0u8; 80];
        bin.extend(1000u32.to_le_bytes());
        assert!(read("big.stl", &bin).is_err());
    }

    proptest::proptest! {
        /// Any bytes read as either kind: an answer or an error, never a panic.
        #[test]
        fn any_bytes_are_safe(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..400), text in "[vf0-9 ./\\-\\n]{0,300}") {
            let _ = read("x.stl", &bytes);
            let _ = read("x.obj", &bytes);
            let _ = read("x.obj", text.as_bytes());
            let mut stl = b"solid x\n".to_vec();
            stl.extend(text.replace('f', "vertex").as_bytes());
            let _ = read("x.stl", &stl);
        }
    }
}
