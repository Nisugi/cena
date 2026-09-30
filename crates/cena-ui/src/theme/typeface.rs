//! A theme's type (`plan/57` §3, step 5): the UI's font and the story's,
//! and their sizes. A font is named by a file in the `fonts` folder of the
//! data folder (its stem: `Iosevka-Regular.ttf` is `Iosevka-Regular`), which
//! the GUI loads once for every theme (§6 item 3, Claude's recommendation
//! taken: a folder first, the system's fonts when someone asks). A font not
//! there, or none named, is egui's own.
//!
//! Fonts are one set for the whole GUI, so a character's own theme brings
//! its sizes to its window and not its fonts.

use serde::{Deserialize, Serialize};

/// A theme's type, whole.
#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    /// The UI's font, by its file's stem; `None`, egui's.
    pub ui_font: Option<String>,
    /// The story's font, by its file's stem; `None`, the UI's.
    pub story_font: Option<String>,
    /// The UI's text size, in points.
    pub ui_size: f32,
    /// The story's text size, in points.
    pub story_size: f32,
}

impl Default for Type {
    /// egui's own fonts at its own sizes.
    fn default() -> Self {
        Self {
            ui_font: None,
            story_font: None,
            ui_size: 13.0,
            story_size: 13.0,
        }
    }
}

/// The type as a file writes it: a part left out is the base's.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeFile {
    /// The UI's font's file stem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_font: Option<String>,
    /// The story's font's file stem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story_font: Option<String>,
    /// The UI's text size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_size: Option<f32>,
    /// The story's text size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story_size: Option<f32>,
}

impl TypeFile {
    /// Whether it sets nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// `kind` with what this file sets laid over it; an empty font name is
    /// egui's own, and a size is kept between 6 and 48.
    #[must_use]
    pub fn over(&self, kind: Type) -> Type {
        let font = |set: &Option<String>, base: Option<String>| match set {
            Some(name) if name.trim().is_empty() => None,
            Some(name) => Some(name.trim().to_owned()),
            None => base,
        };
        Type {
            ui_font: font(&self.ui_font, kind.ui_font),
            story_font: font(&self.story_font, kind.story_font),
            ui_size: self.ui_size.unwrap_or(kind.ui_size).clamp(6.0, 48.0),
            story_size: self.story_size.unwrap_or(kind.story_size).clamp(6.0, 48.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_sets_what_it_says_within_bounds_and_an_empty_font_is_eguis() {
        let file: TypeFile = toml::from_str("ui_font = \"Iosevka\"\nstory_size = 100\n").unwrap();
        let kind = file.over(Type {
            story_font: Some("Hack".to_owned()),
            ..Type::default()
        });
        assert_eq!(kind.ui_font.as_deref(), Some("Iosevka"));
        assert_eq!(kind.story_font.as_deref(), Some("Hack"), "the base's");
        assert!((kind.story_size - 48.0).abs() < f32::EPSILON, "at most");
        assert!((kind.ui_size - 13.0).abs() < f32::EPSILON, "the base's");
        let cleared: TypeFile = toml::from_str("story_font = \"\"\n").unwrap();
        assert_eq!(cleared.over(kind).story_font, None);
        assert!(toml::from_str::<TypeFile>("weight = 700\n").is_err());
        assert!(TypeFile::default().is_empty());
    }
}
