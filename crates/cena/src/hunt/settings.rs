//! `;hunt set|unset|show|help` and `;heal set|unset|show`: a setting changed
//! from the game line, and read back the way the behavior will read it, so
//! a bad value is refused by name rather than saved
//! (`cena_behavior::settings`).

use std::io;
use std::path::Path;

use cena_behavior::heal::{self, HealProfile};
use cena_behavior::hunt::{self, LoadError, command::HELP};
use cena_behavior::settings;
use cena_session::NoticeKind;

use super::Say;

/// The instance and character, when the game has said them.
type Who<'a> = Option<&'a (String, String)>;

/// `;hunt help`, or `;hunt` alone.
pub(super) fn help(say: Say<'_>) {
    for line in HELP {
        say(NoticeKind::Info, format!("Hunt: {line}"));
    }
}

/// `;hunt set <profile> <setting> <value>`: changed in the profile's file.
pub(super) fn set(dir: &Path, who: Who<'_>, profile: &str, key: &str, value: &str, say: Say<'_>) {
    edit(dir, who, profile, say, |text| {
        let (text, was) = settings::set(text, key, settings::typed(value))?;
        let now = settings::text_lines(&text, Some(key))?.join(", ");
        let was = was.map_or_else(|| "unset".to_owned(), |was| was.to_string());
        Ok((text, format!("{now} (was {was})")))
    });
}

/// `;hunt unset <profile> <setting>`: out of the profile's file.
pub(super) fn unset(dir: &Path, who: Who<'_>, profile: &str, key: &str, say: Say<'_>) {
    edit(dir, who, profile, say, |text| {
        let (text, removed) = settings::unset(text, key)?;
        if !removed {
            return Err(format!("{profile} does not set {key}"));
        }
        Ok((text, format!("{key} unset; the default decides it")))
    });
}

/// Change the profile's file with `change`, then load it as this character
/// would: saved when it reads as a profile (its problems said), and put
/// back when it does not.
fn edit(
    dir: &Path,
    who: Who<'_>,
    profile: &str,
    say: Say<'_>,
    change: impl FnOnce(&str) -> Result<(String, String), String>,
) {
    let Some(path) = hunt::chain::profile_path(dir, profile) else {
        say(
            NoticeKind::Error,
            format!("Hunt: {profile:?} is not a name a profile can have."),
        );
        return;
    };
    let old = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            say(
                NoticeKind::Error,
                format!(
                    "Hunt: there is no profile {profile}. `hunt list` shows them; `hunt import <bigshot yaml>` brings one in."
                ),
            );
            return;
        }
        Err(e) => {
            say(
                NoticeKind::Error,
                format!("Hunt: cannot read {profile}: {e}"),
            );
            return;
        }
    };
    let (text, done) = match change(&old) {
        Ok(changed) => changed,
        Err(why) => {
            say(NoticeKind::Error, format!("Hunt: {profile}: {why}"));
            return;
        }
    };
    if let Err(e) = std::fs::write(&path, &text) {
        say(
            NoticeKind::Error,
            format!("Hunt: {profile}: not saved: {e}"),
        );
        return;
    }
    let (instance, character) = split(who);
    match hunt::load(dir, instance, character, profile) {
        Ok(_) => say(NoticeKind::Info, format!("Hunt: {profile}: {done}.")),
        Err(LoadError::Invalid(problems)) => {
            say(NoticeKind::Info, format!("Hunt: {profile}: {done}."));
            for problem in problems {
                say(
                    NoticeKind::Warn,
                    format!("Hunt: {profile} will not run until this is fixed: {problem}"),
                );
            }
        }
        Err(e) => {
            let back = std::fs::write(&path, &old).map_or_else(
                |e| format!(" (and the old file could not be put back: {e})"),
                |()| String::new(),
            );
            say(
                NoticeKind::Error,
                format!("Hunt: {profile}: not saved, it would not read: {e}{back}"),
            );
        }
    }
}

/// `;hunt show <profile> [setting]`: every setting as this character runs
/// the profile, defaults included, or those under one.
pub(super) fn show(dir: &Path, who: Who<'_>, profile: &str, key: Option<&str>, say: Say<'_>) {
    let (instance, character) = split(who);
    let shown = hunt::chain::levels(dir, instance, character, profile)
        .map_err(|e| e.to_string())
        .and_then(|(levels, _)| hunt::chain::merge(levels))
        .and_then(|table| settings::lines(&table, key));
    match shown {
        Ok(lines) => {
            say(
                NoticeKind::Info,
                format!(
                    "Hunt: {profile}, as this character runs it. Change one with `hunt set {profile} <setting> <value>`."
                ),
            );
            for line in lines {
                say(NoticeKind::Info, format!("  {line}"));
            }
        }
        Err(why) => say(NoticeKind::Error, format!("Hunt: {profile}: {why}")),
    }
}

/// `;heal set <setting> <value>`: changed in this character's heal profile.
pub(super) fn heal_set(dir: &Path, who: Who<'_>, key: &str, value: &str, say: Say<'_>) {
    heal_edit(dir, who, say, |text| {
        let (text, _) = settings::set(text, key, settings::typed(value))?;
        let now = settings::text_lines(&text, Some(key))?.join(", ");
        Ok((text, now))
    });
}

/// `;heal unset <setting>`: back to its default.
pub(super) fn heal_unset(dir: &Path, who: Who<'_>, key: &str, say: Say<'_>) {
    heal_edit(dir, who, say, |text| {
        let (text, _) = settings::unset(text, key)?;
        Ok((text, format!("{key} is back to its default")))
    });
}

fn heal_edit(
    dir: &Path,
    who: Who<'_>,
    say: Say<'_>,
    change: impl FnOnce(&str) -> Result<(String, String), String>,
) {
    let Some(path) = who.and_then(|(i, n)| heal::path(dir, i, n)) else {
        say(
            NoticeKind::Error,
            "Heal: the game has not said who this is yet.".to_owned(),
        );
        return;
    };
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let changed = change(&old).and_then(|(text, done)| {
        // Read as the heal will read it before anything is saved.
        HealProfile::parse(&text).map(|_| (text, done))
    });
    let (text, done) = match changed {
        Ok(changed) => changed,
        Err(why) => {
            say(
                NoticeKind::Error,
                format!(
                    "Heal: not saved: {why}. The settings are {}.",
                    heal::profile::KEYS.join(", ")
                ),
            );
            return;
        }
    };
    let written = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text));
    match written {
        Ok(()) => say(NoticeKind::Info, format!("Heal: {done}.")),
        Err(e) => say(
            NoticeKind::Error,
            format!("Heal: {done}, but not saved: {e}"),
        ),
    }
}

/// `;heal show`: this character's heal settings, and the ones not set.
pub(super) fn heal_show(dir: &Path, who: Who<'_>, say: Say<'_>) {
    let profile = who
        .and_then(|(i, n)| heal::path(dir, i, n))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or_else(
            || Ok(HealProfile::default()),
            |text| HealProfile::parse(&text),
        );
    let lines = profile.and_then(|p| p.to_toml()).and_then(|text| {
        let unset = settings::not_set(&text, heal::profile::KEYS)?;
        settings::text_lines(&text, None).map(|lines| (lines, unset))
    });
    match lines {
        Ok((lines, unset)) => {
            say(
                NoticeKind::Info,
                "Heal: change one with `heal set <setting> <value>`.".to_owned(),
            );
            for line in lines {
                say(NoticeKind::Info, format!("  {line}"));
            }
            if !unset.is_empty() {
                say(NoticeKind::Info, format!("  not set: {}", unset.join(", ")));
            }
        }
        Err(why) => say(NoticeKind::Error, format!("Heal: {why}")),
    }
}

fn split(who: Who<'_>) -> (Option<&str>, Option<&str>) {
    who.map_or((None, None), |(instance, character)| {
        (Some(instance.as_str()), Some(character.as_str()))
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use cena_behavior::hunt;
    use cena_session::NoticeKind;

    use super::{heal_set, heal_show, set, show};

    const PROFILE: &str = "# imported\n\ntargets = [{ any = true, routine = \"a\" }]\n\n[rooms]\nhunting = 10\nresting = 20\n\n[routines]\na = [\"volley\", \"fire\"]\n\n[sequences]\nvolley = []\n";

    /// A data directory of its own, holding `ojandhaart`.
    fn dir() -> std::io::Result<PathBuf> {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "cena-settings-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let profiles = hunt::chain::profiles_dir(&dir);
        std::fs::create_dir_all(&profiles)?;
        std::fs::write(profiles.join("ojandhaart.toml"), PROFILE)?;
        Ok(dir)
    }

    fn file(dir: &Path) -> String {
        std::fs::read_to_string(hunt::chain::profiles_dir(dir).join("ojandhaart.toml"))
            .unwrap_or_default()
    }

    #[test]
    fn a_setting_is_saved_and_a_bad_one_is_refused_by_name() {
        let dir = dir().unwrap();
        let who = ("prime".to_owned(), "Nisugi".to_owned());
        let said = RefCell::new(Vec::new());
        let say = |kind: NoticeKind, text: String| said.borrow_mut().push((kind, text));

        set(
            &dir,
            Some(&who),
            "ojandhaart",
            "rooms.resting",
            "29877",
            &say,
        );
        assert!(file(&dir).contains("resting = 29877"), "{}", file(&dir));
        assert!(file(&dir).starts_with("# imported\n"), "the head is kept");

        let before = file(&dir);
        set(&dir, Some(&who), "ojandhaart", "rooms.restin", "5", &say);
        assert_eq!(file(&dir), before, "an unknown key is not saved");
        let last = said.borrow().last().cloned().unwrap();
        assert_eq!(last.0, NoticeKind::Error);
        assert!(last.1.contains("restin"), "{}", last.1);

        set(
            &dir,
            Some(&who),
            "ojandhaart",
            "sequences.volley.steps",
            r#"["store weapon", "ready 2weapon", "weapon volley", "ready weapon"]"#,
            &say,
        );
        set(
            &dir,
            Some(&who),
            "ojandhaart",
            "sequences.volley.when",
            r#"expiring "Briar Betrayer" 7 available "volley""#,
            &say,
        );
        let loaded = hunt::load(&dir, Some("prime"), Some("Nisugi"), "ojandhaart").unwrap();
        let volley = loaded.profile.sequences.get("volley").unwrap();
        assert_eq!((volley.when.len(), volley.steps.len()), (2, 4));

        said.borrow_mut().clear();
        show(&dir, Some(&who), "ojandhaart", Some("rooms"), &say);
        assert!(
            said.borrow()
                .iter()
                .any(|(_, t)| t.contains("rooms.resting = 29877")),
            "{:?}",
            said.borrow()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_heal_profile_is_made_by_its_first_setting() {
        let dir = dir().unwrap();
        let who = ("prime".to_owned(), "Nisugi".to_owned());
        let said = RefCell::new(Vec::new());
        let say = |kind: NoticeKind, text: String| said.borrow_mut().push((kind, text));

        heal_set(&dir, Some(&who), "container", "herb pouch", &say);
        heal_set(&dir, Some(&who), "potions", "on", &say);
        heal_set(&dir, Some(&who), "herbs", "lots", &say);
        let path = cena_behavior::heal::path(&dir, "prime", "Nisugi").unwrap();
        let profile =
            cena_behavior::heal::HealProfile::parse(&std::fs::read_to_string(path).unwrap())
                .unwrap();
        assert_eq!(profile.container, "herb pouch");
        assert!(profile.potions);
        assert_eq!(
            said.borrow().last().map(|(kind, _)| *kind),
            Some(NoticeKind::Error),
            "an unknown setting is refused"
        );
        said.borrow_mut().clear();
        heal_show(&dir, Some(&who), &say);
        let shown: Vec<String> = said.borrow().iter().map(|(_, t)| t.clone()).collect();
        assert!(
            shown
                .iter()
                .any(|t| t.contains("container = \"herb pouch\"")),
            "{shown:?}"
        );
        assert!(
            shown.iter().any(|t| t.contains("not set: stock")),
            "{shown:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
