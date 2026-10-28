//! Cutting mats: the physical mat and the area on it the blade may reach.

/// An axis-aligned rectangle in millimetres, mat coordinates (origin top-left, +y toward the user).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectMm {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl RectMm {
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
}

/// A mat a job is laid out on.
#[derive(Clone, Debug, PartialEq)]
pub struct MatGeometry {
    /// Stable id, as [`crate::CuttingSettings::mat`] names it (`"custom"` for a custom size).
    pub id: &'static str,
    /// English label for menus (the UI translates it).
    pub label: &'static str,
    /// The mat's gridded area, width × height (height along the feed direction). The plastic
    /// beyond the grid isn't modelled: no published figure gives it.
    pub size_mm: [f64; 2],
    /// Where the blade may go, in mat coordinates.
    pub cut_area_mm: RectMm,
    /// The cut area isn't from the maker's published specifications (ARCHITECTURE.md › R9), and
    /// the UI says so.
    pub approximate: bool,
}

impl MatGeometry {
    /// The built-in mat `id`.
    pub fn preset(id: &str) -> Option<&'static MatGeometry> {
        MAT_PRESETS.iter().find(|m| m.id == id)
    }

    /// A custom mat of `size_mm`, cut all over.
    pub fn custom(size_mm: [f64; 2]) -> MatGeometry {
        MatGeometry {
            id: CUSTOM,
            label: "Custom",
            size_mm,
            cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: size_mm[0], y1: size_mm[1] },
            approximate: false,
        }
    }
}

/// [`MatGeometry::id`] of a custom mat.
pub(crate) const CUSTOM: &str = "custom";

/// 12 and 24 inches in millimetres, exactly (written out: `12.0 * 25.4` is 304.79999999999995).
const IN_12: f64 = 304.8;
const IN_24: f64 = 609.6;

/// The built-in mats, the default first.
///
/// Sources (checked 2026-10-28): Silhouette's Cameo 4 specifications give a guaranteed operating
/// area "With a Cutting Mat" of 12 in × 24 in (30.5 cm × 61.0 cm), and list the 12-inch Cameo mat
/// in the box (<https://www.silhouetteamerica.com/cameo-4>). That 12 in width over the whole
/// 12 × 12 in mat is from retailers' listings (e.g. <https://www.swingdesign.com/products/silhouette-12-cutting-mat>)
/// and a university fab lab's sheet ("12" x 12" with a cutting mat",
/// <https://cucfablab.web.illinois.edu/lab-tool/silhouette-cameo-3/>), not from Silhouette itself.
pub static MAT_PRESETS: [MatGeometry; 2] = [
    MatGeometry {
        id: "silhouette.12x12",
        label: "Silhouette 12 × 12 in",
        size_mm: [IN_12, IN_12],
        cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: IN_12, y1: IN_12 },
        approximate: true,
    },
    MatGeometry {
        id: "silhouette.12x24",
        label: "Silhouette 12 × 24 in",
        size_mm: [IN_12, IN_24],
        cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: IN_12, y1: IN_24 },
        approximate: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_unique_ids_and_areas_on_the_mat() {
        for (i, m) in MAT_PRESETS.iter().enumerate() {
            assert!(MAT_PRESETS.iter().skip(i + 1).all(|o| o.id != m.id), "{}", m.id);
            assert_ne!(m.id, CUSTOM);
            let a = m.cut_area_mm;
            assert!(a.x0 >= 0.0 && a.y0 >= 0.0 && a.x1 <= m.size_mm[0] && a.y1 <= m.size_mm[1], "{}", m.id);
            assert!(a.width() > 0.0 && a.height() > 0.0, "{}", m.id);
            assert_eq!(MatGeometry::preset(m.id), Some(m));
        }
        assert_eq!(MatGeometry::preset("nope"), None);
    }

    #[test]
    fn a_custom_mat_is_cut_all_over() {
        let m = MatGeometry::custom([200.0, 300.0]);
        assert_eq!(m.cut_area_mm, RectMm { x0: 0.0, y0: 0.0, x1: 200.0, y1: 300.0 });
    }
}
