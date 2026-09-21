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
//! what it was.
//!
//! A dead or disconnected session sends nothing at all.

use std::time::Duration;

use cena_map::{Located, Map, Origin as Whence, RoomId, Sighting, Uid, title_from_subtitle};
use cena_session::group::{self, GroupEvent};
use cena_session::{
    AuthorityToken, CommandId, Event, Frame, GameState, Gate, Notice, NoticeKind, Origin, Outcome,
    Sent, SessionHandle, Snapshot, State,
};
use tokio::sync::broadcast::{Receiver, error::RecvError, error::TryRecvError};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::hands::{Stored, cast_commands, store_commands, take_back};
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
    joined: (Snapshot, Receiver<Event>),
    map: &Map,
    goal: RoomId,
    notes: &mut TravelNotes,
    wrote: impl FnMut(&TravelNotes),
) -> Travelled {
    if handle.claim(token).await.is_err() {
        return Travelled {
            ended: Ended::Stopped(BehaviorError::AuthorityHeld),
            still_stored: Vec::new(),
            stance_before: None,
            wrong_for_the_map: Vec::new(),
            seed: 0,
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
        events,
        began: Instant::now(),
        was: None,
        stored: Vec::new(),
        stance_before: None,
        heard: 0,
        lost_since: None,
        behind: Vec::new(),
    };
    let ended = driver.walk(&mut trip, map, notes, wrote).await;
    if ended == Ended::Stopped(BehaviorError::Cancelled) {
        driver.take_back_once().await;
    }
    // Released on every exit. A release is not a command.
    handle.release(token);
    for notice in report(ended, &driver.stored, driver.stance_before.as_deref()) {
        handle.say(notice);
    }
    Travelled {
        ended,
        still_stored: driver.stored,
        stance_before: driver.stance_before,
        wrong_for_the_map: trip.wrong_for_the_map().to_vec(),
        seed,
    }
}

/// Which room of the map the model's room is, by the game's number first
/// (`cena_map::locate`). `None` when the game has not said, or the map cannot
/// name it unambiguously.
///
/// Public because the walker is not the only one who asks: showing a route
/// starts from the same question, and must get the same answer.
#[must_use]
pub fn room_of(map: &Map, state: &GameState, whence: Whence) -> Option<RoomId> {
    let room = &state.room;
    let raw = room.id.as_deref()?;
    let title = room.title.as_deref().map(title_from_subtitle);
    // What tells apart rooms that share a number, or have none the map
    // knows. Safe to offer: a text that fits no candidate is ignored by
    // `locate`, not obeyed, because it is the map's text that goes stale.
    let description = room.description.as_ref().map(cena_session::Runs::plain);
    let paths = room.component("room exits").map(cena_session::Runs::plain);
    let sighting = Sighting {
        uid: raw.parse().ok().filter(|uid| *uid != 0).map(Uid),
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
fn report(ended: Ended, stored: &[Stored], stance_before: Option<&str>) -> Vec<Notice> {
    let mut said = Vec::new();
    let why = match ended {
        Ended::Arrived | Ended::Stopped(BehaviorError::Cancelled) => None,
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
    if !stored.is_empty() {
        let names: Vec<&str> = stored.iter().map(|stored| stored.name.as_str()).collect();
        said.push(Notice::line(
            NoticeKind::Warn,
            format!("Travel: still put away -- {}.", names.join(", ")),
        ));
    }
    if let Some(stance) = stance_before {
        said.push(Notice::line(
            NoticeKind::Warn,
            format!("Travel: your stance was {stance}, and I did not put it back."),
        ));
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
    events: Receiver<Event>,
    began: Instant,
    /// The game's room id and the room it was located as, last tick.
    was: Option<(String, RoomId)>,
    stored: Vec<Stored>,
    stance_before: Option<String>,
    /// How many lines of the model's open chunk the trip has heard.
    heard: usize,
    /// When the walker last stopped knowing where it is.
    lost_since: Option<Instant>,
    /// Who a crossing is still waiting for, by id.
    behind: Vec<String>,
}

impl<N: FnMut() -> CommandId> Driver<'_, N> {
    async fn walk(
        &mut self,
        trip: &mut Trip,
        map: &Map,
        notes: &mut TravelNotes,
        mut wrote: impl FnMut(&TravelNotes),
    ) -> Ended {
        loop {
            if self.cancel.is_cancelled() {
                return Ended::Stopped(BehaviorError::Cancelled);
            }
            if let Err(gone) = self.drain(trip) {
                return Ended::Stopped(gone);
            }
            let here = self.locate(map);
            if here.is_some() {
                self.lost_since = None;
            } else if self.lost_since.get_or_insert_with(Instant::now).elapsed() >= LOST_WAIT {
                return Ended::Failed(Why::OffTheMap);
            }
            let now = Now {
                here,
                ms: u64::try_from(self.began.elapsed().as_millis()).unwrap_or(u64::MAX),
            };
            let server = self.state.game_time_now().unwrap_or(0);
            let walker = walker_from(&self.state, notes, server);
            let done = match trip.tick(map, &walker, now) {
                Said::Arrived => return Ended::Arrived,
                Said::Failed(why) => return Ended::Failed(why),
                Said::Hold => self.hold(trip).await,
                // A move causes no roundtime, and is verified by the room it
                // lands in: the trip's own timeout is the deadline.
                Said::Send(line) => self.send(&line).await,
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
            };
            if let Err(ended) = done {
                return ended;
            }
        }
    }

    /// Which room of the map the model's room is, remembering the last one
    /// so that two rooms alike in everything are told apart by the way in.
    fn locate(&mut self, map: &Map) -> Option<RoomId> {
        let raw = self.state.room.id.clone()?;
        let whence = match &self.was {
            Some((id, at)) if *id == raw => Whence::Still(*at),
            Some((_, at)) => Whence::Left(*at),
            None => Whence::Nowhere,
        };
        let here = room_of(map, &self.state, whence)?;
        self.was = Some((raw, here));
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
        match outcome {
            Outcome::Confirmed(_) | Outcome::Timeout | Outcome::Refused(_) => {
                self.drain(trip).map_err(Ended::Stopped)
            }
            Outcome::Interrupted => Err(Ended::Stopped(BehaviorError::Cancelled)),
            Outcome::Dead => Err(Ended::Stopped(BehaviorError::Dead)),
            Outcome::Disconnected => Err(Ended::Stopped(BehaviorError::Disconnected)),
        }
    }

    async fn deed(&mut self, trip: &mut Trip, deed: Deed) -> Result<(), Ended> {
        match deed {
            Deed::EmptyHands => {
                for (stored, command) in store_commands(&self.state) {
                    // Written down before it is sent: a stop between the two
                    // must still know what to take back.
                    self.stored.push(stored);
                    self.exchange(trip, &command).await?;
                }
            }
            Deed::FillHands => {
                // Last stored, first back. One command each (`hands`).
                for stored in std::mem::take(&mut self.stored).into_iter().rev() {
                    let command = take_back(&self.state, &stored);
                    let asked = self.exchange(trip, &command).await;
                    if self.state.hand_holding(&stored.id).is_none() {
                        self.stored.push(stored);
                    }
                    asked?;
                }
            }
            Deed::Cast(spell) => self.cast(trip, &spell, None).await?,
            Deed::CastAt(spell, target) => self.cast(trip, &spell, Some(&target)).await?,
            Deed::Stance(stance) => {
                // The first one replaced is the one to go back to.
                if self.stance_before.is_none() {
                    self.stance_before.clone_from(&self.state.character.stance);
                }
                self.exchange(trip, &format!("stance {stance}")).await?;
            }
            Deed::RestoreStance => {
                if let Some(before) = self.stance_before.take() {
                    self.exchange(trip, &format!("stance {before}")).await?;
                }
            }
            Deed::AwaitFollowers => self.await_followers(trip).await?,
            // Handled where the notes are: `walk`.
            Deed::Remember(..) | Deed::Forget(_) => {}
        }
        Ok(())
    }

    async fn cast(&mut self, trip: &mut Trip, spell: &str, at: Option<&str>) -> Result<(), Ended> {
        let Some(commands) = cast_commands(spell, at) else {
            return Err(Ended::UnknownSpell);
        };
        for command in commands {
            self.exchange(trip, &command).await?;
        }
        Ok(())
    }

    /// Until everyone who set out with the walker has rejoined it, or
    /// [`FOLLOW_WAIT`].
    ///
    /// Upstream's loop, from the six crossings that have it (`keys.rs`,
    /// `with_company`): a ladder or a bridge does not carry a group, so each
    /// follower crosses alone and the leader waits for
    /// `X joins your group.` or `You reach out and hold X's hand.`, striking
    /// each name as it comes, until none is left.
    ///
    /// Who is waited for is **the group, as the model has it now** -- the
    /// port of Lich's `Group` (author, 2026-09-21: *"we don't use
    /// `$group_members` ... we use what we ported from the Group module"*).
    /// Upstream's list is a global nothing in any reference sets; the group
    /// is what it stood for. Asked here and not when the trip set out, so
    /// someone who joined on the road is waited for too.
    ///
    /// Upstream's only way out of a follower who never comes is typing `go`;
    /// here it is the clock, and a stop.
    ///
    /// Listening starts now, as upstream's `clear` has it: a rejoining heard
    /// before the crossing is not one after it.
    async fn await_followers(&mut self, trip: &mut Trip) -> Result<(), Ended> {
        let group = self.state.group.members();
        self.behind = group.iter().map(|member| member.id.clone()).collect();
        let until = Instant::now() + FOLLOW_WAIT;
        while !self.behind.is_empty() && Instant::now() < until {
            self.hold(trip).await?;
        }
        self.behind.clear();
        Ok(())
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
