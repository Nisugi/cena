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

/// Give the session its crit tables, and -- when [`recording`] -- its combat
/// recorder and loot ledger, both writing one database per character.
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
    if !recording() {
        eprintln!("[record] off (--record turns it on)");
        return (session, Vec::new());
    }
    let dir = cena_session::character_store::data_dir();
    let mut flushes = Vec::new();
    let (session, path) =
        match cena_session::combat_recorder::worker::open_live(&dir, game, character) {
            Ok((recorder, flush, path)) => {
                eprintln!("[record] {}", path.display());
                flushes.push(flush);
                (session.with_combat_recorder(recorder), Some(path))
            }
            Err(e) => {
                eprintln!("[record] combat recorder DISABLED -- {e}");
                (session, None)
            }
        };
    // The ledger's tables go in the same file, so one character's hunts and
    // loot are one database (plan/34 section 4).
    let Some(path) = path else {
        return (session, flushes);
    };
    match cena_session::ledger::worker::open_live(&path, character) {
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

/// Whether this run records combat and loot to the character's database.
///
/// **On by default in a debug build, off in a release build**, and either
/// way `--record` or `--no-record` on the command line decides (author,
/// 2026-09-24: *"on by default during testing, off by default for release"*).
/// Nothing at runtime reads what the recorders write -- the hunt, the loot
/// planner and every behavior read the model -- so a run without them is the
/// same run with no reports afterwards.
pub(crate) fn recording() -> bool {
    recording_in(std::env::args().skip(1))
}

/// [`recording`], over any argument list, so it can be tested. The last
/// flag given wins.
fn recording_in(args: impl IntoIterator<Item = String>) -> bool {
    let mut on = cfg!(debug_assertions);
    for arg in args {
        match arg.as_str() {
            "--record" => on = true,
            "--no-record" => on = false,
            _ => {}
        }
    }
    on
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
    use super::recording_in;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_flags_decide_and_the_last_one_wins() {
        assert!(recording_in(args("--record")));
        assert!(!recording_in(args("--no-record")));
        assert!(recording_in(args("--no-record --character X --record")));
        assert!(!recording_in(args("--record --no-record")));
    }

    #[test]
    fn without_a_flag_the_build_decides() {
        assert_eq!(
            recording_in(args("--character X")),
            cfg!(debug_assertions),
            "debug builds record, release builds do not"
        );
    }
}
