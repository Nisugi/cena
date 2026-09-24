//! Travel, wired to this binary: load the map, build the desk, and register
//! it on Hydra's command line (`crate::commands`) for travel's own words.
//!
//! **Nothing here decides what a command means.** The symbol and the claiming
//! are `cena_session::command::claimant`, the routing is `crate::commands`,
//! and the commands and what they do are `cena_behavior::travel`
//! (`command.rs`, `desk.rs`). This file is the join, which is the binary's
//! job and no crate's.
//!
//! # BUILT, NOT RUN
//!
//! Like everything in this binary that reaches the game (`main.rs`'s module
//! docs), this has been compiled and never executed: only the author runs the
//! binary (`CLAUDE.md`, Credentials). Everything it calls is tested against a
//! scripted game in `cena-behavior`; what is untested is the wiring here.
//!
//! # Why no mirror
//!
//! A behavior has no live read of the model: it is handed a snapshot and a
//! subscription and keeps its own `GameState` (`plan/24`, 4a's finding). This
//! file used to keep a second copy for the first command after a login -- a
//! task folding every frame from the first moment -- because a snapshot taken
//! before login is empty.
//!
//! It is gone because it lagged. In the author's live run of 2026-09-23 the
//! login burst outran it: it kept the room's title from the burst's first
//! lines and lost `<app>` and `<nav rm=>` behind it, so the first `;go2` said
//! "the game has not said who this is" and "I cannot tell which room this is"
//! of a login that had said both (the wire capture has them at lines 154 and
//! 157). Every command now asks the session itself
//! (`SessionObserver::subscribe`), whose state is the one the actor folds and
//! which already holds what the character store restored.

use std::path::PathBuf;
use std::sync::Arc;

use crate::commands::Commands;
use cena_behavior::travel::{Command, Desk, Map, Travelled, parse_command, read_map};
use cena_session::command::claimant::{self, Claimed};
use cena_session::{AuthorityToken, Notice, NoticeKind, SessionHandle, SessionObserver};

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

/// What `main` does once the session is up: send `--first`, then put travel
/// on the command line so `;go2` works.
///
/// Here and not in `main` because `main` is at clippy's line limit, and the
/// rule is to move code down.
pub(crate) async fn after_login(
    handle: &SessionHandle,
    observer: SessionObserver,
    commands: &Commands,
) {
    if let Some(first) = first_command(std::env::args().skip(1)) {
        eprintln!("[travel] first: {first}");
        let outcome = crate::watch::send_manual(handle, &first).await;
        eprintln!("[travel] first: {outcome:?}");
    }
    match observer.subscribe().await {
        Ok((snapshot, _)) => open_travel(handle, observer, &snapshot.state, commands),
        Err(e) => eprintln!("[travel] could not read the session to open travel: {e:?}"),
    }
}

/// Register travel on Hydra's command line (`crate::commands`), with the map
/// if there is one.
///
/// Also where the character's own command symbol is applied: the command
/// line was installed at startup with the default, and this is the first
/// moment the login has said who the character is.
fn open_travel(
    handle: &SessionHandle,
    observer: SessionObserver,
    state: &cena_session::GameState,
    commands: &Commands,
) {
    if let Some(symbol) = symbol(handle, state)
        && !handle.set_command_symbol(symbol)
    {
        eprintln!("  !! [commands] no command line to give the symbol {symbol} to");
    }
    let map = load_map(handle).map(Arc::new);
    // Hunt walks with travel's driver, so it takes the same map, or none.
    crate::hunt::open(handle, observer.clone(), state, commands, map.clone());
    let Some(map) = map else {
        // Travel's words are answered with why it cannot travel, and nothing
        // is sent. Every other word is the command line's to route.
        let told = handle.clone();
        commands.travel(Arc::new(move |line: &str| {
            // Travel's word, well-formed or not: without a map, either way
            // the answer is the same.
            let _ = parse_command(line)?;
            told.say(Notice::line(
                NoticeKind::Error,
                format!(
                    "Travel has no map, so nothing was sent. Set {MAP_ENV} to your \
                     combined map file and start Hydra again."
                ),
            ));
            Some(Claimed::Done)
        }));
        return;
    };
    let travel = Desk::new(
        map,
        cena_session::character_store::data_dir(),
        AuthorityToken(2),
    );
    let handler = handle.clone();
    commands.travel(Arc::new(move |line: &str| {
        let command = travel_command(&handler, line)?;
        let (travel, handle) = (Arc::clone(&travel), handler.clone());
        let observer = observer.clone();
        tokio::spawn(async move { run_fresh(&travel, &handle, &observer, command).await });
        Some(Claimed::Done)
    }));
    let symbol = handle.command_symbol().unwrap_or(claimant::DEFAULT_SYMBOL);
    eprintln!("[travel] ready: {symbol}go2 bank, {symbol}go2 targets, {symbol}route2 bank");
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
            first_command(["--first", "south", "--web"]),
            Some("south".to_owned())
        );
        assert_eq!(
            first_command(["--first=go gate"]),
            Some("go gate".to_owned())
        );
        assert_eq!(first_command(["--web"]), None);
        // A flag with nothing after it is not a command.
        assert_eq!(first_command(["--first"]), None);
    }
}
