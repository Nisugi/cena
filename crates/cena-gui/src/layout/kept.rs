//! Where a play window's layout is kept, and reading and writing it: a file
//! per character, by game and name (`plan/50` §6 item 6). Moved out of
//! `layout.rs` when the window lock took it past its cap.

use std::path::{Path, PathBuf};

use super::{Layout, VERSION};

impl Layout {
    /// `character`'s saved layout on `instance` in `dir`: by game and name,
    /// so a character on Prime and one of the same name on Shattered each
    /// keep their own (`plan/50` §6 item 6). One saved under the name alone,
    /// as layouts were kept before, is taken as the first. `None` when there
    /// is none, or it cannot be read, when a fitted one does.
    pub(crate) fn load(dir: &Path, instance: Option<&str>, character: &str) -> Option<Self> {
        let read = |path: PathBuf| {
            let text = std::fs::read_to_string(path).ok()?;
            serde_json::from_str::<Self>(&text)
                .ok()
                .filter(|layout| layout.version == VERSION)
        };
        read(file(dir, instance, character))
            .or_else(|| instance.and_then(|_| read(file(dir, None, character))))
    }

    /// Save this as `character`'s layout on `instance` in `dir`.
    ///
    /// # Errors
    ///
    /// The folder could not be made or the file written.
    pub(crate) fn save(
        &self,
        dir: &Path,
        instance: Option<&str>,
        character: &str,
    ) -> std::io::Result<()> {
        cena_session::store::save_json(dir, &file(dir, instance, character), self)
    }
}

/// `character`'s layout file on `instance`, `prime_nisugi.json`, or under
/// its name alone with no instance: each in lower case, letters and digits
/// only, so the same character is one file on every filesystem (the
/// character store's lesson: one file on NTFS was two on ext4).
pub(super) fn file(dir: &Path, instance: Option<&str>, character: &str) -> PathBuf {
    let clean = |words: &str| -> String {
        words
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect()
    };
    match instance {
        Some(instance) => dir.join(format!("{}_{}.json", clean(instance), clean(character))),
        None => dir.join(format!("{}.json", clean(character))),
    }
}
