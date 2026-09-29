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
use cena_session::player_log::archive::{self, Archive};
use cena_session::player_log::feed::{LogSettings, SECTION as LOG_SECTION};
use cena_session::player_log::{Capture, tap};
use cena_session::player_log::{retention, writer};
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
            log_rows(&section(&file, LOG_SECTION)?, name),
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

fn log_rows(log: &LogSettings, name: &str) -> Vec<Row> {
    let keep_days = log.keep_days.unwrap_or(0);
    let keep = Row {
        key: KEEP.to_owned(),
        label: "Keep logs for (days)".to_owned(),
        help: if keep_days == 0 {
            "0 keeps them forever, and nothing is ever removed.".to_owned()
        } else {
            format!(
                "Days kept, counting today; 0 is forever. {}",
                retention_preview(name, keep_days)
            )
        },
        kind: RowKind::Whole {
            min: 0,
            max: 36_500,
        },
        value: Value::Text(keep_days.to_string()),
        here: log.keep_days.is_some(),
        from: None,
    };
    let archive = Row {
        key: ARCHIVE.to_owned(),
        label: "Archive old days".to_owned(),
        help: format!(
            "Gzip each finished month or week (Eastern, as the game keeps time) into one file. \
             Today's log stays plain text, and ;history and the Log window read archives as \
             they read the rest. {}",
            disk_used(name)
        ),
        kind: RowKind::Choice(
            [
                (Archive::Monthly, "Every month"),
                (Archive::Weekly, "Every week, Sunday to Saturday"),
                (Archive::Off, "Never"),
            ]
            .iter()
            .map(|(choice, called)| (choice.word().to_owned(), (*called).to_owned()))
            .collect(),
        ),
        value: Value::Text(log.archive.unwrap_or_default().word().to_owned()),
        here: log.archive.is_some(),
        from: None,
    };
    let default = Capture::default();
    let named = log
        .feeds
        .keys()
        .filter(|feed| FEEDS.iter().all(|(known, _)| known != feed))
        .map(|feed| (feed.as_str(), "a stream the file names"));
    [archive, keep]
        .into_iter()
        .chain(FEEDS.iter().copied().chain(named).map(|(feed, what)| {
            toggle(
                feed,
                &format!("Log {feed}"),
                &format!("Write {what} to the player log."),
                log.feeds.get(feed).copied(),
                default.wants(feed),
            )
        }))
        .collect()
}

/// The *Player log* page's key for how old days are kept, beside the feeds.
const ARCHIVE: &str = "archive";

/// The *Player log* page's key for how many days are kept.
const KEEP: &str = "keep_days";

/// `login`'s (`GAME:Name`) player log settings, as its settings file says:
/// how closed days are archived and how many are kept. The defaults when it
/// says nothing or cannot be read, which is also what the page shows then.
pub(crate) fn log_settings(dir: &Path, login: &str) -> LogSettings {
    crate::pages::who(login)
        .ok()
        .and_then(|(instance, name)| load(dir, instance, name).ok())
        .and_then(|file| section::<LogSettings>(&file, LOG_SECTION).ok())
        .unwrap_or_default()
}

/// What `name`'s player log takes on disk, as the page says it.
fn disk_used(name: &str) -> String {
    use cena_ui::settings::size;
    match archive::usage(&writer::root(), name) {
        Ok(usage) if usage.days == 0 => "Nothing is kept yet.".to_owned(),
        Ok(usage) => format!(
            "Kept now: {} days, {} in all ({} plain, {} archived).",
            usage.days,
            size(usage.total()),
            size(usage.plain),
            size(usage.archived)
        ),
        Err(why) => format!("The log could not be read: {why}"),
    }
}

/// What keeping `keep_days` days of `name`'s log would remove at the next
/// login, as the player is told it before it happens (`plan/25` §7).
fn retention_preview(name: &str, keep_days: u32) -> String {
    let today = cena_platform::date_dir();
    retention::doomed(&writer::root(), name, keep_days, &today).map_or_else(
        |why| format!("The log could not be read: {why}"),
        |doomed| retention::preview(&doomed),
    )
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
    // Read, changed and written with no other change to the file between
    // (`cena_session::store::changing`; the crate review of 2026-09-28, R5). A name no file can have locks
    // nothing, and `load` refuses it.
    let path = settings_store::settings_path(dir, instance, name).unwrap_or_default();
    cena_session::store::changing(&path, || changed(dir, (instance, name), page, key, to))
}

/// [`change`], with the file's lock held.
fn changed(
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
        (LOG, key) => {
            let mut log: LogSettings = section(&file, LOG_SECTION)?;
            let done = log_change(&mut log, name, key, to)?;
            put(&mut file, LOG_SECTION, &log)?;
            done
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

/// A change on the *Player log* page, made to `log`: how old days are
/// archived, how many are kept, or one feed. What was done.
fn log_change(
    log: &mut LogSettings,
    name: &str,
    key: &str,
    to: Option<&str>,
) -> Result<String, String> {
    Ok(match key {
        ARCHIVE => {
            log.archive = match to {
                None => None,
                Some(word) => Some(Archive::from_word(word).ok_or_else(|| {
                    format!("`{word}` is not a choice: say monthly, weekly or off.")
                })?),
            };
            match log.archive.unwrap_or_default() {
                Archive::Off => "Old days stay plain text.".to_owned(),
                choice => format!(
                    "Old days are archived {}, from the next login.",
                    choice.word()
                ),
            }
        }
        KEEP => {
            log.keep_days = match to {
                None => None,
                Some(days) => Some(
                    settings::typed(days)
                        .as_integer()
                        .and_then(|days| u32::try_from(days).ok())
                        .filter(|days| *days <= 36_500)
                        .ok_or_else(|| {
                            format!("`{days}` is not a number of days: say 0 for forever.")
                        })?,
                ),
            };
            match log.keep_days.unwrap_or(0) {
                0 => "The player log is kept forever.".to_owned(),
                days => format!(
                    "The player log keeps {days} days. {}",
                    retention_preview(name, days)
                ),
            }
        }
        feed => {
            match switch(key, to)? {
                Some(on) => log.feeds.insert(feed.to_owned(), on),
                None => log.feeds.remove(feed),
            };
            let on = Capture::default().with(log).wants(feed);
            format!(
                "The player log {} {feed}, from the next login.",
                if on { "writes" } else { "leaves out" }
            )
        }
    })
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
            Some(Value::Text(".".to_owned()))
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
            Some(Value::Text(".".to_owned()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The archive choice: monthly until changed, saved where the login's
    /// sweep reads it, a word that is not a choice refused, and back to
    /// monthly when put back.
    #[test]
    fn the_archive_choice_is_monthly_until_changed_and_read_at_login() {
        let dir = scratch("archive");
        let login = format!("{DEFAULT_GAME_CODE}:Nisugi");
        let pages = super::pages(&dir, prime(), "Nisugi");
        assert_eq!(
            value(&pages, LOG, ARCHIVE),
            Some(Value::Text("monthly".to_owned()))
        );
        assert_eq!(
            log_settings(&dir, &login).archive.unwrap_or_default(),
            Archive::Monthly
        );

        let kept = kept(&dir);
        kept.change(LOG, ARCHIVE, Some("Weekly")).expect("a choice");
        assert_eq!(
            log_settings(&dir, &login).archive.unwrap_or_default(),
            Archive::Weekly
        );
        assert!(row(&super::pages(&dir, prime(), "Nisugi"), LOG, ARCHIVE).is_some_and(|r| r.here));
        assert!(kept.change(LOG, ARCHIVE, Some("yearly")).is_err());
        assert_eq!(
            log_settings(&dir, &login).archive.unwrap_or_default(),
            Archive::Weekly,
            "unchanged"
        );

        kept.change(LOG, ARCHIVE, None).expect("put back");
        assert_eq!(
            log_settings(&dir, &login).archive.unwrap_or_default(),
            Archive::Monthly
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Days kept: forever (0) until changed, a number saved where the
    /// login's pruning reads it, and what is not a number of days refused.
    #[test]
    fn days_kept_are_forever_until_changed() {
        let dir = scratch("keep");
        let login = format!("{DEFAULT_GAME_CODE}:Nisugi");
        let pages = super::pages(&dir, prime(), "Nisugi");
        assert_eq!(value(&pages, LOG, KEEP), Some(Value::Text("0".to_owned())));
        assert!(row(&pages, LOG, KEEP).is_some_and(|r| r.help.contains("forever")));

        let kept = kept(&dir);
        let said = kept.change(LOG, KEEP, Some("30")).expect("a number");
        assert!(said.starts_with("The player log keeps 30 days."), "{said}");
        assert_eq!(log_settings(&dir, &login).keep_days, Some(30));
        for bad in ["-1", "forever", "99999"] {
            assert!(kept.change(LOG, KEEP, Some(bad)).is_err(), "{bad}");
        }
        assert_eq!(log_settings(&dir, &login).keep_days, Some(30), "unchanged");
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

    /// A change made while another is being made waits for it, and both
    /// are kept (the crate review of 2026-09-28, R5).
    #[test]
    fn a_change_made_meanwhile_waits_and_both_are_kept() {
        let dir = scratch("meanwhile");
        let kept = kept(&dir);
        kept.change(GENERAL, "sorter", Some("on")).expect("a file");
        let path = settings_store::settings_path(&dir, prime(), "Nisugi").expect("a path");
        let other = cena_session::store::changing(&path, || {
            let mut file = load(&dir, prime(), "Nisugi").expect("it reads");
            let other = {
                let kept = kept.clone();
                std::thread::spawn(move || kept.change(RECORDING, "loot", Some("on")))
            };
            std::thread::sleep(std::time::Duration::from_millis(200));
            let mut commands: claimant::Settings =
                section(&file, claimant::SECTION).expect("commands");
            commands.symbol = Some("/".to_owned());
            put(&mut file, claimant::SECTION, &commands).expect("put");
            settings_store::save(&dir, &file).expect("saved");
            other
        });
        other.join().expect("its thread").expect("changed");
        let file = load(&dir, prime(), "Nisugi").expect("it reads");
        let commands: claimant::Settings = section(&file, claimant::SECTION).expect("commands");
        assert_eq!(commands.symbol(), '/', "the change made first");
        let record: Record = section(&file, RECORD).expect("record");
        assert_eq!(record.loot, Some(true), "the change made meanwhile");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
