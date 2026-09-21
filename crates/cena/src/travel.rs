//! `-- --route <target>` and `-- --go <target>`: the route shown, and the
//! route walked (`plan/24`).
//!
//! # BUILT, NOT RUN
//!
//! Like everything in this binary that reaches the game (`main.rs`'s module
//! docs), this has been compiled and never executed: only the author runs the
//! binary (`CLAUDE.md`, Credentials). Everything it calls is tested against a
//! scripted game in `cena-behavior`; what is untested is the wiring here.
//!
//! # Two modes, and the first sends nothing
//!
//! `--route` logs in, works out where the character is, and **says** the
//! route (`cena_session::notice`). It sends no command at all, so it is the
//! one to run first: it proves the map loads, the room is found and the
//! walker's facts are read, with nothing at stake.
//!
//! `--go` does the same and then walks. **Ctrl-C stops the walk, not the
//! process**: the trip gets its one `get` per stored item (`plan/24`, the
//! author's ruling) and the session then quits cleanly as it always does.
//!
//! # Why a mirror
//!
//! A behavior has no live read of the model: it is handed a snapshot and a
//! subscription and keeps its own `GameState` (`plan/24`, 4a's finding). The
//! snapshot taken before login is empty, and the login burst overflows the
//! event ring if nobody is reading it -- MEASURED on the first live session,
//! 99 events dropped (`CLAUDE.md`). So a task reads from the first moment and
//! folds every frame, and hands its state and its subscription to the trip
//! together, with no gap between the two for an event to fall into.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cena_behavior::travel::{
    self, Ended, Map, RoomId, TravelNotes, Whence, destination, itinerary, read_map, room_of,
    table, walker_from,
};
use cena_session::travel_store::{self, TravelFile};
use cena_session::{AuthorityToken, CommandId, Event, Notice, NoticeKind, SessionHandle, Snapshot};
use tokio::sync::broadcast::{Receiver, error::RecvError};
use tokio_util::sync::CancellationToken;

/// Where the combined map is. No default: a wrong map is worse than none.
///
/// ```text
/// $env:CENA_MAP = "E:\Gemstone\data\cena_data\gs.map"
/// ```
pub(crate) const MAP_ENV: &str = "CENA_MAP";

/// What the operator asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Errand {
    None,
    /// Say the route there. Sends nothing.
    Route(String),
    /// Walk there.
    Go(String),
}

impl Errand {
    /// `--route bank`, `--go 228`, `--go=u7120`. The first one wins.
    pub(crate) fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let arg = arg.as_ref();
            for (flag, make) in [
                ("--route", Errand::Route as fn(String) -> Errand),
                ("--go", Errand::Go as fn(String) -> Errand),
            ] {
                if arg == flag {
                    return args
                        .next()
                        .map_or(Errand::None, |to| make(to.as_ref().to_owned()));
                }
                if let Some(to) = arg
                    .strip_prefix(flag)
                    .and_then(|rest| rest.strip_prefix('='))
                {
                    return make(to.to_owned());
                }
            }
        }
        Errand::None
    }
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

/// Run the errand. Returns when it is over, however it ended.
pub(crate) async fn run(
    errand: Errand,
    handle: &SessionHandle,
    joined: (Snapshot, Receiver<Event>),
) {
    let (to, walk) = match &errand {
        Errand::None => return,
        Errand::Route(to) => (to.as_str(), false),
        Errand::Go(to) => (to.as_str(), true),
    };
    let say = |kind, text: String| handle.say(Notice::line(kind, text));
    let Some(map) = load_map(handle) else {
        return;
    };
    let state = &joined.0.state;
    let Some(here) = room_of(&map, state, Whence::Nowhere) else {
        say(
            NoticeKind::Error,
            format!(
                "Travel: I cannot tell which room this is (the game said {:?}).",
                state.room.id
            ),
        );
        return;
    };
    let (file, mut notes) = load_notes(handle, state);
    let walker = walker_from(state, &notes, state.game_time().unwrap_or(0));
    let Some(goal) = destination(&map, &walker, here, to) else {
        say(
            NoticeKind::Error,
            format!("Travel: I do not know a room called {to:?}."),
        );
        return;
    };
    let Some(legs) = itinerary(&map, &walker, here, goal) else {
        say(
            NoticeKind::Error,
            format!(
                "Travel: no way from {} to {} for this character.",
                here.0, goal.0
            ),
        );
        return;
    };
    handle.say(Notice::table(NoticeKind::Info, table(&map, here, &legs)));
    if walk {
        // Boxed: a trip's future holds a whole `GameState`.
        Box::pin(go(handle, joined, &map, goal, &mut notes, file)).await;
    }
}

async fn go(
    handle: &SessionHandle,
    joined: (Snapshot, Receiver<Event>),
    map: &Map,
    goal: RoomId,
    notes: &mut TravelNotes,
    mut file: Option<(PathBuf, TravelFile)>,
) {
    let stop = CancellationToken::new();
    let next = Arc::new(AtomicU64::new(100_000));
    let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
    // A memory is saved when it is made: losing one strands the character.
    let wrote = |notes: &TravelNotes| {
        let Some((dir, file)) = file.as_mut() else {
            return;
        };
        file.memories = notes.memories.clone().into_iter().collect();
        if let Err(e) = travel_store::save(dir, file) {
            eprintln!("  !! [travel] could not save the travel file: {e}");
        }
    };
    eprintln!("[travel] walking to {} -- Ctrl-C stops the walk", goal.0);
    let walking = travel::travel(
        handle,
        &stop,
        ids,
        AuthorityToken(2),
        joined,
        map,
        goal,
        notes,
        wrote,
    );
    tokio::pin!(walking);
    let travelled = tokio::select! {
        travelled = &mut walking => travelled,
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\n[travel] stopping the walk");
            stop.cancel();
            walking.await
        }
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
    if travelled.ended == Ended::Arrived {
        eprintln!("[travel] arrived");
    }
}

fn load_map(handle: &SessionHandle) -> Option<Map> {
    let say = |text: String| handle.say(Notice::line(NoticeKind::Error, text));
    let Some(path) = std::env::var_os(MAP_ENV) else {
        say(format!(
            "Travel: set {MAP_ENV} to the combined map file first."
        ));
        return None;
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            say(format!(
                "Travel: could not read {}: {e}",
                PathBuf::from(&path).display()
            ));
            return None;
        }
    };
    match read_map(&bytes) {
        Ok(map) => {
            eprintln!("[travel] map: {} rooms", map.len());
            Some(map)
        }
        Err(e) => {
            say(format!(
                "Travel: {} is not a map this build reads: {e}",
                PathBuf::from(&path).display()
            ));
            None
        }
    }
}

/// The character's travel file, as the walker's notes. A file that cannot be
/// trusted is **not** replaced: the walk goes on without memories and saves
/// nothing (`travel_store`'s rule).
fn load_notes(
    handle: &SessionHandle,
    state: &cena_session::GameState,
) -> (Option<(PathBuf, TravelFile)>, TravelNotes) {
    let character = &state.character;
    let (Some(instance), Some(name)) = (character.instance.as_deref(), character.name.as_deref())
    else {
        handle.say(Notice::line(
            NoticeKind::Warn,
            "Travel: the game has not said who this is, so nothing will be remembered.",
        ));
        return (None, TravelNotes::default());
    };
    let dir = cena_session::character_store::data_dir();
    match travel_store::load(&dir, instance, name) {
        Ok(file) => {
            let notes = TravelNotes {
                settings: file.settings.clone().into_iter().collect(),
                memories: file.memories.clone().into_iter().collect(),
            };
            (Some((dir, file)), notes)
        }
        Err(e) => {
            handle.say(Notice::line(
                NoticeKind::Warn,
                format!("Travel: {e} -- left alone, and nothing will be remembered."),
            ));
            (None, TravelNotes::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_errand_is_read_from_the_arguments() {
        let read = |args: &[&str]| Errand::from_args(args.iter().copied());
        assert_eq!(read(&[]), Errand::None);
        assert_eq!(read(&["--demo"]), Errand::None);
        assert_eq!(read(&["--route", "bank"]), Errand::Route("bank".into()));
        assert_eq!(read(&["--go=u7120"]), Errand::Go("u7120".into()));
        assert_eq!(
            read(&["--hold", "60", "--go", "228"]),
            Errand::Go("228".into())
        );
        // A flag with nothing after it is not a walk to nowhere.
        assert_eq!(read(&["--go"]), Errand::None);
        // Not a prefix match: `--gone` is someone else's flag.
        assert_eq!(read(&["--gone=1"]), Errand::None);
    }
}
