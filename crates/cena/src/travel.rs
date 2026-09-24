//! Travel, wired to this binary: load the map, build the desk, and give it
//! the lines the player types that begin with the command symbol.
//!
//! **Nothing here decides what a command means.** The symbol and the claiming
//! are `cena_session::command::claimant`; the commands and what they do are
//! `cena_behavior::travel` (`command.rs`, `desk.rs`). This file is the join,
//! which is the binary's job and no crate's.
//!
//! # BUILT, NOT RUN
//!
//! Like everything in this binary that reaches the game (`main.rs`'s module
//! docs), this has been compiled and never executed: only the author runs the
//! binary (`CLAUDE.md`, Credentials). Everything it calls is tested against a
//! scripted game in `cena-behavior`; what is untested is the wiring here.
//!
//! # Why a mirror
//!
//! A behavior has no live read of the model: it is handed a snapshot and a
//! subscription and keeps its own `GameState` (`plan/24`, 4a's finding). The
//! snapshot taken before login is empty, and the login burst overflows the
//! event ring if nobody is reading it -- MEASURED on the first live session,
//! 99 events dropped (`CLAUDE.md`). So a task reads from the first moment and
//! folds every frame, and hands its state and its subscription over together,
//! with no gap between the two for an event to fall into.
//!
//! The desk subscribes afresh for every command after that
//! (`SessionObserver::subscribe`), which needs no mirror. The mirror is for
//! the **first** command after a login, whose state would otherwise be a
//! snapshot of a character the game has not described yet.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cena_behavior::travel::{Command, Desk, Map, Travelled, parse_command, read_map};
use cena_session::command::claimant::{self, Claimed, Runner};
use cena_session::{
    AuthorityToken, Event, Notice, NoticeKind, SessionHandle, SessionObserver, Snapshot,
};
use tokio::sync::broadcast::{Receiver, error::RecvError};
use tokio_util::sync::CancellationToken;

/// Where the combined map is. No default: a wrong map is worse than none.
///
/// ```text
/// $env:CENA_MAP = "E:\Gemstone\data\cena_data\gs.map"
/// ```
pub(crate) const MAP_ENV: &str = "CENA_MAP";

/// `--first south`: a command to send, as the player, before anything else --
/// so one run can be "log in, move south, then walk to the bank" (author,
/// 2026-09-21). The first one wins.
pub(crate) fn first_command<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_ref();
        if arg == "--first" {
            return args.next().map(|command| command.as_ref().to_owned());
        }
        if let Some(command) = arg.strip_prefix("--first=") {
            return Some(command.to_owned());
        }
    }
    None
}

/// Keep a `GameState` current from `joined` until `hand_over` is cancelled,
/// then give back the state and the **same** subscription. See the module
/// docs for why.
pub(crate) async fn mirror(
    joined: (Snapshot, Receiver<Event>),
    hand_over: CancellationToken,
) -> (Snapshot, Receiver<Event>) {
    let (mut snapshot, mut events) = joined;
    let mut restored = false;
    loop {
        let event = tokio::select! {
            biased;
            () = hand_over.cancelled() => return (snapshot, events),
            event = events.recv() => event,
        };
        match event {
            Ok(Event::Frame(frame)) => {
                snapshot.state.apply(&frame);
                restored = restored || restore_stored(&mut snapshot.state);
            }
            Ok(_) => {}
            Err(RecvError::Lagged(missed)) => eprintln!(
                "  !! [travel] {missed} events dropped before the walker could read them: \
                 what it knows of the character may be stale"
            ),
            Err(RecvError::Closed) => return (snapshot, events),
        }
    }
}

/// What the character store knows -- skills, society, citizenship -- put into
/// the mirror's state, **when the session itself does it**: the moment the
/// game has said who this is (`actor/ending.rs`, `load_character`). The
/// session restores into its own state and publishes no frame for it, so a
/// mirror that only folded frames would never learn a skill, and every exit
/// priced on one would read "not known yet". Frames after this overwrite it
/// with what is fresher, exactly as they do there. `true` once it has been
/// tried, whatever came of it.
fn restore_stored(state: &mut cena_session::GameState) -> bool {
    let character = &state.character;
    let (Some(instance), Some(name)) = (character.instance.clone(), character.name.clone()) else {
        return false;
    };
    let dir = cena_session::character_store::data_dir();
    match cena_session::character_store::load(&dir, &instance, &name) {
        Ok(stored) => {
            let took = stored.restore_into(&mut state.character);
            eprintln!("[travel] character store: restored={took}");
        }
        Err(e) => eprintln!("[travel] character store: {e} -- the walker knows only this login"),
    }
    true
}

/// What `main` does once the session is up: send `--first`, take the state
/// back from the mirror, and open the travel desk so `;go2` works.
///
/// Here and not in `main` because `main` is at clippy's line limit, and the
/// rule is to move code down.
pub(crate) async fn after_login(
    mirror: tokio::task::JoinHandle<(Snapshot, Receiver<Event>)>,
    hand_over: &CancellationToken,
    handle: &SessionHandle,
    observer: SessionObserver,
) {
    // Sent while the mirror is still reading, so it sees where this lands.
    if let Some(first) = first_command(std::env::args().skip(1)) {
        eprintln!("[travel] first: {first}");
        let outcome = crate::run::send_manual(handle, &first).await;
        eprintln!("[travel] first: {outcome:?}");
    }
    hand_over.cancel();
    match mirror.await {
        Ok(joined) => open_desk(handle, observer, joined),
        Err(e) => eprintln!("[travel] the mirror task failed: {e}"),
    }
}

/// Load the map and take the player's travel commands from now on.
fn open_desk(
    handle: &SessionHandle,
    observer: SessionObserver,
    joined: (Snapshot, Receiver<Event>),
) {
    let state = joined.0.state.clone();
    let Some(map) = load_map(handle) else {
        // **The desk opens anyway**, answering every command with why it
        // cannot travel. Returning here left the session with no desk, so
        // nothing claimed `;` and `;go2 bank` went to the game -- twice, in
        // the author's first live `--web` run (2026-09-23), with the one
        // "no map" notice long scrolled away. A command of Hydra's must never
        // reach the game because a setting is missing.
        let told = handle.clone();
        let runner: Runner = Arc::new(move |_: &str| {
            told.say(Notice::line(
                NoticeKind::Error,
                format!(
                    "Travel has no map, so nothing was sent. Set {MAP_ENV} to your \
                     combined map file and start Hydra again."
                ),
            ));
            Claimed::Done
        });
        let desk = cena_session::command::claimant::Desk::new(symbol(handle, &state), runner);
        if !handle.set_desk(desk) {
            eprintln!("  !! [travel] something already runs this session's commands");
        }
        return;
    };
    let travel = Desk::new(
        Arc::new(map),
        cena_session::character_store::data_dir(),
        AuthorityToken(2),
    );
    // The login's own subscription, for the first command only.
    let first = Mutex::new(Some(joined));
    let runner: Runner = {
        let handle = handle.clone();
        Arc::new(move |line: &str| {
            let Some(command) = travel_command(&handle, line) else {
                return Claimed::Unknown;
            };
            let (travel, handle) = (Arc::clone(&travel), handle.clone());
            let joined = first.lock().ok().and_then(|mut first| first.take());
            let observer = observer.clone();
            tokio::spawn(async move {
                match joined {
                    Some(joined) => run(&travel, &handle, joined, command).await,
                    None => run_fresh(&travel, &handle, &observer, command).await,
                }
            });
            Claimed::Done
        })
    };
    let desk = cena_session::command::claimant::Desk::new(symbol(handle, &state), runner);
    let symbol = desk.symbol();
    eprintln!("[travel] ready: {symbol}go2 bank, {symbol}go2 targets, {symbol}route2 bank");
    if !handle.set_desk(desk) {
        eprintln!("  !! [travel] something already runs this session's commands");
    }
}

/// What this character marks a command with: the `commands` section of their
/// settings, or `;` (`claimant::Settings`). A settings file that cannot be
/// read is said to the player and leaves the default in force -- a session
/// must not stop over a preference, and must not quietly ignore one.
fn symbol(handle: &SessionHandle, state: &cena_session::GameState) -> Option<char> {
    let character = &state.character;
    let (instance, name) = (character.instance.as_deref()?, character.name.as_deref()?);
    let dir = cena_session::character_store::data_dir();
    let read = cena_session::settings_store::load(&dir, instance, name)
        .map_err(|e| e.to_string())
        .and_then(|file| {
            file.section::<claimant::Settings>(claimant::SECTION)
                .map_err(|e| format!("the {} section is malformed: {e}", claimant::SECTION))
        });
    match read {
        Ok(settings) => Some(settings.symbol()),
        Err(why) => {
            handle.say(Notice::line(
                NoticeKind::Warn,
                format!("Commands: {why} -- using the usual symbol."),
            ));
            None
        }
    }
}

/// The travel command a line is, the symbol already gone. `None`: travel does
/// not know it -- **and the game still never sees it** (`claimant`), which is
/// the author's rule: `;go22 bank` is a mistyped `;go2`, not speech.
fn travel_command(handle: &SessionHandle, line: &str) -> Option<Command> {
    match parse_command(line)? {
        Ok(command) => Some(command),
        // Travel's, and answered here: not another system's to try.
        Err(why) => {
            handle.say(Notice::line(NoticeKind::Error, format!("Travel: {why}")));
            Some(Command::Nothing)
        }
    }
}

async fn run(
    travel: &Arc<Desk>,
    handle: &SessionHandle,
    joined: (Snapshot, Receiver<Event>),
    command: Command,
) {
    if let Some(walk) = travel.run(handle, joined, command) {
        walked(walk.await);
    }
}

async fn run_fresh(
    travel: &Arc<Desk>,
    handle: &SessionHandle,
    observer: &SessionObserver,
    command: Command,
) {
    match observer.subscribe().await {
        Ok(joined) => {
            if let Some(walk) = travel.run(handle, joined, command) {
                walked(walk.await);
            }
        }
        Err(e) => handle.say(Notice::line(
            NoticeKind::Error,
            format!("Travel: I could not read the session -- {e:?}."),
        )),
    }
}

/// What a finished walk leaves in the terminal log. The player has been told
/// by the walk itself (`Notice`); this is for whoever reads stderr.
fn walked(travelled: Result<Travelled, tokio::task::JoinError>) {
    let Ok(travelled) = travelled else {
        eprintln!("  !! [travel] the walk ended badly");
        return;
    };
    eprintln!(
        "[travel] {:?} -- seed {:#x}, {} exit(s) the game said do not exist",
        travelled.ended,
        travelled.seed,
        travelled.wrong_for_the_map.len()
    );
    for (from, to) in &travelled.wrong_for_the_map {
        eprintln!("[travel]   wrong for the map: {} -> {}", from.0, to.0);
    }
}

/// The map, or why there is none. Said to the player, not only to stderr: a
/// walk that cannot happen should say so where the player is looking.
fn load_map(handle: &SessionHandle) -> Option<Map> {
    let say = |text: String| handle.say(Notice::line(NoticeKind::Error, text));
    let Some(path) = std::env::var_os(MAP_ENV) else {
        say(format!(
            "Travel: no map. Set {MAP_ENV} to a combined map file."
        ));
        return None;
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            say(format!(
                "Travel: cannot read the map at {}: {e}",
                PathBuf::from(&path).display()
            ));
            return None;
        }
    };
    match read_map(&bytes) {
        Ok(map) => {
            eprintln!(
                "[travel] map: {} rooms from {}",
                map.rooms().len(),
                PathBuf::from(&path).display()
            );
            Some(map)
        }
        Err(e) => {
            say(format!("Travel: the map file cannot be used: {e}"));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_command_is_read_from_the_arguments() {
        assert_eq!(
            first_command(["--first", "south", "--demo"]),
            Some("south".to_owned())
        );
        assert_eq!(
            first_command(["--first=go gate"]),
            Some("go gate".to_owned())
        );
        assert_eq!(first_command(["--demo"]), None);
        // A flag with nothing after it is not a command.
        assert_eq!(first_command(["--first"]), None);
    }
}
