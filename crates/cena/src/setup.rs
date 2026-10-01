//! What every session is given before it runs: its wire log, its stores, its
//! combat recorder and its player log -- and how each is flushed at the end.
//!
//! Moved down out of `main.rs` under Rule 4.1 when the `--character` path
//! (`plan/29`, `play.rs`) needed the same setup for every session it starts.

use std::io;
use std::sync::Arc;

use cena_platform::{Redactions, SessionSink};
use cena_session::SupervisedSession;

use crate::connector::LiveConnector;

/// Give `session` its wire log, stores, combat recorder and player log. The
/// session table builds the session itself (`cena_host::Host::add`).
///
/// Logging is ON by default. Author's call, 2026-09-18: "we want it on by
/// default during our dev work. That way there's always a log for you."
/// Opt-OUT, not opt-in: a session that fails in an interesting way is exactly
/// the one nobody remembered to enable logging for.
pub(crate) fn attach(
    session: SupervisedSession<LiveConnector>,
    character: &str,
    game: &str,
    account: &str,
) -> (
    SupervisedSession<LiveConnector>,
    Vec<std::thread::JoinHandle<()>>,
    tokio::task::JoinHandle<u64>,
) {
    let session = match open_log(character, account) {
        Ok(sink) => {
            eprintln!(
                "[log] {}
[log] {}",
                sink.bytes_path().display(),
                sink.events_path().display()
            );
            session.with_sink(sink)
        }
        Err(e) => {
            // A log that cannot be opened must not stop a session. Say so
            // loudly -- silence here reads as "logging worked".
            eprintln!("[log] DISABLED -- could not open a log file: {e}");
            session
        }
    };
    // **What the character learns, and what the game teaches about its
    // menus, kept across logins.** Missing until 2026-09-21: the supervised
    // session could not be given either store, so no live run ever wrote one.
    // One directory for both, beside the settings and travel files.
    let data = cena_session::character_store::data_dir();
    eprintln!("[data] {}", data.display());
    let session = session
        .with_character_store(data.clone())
        .with_menu_store(data);
    let (session, record_flush) = attach_recorders(session, game, character);
    let (session, player_flush) = attach_player_log(session, character);
    archive_player_log(character, game);
    (session, record_flush, player_flush)
}

/// Give the session its player log (`plan/25`): what the player saw and sent,
/// per character per day, under `<log_dir>/player/`.
///
/// Nothing here can fail up front -- the writer creates its directory on the
/// first line, and a failure then is counted rather than fatal. The count
/// comes back through the handle, so the exit can say the history has a hole.
///
/// **No account redaction, unlike the wire log, and deliberately.** An account
/// name is often the character's name, and this log is display text: redacting
/// it would replace the character's own name on every line that mentions it.
/// The credentials that reach the WIRE (password hash, launch key) are never
/// display text and never pass through the command queue.
fn attach_player_log(
    session: SupervisedSession<LiveConnector>,
    character: &str,
) -> (
    SupervisedSession<LiveConnector>,
    tokio::task::JoinHandle<u64>,
) {
    let (log, sink) = cena_session::PlayerLog::new();
    let writer =
        cena_session::PlayerWriter::new(cena_session::player_log::writer::root(), character);
    eprintln!("[player log] {}", writer.dir().display());
    (
        session.with_player_log(
            log,
            cena_session::player_log::Capture::default(),
            Some(cena_session::character_store::data_dir()),
        ),
        tokio::spawn(writer.run_reporting(sink)),
    )
}

/// Archive the player log's finished months or weeks, then remove what is
/// older than the days kept, as the character's settings file chooses
/// (`plan/25` steps 4 and 5), on a blocking task: a month of
/// day-files is tens of megabytes to compress, and the session starts
/// without waiting for it.
///
/// Once per login, which is when a change of choice takes effect (the
/// *Player log* page says so). A period that ends while a character stays
/// logged in is archived at the next login. A failure is said and changes
/// nothing: the day-files stay.
fn archive_player_log(character: &str, game: &str) {
    let data = cena_session::character_store::data_dir();
    // **The player log's folder is the name alone** (`writer::dir`), so a
    // character of the same name on another instance shares it, and this
    // one's choices would archive and delete the other's days (the crate
    // review of 2026-10-01, SE-C-2). Until the folder carries the instance,
    // a shared folder is left as it is.
    if let Some(other) = shares_its_log(&data, game, character) {
        eprintln!(
            "[player log] not archived or pruned: {character} on {other} keeps its days in \
             the same folder, and this character's choices would remove them"
        );
        return;
    }
    let log = crate::general::log_settings(&data, &format!("{game}:{character}"));
    let character = character.to_owned();
    tokio::task::spawn_blocking(move || {
        use cena_session::player_log::{archive, retention, writer};
        let root = writer::root();
        let now = cena_platform::eastern::now();
        match archive::sweep(&root, &character, log.archive.unwrap_or_default(), now) {
            Ok(swept) if swept.files > 0 => eprintln!(
                "[player log] archived {} day-files into {}",
                swept.files,
                swept.archives.join(", ")
            ),
            Ok(_) => {}
            Err(e) => eprintln!("[player log] not archived, the day-files are kept: {e}"),
        }
        // After archiving, so a month is judged whole as its archive
        // (`plan/25` step 5): what the Player log page and `;history`
        // previewed is what goes.
        let keep_days = log.keep_days.unwrap_or(0);
        match retention::prune(&root, &character, keep_days, &cena_platform::date_dir()) {
            Ok(gone) if !gone.is_empty() => eprintln!(
                "[player log] kept {keep_days} days: removed {} files, the days up to {}",
                gone.len(),
                gone.last().map_or("", |g| g.newest.as_str())
            ),
            Ok(_) => {}
            Err(e) => eprintln!("[player log] old days not removed: {e}"),
        }
    });
}

/// The other instance a character of `name` has played on, by its
/// character store file, when `game`'s is not the only one: their player
/// logs share a folder.
fn shares_its_log(data: &std::path::Path, game: &str, name: &str) -> Option<&'static str> {
    let this = cena_platform::instance(game)?;
    cena_platform::INSTANCES
        .iter()
        .map(|(_, instance)| *instance)
        .filter(|instance| *instance != this)
        .find(|instance| {
            cena_session::store::character_path(data, instance, name, ".json")
                .is_some_and(|path| path.exists())
        })
}

/// How long a stop waits for a session's logs to finish writing.
///
/// **A bound, because the wait can be for ever.** The player-log writer ends
/// only when every `PlayerLog` has dropped, and every clone of the session's
/// command handle holds one -- the web page, the `;` command line, travel. In
/// the one-character run those all go with the process. On the hub, a
/// character is quit while the rest keep running, and the unbounded wait hung
/// the quit, and every hub request after it (author's live run, 2026-09-24).
/// What was written is on disk either way; this only says whether it closed.
const FLUSH_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Wait, a bounded while, for the player log's last flush, and say so if
/// lines were lost.
pub(crate) async fn flush_player_log(flush: tokio::task::JoinHandle<u64>) {
    match tokio::time::timeout(FLUSH_WAIT, flush).await {
        Ok(Ok(0)) => {}
        Ok(Ok(lost)) => eprintln!("[player log] {lost} lines were NOT recorded; the log has holes"),
        Ok(Err(_)) => {
            eprintln!("[player log] the writer task panicked; the log's tail may be missing");
        }
        Err(_) => eprintln!(
            "[player log] still open while something holds the character's handle; \
             what was written is on disk"
        ),
    }
}

/// Give the session its crit tables, and its combat recorder and loot
/// ledger as the character's settings file turns each on ([`recording`]),
/// both writing one database per character.
///
/// All are optional to a working session and none may stop one, which is
/// `open_log`'s rule: say loudly what is missing, then carry on. Without the
/// tables every hit records no crit; without the database nothing records.
/// The returned threads are the recorders' flushes (`flush_records`).
fn attach_recorders(
    session: SupervisedSession<LiveConnector>,
    game: &str,
    character: &str,
) -> (
    SupervisedSession<LiveConnector>,
    Vec<std::thread::JoinHandle<()>>,
) {
    let session = match cena_session::CritTables::load() {
        Ok(tables) => session.with_crit_tables(Arc::new(tables)),
        Err(e) => {
            eprintln!("[combat] crit tables DISABLED -- {e}");
            session
        }
    };
    let dir = cena_session::character_store::data_dir();
    let record = recording(&dir, game, character);
    let (combat, loot) = (record.combat.unwrap_or(false), record.loot.unwrap_or(false));
    if !combat && !loot {
        eprintln!("[record] off (Settings, Recording turns it on)");
        return (session, Vec::new());
    }
    // The ledger's tables go in the combat recorder's file, so one
    // character's hunts and loot are one database (plan/34 section 4).
    let path = match cena_session::combat_recorder::worker::database_path(&dir, game, character) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("[record] DISABLED -- {e}");
            return (session, Vec::new());
        }
    };
    eprintln!("[record] {}", path.display());
    let mut flushes = Vec::new();
    let session = if combat {
        match cena_session::combat_recorder::worker::open_live(&dir, game, character) {
            Ok((recorder, flush, _)) => {
                flushes.push(flush);
                session.with_combat_recorder(recorder)
            }
            Err(e) => {
                eprintln!("[record] combat recorder DISABLED -- {e}");
                session
            }
        }
    } else {
        session
    };
    if !loot {
        return (session, flushes);
    }
    let opened = std::fs::create_dir_all(&dir)
        .and_then(|()| cena_session::ledger::worker::open_live(&path, character));
    match opened {
        Ok((ledger, flush)) => {
            flushes.push(flush);
            (session.with_ledger(ledger), flushes)
        }
        Err(e) => {
            eprintln!("[record] loot ledger DISABLED -- {e}");
            (session, flushes)
        }
    }
}

/// The `record` section of a character's settings file: what goes to its
/// database, each kind its own switch. The author, 2026-09-27: *"recording
/// should be a setting, it should be per thing, combat, loot, whatever else
/// we decide to record"*, and *"off by default, turn on stay on until turned
/// off"* (`plan/50` §6 item 5). It replaced `--record` and `--no-record`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Record {
    /// Each fight, for `;combat`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) combat: Option<bool>,
    /// What was found and sold, for `;loot`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) loot: Option<bool>,
}

impl Record {
    /// Whether both kinds are recorded: what an agent is told the database
    /// holds (`cena_agent`'s `recording`). One kind alone is `false`, the
    /// safe word: an empty answer may only mean nothing was kept.
    pub(crate) fn everything(self) -> bool {
        self.combat.unwrap_or(false) && self.loot.unwrap_or(false)
    }
}

/// The name of [`Record`]'s section.
pub(crate) const RECORD: &str = "record";

/// What to say when `args` still name `--record` or `--no-record`, which
/// decide nothing now: there is no argument parser to refuse them
/// (`plan/50` §1b), so without this they would be ignored without a word.
pub(crate) fn retired(args: impl IntoIterator<Item = String>) -> Option<&'static str> {
    args.into_iter()
        .any(|arg| arg == "--record" || arg == "--no-record")
        .then_some(
            "[record] --record and --no-record are retired: recording is each character's \
             own setting now, off until turned on (Settings, Recording).",
        )
}

/// What `character` on `game` records, as its settings file says: nothing
/// until a kind is turned on. Nothing at runtime reads what the recorders
/// write -- the hunt, the loot planner and every behavior read the model --
/// so a run without them is the same run with no reports afterwards. A file
/// that cannot be trusted records nothing, and says why.
pub(crate) fn recording(dir: &std::path::Path, game: &str, character: &str) -> Record {
    let Some(instance) = cena_platform::instance(game) else {
        return Record::default();
    };
    let read = cena_session::settings_store::load(dir, instance, character)
        .map_err(|why| why.to_string())
        .and_then(|file| {
            file.section::<Record>(RECORD)
                .map_err(|why| format!("its {RECORD} section does not read: {why}"))
        });
    read.unwrap_or_else(|why| {
        eprintln!("[record] off: the settings file: {why}");
        Record::default()
    })
}

/// Wait, a bounded while ([`FLUSH_WAIT`]), for each recorder to write what
/// is queued and close.
///
/// A recorder's thread ends when the last handle drops. Without this wait the
/// process can exit between the last chunk and its commit.
///
/// The joins run on a detached thread of their own: a blocking `join()` here
/// held an async worker, and `spawn_blocking` would hold the runtime's
/// shutdown instead, for as long as a recorder never ends.
pub(crate) async fn flush_records(flushes: Vec<std::thread::JoinHandle<()>>) {
    if flushes.is_empty() {
        return;
    }
    let (joined, done) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = joined.send(flushes.into_iter().all(|f| f.join().is_ok()));
    });
    match tokio::time::timeout(FLUSH_WAIT, done).await {
        Ok(Ok(true)) => {}
        Ok(Ok(false)) => {
            eprintln!("[record] a recorder thread panicked; the last hunt may be open");
        }
        Ok(Err(_)) | Err(_) => eprintln!(
            "[record] still recording while something holds the character's handle; \
             its hunt closes when that ends"
        ),
    }
}

/// Open this session's log, with the credentials registered for redaction.
///
/// Returns the sink rather than storing it: the session owns it, one per
/// session, no process-global logger (`plan/05` Rule 5.2).
///
/// # The launch key is NOT registered here any more
///
/// It used to be, because the log was opened after the login and there was
/// exactly one key. A supervised session has **one key per generation**
/// (`plan/10` §4.6: the SGE connection is *"strictly single-use per auth"*),
/// and the log is opened *before* the first login -- so the keys arrive later
/// and keep arriving.
///
/// `Connector::take_secrets` is that seam: the supervisor drains it after every
/// `connect` and calls [`SessionSink::redact_key`] before a byte of the new
/// connection is written. Registering a key here would cover the first
/// connection and silently miss every reconnect, which is worse than not
/// pretending to.
///
/// # The ACCOUNT is registered here, and used not to be
///
/// `Redactions::account` existed with **no production caller** (review finding
/// PL-5), so the account name reached the log unredacted wherever the wire
/// carried it -- and it does carry it: every character code is
/// `W_<ACCOUNT>_<SLOT>` (`plan/10` §4.6).
///
/// Unlike the key it is known before the first byte, so it belongs at creation
/// rather than per generation. It does not change across reconnects.
///
/// The holder's REAL NAME is still not registered: it arrives in the `A`
/// response and `authenticate` discards it, so there is nothing to register
/// from. That remains owed, and is the last piece of the redaction set that is
/// known to the protocol but not to the sink.
fn open_log(character: &str, account: &str) -> io::Result<SessionSink> {
    // Filled further per generation by the supervisor, which registers each
    // connection's launch key before a byte of it is written.
    let mut redactions = Redactions::new();
    redactions.account(account);
    let dir = cena_platform::log_dir().join(cena_platform::date_dir());
    SessionSink::create(&dir, character, &cena_platform::file_stamp(), redactions)
}

#[cfg(test)]
mod tests {
    use super::{RECORD, Record, recording, retired};
    use cena_platform::{DEFAULT_GAME_CODE, instance};
    use cena_session::settings_store::{self, SettingsFile};

    /// Nothing is recorded until a kind is turned on in the character's
    /// settings file, and each kind is turned on by itself; a file that
    /// cannot be trusted records nothing.
    #[test]
    fn each_kind_records_once_turned_on() {
        let dir = std::env::temp_dir().join(format!("cena-record-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let game = DEFAULT_GAME_CODE;
        assert_eq!(recording(&dir, game, "Nisugi"), Record::default());

        let prime = instance(game).expect("an instance");
        let mut file = SettingsFile::new(prime, "Nisugi");
        let loot = Record {
            combat: None,
            loot: Some(true),
        };
        file.set_section(RECORD, &loot).expect("a section");
        settings_store::save(&dir, &file).expect("saved");
        assert_eq!(recording(&dir, game, "nisugi"), loot, "by name, any case");

        let path = settings_store::settings_path(&dir, prime, "Nisugi").expect("a path");
        std::fs::write(&path, "{ not json").expect("written");
        assert_eq!(recording(&dir, game, "Nisugi"), Record::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The crate review of 2026-10-01, SE-C-2: the player log's folder is the
    /// name alone, so a same-named character on another instance shares it.
    #[test]
    fn a_name_on_another_instance_shares_the_log_folder() {
        let dir = std::env::temp_dir().join(format!("cena-shared-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("made");
        assert_eq!(super::shares_its_log(&dir, "GS3", "Nisugi"), None);
        let prime =
            cena_session::store::character_path(&dir, "Prime", "Nisugi", ".json").expect("a path");
        std::fs::write(prime, "{}").expect("written");
        assert_eq!(
            super::shares_its_log(&dir, "GS3", "Nisugi"),
            None,
            "its own"
        );
        let test =
            cena_session::store::character_path(&dir, "Test", "Nisugi", ".json").expect("a path");
        std::fs::write(test, "{}").expect("written");
        assert_eq!(super::shares_its_log(&dir, "GS3", "Nisugi"), Some("Test"));
        assert_eq!(super::shares_its_log(&dir, "GST", "nisugi"), Some("Prime"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The retired flags are said to be retired, and nothing else is.
    #[test]
    fn the_retired_flags_are_said() {
        let args = |line: &str| {
            line.split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        assert!(retired(args("--character X --record")).is_some());
        assert!(retired(args("--no-record")).is_some());
        assert_eq!(retired(args("--character X --headless")), None);
    }
}
