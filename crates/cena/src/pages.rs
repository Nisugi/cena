//! The settings menu's pages for one character (`plan/50` §7 step 1). First
//! its own settings file's ([`crate::general`], step 3); then a page per
//! behavior, built from its table of keys and the character's file, a change
//! going through the writer that behavior's `;` command uses
//! ([`crate::hunt::settings::change`]). So the menu and the command cannot
//! disagree about a value, and each file has one writer.
//!
//! A character is named as the roster names it, `GAME:Name`, running or
//! not: its files are named by the instance its game names itself
//! (`cena_platform::instance`).

use std::collections::BTreeSet;
use std::path::Path;

use cena_behavior::settings::{self, Held, KeyKind, Shown, Stored};
use cena_ui::settings::{Change, Page, Row, RowKind, Value};

use crate::hunt::settings::{Profile, change, profiles};

/// The character a roster name means: the instance its files are named by,
/// and its name.
///
/// # Errors
///
/// The name is not a roster name, or names a game Hydra does not know.
pub(crate) fn who(character: &str) -> Result<(&'static str, &str), String> {
    let (game, name) = character
        .split_once(':')
        .ok_or_else(|| format!("{character} is not a roster name (GAME:Name)."))?;
    let instance = cena_platform::instance(game)
        .ok_or_else(|| format!("{game} is not a game Hydra knows."))?;
    Ok((instance, name.trim()))
}

/// Every page for `character` (`GAME:Name`), in the menu's order: its own
/// file's, then travel's, with `map`'s own settings (`Map::setting_names`),
/// then each behavior's.
///
/// # Errors
///
/// The name is not a roster name, or names a game Hydra does not know.
pub(crate) fn pages(
    dir: &Path,
    map: &BTreeSet<String>,
    character: &str,
) -> Result<Vec<Page>, String> {
    let (instance, name) = who(character)?;
    let behaviors = profiles()
        .into_iter()
        .map(|profile| page(dir, &profile, instance, name));
    Ok(crate::general::pages(dir, instance, name)
        .into_iter()
        .chain([crate::travel_page::page(dir, instance, name, map)])
        .chain(behaviors)
        .collect())
}

/// One profile's page: its rows as the file holds them, or why it cannot be
/// changed.
fn page(dir: &Path, profile: &Profile, instance: &str, name: &str) -> Page {
    let mut page = Page {
        id: profile.id.to_owned(),
        title: profile.label.to_owned(),
        file: String::new(),
        takes: profile.takes.to_owned(),
        problem: None,
        rows: Vec::new(),
    };
    let Some(path) = (profile.path)(dir, instance, name) else {
        page.problem = Some(format!("{name} is not a name a file can have."));
        return page;
    };
    page.file = path
        .strip_prefix(dir)
        .unwrap_or(&path)
        .display()
        .to_string();
    let own = match settings::read_text(&path) {
        Stored::Found(text) => text,
        Stored::Missing => String::new(),
        Stored::Broken(why) => {
            page.problem = Some(format!("Nothing here is changed while {why}"));
            return page;
        }
    };
    match (profile.canonical)(&own)
        .and_then(|canonical| settings::shown(&own, &canonical, profile.table))
    {
        Ok(shown) => page.rows = shown.into_iter().map(row).collect(),
        Err(why) => {
            page.problem = Some(format!(
                "Nothing here is changed while it does not read: {why}"
            ));
        }
    }
    page
}

/// A setting as the menu draws it.
fn row(shown: Shown) -> Row {
    let kind = match shown.key.kind {
        KeyKind::Toggle => RowKind::Toggle,
        KeyKind::Whole { min, max } => RowKind::Whole { min, max },
        KeyKind::Number { min, max } => RowKind::Number { min, max },
        KeyKind::Text => RowKind::Text,
        KeyKind::Numbers => RowKind::Numbers,
        KeyKind::Words => RowKind::Words,
        KeyKind::Map => RowKind::Map,
    };
    let value = match shown.value {
        None => Value::Unset,
        Some(Held::Bool(on)) => Value::On(on),
        Some(Held::Text(text)) => Value::Text(text),
        Some(Held::List(items)) => Value::List(items),
        Some(Held::Map(pairs)) => Value::Map(pairs),
    };
    Row {
        key: shown.key.name.to_owned(),
        label: shown.key.label.to_owned(),
        help: shown.key.help.to_owned(),
        kind,
        value,
        here: shown.here,
    }
}

/// Apply `wanted`, through the writer the `;` command uses: what was done,
/// or why nothing was. `map`'s own settings may be set on the travel page.
pub(crate) fn apply(dir: &Path, map: &BTreeSet<String>, wanted: &Change) -> String {
    let (instance, name) = match who(&wanted.character) {
        Ok(who) => who,
        Err(why) => return why,
    };
    if wanted.page == crate::travel_page::PAGE {
        let to = wanted.to.as_deref();
        return match crate::travel_page::change(dir, (instance, name), &wanted.key, to, map) {
            Ok(done) | Err(done) => done,
        };
    }
    if crate::general::owns(&wanted.page) {
        let to = wanted.to.as_deref();
        return match crate::general::change(dir, (instance, name), &wanted.page, &wanted.key, to) {
            Ok(done) | Err(done) => done,
        };
    }
    let Some(profile) = profiles()
        .into_iter()
        .find(|profile| profile.id == wanted.page)
    else {
        return format!("There is no {} page.", wanted.page);
    };
    let Some(path) = (profile.path)(dir, instance, name) else {
        return format!("{name} is not a name a file can have.");
    };
    match change(&profile, &path, &wanted.key, wanted.to.as_deref()) {
        Ok(done) | Err(done) => done,
    }
}

/// Give the window `character`'s pages, as they are now. Says nothing when
/// they are sent: the menu shows them.
pub(crate) fn send(
    dir: &Path,
    map: &BTreeSet<String>,
    character: &str,
    gui: Option<&cena_gui::Sessions>,
) -> String {
    match pages(dir, map, character) {
        Ok(pages) => {
            if let Some(gui) = gui {
                gui.settings(character.to_owned(), pages);
            }
            String::new()
        }
        Err(why) => why,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(test: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cena-pages-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn row<'a>(pages: &'a [Page], page: &str, key: &str) -> Option<&'a Row> {
        pages
            .iter()
            .find(|found| found.id == page)
            .and_then(|found| found.rows.iter().find(|row| row.key == key))
    }

    fn wanted(page: &str, key: &str, to: Option<&str>) -> Change {
        Change {
            character: "GS3:Nisugi".to_owned(),
            page: page.to_owned(),
            key: key.to_owned(),
            to: to.map(str::to_owned),
        }
    }

    /// A character with no files has every page, each setting at its
    /// default and set nowhere, in the file its instance names.
    #[test]
    fn a_character_with_no_files_sees_the_defaults() {
        let dir = scratch("defaults");
        let pages = pages(&dir, &BTreeSet::new(), "GS3:Nisugi").expect("a roster name");
        let ids: Vec<&str> = pages.iter().map(|page| page.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "general", "log", "record", "travel", "heal", "waggle", "keep", "sc"
            ],
            "the character's own file first"
        );
        let heal = &pages[4];
        assert_eq!(
            heal.file,
            std::path::Path::new("hunt")
                .join("heal")
                .join("prime_nisugi.toml")
                .display()
                .to_string()
        );
        assert_eq!(heal.rows.len(), 10);
        let starts = row(&pages, "waggle", "start_at");
        assert_eq!(
            starts.map(|row| (&row.value, row.here, &row.kind)),
            Some((
                &Value::Text("180.0".to_owned()),
                false,
                &RowKind::Number {
                    min: 0.0,
                    max: 250.0
                }
            ))
        );
        assert_eq!(
            row(&pages, "heal", "stock").map(|row| &row.value),
            Some(&Value::Unset),
            "no default"
        );
        assert_eq!(
            row(&pages, "sc", "typed").map(|row| &row.value),
            Some(&Value::On(true))
        );
        assert!(
            super::pages(&dir, &BTreeSet::new(), "Nisugi").is_err(),
            "not a roster name"
        );
        assert!(
            super::pages(&dir, &BTreeSet::new(), "DR:Nisugi").is_err(),
            "not a game Hydra knows"
        );
    }

    /// A change goes through the command's writer: set, it is the file's
    /// own; put back, it is the default again; one the profile would not
    /// read is refused and nothing is saved.
    #[test]
    fn a_change_is_saved_as_the_command_saves_it() {
        let dir = scratch("change");
        let said = apply(
            &dir,
            &BTreeSet::new(),
            &wanted("heal", "container", Some("\"herbsack\"")),
        );
        assert!(said.starts_with("Heal: "), "{said}");
        let pages = pages(&dir, &BTreeSet::new(), "GS3:Nisugi").expect("pages");
        assert_eq!(
            row(&pages, "heal", "container").map(|row| (&row.value, row.here)),
            Some((&Value::Text("herbsack".to_owned()), true))
        );
        apply(
            &dir,
            &BTreeSet::new(),
            &wanted("keep", "spells", Some("[401, 406]")),
        );
        apply(&dir, &BTreeSet::new(), &wanted("sc", "typed", Some("off")));
        let pages = super::pages(&dir, &BTreeSet::new(), "GS3:Nisugi").expect("pages");
        assert_eq!(
            row(&pages, "keep", "spells").map(|row| &row.value),
            Some(&Value::List(vec!["401".to_owned(), "406".to_owned()]))
        );
        assert_eq!(
            row(&pages, "sc", "typed").map(|row| &row.value),
            Some(&Value::On(false))
        );

        let refused = apply(
            &dir,
            &BTreeSet::new(),
            &wanted("heal", "stock", Some("lots")),
        );
        assert!(refused.contains("not saved"), "{refused}");
        apply(&dir, &BTreeSet::new(), &wanted("heal", "container", None));
        let pages = super::pages(&dir, &BTreeSet::new(), "GS3:Nisugi").expect("pages");
        assert_eq!(
            row(&pages, "heal", "container").map(|row| (&row.value, row.here)),
            Some((&Value::Text(String::new()), false)),
            "back to its default"
        );
        assert!(apply(&dir, &BTreeSet::new(), &wanted("hunt", "x", None)).contains("no hunt page"));
        // Travel's page is the travel file's writer's.
        assert_eq!(
            apply(
                &dir,
                &BTreeSet::new(),
                &wanted("travel", "get_silvers", Some("on"))
            ),
            "Travel: get_silvers is true, from the next trip."
        );
        // The character's own file's pages are its own writer's.
        assert_eq!(
            apply(
                &dir,
                &BTreeSet::new(),
                &wanted("general", "sorter", Some("on"))
            ),
            "Container-look sorting on."
        );
        let pages = super::pages(&dir, &BTreeSet::new(), "GS3:Nisugi").expect("pages");
        assert_eq!(
            row(&pages, "general", "sorter").map(|row| (&row.value, row.here)),
            Some((&Value::On(true), true))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file that is there and does not read shows why, has no rows, and
    /// is never written over.
    #[test]
    fn a_broken_file_is_shown_and_left_alone() {
        let dir = scratch("broken");
        let path = cena_behavior::heal::path(&dir, "Prime", "Nisugi").expect("a file name");
        let parent = path.parent().expect("a folder");
        std::fs::create_dir_all(parent).expect("made");
        std::fs::write(&path, "container = [").expect("written");
        let pages = pages(&dir, &BTreeSet::new(), "GS3:Nisugi").expect("pages");
        let heal = pages.iter().find(|page| page.id == "heal").expect("heal");
        assert!(heal.problem.is_some() && heal.rows.is_empty());
        let said = apply(
            &dir,
            &BTreeSet::new(),
            &wanted("heal", "container", Some("\"herbsack\"")),
        );
        assert!(said.contains("nothing was changed"), "{said}");
        assert_eq!(
            std::fs::read_to_string(&path).ok().as_deref(),
            Some("container = [")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
