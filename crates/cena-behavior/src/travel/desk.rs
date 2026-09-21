//! The travel desk: what does a [`Command`] for one session, **while it is
//! being played** -- start a walk, show a route, list the places, save a
//! name, stop.
//!
//! One desk per session; the map is shared between them all (`Arc`), since
//! it is 36,838 rooms and Hydra runs up to twenty-five characters.
//!
//! # What it needs from whoever calls it
//!
//! The session's handle, and a fresh `(Snapshot, Receiver<Event>)` -- what
//! `Session::subscribe` gives at any moment. A behavior has no live read of
//! the model (`plan/24` stage 4), so each command starts from a snapshot taken
//! when it was typed, and a walk folds events from there.
//!
//! # One walk at a time
//!
//! `;go2 bank` while a walk is under way **stops that walk and starts this
//! one**: a player who names a new place means it. (Lich refuses the second
//! go2 as already running, and the player kills the first by hand.) The old
//! walk's stop is the author's one-`get` stop (`drive`), and the new one
//! claims the authority once the old has let it go.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use cena_map::{Map, Origin as Whence, RoomId, Uid};
use cena_session::travel_store::{self, TravelFile, Whose};
use cena_session::{
    AuthorityToken, CommandId, GameState, Notice, NoticeKind, SessionHandle, Snapshot,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::command::Command;
use super::{
    Heard, TravelNotes, Travelled, described, destination, itinerary, places, room_of, table,
    travel, walker_from,
};

/// How many rooms a place that fits several is listed with.
const MAX_LISTED: usize = 40;

pub struct Desk {
    map: Arc<Map>,
    /// Where `travel.json` is kept.
    dir: PathBuf,
    token: AuthorityToken,
    /// The walk under way.
    walking: Mutex<Option<Walk>>,
    ids: Arc<AtomicU64>,
}

/// A walk under way: how to stop it, and how to know it is over.
#[derive(Clone)]
struct Walk {
    stop: CancellationToken,
    /// Cancelled by the walk itself, as the last thing it does.
    over: CancellationToken,
}

/// Who is travelling, and what they keep: the travel file's view, if it
/// could be read, and the notes a trip works from.
struct Traveller {
    file: Option<TravelFile>,
    notes: TravelNotes,
}

impl Desk {
    #[must_use]
    pub fn new(map: Arc<Map>, dir: PathBuf, token: AuthorityToken) -> Arc<Desk> {
        Arc::new(Desk {
            map,
            dir,
            token,
            walking: Mutex::new(None),
            // Clear of the ids the binary's own commands use.
            ids: Arc::new(AtomicU64::new(100_000)),
        })
    }

    /// Stop the walk under way. `false`: there was none.
    pub fn stop(&self) -> bool {
        let walking = self
            .walking
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        walking.is_some_and(|walk| {
            walk.stop.cancel();
            true
        })
    }

    /// Do what was asked. Everything but a walk is done before this returns;
    /// **a walk is started and left walking**, and its task is handed back for
    /// a caller that wants to wait for it.
    pub fn run(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        command: Command,
    ) -> Option<JoinHandle<Travelled>> {
        let say = |kind, text: String| handle.say(Notice::line(kind, format!("Travel: {text}")));
        let state = &joined.0.state;
        let mut traveller = self.traveller(handle, state);
        let walker = walker_from(state, &traveller.notes, state.game_time().unwrap_or(0));
        match &command {
            Command::Stop => {
                if !self.stop() {
                    say(NoticeKind::Info, "I am not walking anywhere.".to_owned());
                }
                return None;
            }
            Command::Places => {
                let lines = places(&self.map, &walker, &traveller.notes.targets);
                handle.say(Notice::table(NoticeKind::Info, lines));
                return None;
            }
            Command::List => {
                let mut lines = vec!["Travel: the names you have saved:".to_owned()];
                lines.extend(
                    traveller
                        .notes
                        .targets
                        .iter()
                        .map(|(name, rooms)| format!("   {name:<15} = {rooms:?}")),
                );
                handle.say(Notice::table(NoticeKind::Info, lines));
                return None;
            }
            Command::Forget { name, global } => {
                let saved = self.save_target(&traveller, name, &[], *global);
                match saved {
                    Ok(()) => say(NoticeKind::Info, format!("{name:?} is forgotten.")),
                    Err(why) => say(
                        NoticeKind::Error,
                        format!("{name:?} was not forgotten -- {why}."),
                    ),
                }
                return None;
            }
            _ => {}
        }
        let hint = traveller.notes.last_room.map(RoomId);
        let here = room_of(&self.map, state, Whence::Nowhere)
            .or_else(|| room_of(&self.map, state, Whence::Still(hint?)));
        let Some(here) = here else {
            say(
                NoticeKind::Error,
                format!(
                    "I cannot tell which room this is (number {:?}, name {:?}).",
                    state.room.id, state.room.title
                ),
            );
            return None;
        };
        let to = match command {
            Command::Save {
                name,
                rooms,
                global,
            } => {
                self.save(handle, &traveller, here, &name, &rooms, global);
                return None;
            }
            Command::Go(to) | Command::Route(to) if to.trim().is_empty() => {
                say(NoticeKind::Error, "where to?".to_owned());
                return None;
            }
            Command::Go(ref to) | Command::Route(ref to) => to.clone(),
            _ => return None,
        };
        let Some(goal) = destination(&self.map, &walker, here, &to, &traveller.notes.targets)
        else {
            self.say_what_fits(handle, &to);
            return None;
        };
        let Some(legs) = itinerary(&self.map, &walker, here, goal) else {
            say(
                NoticeKind::Error,
                format!("no way from {} to {} for this character.", here.0, goal.0),
            );
            return None;
        };
        handle.say(Notice::table(
            NoticeKind::Info,
            table(&self.map, here, &legs),
        ));
        if !matches!(command, Command::Go(_)) {
            return None;
        }
        traveller.notes.last_room = Some(here.0);
        Some(self.walk(handle.clone(), (joined.0, joined.1.into()), goal, traveller))
    }

    /// The walk, as a task of its own.
    fn walk(
        self: &Arc<Self>,
        handle: SessionHandle,
        joined: (Snapshot, Heard),
        goal: RoomId,
        traveller: Traveller,
    ) -> JoinHandle<Travelled> {
        let walk = Walk {
            stop: CancellationToken::new(),
            over: CancellationToken::new(),
        };
        let before = self
            .walking
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(walk.clone());
        if let Some(before) = &before {
            before.stop.cancel();
        }
        let desk = Arc::clone(self);
        tokio::spawn(async move {
            let Walk { stop, over } = walk;
            // The walk before this one holds the authority under the same
            // token until it has stopped: claiming before then would be
            // released from under this walk when that one lets go.
            if let Some(before) = before {
                before.over.cancelled().await;
            }
            let Traveller {
                mut file,
                mut notes,
            } = traveller;
            let next = Arc::clone(&desk.ids);
            let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
            let ended = {
                // A memory is saved when it is made: losing one strands the
                // character at whatever event it records.
                let wrote = |notes: &TravelNotes| desk.keep(&handle, file.as_mut(), notes);
                // Boxed: a trip's future holds a whole `GameState`.
                Box::pin(travel(
                    &handle, &stop, ids, desk.token, joined, &desk.map, goal, &mut notes, wrote,
                ))
                .await
            };
            // Where the walk ended, for the next login to break a tie with.
            if let Some(ended_in) = ended.last_room {
                notes.last_room = Some(ended_in.0);
            }
            desk.keep(&handle, file.as_mut(), &notes);
            // Still the walk under way, unless it was stopped or replaced --
            // and either of those cancels `stop` and owns the slot.
            if !stop.is_cancelled() {
                *desk.walking.lock().unwrap_or_else(PoisonError::into_inner) = None;
            }
            over.cancel();
            ended
        })
    }

    /// Write the character's spot from the notes. A file that could not be
    /// read is left alone, and nothing is remembered.
    fn keep(&self, handle: &SessionHandle, file: Option<&mut TravelFile>, notes: &TravelNotes) {
        let Some(file) = file else { return };
        file.settings = notes.settings.clone().into_iter().collect();
        file.memories = notes.memories.clone().into_iter().collect();
        file.last_room = notes.last_room;
        if let Err(why) = travel_store::save(&self.dir, file) {
            handle.say(Notice::line(
                NoticeKind::Error,
                format!("Travel: what I know could not be saved -- {why}."),
            ));
        }
    }

    fn traveller(&self, handle: &SessionHandle, state: &GameState) -> Traveller {
        let character = &state.character;
        let (Some(instance), Some(name)) =
            (character.instance.as_deref(), character.name.as_deref())
        else {
            handle.say(Notice::line(
                NoticeKind::Warn,
                "Travel: the game has not said who this is, so nothing will be remembered.",
            ));
            return Traveller {
                file: None,
                notes: TravelNotes::default(),
            };
        };
        match travel_store::load(&self.dir, instance, name) {
            Ok(file) => Traveller {
                notes: TravelNotes {
                    settings: file.settings.clone().into_iter().collect(),
                    memories: file.memories.clone().into_iter().collect(),
                    targets: file.targets.clone(),
                    last_room: file.last_room,
                },
                file: Some(file),
            },
            Err(why) => {
                handle.say(Notice::line(
                    NoticeKind::Warn,
                    format!("Travel: {why} -- left alone, and nothing will be remembered."),
                ));
                Traveller {
                    file: None,
                    notes: TravelNotes::default(),
                }
            }
        }
    }

    fn save_target(
        &self,
        traveller: &Traveller,
        name: &str,
        rooms: &[u32],
        global: bool,
    ) -> Result<(), String> {
        let file = traveller
            .file
            .as_ref()
            .ok_or("the travel file could not be read, so it was left alone")?;
        let whose = if global {
            Whose::Everyone
        } else {
            Whose::Character {
                instance: &file.instance,
                character: &file.character,
            }
        };
        travel_store::save_target(&self.dir, whose, name, rooms)
            .map(|_| ())
            .map_err(|why| why.to_string())
    }

    /// go2's `save` (`go2.lic:1117-1160`): each room a number, a `u` number,
    /// or `current`; none given is `current`; and a room the map does not
    /// have stops the whole save, as upstream has it.
    fn save(
        &self,
        handle: &SessionHandle,
        traveller: &Traveller,
        here: RoomId,
        name: &str,
        rooms: &[String],
        global: bool,
    ) {
        let say = |kind, text: String| handle.say(Notice::line(kind, format!("Travel: {text}")));
        let room = |said: &String| match said.as_str() {
            "current" => Some(here),
            other => match other.strip_prefix('u') {
                Some(uid) => self
                    .map
                    .ids_for_uid(Uid(uid.parse().ok()?))
                    .first()
                    .copied(),
                None => self
                    .map
                    .room(RoomId(other.parse().ok()?))
                    .map(|room| room.id),
            },
        };
        let unknown: Vec<&String> = rooms.iter().filter(|said| room(said).is_none()).collect();
        if !unknown.is_empty() {
            say(
                NoticeKind::Error,
                format!("these rooms are not in the map -- {unknown:?}."),
            );
            return;
        }
        let mut ids: Vec<u32> = rooms.iter().filter_map(room).map(|room| room.0).collect();
        if ids.is_empty() {
            ids.push(here.0);
        }
        // Said only of a save that happened.
        match self.save_target(traveller, name, &ids, global) {
            Ok(()) => say(
                NoticeKind::Info,
                format!(
                    "{name:?} means {ids:?} from now on, for {}.",
                    if global {
                        "every character"
                    } else {
                        "this character"
                    }
                ),
            ),
            Err(why) => say(
                NoticeKind::Error,
                format!("{name:?} was not saved -- {why}."),
            ),
        }
    }

    /// go2 lists what the words fit and asks which; say which to ask for.
    fn say_what_fits(&self, handle: &SessionHandle, to: &str) {
        let fits = described(&self.map, to);
        if fits.is_empty() {
            handle.say(Notice::line(
                NoticeKind::Error,
                format!("Travel: I do not know a room called {to:?}."),
            ));
            return;
        }
        let mut lines = vec![format!(
            "Travel: {to:?} fits {} rooms. Ask for one by number:",
            fits.len()
        )];
        lines.extend(fits.iter().take(MAX_LISTED).filter_map(|id| {
            let room = self.map.room(*id)?;
            let title = room.title.first().map_or("", String::as_str);
            let at = room.location.as_deref().unwrap_or("");
            Some(format!("{:>7}  {title}  {at}", id.0))
        }));
        handle.say(Notice::table(NoticeKind::Warn, lines));
    }
}
