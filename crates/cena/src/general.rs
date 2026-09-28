//! The character's own settings file, as the settings menu's first pages
//! (`plan/50` §7 step 3): `<instance>_<character>.settings.json`
//! ([`settings_store`]), until now a hand edit only. Its sections, each a
//! system's:
//!
//! - **General**: the command symbol (`commands`, §6 item 3) and container-
//!   look sorting (`sorter`), which `;sorter` now saves (§6 item 4: *"sure
//!   and persistent"*). Both reach a running character at once.
//! - **Player log**: which feeds are written (`player_log`, §6 item 3), read
//!   when the character logs in.
//! - **Recording**: combat and loot, each its own switch (§6 item 5: *"per
//!   thing"*), off until turned on and then kept on, read when the character
//!   logs in. `--record` and `--no-record` are retired.
//!
//! A value is written as the menu writes it (`cena_gui::typed`), read here
//! as `;heal set` reads one ([`settings::typed`]). A file that is there and
//! cannot be trusted is shown with why and never written over: the store's
//! own rule.

use std::path::{Path, PathBuf};

use cena_behavior::settings;
use cena_session::SessionHandle;
use cena_session::command::claimant;
use cena_session::player_log::feed::{LogSettings, SECTION as LOG_SECTION};
use cena_session::player_log::{Capture, tap};
use cena_session::settings_store::{self, SettingsFile};
use cena_ui::settings::{Page, Row, RowKind, Value};

use crate::setup::{RECORD, Record};
use crate::sorter::{SECTION as SORTER, Saved as Sorter};

/// The *General* page's id.
pub(crate) const GENERAL: &str = "general";
/// The *Player log* page's id.
pub(crate) const LOG: &str = "log";
/// The *Recording* page's id.
pub(crate) const RECORDING: &str = "record";

/// The feeds the *Player log* page names, with what each is: every text
/// feed, then the readouts ([`tap::READOUTS`]), which cost the most disk.
/// A feed the file names that is not here is shown too.
const FEEDS: &[(&str, &str)] = &[
    (tap::MAIN, "the story"),
    (tap::COMMANDS, "what was sent"),
    (tap::NOTICES, "Hydra's own messages"),
    ("thoughts", "thoughts"),
    ("room", "the room"),
    ("inv", "the inventory window"),
    ("bounty", "the bounty window"),
    ("society", "the society window"),
    ("charprofile", "the character profile"),
    ("Spells", "the active spells"),
];

/// Whether this module owns the page `id`.
pub(crate) fn owns(id: &str) -> bool {
    [GENERAL, LOG, RECORDING].contains(&id)
}

/// The character's settings file, trusted, or why not.
fn load(dir: &Path, instance: &str, name: &str) -> Result<SettingsFile, String> {
    settings_store::load(dir, instance, name).map_err(|why| why.to_string())
}

/// One section of `file`, or why it does not read.
fn section<T: serde::de::DeserializeOwned + Default>(
    file: &SettingsFile,
    name: &str,
) -> Result<T, String> {
    file.section(name)
        .map_err(|why| format!("its {name} section does not read: {why}"))
}

/// The three pages for `name` on `instance`, as its file holds them.
pub(crate) fn pages(dir: &Path, instance: &str, name: &str) -> Vec<Page> {
    let file_name = settings_store::settings_path(dir, instance, name).map_or_else(
        || format!("{instance}_{name}.settings.json"),
        |path| {
            path.strip_prefix(dir)
                .unwrap_or(&path)
                .display()
                .to_string()
        },
    );
    let page = |id: &str, title: &str, takes: &str| Page {
        id: id.to_owned(),
        title: title.to_owned(),
        file: file_name.clone(),
        takes: takes.to_owned(),
        problem: None,
        rows: Vec::new(),
    };
    let mut general = page(GENERAL, "General", "at once");
    let mut log = page(LOG, "Player log", "the next time the character logs in");
    let mut recording = page(
        RECORDING,
        "Recording",
        "the next time the character logs in",
    );
    let rows = load(dir, instance, name).and_then(|file| {
        Ok((
            general_rows(&file)?,
            log_rows(&section(&file, LOG_SECTION)?),
            record_rows(section(&file, RECORD)?),
        ))
    });
    match rows {
        Ok((general_rows, log_rows, record_rows)) => {
            general.rows = general_rows;
            log.rows = log_rows;
            recording.rows = record_rows;
        }
        Err(why) => {
            for page in [&mut general, &mut log, &mut recording] {
                page.problem = Some(format!("Nothing here is changed while {why}"));
            }
        }
    }
    vec![general, log, recording]
}

/// A switch's row.
fn toggle(key: &str, label: &str, help: &str, set: Option<bool>, default: bool) -> Row {
    Row {
        key: key.to_owned(),
        label: label.to_owned(),
        help: help.to_owned(),
        kind: RowKind::Toggle,
        value: Value::On(set.unwrap_or(default)),
        here: set.is_some(),
        from: None,
    }
}

fn general_rows(file: &SettingsFile) -> Result<Vec<Row>, String> {
    let commands: claimant::Settings = section(file, claimant::SECTION)?;
    let sorter: Sorter = section(file, SORTER)?;
    Ok(vec![
        Row {
            key: "symbol".to_owned(),
            label: "Command symbol".to_owned(),
            help: "What begins a line that is Hydra's, not the game's: `;` as in Lich.".to_owned(),
            kind: RowKind::Text,
            value: Value::Text(commands.symbol().to_string()),
            here: commands.symbol.is_some(),
            from: None,
        },
        toggle(
            "sorter",
            "Sort container looks",
            "A container look shown one line per kind of thing in it: ;sorter.",
            sorter.enabled,
            false,
        ),
    ])
}

fn log_rows(log: &LogSettings) -> Vec<Row> {
    let default = Capture::default();
    let named = log
        .feeds
        .keys()
        .filter(|feed| FEEDS.iter().all(|(known, _)| known != feed))
        .map(|feed| (feed.as_str(), "a stream the file names"));
    FEEDS
        .iter()
        .copied()
        .chain(named)
        .map(|(feed, what)| {
            toggle(
                feed,
                &format!("Log {feed}"),
                &format!("Write {what} to the player log."),
                log.feeds.get(feed).copied(),
                default.wants(feed),
            )
        })
        .collect()
}

fn record_rows(record: Record) -> Vec<Row> {
    vec![
        toggle(
            "combat",
            "Record combat",
            "Each fight, to the character's database, for ;combat. Off until turned on.",
            record.combat,
            false,
        ),
        toggle(
            "loot",
            "Record loot",
            "What was found and sold, to the character's database, for ;loot. Off until turned on.",
            record.loot,
            false,
        ),
    ]
}

/// A switch as the menu writes it, or back to its default (`None`).
fn switch(key: &str, to: Option<&str>) -> Result<Option<bool>, String> {
    match to.map(|to| settings::typed(to).as_bool()) {
        None => Ok(None),
        Some(Some(on)) => Ok(Some(on)),
        Some(None) => Err(format!("{key} is on or off.")),
    }
}

/// Set `key` on page `page` of `name`'s file to `to`, as the menu writes it,
/// or back to its default (`None`), and save. What was done.
///
/// # Errors
///
/// Why nothing was changed.
pub(crate) fn change(
    dir: &Path,
    (instance, name): (&str, &str),
    page: &str,
    key: &str,
    to: Option<&str>,
) -> Result<String, String> {
    let mut file =
        load(dir, instance, name).map_err(|why| format!("Nothing was changed: {why}"))?;
    let done = match (page, key) {
        (GENERAL, "symbol") => {
            let mut commands: claimant::Settings = section(&file, claimant::SECTION)?;
            commands.symbol = match to {
                None => None,
                Some(to) => Some(
                    settings::typed(to)
                        .as_str()
                        .filter(|symbol| symbol_ok(symbol))
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            "The command symbol is one mark, such as ; or /: not a letter, a digit or a space."
                                .to_owned()
                        })?,
                ),
            };
            put(&mut file, claimant::SECTION, &commands)?;
            format!("The command symbol is {}.", commands.symbol())
        }
        (GENERAL, "sorter") => {
            let enabled = switch(key, to)?;
            put(&mut file, SORTER, &Sorter { enabled })?;
            format!(
                "Container-look sorting {}.",
                if enabled.unwrap_or(false) {
                    "on"
                } else {
                    "off"
                }
            )
        }
        (LOG, feed) => {
            let mut log: LogSettings = section(&file, LOG_SECTION)?;
            match switch(key, to)? {
                Some(on) => log.feeds.insert(feed.to_owned(), on),
                None => log.feeds.remove(feed),
            };
            put(&mut file, LOG_SECTION, &log)?;
            let on = Capture::default().with(&log).wants(feed);
            format!(
                "The player log {} {feed}, from the next login.",
                if on { "writes" } else { "leaves out" }
            )
        }
        (RECORDING, kind @ ("combat" | "loot")) => {
            let mut record: Record = section(&file, RECORD)?;
            let on = switch(key, to)?;
            if kind == "combat" {
                record.combat = on;
            } else {
                record.loot = on;
            }
            put(&mut file, RECORD, &record)?;
            format!(
                "{} is {}recorded, from the next login.",
                if kind == "combat" { "Combat" } else { "Loot" },
                if on.unwrap_or(false) { "" } else { "not " }
            )
        }
        (page, key) => return Err(format!("The {page} page has no setting {key}.")),
    };
    settings_store::save(dir, &file).map_err(|why| format!("Not saved: {why}"))?;
    Ok(done)
}

/// A symbol a player can type before a command: one mark.
fn symbol_ok(symbol: &str) -> bool {
    let mut chars = symbol.chars();
    matches!((chars.next(), chars.next()), (Some(mark), None) if !mark.is_alphanumeric() && !mark.is_whitespace())
}

/// Lay `value` over `file`'s section `name`.
fn put<T: serde::Serialize>(file: &mut SettingsFile, name: &str, value: &T) -> Result<(), String> {
    file.set_section(name, value)
        .map_err(|why| format!("Nothing was changed: {why}"))
}

/// A character's settings file: the folder it is in, and whose it is.
#[derive(Clone, Debug)]
pub(crate) struct Kept {
    dir: PathBuf,
    instance: &'static str,
    name: String,
}

impl Kept {
    /// The file in `dir` of `character`, as the roster names it
    /// (`GAME:Name`); `None` for a name that is not one, or a game Hydra
    /// does not know.
    pub(crate) fn of(dir: &Path, character: &str) -> Option<Self> {
        let (instance, name) = crate::pages::who(character).ok()?;
        Some(Self {
            dir: dir.to_owned(),
            instance,
            name: name.to_owned(),
        })
    }

    /// [`change`], on this file.
    ///
    /// # Errors
    ///
    /// Why nothing was changed.
    pub(crate) fn change(&self, page: &str, key: &str, to: Option<&str>) -> Result<String, String> {
        change(&self.dir, (self.instance, &self.name), page, key, to)
    }

    /// What a running character takes at once from its file: its command
    /// symbol and whether it sorts container looks. A file that cannot be
    /// trusted changes nothing.
    pub(crate) fn take(&self, handle: &SessionHandle) {
        let Ok(file) = load(&self.dir, self.instance, &self.name) else {
            return;
        };
        if let Ok(commands) = section::<claimant::Settings>(&file, claimant::SECTION)
            && !handle.set_command_symbol(commands.symbol())
        {
            eprintln!(
                "  !! [commands] no command line to give the symbol {} to",
                commands.symbol()
            );
        }
        if let Ok(sorter) = section::<Sorter>(&file, SORTER) {
            handle.sort_containers(sorter.enabled.unwrap_or(false));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Commands;
    use cena_platform::{AnsweringSource, DEFAULT_GAME_CODE, instance};

    fn scratch(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cena-general-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn prime() -> &'static str {
        instance(DEFAULT_GAME_CODE).expect("an instance")
    }

    fn kept(dir: &Path) -> Kept {
        Kept::of(dir, &format!("{DEFAULT_GAME_CODE}:Nisugi")).expect("a roster name")
    }

    fn row<'a>(pages: &'a [Page], page: &str, key: &str) -> Option<&'a Row> {
        pages
            .iter()
            .find(|found| found.id == page)
            .and_then(|found| found.rows.iter().find(|row| row.key == key))
    }

    fn value(pages: &[Page], page: &str, key: &str) -> Option<Value> {
        row(pages, page, key).map(|row| row.value.clone())
    }

    /// With no file, every page shows its settings at their defaults: `;`
    /// for the symbol, sorting and recording off, the text feeds logged and
    /// the readouts not.
    #[test]
    fn with_no_file_every_setting_is_its_default() {
        let dir = scratch("defaults");
        let pages = super::pages(&dir, prime(), "Nisugi");
        let ids: Vec<&str> = pages.iter().map(|page| page.id.as_str()).collect();
        assert_eq!(ids, [GENERAL, LOG, RECORDING]);
        assert_eq!(
            value(&pages, GENERAL, "symbol"),
            Some(Value::Text(";".to_owned()))
        );
        assert_eq!(value(&pages, GENERAL, "sorter"), Some(Value::On(false)));
        assert_eq!(value(&pages, LOG, "main"), Some(Value::On(true)));
        assert_eq!(value(&pages, LOG, "inv"), Some(Value::On(false)));
        assert_eq!(value(&pages, RECORDING, "combat"), Some(Value::On(false)));
        assert_eq!(value(&pages, RECORDING, "loot"), Some(Value::On(false)));
        assert!(
            pages
                .iter()
                .flat_map(|page| &page.rows)
                .all(|row| !row.here)
        );
    }

    /// A change is saved in its section, where the session's own readers
    /// look for it; a running character takes the symbol and the sorter at
    /// once; `None` puts a setting back to its default.
    #[test]
    fn a_change_is_saved_where_its_reader_looks() {
        let dir = scratch("change");
        let kept = kept(&dir);
        assert_eq!(
            kept.change(GENERAL, "symbol", Some("\"/\"")).as_deref(),
            Ok("The command symbol is /.")
        );
        kept.change(GENERAL, "sorter", Some("on")).expect("sorter");
        kept.change(LOG, "inv", Some("on")).expect("a feed");
        kept.change(LOG, "familiar", Some("off")).expect("a stream");
        kept.change(RECORDING, "loot", Some("on")).expect("loot");

        let file = settings_store::load(&dir, prime(), "Nisugi").expect("it reads");
        let commands: claimant::Settings = section(&file, claimant::SECTION).expect("commands");
        assert_eq!(commands.symbol(), '/');
        let log: LogSettings = section(&file, LOG_SECTION).expect("player_log");
        let capture = Capture::default().with(&log);
        assert!(capture.wants("inv") && !capture.wants("familiar"));
        let record: Record = section(&file, RECORD).expect("record");
        assert_eq!((record.combat, record.loot), (None, Some(true)));

        let pages = super::pages(&dir, prime(), "Nisugi");
        assert!(
            row(&pages, LOG, "familiar").is_some_and(|row| row.here),
            "a stream it names"
        );
        kept.change(LOG, "familiar", None).expect("put back");
        let pages = super::pages(&dir, prime(), "Nisugi");
        assert!(
            row(&pages, LOG, "familiar").is_none(),
            "no longer named, it is a stream's default"
        );

        let (source, _) = AnsweringSource::new(b"");
        let session = cena_session::Session::new(source);
        let handle = session.handle();
        let _commands = Commands::install(&handle);
        kept.take(&handle);
        assert_eq!(handle.command_symbol(), Some('/'));
        assert!(handle.sorts_containers());

        kept.change(GENERAL, "symbol", None).expect("put back");
        let pages = super::pages(&dir, prime(), "Nisugi");
        assert!(row(&pages, GENERAL, "symbol").is_some_and(|row| !row.here));
        assert_eq!(
            value(&pages, GENERAL, "symbol"),
            Some(Value::Text(";".to_owned()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What is not of a setting's kind is refused; a file that cannot be
    /// trusted is shown with why, has no rows, and is never written over.
    #[test]
    fn a_bad_value_or_a_broken_file_changes_nothing() {
        let dir = scratch("broken");
        let kept = kept(&dir);
        for (page, key, to) in [
            (GENERAL, "symbol", "\"a\""),
            (GENERAL, "symbol", "\";;\""),
            (GENERAL, "symbol", "\" \""),
            (GENERAL, "sorter", "7"),
            (RECORDING, "combat", "\"yes please\""),
            (GENERAL, "volume", "on"),
        ] {
            assert!(
                kept.change(page, key, Some(to)).is_err(),
                "{page} {key} {to}"
            );
        }
        let path = settings_store::settings_path(&dir, prime(), "Nisugi").expect("a path");
        std::fs::create_dir_all(&dir).expect("made");
        std::fs::write(&path, "{ not json").expect("written");
        let pages = super::pages(&dir, prime(), "Nisugi");
        assert!(
            pages
                .iter()
                .all(|page| page.rows.is_empty() && page.problem.is_some())
        );
        assert!(kept.change(GENERAL, "sorter", Some("on")).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).ok().as_deref(),
            Some("{ not json")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
