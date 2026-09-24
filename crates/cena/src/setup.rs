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

/// Build the session, attaching a log unless one cannot be opened.
///
/// Split from `main` under `plan/05` Rule 4.1 -- move code down, do not raise
/// the cap -- when clippy caught `main` at 112 lines against a 100 limit.
pub(crate) fn open_session(
    connector: LiveConnector,
) -> (
    SupervisedSession<LiveConnector>,
    cena_session::SessionHandle,
    Option<std::thread::JoinHandle<()>>,
    tokio::task::JoinHandle<u64>,
) {
    let character = connector.character().to_owned();
    let game = connector.game_code().to_owned();
    let account = connector.account_for_redaction().to_owned();
    // Logging is ON by default. Author's call, 2026-09-18: "we want it on by
    // default during our dev work. That way there's always a log for you."
    //
    // Opt-OUT, not opt-in: a session that fails in an interesting way is
    // exactly the one nobody remembered to enable logging for.
    let (session, handle) = SupervisedSession::new(connector);
    let (session, combat_flush, player_flush) = attach(session, &character, &game, &account);
    (session, handle, combat_flush, player_flush)
}

/// Give `session` everything [`open_session`] does, for a session built
/// elsewhere -- the session table builds its own (`cena_host::Host::add`).
pub(crate) fn attach(
    session: SupervisedSession<LiveConnector>,
    character: &str,
    game: &str,
    account: &str,
) -> (
    SupervisedSession<LiveConnector>,
    Option<std::thread::JoinHandle<()>>,
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
    let (session, combat_flush) = attach_combat(session, game, character);
    let (session, player_flush) = attach_player_log(session, character);
    (session, combat_flush, player_flush)
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

/// Wait for the player log's last flush, and say so if lines were lost.
///
/// The writer ends when the last `PlayerLog` drops, which the supervisor's
/// return just did.
pub(crate) async fn flush_player_log(flush: tokio::task::JoinHandle<u64>) {
    match flush.await {
        Ok(0) => {}
        Ok(lost) => eprintln!("[player log] {lost} lines were NOT recorded; the log has holes"),
        Err(_) => eprintln!("[player log] the writer task panicked; the log's tail may be missing"),
    }
}

/// Give the session its crit tables and its combat database.
///
/// Both are optional to a working session and neither may stop one, which is
/// `open_log`'s rule: say loudly what is missing, then carry on. Without the
/// tables every hit records no crit; without the database nothing records.
fn attach_combat(
    session: SupervisedSession<LiveConnector>,
    game: &str,
    character: &str,
) -> (
    SupervisedSession<LiveConnector>,
    Option<std::thread::JoinHandle<()>>,
) {
    let session = match cena_session::CritTables::load() {
        Ok(tables) => session.with_crit_tables(Arc::new(tables)),
        Err(e) => {
            eprintln!("[combat] crit tables DISABLED -- {e}");
            session
        }
    };
    let dir = cena_session::character_store::data_dir();
    match cena_session::combat_recorder::worker::open_live(&dir, game, character) {
        Ok((recorder, flush, path)) => {
            eprintln!("[combat] {}", path.display());
            (session.with_combat_recorder(recorder), Some(flush))
        }
        Err(e) => {
            eprintln!("[combat] recorder DISABLED -- {e}");
            (session, None)
        }
    }
}

/// Wait for the recorder to write what is queued and close its hunt.
///
/// Its thread ends when the last handle drops, which the supervisor's return
/// just did. Without this wait the process can exit between the last chunk
/// and its commit.
pub(crate) fn flush_combat(flush: Option<std::thread::JoinHandle<()>>) {
    if let Some(flush) = flush
        && flush.join().is_err()
    {
        eprintln!("[combat] the recorder thread panicked; the last hunt may be open");
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
