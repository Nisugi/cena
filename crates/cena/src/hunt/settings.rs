//! `;hunt set|unset|show|help|setup`, and `set|unset|show|help` for `;heal`
//! and `;waggle`: a setting changed from the game line, and read back the
//! way the behavior will read it, so a bad value is refused by name rather
//! than saved (`cena_behavior::settings`). A file that is there and does
//! not read is never written over (`plan/44` Q05).

use std::path::{Path, PathBuf};

use cena_behavior::heal::{self, HealProfile};
use cena_behavior::hunt::command::{Of, Setting, Topic, help as help_for};
use cena_behavior::hunt::{self, LoadError};
use cena_behavior::keep::{self, KeepProfile};
use cena_behavior::loot;
use cena_behavior::settings::{self, Key, Stored};
use cena_behavior::spellcaster::{self, CasterProfile};
use cena_behavior::waggle::{self, WaggleProfile};
use cena_session::NoticeKind;

use super::Say;

/// The instance and character, when the game has said them.
type Who<'a> = Option<&'a (String, String)>;

/// `;hunt help` (or `;hunt` alone), `;heal help`, `;waggle help`.
pub(super) fn help(topic: Topic, say: Say<'_>) {
    let label = match topic {
        Topic::Hunt => "Hunt",
        Topic::Heal => "Heal",
        Topic::Waggle => "Waggle",
        Topic::Sc => "Sc",
        Topic::Loot => "Loot",
    };
    for line in help_for(topic) {
        say(NoticeKind::Info, format!("{label}: {line}"));
    }
}

/// `;hunt setup`: where the map's setup page is, and what else there is.
pub(super) fn setup(say: Say<'_>) {
    say(
        NoticeKind::Info,
        "Hunt: the setup page makes a new profile by picking rooms on the map: the \"Configure hunt\" button under the character page's map. It is there when Hydra is started with --web --hunt-setup and a map (CENA_MAP).".to_owned(),
    );
    say(
        NoticeKind::Info,
        "Hunt: from here, `hunt import <bigshot yaml>` brings a profile in, and `hunt show <profile>` / `hunt set <profile> <setting> <value>` see and change one.".to_owned(),
    );
}

/// `;hunt set <profile> <setting> <value>`: changed in the profile's file.
pub(super) fn set(dir: &Path, who: Who<'_>, profile: &str, key: &str, value: &str, say: Say<'_>) {
    tell(
        profile,
        edited(dir, split(who), profile, to(key, Some(value))),
        say,
    );
}

/// `;hunt unset <profile> <setting>`: out of the profile's file.
pub(super) fn unset(dir: &Path, who: Who<'_>, profile: &str, key: &str, say: Say<'_>) {
    tell(
        profile,
        edited(dir, split(who), profile, to(key, None)),
        say,
    );
}

/// The change `;hunt set` makes to a profile's text, `key` set to `value`,
/// or `;hunt unset`'s, `key` taken out (`None`) so the level below decides
/// it; and what was done.
pub(crate) fn to<'a>(
    key: &'a str,
    value: Option<&'a str>,
) -> impl FnOnce(&str) -> Result<(String, String), String> + 'a {
    move |text| {
        let Some(value) = value else {
            let (text, removed) = settings::unset(text, key)?;
            if !removed {
                return Err(format!("the profile does not set {key}"));
            }
            return Ok((text, format!("{key} unset; the default decides it")));
        };
        let (text, was) = settings::set(text, key, settings::typed(value))?;
        let now = settings::text_lines(&text, Some(key))?.join(", ");
        let was = was.map_or_else(|| "unset".to_owned(), |was| was.to_string());
        Ok((text, format!("{now} (was {was})")))
    }
}

/// Say what [`edited`] did, as `;hunt set` does: what was done and each
/// thing that would stop the hunt running, or why nothing was.
fn tell(profile: &str, done: Result<(String, Vec<String>), String>, say: Say<'_>) {
    match done {
        Ok((done, problems)) => {
            say(NoticeKind::Info, format!("Hunt: {profile}: {done}."));
            for problem in problems {
                say(
                    NoticeKind::Warn,
                    format!("Hunt: {profile} will not run until this is fixed: {problem}"),
                );
            }
        }
        Err(why) => say(NoticeKind::Error, format!("Hunt: {why}")),
    }
}

/// Change the profile's file with `change`, and load the result as this
/// character would before it is saved: saved when it reads as a profile,
/// with what would stop it running; never written when it does not. Read,
/// checked and written with no other change to the file between
/// (`cena_session::store::changing`; the crate review of 2026-09-28, R5). `;hunt set` and the settings menu's hunt pages
/// (`crate::hunt_pages`) both change a profile here.
///
/// # Errors
///
/// Why nothing was changed, after `Hunt: ` in what is said.
pub(crate) fn edited(
    dir: &Path,
    (instance, character): (Option<&str>, Option<&str>),
    profile: &str,
    change: impl FnOnce(&str) -> Result<(String, String), String>,
) -> Result<(String, Vec<String>), String> {
    let path = hunt::chain::profile_path(dir, profile)
        .ok_or_else(|| format!("{profile:?} is not a name a profile can have."))?;
    cena_session::store::changing(&path, || {
        let old = match settings::read_text(&path) {
            Stored::Found(text) => text,
            Stored::Missing => {
                return Err(format!(
                    "there is no profile {profile}. `hunt list` shows them; `hunt import <bigshot yaml>` brings one in."
                ));
            }
            Stored::Broken(why) => return Err(format!("nothing was changed: {why}")),
        };
        let (text, done) = change(&old).map_err(|why| format!("{profile}: {why}"))?;
        let problems = match hunt::chain::load_edited(dir, instance, character, profile, &text) {
            Ok(_) => Vec::new(),
            Err(LoadError::Invalid(problems)) => problems,
            Err(e) => return Err(format!("{profile}: not saved, it would not read: {e}")),
        };
        settings::save(&path, &text).map_err(|e| format!("{profile}: not saved: {e}"))?;
        Ok((done, problems))
    })
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

/// The profile at `path` read with `parse`, or its defaults when there is
/// no file; the reason when the file is there and does not read, so the
/// caller changes nothing (`keep_edit`, `sc_edit`).
pub(super) fn stored<T: Default>(
    path: &Path,
    parse: impl FnOnce(&str) -> Result<T, String>,
) -> Result<T, String> {
    match settings::read(path, parse) {
        Stored::Found(value) => Ok(value),
        Stored::Missing => Ok(T::default()),
        Stored::Broken(why) => Err(why),
    }
}

/// A character's own profile: what it is called, where it is, how the
/// behavior reads it, and its settings. The `;heal` and `;waggle` commands
/// and the settings menu (`crate::pages`) change it through [`change`], the
/// one writer, so the two cannot disagree about a value.
pub(crate) struct Profile {
    /// What the menu names its page by.
    pub(crate) id: &'static str,
    /// What a player calls it.
    pub(crate) label: &'static str,
    /// Where the character's file is.
    pub(crate) path: fn(&Path, &str, &str) -> Option<PathBuf>,
    /// The file's text read as the behavior reads it and written back whole:
    /// every setting, the defaults included.
    pub(crate) canonical: fn(&str) -> Result<String, String>,
    /// Its settings, in the struct's order.
    pub(crate) table: &'static [Key],
    /// When a change takes effect, as the menu says it.
    pub(crate) takes: &'static str,
}

fn heal_canonical(text: &str) -> Result<String, String> {
    HealProfile::parse(text)?.to_toml()
}

fn waggle_canonical(text: &str) -> Result<String, String> {
    WaggleProfile::parse(text)?.to_toml()
}

fn keep_canonical(text: &str) -> Result<String, String> {
    KeepProfile::parse(text)?.to_toml()
}

fn caster_canonical(text: &str) -> Result<String, String> {
    CasterProfile::parse(text)?.to_toml()
}

/// Every character profile the menu shows, in its order.
pub(crate) fn profiles() -> [Profile; 7] {
    [
        Profile {
            id: "heal",
            label: "Heal",
            path: heal::path,
            canonical: heal_canonical,
            table: heal::profile::TABLE,
            takes: "the next time Heal runs",
        },
        Profile {
            id: "waggle",
            label: "Waggle",
            path: waggle::path,
            canonical: waggle_canonical,
            table: waggle::TABLE,
            takes: "the next time Waggle runs",
        },
        Profile {
            id: "keep",
            label: "Keep",
            path: keep::path,
            canonical: keep_canonical,
            table: keep::TABLE,
            takes: "the next time Keep runs",
        },
        Profile {
            id: "sc",
            label: "Spellcaster",
            path: spellcaster::path,
            canonical: caster_canonical,
            table: spellcaster::TABLE,
            takes: "at once",
        },
        // The loot profile, as three pages over one file (`plan/50` §7
        // step 5): its own settings, `[skin]` and `[town]`.
        Profile {
            id: "loot",
            label: "Loot",
            path: loot::path,
            canonical: loot_canonical,
            table: loot::profile::TABLE,
            takes: "the next time the hunt loots",
        },
        Profile {
            id: "skin",
            label: "Skinning",
            path: loot::path,
            canonical: loot_canonical,
            table: loot::profile::SKIN_TABLE,
            takes: "the next time the hunt skins",
        },
        Profile {
            id: "town",
            label: "Selling",
            path: loot::path,
            canonical: loot_canonical,
            table: cena_behavior::town::settings::TABLE,
            takes: "at the next selling round",
        },
    ]
}

/// The loot profile with every setting written out, the three pages' values.
fn loot_canonical(text: &str) -> Result<String, String> {
    loot::LootProfile::parse(text)?.to_toml_whole()
}

fn of_profile(of: Of) -> Profile {
    // The loot profile's own page: its file holds `[skin]` and `[town]` too,
    // which `loot set skin.enable on` and `loot set town.<setting>` reach.
    let [heal, waggle, _, _, loot, ..] = profiles();
    match of {
        Of::Heal => heal,
        Of::Waggle => waggle,
        Of::Loot => loot,
    }
}

/// Set `key` in the profile at `path` to `value`, or put it back to its
/// default when `None`. The result is read back as the behavior will read
/// it before anything is saved, and a file that is there and does not read
/// is never written over. Says what was done, or why nothing was.
///
/// # Errors
///
/// Why nothing was saved, in words for the player.
pub(crate) fn change(
    profile: &Profile,
    path: &Path,
    key: &str,
    value: Option<&str>,
) -> Result<String, String> {
    // Read, checked and written with no other change to the file between
    // (`cena_session::store::changing`; the crate review of 2026-09-28, R5).
    cena_session::store::changing(path, || changed(profile, path, key, value))
}

/// [`change`], with the file's lock held.
fn changed(
    profile: &Profile,
    path: &Path,
    key: &str,
    value: Option<&str>,
) -> Result<String, String> {
    let label = profile.label;
    let old = match settings::read_text(path) {
        Stored::Found(text) => text,
        Stored::Missing => String::new(),
        Stored::Broken(why) => return Err(format!("{label}: nothing was changed: {why}")),
    };
    let changed = match value {
        Some(value) => settings::set(&old, key, settings::typed(value)).and_then(|(text, _)| {
            let now = settings::text_lines(&text, Some(key))?.join(", ");
            Ok((text, now))
        }),
        None => settings::unset(&old, key)
            .map(|(text, _)| (text, format!("{key} is back to its default"))),
    };
    let (text, done) = changed
        .and_then(|(text, done)| (profile.canonical)(&text).map(|_| (text, done)))
        .map_err(|why| {
            format!(
                "{label}: not saved: {why}. The settings are {}.",
                settings::names(profile.table).join(", ")
            )
        })?;
    settings::save(path, &text).map_err(|e| format!("{label}: {done}, but not saved: {e}"))?;
    Ok(format!("{label}: {done}."))
}

/// `;heal set|unset|show`, `;waggle set|unset|show`: this character's
/// profile, changed or listed. The first `set` makes it.
pub(super) fn profile(dir: &Path, who: Who<'_>, of: Of, setting: &Setting, say: Say<'_>) {
    let profile = of_profile(of);
    let label = profile.label;
    let Some(path) = who.and_then(|(i, n)| (profile.path)(dir, i, n)) else {
        say(
            NoticeKind::Error,
            format!("{label}: the game has not said who this is yet."),
        );
        return;
    };
    let done = match setting {
        Setting::Show => {
            return match settings::read_text(&path) {
                Stored::Found(text) => show_profile(&profile, &text, say),
                Stored::Missing => show_profile(&profile, "", say),
                Stored::Broken(why) => say(NoticeKind::Error, format!("{label}: {why}")),
            };
        }
        Setting::Set { key, value } => change(&profile, &path, key, Some(value)),
        Setting::Unset(key) => change(&profile, &path, key, None),
    };
    match done {
        Ok(done) => say(NoticeKind::Info, done),
        Err(why) => say(NoticeKind::Error, why),
    }
}

/// Every setting, the defaults included, and the ones not set.
fn show_profile(profile: &Profile, text: &str, say: Say<'_>) {
    let label = profile.label;
    let shown = (profile.canonical)(text).and_then(|canonical| {
        let unset = settings::not_set(&canonical, &settings::names(profile.table))?;
        settings::text_lines(&canonical, None).map(|lines| (lines, unset))
    });
    match shown {
        Ok((lines, unset)) => {
            let word = label.to_ascii_lowercase();
            say(
                NoticeKind::Info,
                format!("{label}: change one with `{word} set <setting> <value>`."),
            );
            for line in lines {
                say(NoticeKind::Info, format!("  {line}"));
            }
            if !unset.is_empty() {
                say(NoticeKind::Info, format!("  not set: {}", unset.join(", ")));
            }
        }
        Err(why) => say(NoticeKind::Error, format!("{label}: {why}")),
    }
}

/// `;loot reset unskinnable [creature]`: the creatures the loot profile
/// learned cannot be skinned, forgotten, every one or the one named
/// (`cena_behavior::loot::forget_unskinnable`). Read, changed and written
/// with no other change to the file between, as every writer here.
pub(super) fn reset_unskinnable(dir: &Path, who: Who<'_>, creature: Option<&str>, say: Say<'_>) {
    let Some(path) = who.and_then(|(i, n)| loot::path(dir, i, n)) else {
        say(
            NoticeKind::Error,
            "Loot: the game has not said who this is yet.".to_owned(),
        );
        return;
    };
    let done = cena_session::store::changing(&path, || {
        let text = match settings::read_text(&path) {
            Stored::Found(text) => text,
            Stored::Missing => {
                return Err(
                    "there is no loot profile yet; `hunt import-loot <eloot yaml>` brings one in."
                        .to_owned(),
                );
            }
            Stored::Broken(why) => return Err(format!("nothing was changed: {why}")),
        };
        let (written, said) = loot::forget_unskinnable(&text, creature)?;
        if let Some(text) = written {
            settings::save(&path, &text).map_err(|e| format!("not saved: {e}"))?;
        }
        Ok(said)
    });
    match done {
        Ok(said) => say(
            NoticeKind::Info,
            format!("Loot: {said} It takes effect the next time the hunt loots."),
        ),
        Err(why) => say(NoticeKind::Error, format!("Loot: {why}")),
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

    use cena_behavior::hunt;
    use cena_session::NoticeKind;

    use cena_behavior::hunt::command::{Of, Setting};

    use super::{profile, reset_unskinnable, set, show};

    fn heal_set(
        dir: &Path,
        who: Option<&(String, String)>,
        key: &str,
        value: &str,
        say: super::Say<'_>,
    ) {
        let setting = Setting::Set {
            key: key.to_owned(),
            value: value.to_owned(),
        };
        profile(dir, who, Of::Heal, &setting, say);
    }

    const PROFILE: &str = "# imported\n\ntargets = [{ any = true, routine = \"a\" }]\n\n[rooms]\nhunting = 10\nresting = 20\n\n[routines]\na = [\"volley\", \"fire\"]\n\n[sequences]\nvolley = []\n";

    /// A data directory of its own, holding `ojandhaart`: named by the
    /// process and the calling test, so no counter is needed.
    fn dir(test: &str) -> std::io::Result<PathBuf> {
        let dir = std::env::temp_dir().join(format!("cena-settings-{}-{test}", std::process::id()));
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
        let dir = dir("a_setting_is_saved_and_a_bad_one_is_refused_by_name").unwrap();
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
        let dir = dir("the_heal_profile_is_made_by_its_first_setting").unwrap();
        let who = ("prime".to_owned(), "Nisugi".to_owned());
        let said = RefCell::new(Vec::new());
        let say = |kind: NoticeKind, text: String| said.borrow_mut().push((kind, text));

        heal_set(&dir, Some(&who), "container", "herb pouch", &say);
        heal_set(&dir, Some(&who), "potions", "on", &say);
        heal_set(&dir, Some(&who), "herbs", "lots", &say);
        let path = cena_behavior::heal::path(&dir, "prime", "Nisugi").unwrap();
        let saved =
            cena_behavior::heal::HealProfile::parse(&std::fs::read_to_string(path).unwrap())
                .unwrap();
        assert_eq!(saved.container, "herb pouch");
        assert!(saved.potions);
        assert_eq!(
            said.borrow().last().map(|(kind, _)| *kind),
            Some(NoticeKind::Error),
            "an unknown setting is refused"
        );
        said.borrow_mut().clear();
        profile(&dir, Some(&who), Of::Heal, &Setting::Show, &say);
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

    /// `loot reset unskinnable Krag Dweller` takes the one name off, in any
    /// case; with none, the list is cleared; the head of the file is kept.
    #[test]
    fn the_unskinnable_list_is_reset_from_the_line() {
        let dir = dir("the_unskinnable_list_is_reset_from_the_line").unwrap();
        let who = ("prime".to_owned(), "Nisugi".to_owned());
        let said = RefCell::new(Vec::new());
        let say = |kind: NoticeKind, text: String| said.borrow_mut().push((kind, text));
        let path = cena_behavior::loot::path(&dir, "prime", "Nisugi").unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "# imported\n\n[skin]\nenable = true\nunskinnable = [\"cave troll\", \"krag dweller\"]\n",
        )
        .unwrap();
        let unskinnable = || {
            cena_behavior::loot::LootProfile::parse(&std::fs::read_to_string(&path).unwrap())
                .unwrap()
                .skin
                .unskinnable
        };
        reset_unskinnable(&dir, Some(&who), Some("Krag Dweller"), &say);
        assert_eq!(unskinnable(), ["cave troll"]);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("# imported\n")
        );
        reset_unskinnable(&dir, Some(&who), Some("wolf"), &say);
        assert_eq!(
            said.borrow().last().map(|(kind, _)| *kind),
            Some(NoticeKind::Error)
        );
        reset_unskinnable(&dir, Some(&who), None, &say);
        assert!(unskinnable().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `plan/44` Q05's acceptance: a malformed file is left byte for byte,
    /// and the error names it; a missing one is made.
    #[test]
    fn a_broken_profile_is_never_written_over() {
        let dir = dir("a_broken_profile_is_never_written_over").unwrap();
        let who = ("prime".to_owned(), "Nisugi".to_owned());
        let said = RefCell::new(Vec::new());
        let say = |kind: NoticeKind, text: String| said.borrow_mut().push((kind, text));
        let path = cena_behavior::waggle::path(&dir, "prime", "Nisugi").unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let broken = "cast_list = [101,
start_at = 90
";
        std::fs::write(&path, broken).unwrap();

        let setting = Setting::Set {
            key: "bail".to_owned(),
            value: "on".to_owned(),
        };
        profile(&dir, Some(&who), Of::Waggle, &setting, &say);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
        let last = said.borrow().last().cloned().unwrap();
        assert_eq!(last.0, NoticeKind::Error);
        assert!(last.1.contains("waggle"), "names the file: {}", last.1);

        std::fs::remove_file(&path).unwrap();
        let setting = Setting::Set {
            key: "cast_list".to_owned(),
            value: "[101, 107, 401]".to_owned(),
        };
        profile(&dir, Some(&who), Of::Waggle, &setting, &say);
        let made =
            cena_behavior::waggle::WaggleProfile::parse(&std::fs::read_to_string(&path).unwrap())
                .unwrap();
        assert_eq!(made.cast_list, [101, 107, 401]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A change that would not read is never written, so it cannot put
    /// back what another change saved meanwhile; one made while another is
    /// being made waits for it, and both are kept (the crate review of
    /// 2026-09-28, R5).
    #[test]
    fn a_change_neither_puts_back_nor_loses_another() {
        let dir = dir("a_change_neither_puts_back_nor_loses_another").unwrap();
        let path = hunt::chain::profile_path(&dir, "ojandhaart").unwrap();
        let who = (Some("prime"), Some("Nisugi"));

        let refused = super::edited(&dir, who, "ojandhaart", |old| {
            // Another change, saved while this one is made.
            std::fs::write(&path, old.replace("resting = 20", "resting = 29877")).unwrap();
            Ok((
                old.replace("hunting = 10", "hunting = \"ten\""),
                "hunting".to_owned(),
            ))
        });
        assert!(
            refused
                .as_ref()
                .is_err_and(|why| why.contains("would not read")),
            "{refused:?}"
        );
        assert!(file(&dir).contains("resting = 29877"), "{}", file(&dir));
        assert!(file(&dir).contains("hunting = 10"), "{}", file(&dir));

        let other = cena_session::store::changing(&path, || {
            let old = file(&dir);
            let other = {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    super::edited(&dir, who, "ojandhaart", |old| {
                        Ok((
                            old.replace("hunting = 10", "hunting = 11"),
                            "hunting".to_owned(),
                        ))
                    })
                })
            };
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::fs::write(&path, old.replace("resting = 29877", "resting = 21")).unwrap();
            other
        });
        assert!(other.join().unwrap().is_ok());
        assert!(file(&dir).contains("hunting = 11"), "{}", file(&dir));
        assert!(file(&dir).contains("resting = 21"), "{}", file(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A heal or waggle profile's change made while another is being made
    /// waits for it, and both are kept (R5).
    #[test]
    fn a_profile_change_made_meanwhile_waits() {
        let dir = dir("a_profile_change_made_meanwhile_waits").unwrap();
        let path = cena_behavior::waggle::path(&dir, "prime", "Nisugi").unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "start_at = 90\n").unwrap();
        let other = cena_session::store::changing(&path, || {
            let old = std::fs::read_to_string(&path).unwrap();
            let other = {
                let path = path.clone();
                std::thread::spawn(move || {
                    super::change(&super::of_profile(Of::Waggle), &path, "bail", Some("on"))
                })
            };
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::fs::write(&path, old.replace("start_at = 90", "start_at = 80")).unwrap();
            other
        });
        assert!(other.join().unwrap().is_ok());
        let made =
            cena_behavior::waggle::WaggleProfile::parse(&std::fs::read_to_string(&path).unwrap())
                .unwrap();
        assert!(made.bail, "the change made meanwhile");
        assert!(
            (made.start_at - 80.0).abs() < f64::EPSILON,
            "the change made first"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
