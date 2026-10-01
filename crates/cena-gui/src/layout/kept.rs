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

    /// Save this as `character`'s layout on `instance` in `dir`. **A file
    /// already there that [`Layout::load`] could not read is kept**, beside
    /// it as `.unread`: a hand edit that broke it, or a later build's
    /// layout, was fitted over and then written over by the first window
    /// dragged (the review of 2026-09-29). The keys' writer refuses such a
    /// file; a layout is saved on every drag, so this one steps round it.
    ///
    /// # Errors
    ///
    /// The folder could not be made or the file written, or an unread file
    /// could not be kept: one kept earlier is still there.
    pub(crate) fn save(
        &self,
        dir: &Path,
        instance: Option<&str>,
        character: &str,
    ) -> std::io::Result<()> {
        let path = file(dir, instance, character);
        keep_unread(&path, |text| {
            serde_json::from_str::<Layout>(text).is_ok_and(|kept| kept.version == VERSION)
        })?;
        cena_session::store::save_json(dir, &path, self)
    }
}

/// The file at `path` moved beside itself when it is there and does not
/// read as this build's: the layout's, and the preset library's (the crate
/// review of 2026-10-01, GU-B-1).
pub(super) fn keep_unread(path: &Path, reads: impl Fn(&str) -> bool) -> std::io::Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(why) => return Err(why),
    };
    if reads(&text) {
        return Ok(());
    }
    let aside = unread(path);
    if aside.exists() {
        return Err(std::io::Error::other(format!(
            "{} cannot be read, and neither could the one kept as {}: move one away",
            path.display(),
            aside.display()
        )));
    }
    std::fs::rename(path, aside)
}

/// Where a layout that could not be read is kept: `prime_nisugi.json.unread`.
pub(crate) fn unread(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".unread");
    PathBuf::from(name)
}

/// `character`'s layout file on `instance`, `prime_nisugi.json`, or under
/// its name alone with no instance: each in lower case, letters and digits
/// only, so the same character is one file on every filesystem (the
/// character store's lesson: one file on NTFS was two on ext4).
pub(crate) fn file(dir: &Path, instance: Option<&str>, character: &str) -> PathBuf {
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
