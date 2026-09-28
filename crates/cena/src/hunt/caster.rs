//! The spellcaster profile, held so a typed line is judged without reading
//! the file, and read again when the file has changed: by `;sc`, the settings
//! menu, or a hand (`plan/50` §2 item 3, §7 step 1).

use std::path::Path;

use cena_behavior::spellcaster::{self, CasterProfile};

/// The spellcaster profile as last read, with its file's time then.
pub(super) struct Caster {
    profile: CasterProfile,
    read: Option<std::time::SystemTime>,
}

impl Caster {
    /// The profile as the file holds it now.
    pub(super) fn read(dir: &Path, who: Option<&(String, String)>) -> Self {
        Self {
            read: modified(dir, who),
            profile: read_caster(dir, who),
        }
    }

    /// The profile, read again first if its file has changed since.
    pub(super) fn current(&mut self, dir: &Path, who: Option<&(String, String)>) -> &CasterProfile {
        let now = modified(dir, who);
        if now != self.read {
            *self = Self::read(dir, who);
        }
        &self.profile
    }
}

/// When the character's spellcaster file last changed; `None` with no file.
fn modified(dir: &Path, who: Option<&(String, String)>) -> Option<std::time::SystemTime> {
    who.and_then(|(i, n)| spellcaster::path(dir, i, n))
        .and_then(|path| std::fs::metadata(path).ok())
        .and_then(|meta| meta.modified().ok())
}

/// The character's spellcaster profile, or the default when there is no
/// file or it does not read.
fn read_caster(dir: &Path, who: Option<&(String, String)>) -> CasterProfile {
    who.and_then(|(i, n)| spellcaster::path(dir, i, n))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| CasterProfile::parse(&text).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A change to the file, by whatever writer, is read at the next typed
    /// line; an unchanged file is not read again.
    #[test]
    fn a_changed_file_is_read_again() {
        let dir = std::env::temp_dir().join(format!("cena-caster-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let who = ("Prime".to_owned(), "Nisugi".to_owned());
        let mut held = Caster::read(&dir, Some(&who));
        assert!(
            held.current(&dir, Some(&who)).typed,
            "the default, with no file"
        );

        let path = spellcaster::path(&dir, "Prime", "Nisugi").expect("a file name");
        let written = |text: &str, at: std::time::SystemTime| {
            let parent = path.parent().expect("a folder");
            std::fs::create_dir_all(parent).expect("made");
            std::fs::write(&path, text).expect("written");
            std::fs::File::options()
                .write(true)
                .open(&path)
                .and_then(|file| file.set_modified(at))
                .expect("dated");
        };
        let then = std::time::SystemTime::now();
        written("typed = false\n", then);
        assert!(!held.current(&dir, Some(&who)).typed, "read again");

        // Changed behind its back but with the same time: not read again.
        written("typed = true\n", then);
        assert!(!held.current(&dir, Some(&who)).typed, "kept");
        written("typed = true\n", then + std::time::Duration::from_secs(1));
        assert!(held.current(&dir, Some(&who)).typed, "read again");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
