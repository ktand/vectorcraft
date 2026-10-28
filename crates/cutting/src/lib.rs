//! The cutting workspace's core (fork: `docs/cutting/ARCHITECTURE.md`): what a document says
//! about cutting, read from and written to the slots VectorCraft already keeps (§4), so the
//! document model and the native format are unchanged.
//!
//! - [`ops`]: each object's operation, a foreign attribute (`cut:op`), inherited from its
//!   ancestors and otherwise the document's default.
//! - [`settings`] and [`set_settings`]: the document's [`CuttingSettings`], under
//!   `Document::unknown["cutting"]`.
//! - [`placement`]: where the settings' artboard lies on the mat, document points to mat mm.
//! - [`select`]: which objects a job cuts, and with which operation.
//! - [`geometry`]: the outlines each of them cuts along, as drawn.
//! - [`prepare`]: those outlines on the mat, in millimetres, flattened.
#![forbid(unsafe_code)]

pub mod geometry;
pub mod ops;
pub mod placement;
pub mod prepare;
pub mod select;

use vectorcraft_cutjob::{CuttingSettings, DOCUMENT_KEY};
use vectorcraft_doc::Document;

/// The document's cutting settings (the defaults when it has none, read leniently when damaged).
pub fn settings(d: &Document) -> CuttingSettings {
    CuttingSettings::from_value(d.unknown.get(DOCUMENT_KEY))
}

/// Keeps `s` as the document's cutting settings.
pub fn set_settings(d: &mut Document, s: &CuttingSettings) {
    d.unknown.insert(DOCUMENT_KEY.to_string(), s.to_value());
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_cutjob::Orientation;

    use super::*;

    #[test]
    fn settings_live_in_the_documents_foreign_data() {
        let mut d = Document::new(100.0, 100.0);
        assert_eq!(settings(&d), CuttingSettings::default());
        let s = CuttingSettings { orientation: Orientation::Landscape, ..CuttingSettings::default() };
        set_settings(&mut d, &s);
        assert_eq!(settings(&d), s);
        assert_eq!(d.unknown[DOCUMENT_KEY]["orientation"], json!("landscape"));
        d.unknown.insert(DOCUMENT_KEY.into(), json!("damaged"));
        assert_eq!(settings(&d), CuttingSettings::default());
    }
}
