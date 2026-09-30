//! A character's own theme and accent, read from its settings file
//! (`plan/57` §3c, step 3): the `theme` section the binary's *Theme* page
//! writes ([`Chosen`]), which its play window wears in place of Hydra's.
//!
//! The file is the binary's to write; this reads it, once a second at most
//! by its modified time, and keeps the palette worked out from it until the
//! file or Hydra's own theme changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use cena_session::settings_store;
use cena_ui::theme::{Chosen, Outfit, Themes};

use super::App;

/// How often a character's file is looked at.
const EVERY: Duration = Duration::from_secs(1);

/// One character's look, as its file last said.
#[derive(Debug)]
pub(super) struct Looked {
    /// When the file was last looked at; `None`, never, or not since Hydra's
    /// theme changed.
    checked: Option<Instant>,
    /// The file's modified time when last read; `None`, no file.
    modified: Option<SystemTime>,
    /// What it chose.
    chosen: Chosen,
    /// The outfit its window wears, worked out from what it chose over
    /// Hydra's own theme; `None` when it chose nothing, so the window wears
    /// Hydra's as it is.
    outfit: Option<Outfit>,
}

/// The characters' looks, by settings file.
#[derive(Debug, Default)]
pub(super) struct Looks {
    looked: HashMap<PathBuf, Looked>,
}

impl Looks {
    /// Forget every outfit worked out, so each is worked out again over
    /// the theme Hydra wears now.
    pub(super) fn wear_again(&mut self) {
        for looked in self.looked.values_mut() {
            looked.outfit = None;
            looked.checked = None;
            looked.modified = None;
        }
    }

    /// The outfit the character whose settings file is `file` wears, read
    /// from the file when it has changed; `None`, Hydra's own.
    pub(super) fn outfit(&mut self, file: &Path, themes: &Themes, hydras: &str) -> Option<Outfit> {
        let now = Instant::now();
        let looked = self
            .looked
            .entry(file.to_owned())
            .or_insert_with(|| Looked {
                checked: None,
                modified: None,
                chosen: Chosen::default(),
                outfit: None,
            });
        let due = looked
            .checked
            .is_none_or(|checked| now.duration_since(checked) >= EVERY);
        if due {
            looked.checked = Some(now);
            let modified = std::fs::metadata(file).and_then(|m| m.modified()).ok();
            if modified != looked.modified || looked.outfit.is_none() {
                looked.modified = modified;
                looked.chosen = read(file);
                looked.outfit = if looked.chosen == Chosen::default() {
                    None
                } else {
                    themes.outfit_for(hydras, &looked.chosen).ok()
                };
            }
        }
        looked.outfit
    }
}

/// The `theme` section of the settings file at `file`; nothing chosen where
/// there is no file or it does not read.
fn read(file: &Path) -> Chosen {
    let Some(stem) = file.file_name().and_then(|n| n.to_str()) else {
        return Chosen::default();
    };
    let Some(dir) = file.parent() else {
        return Chosen::default();
    };
    let Some((instance, rest)) = stem.split_once('_') else {
        return Chosen::default();
    };
    let name = rest.trim_end_matches(".settings.json");
    settings_store::load(dir, instance, name)
        .ok()
        .and_then(|file| file.section::<Chosen>(Chosen::SECTION).ok())
        .unwrap_or_default()
}

impl App {
    /// The settings file of the character `name` of the game `game`, in the
    /// data folder; `None` where nothing is kept, or the game is unknown.
    pub(super) fn settings_path(&self, game: &str, name: &str) -> Option<PathBuf> {
        let data = self.keys_file.as_deref()?.parent()?;
        let instance = cena_session::instance(game)?;
        settings_store::settings_path(data, instance, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_ui::theme::Token;

    #[test]
    fn a_characters_own_theme_and_accent_are_read_and_worn() {
        let dir = std::env::temp_dir().join(format!("cena-looks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("made");
        let themes = Themes::built_in();
        let file = settings_store::settings_path(&dir, "prime", "Ashryn").expect("a path");
        let mut looks = Looks::default();
        assert_eq!(
            looks.outfit(&file, &themes, "Despana"),
            None,
            "no file: Hydra's"
        );

        let mut settings = settings_store::SettingsFile::new("prime", "Ashryn");
        settings
            .set_section(
                Chosen::SECTION,
                &Chosen {
                    theme: Some("Light".to_owned()),
                    accent: Some("#c9733a".to_owned()),
                },
            )
            .expect("set");
        settings_store::save(&dir, &settings).expect("saved");
        looks.wear_again();
        let palette = looks
            .outfit(&file, &themes, "Despana")
            .expect("its own")
            .palette;
        assert_eq!(
            palette.get(Token::Accent),
            [0xc9, 0x73, 0x3a],
            "the accent pinned"
        );
        assert_eq!(
            palette.get(Token::Canvas),
            themes
                .outfit("Light")
                .expect("light")
                .palette
                .get(Token::Canvas),
            "the Light theme's"
        );
        // An accent alone is pinned over Hydra's own theme.
        settings
            .set_section(
                Chosen::SECTION,
                &Chosen {
                    theme: None,
                    accent: Some("#c9733a".to_owned()),
                },
            )
            .expect("set");
        settings_store::save(&dir, &settings).expect("saved");
        looks.wear_again();
        let palette = looks
            .outfit(&file, &themes, "Despana")
            .expect("its own")
            .palette;
        assert_eq!(palette.get(Token::Accent), [0xc9, 0x73, 0x3a]);
        assert_eq!(palette.get(Token::Canvas), [0x0d, 0x11, 0x15], "Despana's");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
