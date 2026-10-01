//! Creatures the game will not let the hunt target, **remembered by name
//! across hunts**, as bigshot's `CharSettings['untargetable']` is
//! (`bigshot.lic:3287`, `:8777-8783`; the author, 2026-10-01: *"yes"*).
//!
//! The game's `You can't target that.` marks the creature then targeted
//! (`hunt/replies.rs`, `Reply::Untargetable`); that one id is never chosen
//! again at once. At the next tick its name is learned from the state: no
//! creature by that name is chosen again, and the name goes to the
//! character's settings file (its `hunt` section) for the hunts after this
//! one, read when a hunt starts. bigshot uses the list the same two ways as
//! [`Profile::never_attack`](super::profile::Profile::never_attack): never
//! attacked, never counted toward fleeing.

use std::collections::BTreeSet;
use std::path::Path;

use cena_session::{GameState, Notice, NoticeKind, SessionHandle, settings_store};
use serde::{Deserialize, Serialize};

use super::engine::Hunt;

/// The character's settings file's section for what the hunt learns.
pub const SECTION: &str = "hunt";

/// What the hunt keeps in its section.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Kept {
    /// Creature names the game would not let the hunt target, lowercase.
    #[serde(default)]
    untargetable: BTreeSet<String>,
}

impl Hunt {
    /// Start knowing these names untargetable, from earlier hunts.
    #[must_use]
    pub fn with_untargetable(mut self, names: BTreeSet<String>) -> Self {
        self.heard.untargetable_names = names;
        self
    }

    /// Names learned untargetable since this was last asked, to be kept.
    pub fn take_untargetable(&mut self) -> Vec<String> {
        std::mem::take(&mut self.heard.untargetable_learned)
    }

    /// Whether a creature by this name is never chosen.
    pub(super) fn untargetable_named(&self, name: &str) -> bool {
        self.heard
            .untargetable_names
            .contains(&name.trim().to_lowercase())
    }

    /// The names of creatures refused by id, learned from the state.
    pub(super) fn name_untargetable(&mut self, state: &GameState) {
        let unnamed: Vec<i64> = self
            .heard
            .untargetable
            .difference(&self.heard.untargetable_named_ids)
            .copied()
            .collect();
        for id in unnamed {
            let Some(creature) = state.creatures().get(id) else {
                continue;
            };
            self.heard.untargetable_named_ids.insert(id);
            let name = creature.name.trim().to_lowercase();
            if !name.is_empty() && self.heard.untargetable_names.insert(name.clone()) {
                self.heard.untargetable_learned.push(name);
            }
        }
    }
}

/// The names a character's earlier hunts learned untargetable. Empty when
/// there is no file yet; a file that cannot be read is the caller's to say.
///
/// # Errors
///
/// The settings file exists and cannot be read, or its section is not this
/// shape.
pub fn remembered(dir: &Path, instance: &str, name: &str) -> Result<BTreeSet<String>, String> {
    let file = settings_store::load(dir, instance, name).map_err(|e| e.to_string())?;
    file.section::<Kept>(SECTION)
        .map(|kept| kept.untargetable)
        .map_err(|e| format!("the {SECTION} section is malformed: {e}"))
}

/// Add `names` to what the character's hunts remember untargetable.
///
/// # Errors
///
/// The settings file could not be read or written.
pub fn remember(dir: &Path, instance: &str, name: &str, names: &[String]) -> Result<(), String> {
    let path = settings_store::settings_path(dir, instance, name)
        .ok_or_else(|| "the character has no usable file name".to_owned())?;
    // With no other change to the file between its reading and its
    // writing: the player may be changing a setting from the menu.
    cena_session::store::changing(&path, || {
        let mut file = settings_store::load(dir, instance, name).map_err(|e| e.to_string())?;
        let mut kept = file
            .section::<Kept>(SECTION)
            .map_err(|e| format!("the {SECTION} section is malformed: {e}"))?;
        kept.untargetable.extend(names.iter().cloned());
        file.set_section(SECTION, &kept)
            .map_err(|e| e.to_string())?;
        settings_store::save(dir, &file).map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// The machine, knowing what the character's earlier hunts learned; a file
/// that cannot be read is said, and the hunt goes on knowing nothing.
#[must_use]
pub(super) fn read(
    handle: &SessionHandle,
    dir: &Path,
    who: Option<&(String, String)>,
    machine: Hunt,
) -> Hunt {
    let Some((instance, name)) = who else {
        return machine;
    };
    match remembered(dir, instance, name) {
        Ok(names) => machine.with_untargetable(names),
        Err(why) => {
            handle.say(Notice::line(
                NoticeKind::Warn,
                format!("Hunt: the untargetable creatures could not be read: {why}"),
            ));
            machine
        }
    }
}

/// Keep `names` for the character's later hunts, and say so either way.
pub(super) fn keep(
    handle: &SessionHandle,
    dir: &Path,
    who: Option<&(String, String)>,
    names: &[String],
) {
    let Some((instance, name)) = who else {
        return;
    };
    let said = match remember(dir, instance, name, names) {
        Ok(()) => (
            NoticeKind::Info,
            format!(
                "Hunt: {} cannot be targeted; later hunts leave it be.",
                names.join(", ")
            ),
        ),
        Err(why) => (
            NoticeKind::Warn,
            format!(
                "Hunt: {} cannot be targeted, but it could not be kept: {why}",
                names.join(", ")
            ),
        ),
    };
    handle.say(Notice::line(said.0, said.1));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What one hunt keeps, the next one reads; other sections are kept.
    #[test]
    fn names_kept_are_read_by_the_next_hunt() {
        let dir = std::env::temp_dir().join(format!("cena-untargetable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(remembered(&dir, "GS3", "Nisugi").unwrap().is_empty());
        let mut file = settings_store::load(&dir, "GS3", "Nisugi").unwrap();
        file.set_section("other", &1).unwrap();
        settings_store::save(&dir, &file).unwrap();

        remember(&dir, "GS3", "Nisugi", &["golem".to_owned()]).unwrap();
        remember(&dir, "GS3", "Nisugi", &["warg".to_owned()]).unwrap();
        let names = remembered(&dir, "GS3", "Nisugi").unwrap();
        assert_eq!(names, ["golem".to_owned(), "warg".to_owned()].into());
        let file = settings_store::load(&dir, "GS3", "Nisugi").unwrap();
        assert_eq!(file.section::<i32>("other").unwrap(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
