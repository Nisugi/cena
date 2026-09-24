//! `--character Nisugi --character Nerten`: several characters in one Hydra
//! (`plan/29`, the binary on the session table).
//!
//! Every login is settled **before** anything connects: a character the
//! roster knows (`roster.rs`) takes its account and game from there and its
//! password from the ladder (`secrets.rs`); one it does not is asked for, at
//! the terminal. So the prompts happen once, up front, one after another --
//! and a run whose characters are all in the roster and the keyring needs no
//! terminal at all, which is the headless run the author wants.
//!
//! Then each goes on the table (`cena_host::Host`) with everything a session
//! is given (`setup.rs`), Hydra's command line and travel. The terminal shows
//! Hydra's own lines, each tagged with its character, and **no game text**:
//! two characters' story in one window is what `plan/29` §5a R2 rules out.
//! Ctrl-C quits every session together.
//!
//! The M1 walkthroughs (`--demo`, `--capture`, `--typeahead`) are
//! single-session tools and do not run here.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use cena_host::{Host, Who, stop_all};
use cena_session::{Event, SessionHandle, SessionObserver, State, StoppedBecause};

use crate::ask::{self, Typed};
use crate::commands::Commands;
use crate::connector::LiveConnector;
use crate::{frontend, interrupt, roster, run, secrets, setup, travel};

/// The characters named with `--character`, in order. Empty means the
/// single-session path, which asks for one character at the prompt.
pub(crate) fn characters() -> Vec<String> {
    characters_in(std::env::args().skip(1))
}

/// [`characters`], over any argument list, so it can be tested.
fn characters_in(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut named = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if let Some(name) = arg.strip_prefix("--character=") {
            named.push(name.to_owned());
        } else if arg == "--character"
            && let Some(name) = args.next()
        {
            named.push(name);
        }
    }
    named
}

/// One started session's loose ends, for the shutdown.
struct Started {
    character: String,
    watcher: tokio::task::JoinHandle<()>,
    combat: Option<std::thread::JoinHandle<()>>,
    player: tokio::task::JoinHandle<u64>,
}

/// Run every named character until Ctrl-C, or until all have stopped.
///
/// # Errors
///
/// A login could not be settled -- a character unknown to the roster with
/// nobody at the terminal, or no password -- before anything connected; or
/// every session's login was refused.
pub(crate) async fn play(names: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let dir = cena_session::character_store::data_dir();
    let mut logins = Vec::new();
    for name in &names {
        logins.push(login_for(&dir, name)?);
    }
    eprintln!();
    let interrupt = interrupt::on_ctrl_c();
    let pin = dir.join(cena_platform::PIN_FILENAME);
    let turn = Arc::default();
    let mut host = Host::new();
    // One listener for every character, each with its own page (`plan/29`
    // step 5); each page's link is printed when its character is `Ready`.
    let frontend = frontend::Frontend::open().await;
    let mut started = Vec::new();
    for typed in logins {
        if let Some(one) = start(&mut host, typed, &pin, &turn, frontend.as_ref()) {
            started.push(one);
        }
    }

    eprintln!(
        "[play] {} session(s) running; Ctrl-C quits them all",
        started.len()
    );
    tokio::select! {
        () = interrupt.cancelled() => {}
        () = all_stopped(&host) => eprintln!("[play] every session has stopped"),
    }
    if let Some(frontend) = frontend {
        frontend.shutdown().await;
    }

    eprintln!("\n[disconnect] quitting every session");
    let ended = stop_all(host.take_all()).await;
    let mut refused = 0;
    for (one, end) in started.into_iter().zip(ended) {
        one.watcher.abort();
        setup::flush_combat(one.combat);
        setup::flush_player_log(one.player).await;
        let character = &one.character;
        match end.map(|end| end.stopped_because) {
            Some(StoppedBecause::Fatal(error)) => {
                refused += 1;
                eprintln!("[{character}] never started: {error}");
            }
            Some(because) => eprintln!("[{character}] stopped: {because:?}"),
            None => eprintln!("[{character}] did not end cleanly; its task was aborted"),
        }
    }
    if refused > 0 && refused == names.len() {
        return Err("every login was refused".into());
    }
    Ok(())
}

/// Settle one character's login: from the roster, or asked for.
fn login_for(dir: &Path, name: &str) -> std::io::Result<Typed> {
    match roster::find(dir, name)? {
        Some(entry) => ask::from_roster(&entry),
        // A `GAME:` prefix names a game for a known character; for a new
        // one, the game is asked for with the account.
        None => ask::ask_for(name.split_once(':').map_or(name, |(_, n)| n)),
    }
}

/// Put one character on the table, and start what hangs off it.
fn start(
    host: &mut Host,
    typed: Typed,
    pin: &Path,
    turn: &Arc<std::sync::Mutex<()>>,
    web: Option<&frontend::Frontend>,
) -> Option<Started> {
    let character = typed.character.clone();
    let (account, game) = (typed.account.clone(), typed.game_code.clone());
    let proven = Proven::of(&typed);
    let who = Who {
        account: account.clone(),
        character: character.clone(),
    };
    let connector = LiveConnector::new(typed, run::login_provider(), pin.to_path_buf());
    let mut attached = None;
    let added = host.add(who, connector, |session| {
        // Subscribed before it runs, so the watcher sees the whole login.
        let (_, events) = session.subscribe();
        let (session, combat, player) = setup::attach(session, &character, &game, &account);
        attached = Some((events, combat, player));
        session
    });
    let id = match added {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[{character}] not started: {e}");
            return None;
        }
    };
    let (events, combat, player) = attached?;
    let hosted = host.get(id)?;
    let commands = Commands::install(&hosted.handle);
    if let Some(web) = web {
        web.attach(
            Some(&character),
            hosted.observer.clone(),
            hosted.handle.clone(),
        );
    }
    let watcher = tokio::spawn(run::watch_events(events, false, format!("[{character}]")));
    proven.on_ready(&hosted.observer, turn);
    tokio::spawn(after_ready(
        hosted.handle.clone(),
        hosted.observer.clone(),
        commands,
    ));
    Some(Started {
        character,
        watcher,
        combat,
        player,
    })
}

/// Once the login is proven, open travel.
async fn after_ready(handle: SessionHandle, observer: SessionObserver, commands: Commands) {
    if until_ready(&observer).await {
        travel::after_login(&handle, observer, &commands).await;
    }
}

/// What a login leaves behind once it is proven: its roster entry, and --
/// for a typed password -- the offer to keep it in the keyring. Taken from
/// the login before it is handed to the connector, started once the session
/// exists.
pub(crate) struct Proven {
    entry: roster::Entry,
    typed_password: Option<(String, String)>,
}

impl Proven {
    pub(crate) fn of(typed: &Typed) -> Self {
        Self {
            entry: roster::Entry::of(typed),
            typed_password: (typed.password_from == secrets::Source::Prompt)
                .then(|| (typed.account.clone(), typed.password.clone())),
        }
    }

    /// When `observer`'s login reaches `Ready`: record the roster entry, and
    /// offer a typed password to the keyring, one question at a time (`turn`).
    pub(crate) fn on_ready(self, observer: &SessionObserver, turn: &Arc<std::sync::Mutex<()>>) {
        if let Some((account, password)) = self.typed_password {
            tokio::spawn(secrets::offer_to_remember(
                account,
                password,
                observer.clone(),
                Arc::clone(turn),
            ));
        }
        let observer = observer.clone();
        let entry = self.entry;
        tokio::spawn(async move {
            if until_ready(&observer).await {
                remember(&cena_session::character_store::data_dir(), entry);
            }
        });
    }
}

/// Record `entry` in the roster, saying so if it cannot be.
fn remember(dir: &Path, entry: roster::Entry) {
    let character = entry.character.clone();
    if let Err(e) = roster::record(dir, entry) {
        eprintln!(
            "[{character}] could not be added to the roster ({e}); `--character {character}` will ask again"
        );
    }
}

/// Resolves `true` once `observer`'s session is `Ready`; `false` if it ended
/// first.
async fn until_ready(observer: &SessionObserver) -> bool {
    let Ok((snapshot, mut events)) = observer.subscribe().await else {
        return false;
    };
    if snapshot.lifecycle == State::Ready {
        return true;
    }
    loop {
        match events.recv().await {
            Ok(o) if o.event == Event::StateChanged(State::Ready) => return true,
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return false,
        }
    }
}

/// Resolves when no session in `host` is still running.
async fn all_stopped(host: &Host) {
    while host.sessions().any(|(_, hosted)| hosted.is_running()) {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::characters_in;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn every_character_is_taken_in_order_in_either_spelling() {
        assert_eq!(
            characters_in(args("--web --character Nisugi --character=Nerten --hold 5")),
            ["Nisugi", "Nerten"]
        );
        assert_eq!(
            characters_in(args("--character GS3:Nisugi")),
            ["GS3:Nisugi"]
        );
    }

    #[test]
    fn no_character_is_the_one_session_path() {
        assert!(characters_in(args("--web --demo")).is_empty());
        // A trailing flag with no name names nobody, rather than the next run.
        assert!(characters_in(args("--character")).is_empty());
    }
}
