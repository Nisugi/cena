//! A behavior's profile as a file: read, written, and where it is kept.
//!
//! Six profiles read and wrote themselves with the same two lines each,
//! and five named their file by the same two (the review of 2026-09-29;
//! `plan/05` §-1, the rule of three). Each keeps its own `parse`, `to_toml`
//! and `path`, which its callers know it by, and each is this.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// A profile file's text, read: not TOML, or a key the profile does not
/// know, is why not.
pub(crate) fn read<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

/// A profile as its file's text.
pub(crate) fn written<T: Serialize>(profile: &T) -> Result<String, String> {
    toml::to_string_pretty(profile).map_err(|e| e.to_string())
}

/// A character's profile of `kind`:
/// `<data>/hunt/<kind>/<instance>_<character>.toml`, beside the hunt
/// chain's files. `None` when either name cannot be part of a file name.
///
/// **Each name by itself.** The five copies this replaced cleaned the two
/// joined, so the `_` between them was always something left: a character
/// not yet named, on a game not yet stated, was kept in `_.toml`, every
/// such character in the one file, where each copy's doc said `None`.
pub(crate) fn path(dir: &Path, kind: &str, instance: &str, character: &str) -> Option<PathBuf> {
    let instance = crate::hunt::chain::file_name(instance)?;
    let character = crate::hunt::chain::file_name(character)?;
    Some(
        dir.join("hunt")
            .join(kind)
            .join(format!("{instance}_{character}.toml")),
    )
}

#[cfg(test)]
mod tests {
    use super::path;
    use std::path::Path;

    /// Every kind is kept the one way: the folder is the kind's, the file
    /// the character's on its game.
    #[test]
    fn a_profile_is_kept_by_kind_then_game_and_name() {
        let kept = path(Path::new("data"), "waggle", "GSIV", "Ashryn");
        let name = kept
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase);
        assert_eq!(name.as_deref(), Some("gsiv_ashryn.toml"));
        assert!(kept.is_some_and(
            |kept| kept.parent() == Some(&Path::new("data").join("hunt").join("waggle"))
        ),);
        assert_eq!(
            path(Path::new("data"), "sc", "", ""),
            None,
            "no name, no file"
        );
        assert_eq!(path(Path::new("data"), "sc", "GSIV", " "), None);
        assert_eq!(path(Path::new("data"), "sc", "", "Ashryn"), None);
    }
}
