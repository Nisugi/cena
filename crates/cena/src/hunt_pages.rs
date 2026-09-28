//! The settings menu's hunt pages (`plan/50` §7 step 6): a page per hunt
//! profile, as the character runs it. Every setting in effect is shown with
//! where it came from -- built in, `global.toml`, the profile, or the
//! character's own file -- which nothing showed before (§2 item 7). The
//! author asked for *"both"*: that view, and each setting changed there
//! (§6 item 8).
//!
//! A change is written into the profile through `;hunt set`'s own writer
//! ([`crate::hunt::settings::edited`]), which loads the profile as this
//! character would after saving: a change it cannot read is put back, and
//! what would stop the hunt running is said. So *check* is beside every
//! *save*. A setting the character's own file sets is shown and not
//! changed here: that file wins over the profile, and has no writer yet.
//!
//! The rows come from the profile's own table, grouped by the table each
//! is in, so a setting added to the hunt is shown without being listed here.

use std::path::Path;

use cena_behavior::hunt::chain::{self, Level};
use cena_ui::settings::{Page, Row, RowKind, Value};

/// What a hunt page's id begins with; the profile's name follows.
pub(crate) const PREFIX: &str = "hunt:";

/// Whether `page` is a hunt page.
pub(crate) fn owns(page: &str) -> bool {
    page.starts_with(PREFIX)
}

/// A page for each hunt profile there is, as `name` on `instance` runs it.
pub(crate) fn pages(dir: &Path, instance: &str, name: &str) -> Vec<Page> {
    match chain::profile_names(dir) {
        Ok(profiles) => profiles
            .iter()
            .map(|profile| page(dir, instance, name, profile))
            .collect(),
        Err(why) => vec![Page {
            id: PREFIX.to_owned(),
            title: "Hunt".to_owned(),
            file: chain::profiles_dir(dir).display().to_string(),
            takes: "the next time the hunt starts".to_owned(),
            problem: Some(format!("The profiles cannot be read: {why}")),
            rows: Vec::new(),
        }],
    }
}

/// One profile's page: every setting in effect, and where it came from.
fn page(dir: &Path, instance: &str, name: &str, profile: &str) -> Page {
    let file = chain::profile_path(dir, profile).map_or_else(String::new, |path| {
        path.strip_prefix(dir)
            .unwrap_or(&path)
            .display()
            .to_string()
    });
    let mut page = Page {
        id: format!("{PREFIX}{profile}"),
        title: format!("Hunt: {profile}"),
        file,
        takes: "the next time the hunt starts".to_owned(),
        problem: None,
        rows: Vec::new(),
    };
    let found = chain::named_levels(dir, Some(instance), Some(name), profile)
        .map_err(|why| why.to_string())
        .and_then(|levels| {
            let merged = chain::merge(levels.iter().map(|(_, table)| table.clone()).collect())?;
            Ok((chain::origins(levels)?, merged))
        });
    match found {
        Ok((settings, merged)) => {
            page.rows = settings
                .into_iter()
                .map(|(key, value, level)| row(&merged, key, &value, level))
                .collect();
            // The profile's own settings first, then each table's together.
            page.rows.sort_by_key(|row| row.key.contains('.'));
        }
        Err(why) => page.problem = Some(format!("Nothing here is changed while {why}")),
    }
    page
}

/// One setting as the page shows it.
fn row(merged: &toml::Table, key: String, value: &toml::Value, level: Level) -> Row {
    let theirs = level == Level::Character;
    let (kind, shown) = if theirs {
        (RowKind::Map, Value::Text(text(value)))
    } else {
        kind(merged, &key, value)
    };
    Row {
        label: key.clone(),
        help: if theirs {
            "Set in the character's own file, which wins over the profile: changed there."
                .to_owned()
        } else {
            format!("From {}.", level.name())
        },
        kind,
        value: shown,
        here: level == Level::Profile,
        from: Some(level.name().to_owned()),
        key,
    }
}

/// How a setting of this value is edited, and its value as the menu holds it.
fn kind(merged: &toml::Table, key: &str, value: &toml::Value) -> (RowKind, Value) {
    use toml::Value as Toml;
    match value {
        Toml::Boolean(on) => (RowKind::Toggle, Value::On(*on)),
        Toml::Integer(n) if *n >= 0 => (
            RowKind::Whole {
                min: 0,
                max: u32::MAX,
            },
            Value::Text(n.to_string()),
        ),
        Toml::Integer(_) | Toml::Float(_) => (
            RowKind::Number {
                min: -1e12,
                max: 1e12,
            },
            Value::Text(value.to_string()),
        ),
        Toml::String(words) => (RowKind::Text, Value::Text(words.clone())),
        Toml::Array(items) if items.is_empty() => {
            let kind = if chain::holds_numbers(merged, key) {
                RowKind::Numbers
            } else {
                RowKind::Words
            };
            (kind, Value::List(Vec::new()))
        }
        Toml::Array(items) if items.iter().all(Toml::is_integer) => (
            RowKind::Numbers,
            Value::List(items.iter().map(Toml::to_string).collect()),
        ),
        Toml::Array(items) if items.iter().all(Toml::is_str) => (
            RowKind::Words,
            Value::List(items.iter().map(text).collect()),
        ),
        other => (RowKind::Map, Value::Text(text(other))),
    }
}

/// A value as words: a string as it is, anything else as TOML writes it.
fn text(value: &toml::Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

/// Change `key` in the profile a hunt page is, as the menu writes it, or
/// take it out (`None`) so the level below decides it: through `;hunt
/// set`'s writer, then loaded as `name` on `instance` would run it. What
/// was done, and what would stop the hunt running; or why nothing was.
pub(crate) fn change(
    dir: &Path,
    (instance, name): (&str, &str),
    page: &str,
    key: &str,
    to: Option<&str>,
) -> String {
    let Some(profile) = page.strip_prefix(PREFIX) else {
        return format!("There is no {page} page.");
    };
    let who = (Some(instance), Some(name));
    let done = crate::hunt::settings::edited(dir, who, profile, crate::hunt::settings::to(key, to));
    match done {
        Ok((done, problems)) if problems.is_empty() => format!("Hunt: {profile}: {done}."),
        Ok((done, problems)) => format!(
            "Hunt: {profile}: {done}. It will not run until this is fixed: {}",
            problems.join("; ")
        ),
        Err(why) => format!("Hunt: {why}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_behavior::hunt::chain;

    /// A profile that runs: one routine, and the rooms it hunts and rests in.
    const PROFILE: &str = "targets = [{ any = true, routine = \"a\" }]\n\n[rooms]\nhunting = 10\nresting = 20\n\n[routines]\na = [\"fire\"]\n";

    /// A data folder with the profile `p`, a global file and the character's
    /// own.
    fn scratch(test: &str) -> std::io::Result<std::path::PathBuf> {
        let dir =
            std::env::temp_dir().join(format!("cena-hunt-pages-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let write = |path: std::path::PathBuf, text: &str| -> std::io::Result<()> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, text)
        };
        let profile =
            chain::profile_path(&dir, "p").ok_or_else(|| std::io::Error::other("no file name"))?;
        write(profile, PROFILE)?;
        write(chain::global_path(&dir), "[rooms]\nrally = [7]\n")?;
        let own = chain::character_path(&dir, "Prime", "Nisugi")
            .ok_or_else(|| std::io::Error::other("no file name"))?;
        write(own, "prepare = [\"stance defensive\"]\n")?;
        Ok(dir)
    }

    fn row<'a>(page: &'a Page, key: &str) -> Option<&'a Row> {
        page.rows.iter().find(|row| row.key == key)
    }

    /// A page per profile, every setting in effect on it with where it came
    /// from; what the character's own file sets is shown and not changed
    /// here; an empty list asks for its own kind.
    #[test]
    fn every_setting_is_shown_with_where_it_came_from() {
        let dir = scratch("shown").expect("a folder");
        let pages = pages(&dir, "Prime", "Nisugi");
        assert_eq!(pages.len(), 1);
        let page = &pages[0];
        assert_eq!(
            (page.id.as_str(), page.title.as_str()),
            ("hunt:p", "Hunt: p")
        );
        let at = |key: &str| {
            row(page, key).map(|row| {
                (
                    row.kind.clone(),
                    row.value.clone(),
                    row.here,
                    row.from.clone(),
                )
            })
        };
        assert_eq!(
            at("rooms.hunting"),
            Some((
                RowKind::Whole {
                    min: 0,
                    max: u32::MAX
                },
                Value::Text("10".to_owned()),
                true,
                Some("the profile".to_owned())
            ))
        );
        assert_eq!(
            at("rooms.rally"),
            Some((
                RowKind::Numbers,
                Value::List(vec!["7".to_owned()]),
                false,
                Some("global".to_owned())
            ))
        );
        assert!(at("prepare").is_some_and(|(kind, _, here, from)| {
            kind == RowKind::Map && !here && from.as_deref() == Some("the character's file")
        }));
        assert!(at("check_favor").is_some_and(|(kind, _, _, from)| {
            kind == RowKind::Toggle && from.as_deref() == Some("built in")
        }));
        assert_eq!(
            at("rooms.boundaries").map(|(kind, ..)| kind),
            Some(RowKind::Numbers)
        );
        assert_eq!(at("signs").map(|(kind, ..)| kind), Some(RowKind::Words));
        let first_table = page.rows.iter().position(|row| row.key.contains('.'));
        assert!(
            first_table.is_some_and(|at| page.rows[at..].iter().all(|row| row.key.contains('.'))),
            "the profile's own settings first, then the tables'"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A change goes through `;hunt set`'s writer and is checked: saved and
    /// said, put back to the level below, or refused and the file left as
    /// it was when the profile would not read.
    #[test]
    fn a_change_is_checked_as_it_is_saved() {
        let dir = scratch("change").expect("a folder");
        let who = ("Prime", "Nisugi");
        let file = || {
            chain::profile_path(&dir, "p")
                .and_then(|path| std::fs::read_to_string(path).ok())
                .unwrap_or_default()
        };
        assert_eq!(
            change(&dir, who, "hunt:p", "rooms.resting", Some("29877")),
            "Hunt: p: rooms.resting = 29877 (was 20)."
        );
        assert!(file().contains("resting = 29877"), "{}", file());
        let said = change(&dir, who, "hunt:p", "rooms.resting", None);
        assert!(said.contains("the default decides it"), "{said}");
        assert!(!file().contains("resting"));

        // Saved, and what would stop the hunt running said with it.
        let said = change(&dir, who, "hunt:p", "rooms.allowed", Some("[]"));
        assert!(
            said.starts_with("Hunt: p: rooms.allowed = [] (was unset). It will not run until this is fixed: rooms.allowed must not be empty"),
            "{said}"
        );
        change(&dir, who, "hunt:p", "rooms.allowed", None);

        let before = file();
        let said = change(&dir, who, "hunt:p", "rooms.hunting", Some("\"ten\""));
        assert!(said.contains("it would not read"), "{said}");
        assert_eq!(file(), before, "put back");
        let said = change(&dir, who, "hunt:nope", "rooms.hunting", Some("1"));
        assert!(said.contains("there is no profile nope"), "{said}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
