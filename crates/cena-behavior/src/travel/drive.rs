//! The thin `async` layer round [`Trip`] (`plan/24` stage 4c).
//!
//! Everything that decides is in [`Trip`], which has no socket and no clock.
//! This file is what has them: it holds the authority, keeps a [`GameState`]
//! by folding the session's events (a behavior has no other read of the
//! model), says where the character is, sends what the trip says to send, and
//! does the [`Deed`]s.
//!
//! # Stopping (`plan/12` §4.3, and the author's one exception)
//!
//! Every await is raced against the stop token, as `look` documents. A
//! stopped behavior's cleanup "cannot send commands" -- otherwise *stop*
//! becomes *send more*. The author, 2026-09-21, allows exactly one thing:
//! **one command per stored item, to take it back, and then it stops.** No
//! retry and no waiting to see, because the character may not be able to hold
//! a shield where it now stands. What did not come back is in
//! [`Travelled::still_stored`], for a frontend to say.
//!
//! The stance is **not** put back on a stop: that would be a second kind of
//! command, and the ruling was for one. [`Travelled::stance_before`] says
//! what it was. Nor is a key a crossing took out ([`Deed::TakeOut`]): it is
//! not *stored*, so the ruling does not reach it, and
//! [`Travelled::still_out`] says what is in the walker's hand instead of in
//! its container.
//!
//! A dead or disconnected session sends nothing at all.
//!
//! # A command from an older connection is a disconnection
//!
//! `Outcome::Interrupted` and `Sent::Interrupted` have one producer each:
//! the actor's stale-generation discard (`cena-session`, `actor/io.rs`), a
//! command stamped for a connection that is no longer the one open. That is
//! the connection changing under the walk, and it ends the walk as
//! [`BehaviorError::Disconnected`] -- **not** `Cancelled`, which is the
//! player's stop. The two differ in what follows: a stop sends its one
//! take-back, and a disconnection sends nothing on a connection the walk did
//! not start on, nor tells the player they stopped it. `send` always read it
//! this way; `exchange` read it as a stop.

use std::collections::HashMap;
use std::time::Duration;

use cena_map::{
    Located, Map, Origin as Whence, RoomId, Sighting, Uid, Walker, title_from_subtitle,
};
use cena_session::group::{self, GroupEvent};
use cena_session::{
    AuthorityToken, ChunkLine, CommandId, Event, Frame, GameState, Gate, Notice, NoticeKind,
    Origin, Sent, SessionHandle, Snapshot, State,
};
use tokio::sync::broadcast::{error::RecvError, error::TryRecvError};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::hands::{Stored, take_back};
use super::heard::Heard;
use super::kept;

mod deeds;
mod preflight;
mod solve;
use super::{Deed, Now, Said, TravelNotes, Trip, Why, walker_from};
use crate::BehaviorError;

/// How long a [`Said::Hold`] waits for the game before ticking anyway: the
/// trip's own timeouts only run when it is ticked.
pub const BEAT: Duration = Duration::from_millis(250);

/// How long one command of a deed may go unanswered. Lich's `dothistimeout`
/// for a society power is 3 (`effect-list.xml`, 9704's `cast-proc`).
pub const DEED_DEADLINE: Duration = Duration::from_secs(3);

/// How long the walker waits for its group before going on without it.
pub const FOLLOW_WAIT: Duration = Duration::from_secs(30);

/// How long the walker may not know where it is before the trip ends. A room's
/// title arrives a moment after its number, so *some* of this is ordinary; a
/// room the map cannot name at all is not, and `plan/12` section 5.5 gives every
/// wait a deadline. Found by a mutation that hung the suite instead of failing
/// it.
pub const LOST_WAIT: Duration = Duration::from_secs(10);

/// How a trip ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Arrived,
    /// The trip gave up, and why.
    Failed(Why),
    /// A spell the map names is not in the spell table.
    UnknownSpell,
    /// Stopped, or the session went away.
    Stopped(BehaviorError),
    /// A routine met something only the player can settle, and stopped the
    /// trip to say so: [`Travelled::halted`] has its words.
    Halted,
}

/// What a finished trip leaves for whoever started it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Travelled {
    pub ended: Ended,
    /// Stored by the trip and not seen to come back.
    pub still_stored: Vec<Stored>,
    /// The stance the trip replaced and did not put back.
    pub stance_before: Option<String>,
    /// Exits the game said do not exist, as `(from, to)`.
    pub wrong_for_the_map: Vec<(RoomId, RoomId)>,
    /// What the trip's choices were seeded with ([`seed_for`]), for a log.
    pub seed: u64,
    /// Why a routine stopped the trip ([`Ended::Halted`]), as it told the player.
    pub halted: Option<String>,
    /// The last room the walk knew itself to be in: what a caller writes
    /// down as `TravelNotes::last_room`. `None` if it never knew.
    pub last_room: Option<RoomId>,
    /// What a crossing took out of a container ([`Deed::TakeOut`], as the map
    /// names it: `heavy key`) and did not put back, because the trip ended
    /// between the two.
    pub still_out: Option<String>,
}

/// What a crossing took out, and the container it came from, as ids; and
/// what the map called it, for the player.
#[derive(Debug, Clone)]
struct Taken {
    thing: String,
    container: String,
    name: String,
}

/// Walk to `goal`.
///
/// `joined` is `Session::subscribe`'s pair. `notes` is the character's travel
/// file; `wrote` is called each time a crossing changes it, so the caller can
/// save it then and not at the end -- a memory lost strands the character.
#[allow(clippy::too_many_arguments)]
pub async fn travel(
    handle: &SessionHandle,
    cancel: &CancellationToken,
    next_id: impl FnMut() -> CommandId,
    token: AuthorityToken,
    joined: (Snapshot, impl Into<Heard>),
    map: &Map,
    goal: RoomId,
    notes: &mut TravelNotes,
    wrote: impl FnMut(&TravelNotes) + Send,
) -> Travelled {
    if handle.claim(token).await.is_err() {
        let ended = Ended::Stopped(BehaviorError::AuthorityHeld);
        // Said, like every other ending: the player typed a destination, and
        // a walk that never started is news. This returned silently, and the
        // desk's `;go2 stop` then `;go2 bank` reached it every time (review,
        // 2026-09-23).
        for notice in report(ended, &[], &[], None) {
            handle.say(notice);
        }
        return Travelled {
            ended,
            still_stored: Vec::new(),
            stance_before: None,
            wrong_for_the_map: Vec::new(),
            seed: 0,
            halted: None,
            last_room: None,
            still_out: None,
        };
    }
    let (snapshot, events) = joined;
    let seed = seed_for(&snapshot.state, goal);
    let mut trip = Trip::seeded(goal, seed);
    let mut driver = Driver {
        handle,
        cancel,
        token,
        next_id,
        state: snapshot.state,
        events: events.into(),
        began: Instant::now(),
        was: None,
        hint: notes.last_room.map(RoomId),
        stored: Vec::new(),
        stance_before: None,
        heard: 0,
        lost_since: None,
        behind: Vec::new(),
        answer: Vec::new(),
        speech_before: None,
        taken: None,
        halted: None,
        found: HashMap::new(),
        kept: super::routines::Kept::default(),
    };
    let mut wrote = wrote;
    let mut cx = Cx {
        trip: &mut trip,
        map,
        notes,
        wrote: &mut wrote,
    };
    let ended = match driver.preflight(&mut cx, goal).await {
        Ok(()) => driver.walk(&mut cx).await,
        Err(ended) => ended,
    };
    if ended == Ended::Stopped(BehaviorError::Cancelled) {
        driver.take_back_once().await;
    }
    // Released on every exit. A release is not a command.
    handle.release(token);
    let changed = [
        ("stance", driver.stance_before.as_deref()),
        ("language", driver.speech_before.as_deref()),
    ];
    if let Some(why) = &driver.halted {
        handle.say(Notice::line(NoticeKind::Error, format!("Travel: {why}")));
    }
    let still_out = driver.taken.map(|taken| taken.name);
    for notice in report(ended, &driver.stored, &changed, still_out.as_deref()) {
        handle.say(notice);
    }
    Travelled {
        ended,
        still_stored: driver.stored,
        stance_before: driver.stance_before,
        wrong_for_the_map: trip.wrong_for_the_map().to_vec(),
        seed,
        halted: driver.halted,
        last_room: driver.was.map(|(_, room)| room),
        still_out,
    }
}

/// Which room of the map the model's room is, by the game's number first
/// (`cena_map::locate`). `None` when the map cannot name it unambiguously.
///
/// **A room with no number yet is still looked for**, by its title,
/// description and exits. This returned `None` without a number, and the
/// first live login showed why that is wrong: the burst describes the room
/// twice, and the number comes only with the second -- MEASURED on that
/// session's log, the title at frame 11 and `<nav rm='7086'/>` at frame 378.
/// Asked in between, this said "I cannot tell which room this is" of a room
/// whose name, description and exits it had been holding for 367 frames.
///
/// Public because the walker is not the only one who asks: showing a route
/// starts from the same question, and must get the same answer.
#[must_use]
pub fn room_of(map: &Map, state: &GameState, whence: Whence) -> Option<RoomId> {
    let room = &state.room;
    let title = room.title.as_deref().map(title_from_subtitle);
    // What tells apart rooms that share a number, or have none the map
    // knows. Safe to offer: a text that fits no candidate is ignored by
    // `locate`, not obeyed, because it is the map's text that goes stale.
    let description = room.description.as_ref().map(cena_session::Runs::plain);
    let paths = room.component("room exits").map(cena_session::Runs::plain);
    let sighting = Sighting {
        uid: room
            .id
            .as_deref()
            .and_then(|raw| raw.parse().ok())
            .filter(|uid| *uid != 0)
            .map(Uid),
        title: title.as_deref(),
        description: description.as_deref(),
        paths: paths.as_deref(),
        location: None,
    };
    match map.locate(&sighting, whence) {
        Located::Here { room, .. } => Some(room),
        _ => None,
    }
}

/// What a trip that did not simply arrive tells the player (`Notice`): why it
/// ended, and what it changed and could not put back. Arriving with nothing
/// owed says nothing -- the room is the news.
///
/// The return value carries the same facts for a caller; this is for the
/// person, who should not depend on a caller remembering to print them.
fn report(
    ended: Ended,
    stored: &[Stored],
    changed: &[(&str, Option<&str>)],
    out: Option<&str>,
) -> Vec<Notice> {
    let mut said = Vec::new();
    let why = match ended {
        // A halt has said why already, in the routine's own words.
        Ended::Arrived | Ended::Halted | Ended::Stopped(BehaviorError::Cancelled) => None,
        Ended::Stopped(BehaviorError::AuthorityHeld) => {
            Some("something else is driving this character, so I did not set out.")
        }
        Ended::Failed(Why::NoRoute) => Some("there is no way there that this character can take."),
        Ended::Failed(Why::OffTheMap) => {
            Some("I do not know what room this is, so I have stopped.")
        }
        Ended::Failed(Why::TooManyReplans) => {
            Some("I keep being carried off the route, so I have stopped.")
        }
        Ended::UnknownSpell => Some("the map names a spell that is not in the spell table."),
        Ended::Stopped(_) => Some("stopped, because the session went away."),
    };
    if let Some(why) = why {
        said.push(Notice::line(NoticeKind::Error, format!("Travel: {why}")));
    }
    if let Some(out) = out {
        said.push(Notice::line(
            NoticeKind::Warn,
            format!("Travel: your {out} is out, and I did not put it back."),
        ));
    }
    if !stored.is_empty() {
        let names: Vec<&str> = stored.iter().map(|stored| stored.name.as_str()).collect();
        said.push(Notice::line(
            NoticeKind::Warn,
            format!("Travel: still put away -- {}.", names.join(", ")),
        ));
    }
    for (what, before) in changed {
        if let Some(before) = before {
            said.push(Notice::line(
                NoticeKind::Warn,
                format!("Travel: your {what} was {before}, and I did not put it back."),
            ));
        }
    }
    said
}

/// The seed for the choices a maze asks of this trip.
///
/// # From the wire, so a replay needs nothing new
///
/// The recorder keeps the bytes that crossed the wire and nothing else
/// (`cena_platform::record`), so a seed drawn from the machine would be lost
/// and a replay would wander differently. This one is made of three things
/// the wire already said: **the game's clock at the last prompt**, the room
/// the character is in, and where it is going. Two trips through the same
/// maze differ, because the clock has moved; a replay of either does not,
/// because its prompts say the same second.
///
/// `game_time`, never `game_time_now`: the second adds the machine's own
/// elapsed time, which is exactly what a replay cannot reproduce.
#[must_use]
pub fn seed_for(state: &GameState, goal: RoomId) -> u64 {
    let room = state.room.id.as_deref().unwrap_or("");
    let mut seed = u64::from(state.game_time().unwrap_or(0));
    seed = mix(seed ^ u64::from(goal.0).rotate_left(32));
    for byte in room.bytes() {
        seed = mix(seed ^ u64::from(byte));
    }
    seed
}

/// `splitmix64`'s finaliser: every input bit reaches every output bit, so
/// clocks a second apart do not make seeds a bit apart.
const fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

struct Driver<'a, N> {
    handle: &'a SessionHandle,
    cancel: &'a CancellationToken,
    token: AuthorityToken,
    next_id: N,
    state: GameState,
    events: Heard,
    began: Instant,
    /// The model's arrival count and the room it was located as, last tick.
    was: Option<(u32, RoomId)>,
    /// Where the character was last known to be, from an earlier session:
    /// what breaks a tie before this walk has placed itself once.
    hint: Option<RoomId>,
    stored: Vec<Stored>,
    stance_before: Option<String>,
    /// How many lines of the model's open chunk the trip has heard.
    heard: usize,
    /// When the walker last stopped knowing where it is.
    lost_since: Option<Instant>,
    /// Who a crossing is still waiting for, by id.
    behind: Vec<String>,
    /// The lines heard since the last deed's command went out: its answer.
    answer: Vec<ChunkLine>,
    /// The language spoken before a crossing changed it.
    speech_before: Option<String>,
    /// What a crossing took out, and where from.
    taken: Option<Taken>,
    /// Why a routine stopped the trip, in its own words.
    halted: Option<String>,
    /// Facts asked for before the first plan (`preflight`): `urchin_access`.
    found: HashMap<String, bool>,
    /// What routines have learned on this trip (`routines::Kept`).
    kept: super::routines::Kept,
}

/// What a walk is over: the trip, and what it reads and writes.
struct Cx<'a> {
    trip: &'a mut Trip,
    map: &'a Map,
    notes: &'a mut TravelNotes,
    wrote: &'a mut (dyn FnMut(&TravelNotes) + Send),
}

/// What one [`Driver::turn`] came to.
enum Turn {
    On,
    Arrived,
    /// The steps handed to `Trip::aside` are over, and whether they worked.
    Aside(bool),
}

impl<N: FnMut() -> CommandId> Driver<'_, N> {
    async fn walk(&mut self, cx: &mut Cx<'_>) -> Ended {
        loop {
            match self.turn(cx).await {
                Ok(Turn::Arrived) => return Ended::Arrived,
                Ok(_) => {}
                Err(ended) => return ended,
            }
        }
    }

    /// One tick of the trip, and what it asked for done.
    async fn turn(&mut self, cx: &mut Cx<'_>) -> Result<Turn, Ended> {
        let Cx {
            trip,
            map,
            notes,
            wrote,
        } = cx;
        {
            if self.cancel.is_cancelled() {
                return Err(Ended::Stopped(BehaviorError::Cancelled));
            }
            self.drain(trip).map_err(Ended::Stopped)?;
            let before = self.was;
            let here = self.locate(map);
            if before.is_some() && before != self.was {
                self.arrived(notes).await?;
            }
            if here.is_some() {
                self.lost_since = None;
            } else if self.lost_since.get_or_insert_with(Instant::now).elapsed() >= LOST_WAIT {
                return Err(Ended::Failed(Why::OffTheMap));
            }
            let now = Now {
                here,
                ms: u64::try_from(self.began.elapsed().as_millis()).unwrap_or(u64::MAX),
            };
            let walker = self.walker(notes);
            match trip.tick(map, &walker, now) {
                Said::Arrived => return Ok(Turn::Arrived),
                Said::Failed(why) => return Err(Ended::Failed(why)),
                Said::Aside(worked) => return Ok(Turn::Aside(worked)),
                Said::Routine(routine) => {
                    // A routine moves by asking the trip to (`solve`), which
                    // comes back through here: boxed, as recursion must be.
                    if !Box::pin(self.solve(cx, &routine)).await? {
                        cx.trip.could_not();
                    }
                    Ok(())
                }
                Said::Hold => self.hold(trip).await,
                // A move causes no roundtime, and is verified by the room it
                // lands in: the trip's own timeout is the deadline.
                Said::Send(line) => self.send_for(trip, &line).await,
                Said::Do(Deed::Remember(name, value)) => {
                    notes.memories.insert(name, value);
                    wrote(notes);
                    Ok(())
                }
                Said::Do(Deed::Forget(name)) => {
                    notes.memories.remove(&name);
                    wrote(notes);
                    Ok(())
                }
                Said::Do(deed) => self.deed(trip, deed).await,
            }?;
        }
        Ok(Turn::On)
    }

    /// go2's two rules for a room just walked into (`go2.lic:2405-2406`):
    /// `delay` seconds are waited, and `stop_for_dead` halts the trip where
    /// someone lies dead -- upstream pauses; this stops, and is started
    /// again. Not asked of the room the trip began in, so starting again
    /// beside the body walks on.
    async fn arrived(&mut self, notes: &TravelNotes) -> Result<(), Ended> {
        let dead = |player: &cena_session::RoomItem| {
            player
                .status
                .as_ref()
                .is_some_and(|status| status.as_str().contains("dead"))
        };
        let stops = notes
            .settings
            .get("stop_for_dead")
            .is_some_and(|is| is == "true");
        if stops && self.state.room.players.iter().any(dead) {
            self.halted = Some("someone here is dead, and `stop_for_dead` is on.".to_owned());
            return Err(Ended::Halted);
        }
        let delay = notes
            .settings
            .get("delay")
            .and_then(|delay| delay.parse::<f64>().ok());
        if let Some(delay) = delay.filter(|delay| delay.is_finite() && *delay > 0.0) {
            let pause = tokio::time::sleep(Duration::from_secs_f64(delay.min(600.0)));
            tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Err(Ended::Stopped(BehaviorError::Cancelled)),
                () = pause => {}
            }
        }
        Ok(())
    }

    /// The walker's facts now: the model's, and what pre-flight found out.
    fn walker(&self, notes: &TravelNotes) -> Walker {
        let server = self.state.game_time_now().unwrap_or(0);
        let mut walker = walker_from(&self.state, notes, server);
        walker.flags.extend(self.found.clone());
        walker
    }

    /// Which room of the map the model's room is, remembering the last one
    /// so that two rooms alike in everything are told apart by the way in.
    ///
    /// **"Has it moved" is the model's arrival count, not the room number.**
    /// This compared numbers, which cannot answer for a room that has none --
    /// and two unnumbered rooms that read alike are exactly the pair the way
    /// in has to tell apart (`GameState::arrivals` exists for this).
    fn locate(&mut self, map: &Map) -> Option<RoomId> {
        let arrivals = self.state.arrivals;
        let whence = match self.was {
            Some((then, at)) if then == arrivals => Whence::Still(at),
            Some((_, at)) => Whence::Left(at),
            // Not placed yet this walk: an earlier session's word is a hint
            // that it has not moved, which only ever breaks a tie.
            None => self.hint.map_or(Whence::Nowhere, Whence::Still),
        };
        let here = room_of(map, &self.state, whence)?;
        self.was = Some((arrivals, here));
        Some(here)
    }

    /// Fold every event already waiting, without waiting for more.
    fn drain(&mut self, trip: &mut Trip) -> Result<(), BehaviorError> {
        loop {
            match self.events.try_recv() {
                Ok(event) => self.fold(trip, &event)?,
                // Events were dropped: what is known is a little old, and the
                // game's next word mends it.
                Err(TryRecvError::Lagged(_)) => {}
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Closed) => return Err(BehaviorError::Dead),
            }
        }
    }

    fn fold(&mut self, trip: &mut Trip, event: &Event) -> Result<(), BehaviorError> {
        match event {
            Event::Frame(frame) => {
                self.state.apply(frame);
                match &**frame {
                    // The chunk closed, and took its lines with it.
                    Frame::Prompt { .. } => {
                        self.heard = 0;
                        trip.prompted();
                    }
                    Frame::Text(_) => self.hear(trip),
                    _ => {}
                }
                Ok(())
            }
            // `plan/12` §5.1: no automation runs without a transport. Only
            // the two states that mean it is gone: a session still on its
            // way up to `Ready` announces each step, and has lost nothing.
            //
            // **`Closed` is the session ending, and only that.** A supervised
            // connection that drops publishes `Reconnecting` and not `Closed`
            // (`cena-session`, 2026-09-23); it used to publish `Closed` on the
            // way, which this read as death, ending a walk as `Dead` for a
            // drop the supervisor was about to mend. Under that contract the
            // mapping is exact: `Reconnecting` stops this run and expects the
            // session back, `Closed` expects nothing.
            Event::StateChanged(State::Reconnecting) => Err(BehaviorError::Disconnected),
            Event::StateChanged(State::Closed) => Err(BehaviorError::Dead),
            _ => Ok(()),
        }
    }

    /// Tell the trip the whole lines the model has finished since it last
    /// heard any.
    ///
    /// **The model rebuilds lines, once, for everyone** (`route_text`): the
    /// frontend's stream buffers and every classifier's chunk come from the
    /// same reassembly. A first version joined the pieces again here, which
    /// is a second reader of `ends_line` waiting to disagree with the first
    /// (author, 2026-09-21: *"the lines have to be rebuilt for other folks
    /// too"*). The open chunk is the main window's lines since the prompt.
    fn hear(&mut self, trip: &mut Trip) {
        let chunk = self.state.open_chunk();
        let lines = chunk.lines();
        // Counted with what a long chunk dropped from its front, so a line
        // is heard once however the buffer slides.
        let total = chunk.dropped() + lines.len();
        let fresh = total.saturating_sub(self.heard).min(lines.len());
        for line in &lines[lines.len() - fresh..] {
            trip.heard(&line.text());
            self.answer.push(line.clone());
            // Whoever rejoins is no longer behind (`await_followers`). The
            // model's classifier reads the line; this only keeps the count.
            if let Some(GroupEvent::Joined(member) | GroupEvent::Added(member)) =
                group::classify(line)
            {
                self.behind.retain(|id| *id != member.id);
            }
        }
        self.heard = total;
    }

    /// Wait for the game's next word, or a beat, whichever is first.
    async fn hold(&mut self, trip: &mut Trip) -> Result<(), Ended> {
        let event = tokio::select! {
            biased;
            () = self.cancel.cancelled() => return Err(Ended::Stopped(BehaviorError::Cancelled)),
            event = self.events.recv() => event,
            () = tokio::time::sleep(BEAT) => return Ok(()),
        };
        match event {
            Ok(event) => self.fold(trip, &event).map_err(Ended::Stopped),
            Err(RecvError::Lagged(_)) => Ok(()),
            Err(RecvError::Closed) => Err(Ended::Stopped(BehaviorError::Dead)),
        }
    }

    /// Send a line of the trip's, with the game's id for anything it names
    /// (`{item:…}`). A thing nobody has is a crossing that cannot be made.
    async fn send_for(&mut self, trip: &mut Trip, line: &str) -> Result<(), Ended> {
        let Some(line) = kept::with_items(line, &self.state) else {
            trip.cannot_send();
            return Ok(());
        };
        self.send(&line).await
    }

    async fn send(&mut self, line: &str) -> Result<(), Ended> {
        let sent = tokio::select! {
            biased;
            () = self.cancel.cancelled() => return Err(Ended::Stopped(BehaviorError::Cancelled)),
            sent = self.handle.send_now(line, Origin::Behavior(self.token), Gate::None) => sent,
        };
        match sent {
            // A refusal is a full queue: the trip's timeout sends it again.
            Sent::Ok { .. } | Sent::Refused(_) => Ok(()),
            Sent::Dead => Err(Ended::Stopped(BehaviorError::Dead)),
            Sent::Interrupted => Err(Ended::Stopped(BehaviorError::Disconnected)),
        }
    }

    /// Send one command of a deed and wait for the prompt that answers it.
    async fn exchange(&mut self, trip: &mut Trip, line: &str) -> Result<(), Ended> {
        self.answer.clear();
        let outcome = tokio::select! {
            biased;
            () = self.cancel.cancelled() => return Err(Ended::Stopped(BehaviorError::Cancelled)),
            outcome = self.handle.send_and_await(
                (self.next_id)(),
                line,
                Origin::Behavior(self.token),
                DEED_DEADLINE,
                |frame| matches!(frame, Frame::Prompt { .. }),
            ) => outcome,
        };
        // An older connection's command is not a stop (module docs).
        match BehaviorError::from_outcome(&outcome) {
            Some(gone) => Err(Ended::Stopped(gone)),
            None => self.drain(trip).map_err(Ended::Stopped),
        }
    }

    /// The author's one exception to "cleanup cannot send": one command per
    /// stored item, not waited for. They stay in `stored`, since nothing saw
    /// them come back.
    async fn take_back_once(&mut self) {
        for stored in self.stored.iter().rev() {
            let command = take_back(&self.state, stored);
            let origin = Origin::Behavior(self.token);
            let _ = self.handle.send_now(&command, origin, Gate::None).await;
        }
    }
}
