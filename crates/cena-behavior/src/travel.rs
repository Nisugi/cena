//! Travel: the walk, as a **pure state machine** (`plan/24`, `plan/21` §4.0).
//!
//! [`Trip::tick`] is told where the walker is and what time it is, and
//! answers with what to do. It owns no socket, reads no clock and draws no
//! random numbers, so a replay walks the same way (`plan/12` §7.2 criterion 7)
//! and every rule below can be tested by table. The driver that feeds it
//! frames and sends what it asks for is a later stage (`plan/24` §3, stage 4),
//! and is thin by design.
//!
//! # What it crosses so far
//!
//! [`Crossing::Command`] and [`Crossing::PassThrough`] -- 83,270 of the map's
//! 84,867 exits -- and it **routes around everything else**: an exit this
//! build of the walker cannot cross yet is priced shut by the trip's own
//! pricing, exactly as a banned one is, so the pathfinder never offers it.
//! Steps and routines arrive with the stages that can run them.
//!
//! # Arrival is where the walker *is*, not what the game said
//!
//! A move has arrived when the located room is the one expected (Vellum,
//! `executor.rs:1035`). Landing anywhere else is not an error to explain but
//! a fact to plan from: the trip replans from wherever it now is.
//!
//! # When a move does not simply work (stage 2)
//!
//! The driver hands every line `cena_model::movement` can name to
//! [`Trip::heard`]. Nothing is done with it there: it waits for the next
//! tick, **so that a room change is always looked at first**. A failure line
//! that raced an arrival belongs to a move that worked, and Vellum recorded
//! acting on one as a live bug (`plan/21` §2c). The remedies and their
//! budgets are Lich's ([`recovery`]). The rules around them are Vellum's
//! lessons, kept as requirements:
//!
//! - **what is sent again is what was sent**, never the map's text, since a
//!   remedy may have rewritten it;
//! - after an exit is given up there is an **orphan window** in which lines
//!   are ignored, because they are about the move just abandoned;
//! - **silence is not failure**: a move that gets no answer is sent again a
//!   couple of times, and the exit is banned only if the trip has never left
//!   its first room. Lag further along must never cost the map an exit.
//!
//! # Crossings that are lists of steps (stage 3)
//!
//! [`steps`] runs them, and a plain exit is the one-step list `[move]`, so
//! there is one way across. What the trip cannot spell as a command it hands
//! to the driver as a [`Deed`]. **Whatever a crossing changed is owed back
//! before the trip says it is over** -- hands, stance -- on arrival, on
//! failure, and on a user's stop ([`Trip::owed`]), which is the one Vellum
//! skipped (`plan/21` §4.0).

mod drive;
mod facts;
mod hands;
mod itinerary;
mod kept;
mod knows;
mod mover;
mod preflight;
mod recovery;
mod replies;
mod routines;
mod steps;

use std::collections::HashSet;

use cena_map::{Action, Crossing, Exit, Room, Routine, Step, Target, Walker, priced_for};
use cena_session::{MoveFeedback, movement};

// What a caller needs to start a trip or show a route, so that it does not
// reach past this crate for them: the binary has no edge to `cena-map`
// (`layering.rs`), and should not need one to say "walk to the bank".
pub use cena_map::binary::{LoadError, decode as read_map};
pub use cena_map::{Map, Origin as Whence, RoomId};
pub use drive::{
    BEAT, DEED_DEADLINE, Ended, FOLLOW_WAIT, LOST_WAIT, Travelled, room_of, seed_for, travel,
};
pub use facts::{TravelNotes, walker_from};
pub use hands::{Stored, cast_commands, store_commands, take_back};
pub use itinerary::{Leg, Shut, ShutWhy, destination, itinerary, table};
pub use recovery::{MAX_REMEDIES, MAX_ROLLS};
pub use steps::{Deed, EXCHANGE_TIMEOUT_MS, MAX_RESENDS, MAX_TURNS, MAX_WAIT_MS, STEP_TIMEOUT_MS};
use steps::{Out, Owes, Run, Tick};

/// How many times one trip may plan again before it gives up. A walker that
/// is carried somewhere unexpected replans; one that is carried somewhere
/// unexpected *forever* has met something this machine does not understand,
/// and says so rather than walking in circles.
pub const MAX_REPLANS: u32 = 20;

/// After giving an exit up, how long lines are ignored: they are about the
/// move just abandoned. Vellum's orphan window.
pub const ORPHAN_MS: u64 = 1500;

/// How many times the trip will send `stand` before a move. Vellum's
/// `MAX_STAND_ATTEMPTS`.
pub const MAX_STANDS: u32 = 5;

/// What the trip asks of whoever drives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// Send this, and tick again.
    Send(String),
    /// Do this to completion, and tick again.
    Do(Deed),
    /// The next exit is crossed by a named routine (`cena_map::Routine`):
    /// run it to its end, say [`Trip::could_not`] if it failed, and tick
    /// again. Where it landed is then looked at like any other arrival.
    Routine(Routine),
    /// The steps handed to [`Trip::aside`] are over, and whether they
    /// worked.
    Aside(bool),
    /// Nothing to do yet: the room is not known, a move is under way, or a
    /// wait has not run out. Tick again on the next line, or the next beat
    /// of the clock.
    Hold,
    /// The walker is at the destination. The trip is over.
    Arrived,
    /// The trip is over, and did not get there.
    Failed(Why),
}

/// Why a trip ended short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The map has no way from here to there that this walker can take.
    NoRoute,
    /// The walker is in a room the map does not have.
    OffTheMap,
    /// Planned again [`MAX_REPLANS`] times without arriving.
    TooManyReplans,
}

/// What the driver knows this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Now {
    /// The located room; `None` while it has not been identified.
    pub here: Option<RoomId>,
    /// Milliseconds on any steady clock. Passed in, never read, so a replay
    /// keeps the same time.
    pub ms: u64,
}

/// One journey to one room.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // each is its own fact about the walk
pub struct Trip {
    goal: RoomId,
    /// The rooms still to pass through, the next first and the goal last.
    /// Empty until planned, and again once there.
    ahead: Vec<RoomId>,
    /// The exit being crossed.
    run: Option<Run>,
    /// Exits this trip will not try again, as `(from, to)`.
    banned: HashSet<(RoomId, RoomId)>,
    /// Of those, the ones the game said do not exist: facts about the map.
    wrong: Vec<(RoomId, RoomId)>,
    /// How many times the trip has planned, the first included.
    plans: u32,
    /// Set once the trip has ended short; the answer it goes on giving.
    ended: Option<Why>,
    /// Lines heard since the last tick: named, and as text.
    feedback: Vec<MoveFeedback>,
    lines: Vec<String>,
    prompted: bool,
    deaf_until: u64,
    /// The game said a move landed where no room change could show it (pitch
    /// dark, a long swim): while the walker still seems to be in `.0` it is
    /// taken to be in `.1`.
    believed: Option<(RoomId, RoomId)>,
    first_room: Option<RoomId>,
    left_first_room: bool,
    stands: u32,
    /// What the crossings have changed and not yet put back.
    owes: Owes,
    /// The driver could not do the deed it was last handed.
    could_not: bool,
    /// A routine was handed over: the exit it crosses, as `(leaving,
    /// expected)`, and the room the walker was then in.
    routine: Option<(RoomId, RoomId, RoomId)>,
    /// Steps to run beside the plan, not yet begun ([`Self::aside`]).
    aside: Option<(Vec<Step>, Option<RoomId>)>,
    /// The crossing under way is those steps.
    in_aside: bool,
    /// xorshift64. Seeded, so a replay takes the same turns in a maze.
    random: u64,
}

impl Trip {
    /// A trip to `goal`. Nothing is planned until the first [`Self::tick`],
    /// because where it starts is not known until then.
    #[must_use]
    pub fn to(goal: RoomId) -> Trip {
        Trip::seeded(goal, 0x9E37_79B9_7F4A_7C15)
    }

    /// [`Self::to`], with the seed for the choices a maze asks for. The
    /// session records it, so a replay walks the same way.
    #[must_use]
    pub fn seeded(goal: RoomId, seed: u64) -> Trip {
        Trip {
            goal,
            ahead: Vec::new(),
            run: None,
            banned: HashSet::new(),
            wrong: Vec::new(),
            plans: 0,
            ended: None,
            feedback: Vec::new(),
            lines: Vec::new(),
            prompted: false,
            deaf_until: 0,
            believed: None,
            first_room: None,
            left_first_room: false,
            stands: 0,
            owes: Owes::default(),
            could_not: false,
            routine: None,
            aside: None,
            in_aside: false,
            // xorshift has one bad seed.
            random: seed.max(1),
        }
    }

    /// Never offer this exit to the trip again.
    pub fn ban(&mut self, from: RoomId, to: RoomId) {
        self.banned.insert((from, to));
        self.ahead.clear();
    }

    /// How many times this trip has planned again.
    #[must_use]
    pub fn replans(&self) -> u32 {
        self.plans.saturating_sub(1)
    }

    /// The rooms still ahead, the goal last. Empty before the first plan.
    #[must_use]
    pub fn ahead(&self) -> &[RoomId] {
        &self.ahead
    }

    /// Exits the game said do not exist -- "You can't go there" -- as
    /// `(from, to)`. The trip only goes round them; whether the map should
    /// change is for whoever reads this.
    #[must_use]
    pub fn wrong_for_the_map(&self) -> &[(RoomId, RoomId)] {
        &self.wrong
    }

    /// A line of text from the game. Kept for the next [`Self::tick`]: see
    /// the module docs for why nothing happens here.
    pub fn heard(&mut self, line: &str) {
        self.feedback.extend(movement::classify(line));
        self.lines.push(line.to_owned());
    }

    /// The game has prompted: whatever was sent has been answered.
    pub fn prompted(&mut self) {
        self.prompted = true;
    }

    /// The deed just handed over could not be done: there is no such key.
    /// The crossing that asked for it is given up, and the trip goes round.
    pub fn could_not(&mut self) {
        self.could_not = true;
    }

    /// Run these steps from wherever the walker is, **beside the plan**: a
    /// routine's `go door` gets the whole ladder a plain exit has, and its
    /// deeds are owed back like any crossing's. `to` is where they should
    /// land, when that is known. The answer is [`Said::Aside`].
    pub fn aside(&mut self, steps: Vec<Step>, to: Option<RoomId>) {
        self.aside = Some((steps, to));
    }

    /// The rooms this trip would walk from `from` to `to`, **both included**,
    /// by its own pricing. For pre-flight, which prices a walk in silver.
    #[must_use]
    pub fn path_from(
        &self,
        map: &Map,
        walker: &Walker,
        from: RoomId,
        to: RoomId,
    ) -> Option<Vec<RoomId>> {
        let routes = map.routes(from, Target::Room(to), self.pricing(walker));
        let mut path = vec![from];
        path.extend(routes.path_to(to)?);
        Some(path)
    }

    /// Where the routine just handed over is meant to land.
    #[must_use]
    pub fn routine_to(&self) -> Option<RoomId> {
        self.routine.map(|(_, expected, _)| expected)
    }

    /// A number from the trip's seed, for a routine's own choices.
    pub fn draw(&mut self) -> u64 {
        self.next_random()
    }

    /// What was last asked to be sent names a thing nobody has (`{item:…}`):
    /// the exit is given up for this trip.
    pub fn cannot_send(&mut self) {
        if let Some(run) = self.run.take() {
            self.banned.insert((run.leaving, run.expected));
            self.owes = run.owes;
        }
        self.ahead.clear();
    }

    /// What the trip has changed and not yet put back, **for a driver that is
    /// stopping it**: a user's stop asks once for what is stored (`drive`). Taking
    /// it clears it.
    pub fn owed(&mut self) -> Vec<Deed> {
        let mut owed = Vec::new();
        // The key first: it goes back with the hands as they are.
        if std::mem::take(&mut self.owes.taken) {
            owed.push(Deed::PutBack);
        }
        if std::mem::take(&mut self.owes.speech) {
            owed.push(Deed::RestoreSpeech);
        }
        if std::mem::take(&mut self.owes.stance) {
            owed.push(Deed::RestoreStance);
        }
        if std::mem::take(&mut self.owes.hands) {
            owed.push(Deed::FillHands);
        }
        owed
    }

    /// Where the walker is, what time it is, and what it knows of itself.
    /// Answers with what to do.
    ///
    /// Safe to call as often as the driver likes: a move under way answers
    /// [`Said::Hold`] and is not sent twice, and a finished trip goes on
    /// giving the answer it finished with.
    pub fn tick(&mut self, map: &Map, walker: &Walker, now: Now) -> Said {
        let said = self.tick_inner(map, walker, now);
        // Whatever ends the trip, what it changed is put back first.
        if matches!(said, Said::Arrived | Said::Failed(_))
            && let Some(deed) = self.owed().into_iter().next()
        {
            return Said::Do(deed);
        }
        said
    }

    fn tick_inner(&mut self, map: &Map, walker: &Walker, now: Now) -> Said {
        let Some(seen) = now.here else {
            return Said::Hold;
        };
        let here = match self.believed {
            Some((from, landed)) if seen == from => landed,
            _ => {
                self.believed = None;
                seen
            }
        };
        let feedback = std::mem::take(&mut self.feedback);
        let lines = std::mem::take(&mut self.lines);
        let prompted = std::mem::take(&mut self.prompted);
        let could_not = std::mem::take(&mut self.could_not);
        if let Some((steps, to)) = self.aside.take() {
            // Nowhere the map has, when where it lands is not known.
            let to = to.unwrap_or(RoomId(u32::MAX));
            self.run = Some(Run::new(here, here, to, steps));
            self.in_aside = true;
        }
        // A routine's own moves are asides: it is over when none is under way.
        if !self.in_aside
            && let Some((leaving, expected, from)) = self.routine.take()
            && here != expected
        {
            // It said it could not, or it ran and the walker never moved:
            // trying it again would do the same again.
            if could_not || here == from {
                self.banned.insert((leaving, expected));
            }
            return self.replan(map, walker, here, now.ms);
        }
        // At the goal -- but a crossing under way is finished first: the door
        // behind the walker is still to be closed and locked, the hands still
        // to be filled.
        if here == self.goal && self.run.is_none() {
            self.ahead.clear();
            return Said::Arrived;
        }
        if let Some(why) = self.ended {
            // Finished short, and then ticked again: the answer stands.
            return Said::Failed(why);
        }
        if *self.first_room.get_or_insert(here) != here {
            self.left_first_room = true;
        }
        let Some(mut run) = self.run.take() else {
            return self.step_from(map, walker, here, now.ms);
        };
        // Inside the orphan window, what is heard is about a move given up.
        let deaf = now.ms < self.deaf_until;
        let out = run.tick(&Tick {
            walker,
            here,
            ms: now.ms,
            feedback: if deaf { &[] } else { &feedback },
            lines: if deaf { &[] } else { &lines },
            prompted,
            left_first_room: self.left_first_room,
            random: self.next_random(),
            could_not,
        });
        self.owes = run.owes;
        if self.in_aside {
            return self.aside_said(run, out, now.ms);
        }
        match out {
            Out::Send(command) => {
                self.run = Some(run);
                Said::Send(command)
            }
            Out::Deed(deed) => {
                self.run = Some(run);
                Said::Do(deed)
            }
            Out::Hold => {
                self.run = Some(run);
                Said::Hold
            }
            Out::Believed(landed) => {
                self.believed = Some((seen, landed));
                if landed == self.goal {
                    return Said::Arrived;
                }
                self.step_from(map, walker, landed, now.ms)
            }
            Out::GiveUp { ban, wrong } => {
                self.deaf_until = now.ms + ORPHAN_MS;
                if ban {
                    self.banned.insert((run.leaving, run.expected));
                }
                if wrong {
                    self.wrong.push((run.leaving, run.expected));
                }
                self.replan(map, walker, here, now.ms)
            }
            Out::Replan => self.replan(map, walker, here, now.ms),
            Out::Done => {
                if here == run.expected {
                    return self.step_from(map, walker, here, now.ms);
                }
                // Every step ran and the walker is not where the exit leads.
                // If it never moved at all, this exit did nothing and trying
                // it again would do nothing again.
                if here == run.from {
                    self.banned.insert((run.leaving, run.expected));
                }
                self.replan(map, walker, here, now.ms)
            }
        }
    }

    /// What a tick of steps run beside the plan comes to ([`Self::aside`]).
    fn aside_said(&mut self, run: Run, out: Out, ms: u64) -> Said {
        match out {
            Out::Send(command) => {
                self.run = Some(run);
                Said::Send(command)
            }
            Out::Deed(deed) => {
                self.run = Some(run);
                Said::Do(deed)
            }
            Out::Hold => {
                self.run = Some(run);
                Said::Hold
            }
            Out::Done | Out::Believed(_) => {
                self.in_aside = false;
                Said::Aside(true)
            }
            Out::GiveUp { .. } | Out::Replan => {
                self.in_aside = false;
                self.deaf_until = ms + ORPHAN_MS;
                Said::Aside(false)
            }
        }
    }

    fn step_from(&mut self, map: &Map, walker: &Walker, here: RoomId, ms: u64) -> Said {
        if here == self.goal {
            self.ahead.clear();
            return Said::Arrived;
        }
        if map.room(here).is_none() {
            return self.fail(Why::OffTheMap);
        }
        if self.ahead.is_empty()
            && let Err(why) = self.plan(map, walker, here)
        {
            return self.fail(why);
        }
        // The room the next exit leaves. It is `here` unless the path passes
        // through a room that exists only in the map (an urchin hub): that
        // hop sends nothing, and the hub's own exit is sent from `here`.
        let mut leaving = here;
        loop {
            let Some(next) = self.ahead.first().copied() else {
                return self.fail(Why::NoRoute);
            };
            let Some(exit) = self.exit_between(map, walker, leaving, next) else {
                // The plan names an exit the map no longer has.
                return self.replan(map, walker, here, ms);
            };
            let steps = match &exit.crossing {
                Crossing::PassThrough(_) => {
                    self.ahead.remove(0);
                    leaving = next;
                    continue;
                }
                Crossing::Command(command) => vec![Step {
                    action: Action::Move(command.clone()),
                    when: None,
                }],
                Crossing::Steps(steps) => steps.clone(),
                Crossing::Routine(routine) => {
                    self.ahead.remove(0);
                    self.routine = Some((leaving, next, here));
                    return Said::Routine(routine.clone());
                }
                _ => return self.replan(map, walker, here, ms),
            };
            if steps::is_stunned(walker) {
                return Said::Hold;
            }
            if self.must_stand_first(walker, &steps) {
                return Said::Send("stand".to_owned());
            }
            self.ahead.remove(0);
            self.stands = 0;
            // What an earlier crossing changed is still owed: upstream climbs
            // a ledge with empty hands and fills them at the top of the next.
            let mut run = Run::new(here, leaving, next, steps);
            run.owes = self.owes;
            self.run = Some(run);
            // The crossing's first step happens on this same tick.
            return self.tick_inner(
                map,
                walker,
                Now {
                    here: Some(self.believed.map_or(here, |(seen, _)| seen)),
                    ms,
                },
            );
        }
    }

    /// Vellum stands before it moves, unless the move is a swim or a pedal
    /// (`tick_prepare`). A walker whose posture is not known is left alone:
    /// the game will say "you must be standing" if it matters, and that has
    /// its own remedy.
    fn must_stand_first(&mut self, walker: &Walker, steps: &[Step]) -> bool {
        let down = walker.posture.as_deref().is_some_and(|is| is != "standing");
        let afloat = steps.iter().any(|step| match &step.action {
            Action::Move(command) => command.contains("swim") || command.contains("pedal"),
            _ => false,
        });
        if down && !afloat && self.stands < MAX_STANDS {
            self.stands += 1;
            return true;
        }
        false
    }

    fn replan(&mut self, map: &Map, walker: &Walker, here: RoomId, ms: u64) -> Said {
        self.ahead.clear();
        self.run = None;
        match self.plan(map, walker, here) {
            Ok(()) => self.step_from(map, walker, here, ms),
            Err(why) => self.fail(why),
        }
    }

    fn plan(&mut self, map: &Map, walker: &Walker, here: RoomId) -> Result<(), Why> {
        if self.plans > MAX_REPLANS {
            return Err(Why::TooManyReplans);
        }
        self.plans += 1;
        let routes = map.routes(here, Target::Room(self.goal), self.pricing(walker));
        self.ahead = routes.path_to(self.goal).ok_or(Why::NoRoute)?;
        Ok(())
    }

    /// The trip's own pricing: what the walker can pay, less what this stage
    /// cannot cross and what the trip has banned.
    pub(crate) fn pricing<'a>(
        &'a self,
        walker: &'a Walker,
    ) -> impl Fn(&Room, &Exit) -> Option<f64> + 'a {
        let theirs = priced_for(walker);
        move |room, exit| {
            (can_cross(&exit.crossing) && !self.banned.contains(&(room.id, exit.to)))
                .then(|| theirs(room, exit))
                .flatten()
        }
    }

    /// The exit the plan means. Two exits may join the same two rooms -- a
    /// cheap one only some walkers can take, and a dear one anyone can -- and
    /// the pathfinder chose the cheapest *this* walker can pay, so that is
    /// the one to send.
    fn exit_between<'m>(
        &self,
        map: &'m Map,
        walker: &Walker,
        from: RoomId,
        to: RoomId,
    ) -> Option<&'m Exit> {
        let room = map.room(from)?;
        let price = self.pricing(walker);
        room.exits
            .iter()
            .filter(|exit| exit.to == to)
            .filter_map(|exit| Some((price(room, exit)?, exit)))
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, exit)| exit)
    }

    fn fail(&mut self, why: Why) -> Said {
        self.ended = Some(why);
        self.run = None;
        self.ahead.clear();
        Said::Failed(why)
    }

    fn next_random(&mut self) -> u64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        self.random
    }
}

/// What the walker can cross so far. The rest is priced shut, so the
/// pathfinder goes round it rather than the trip failing at it.
fn can_cross(crossing: &Crossing) -> bool {
    match crossing {
        Crossing::Command(_) | Crossing::PassThrough(_) | Crossing::Steps(_) => true,
        Crossing::Routine(routine) => routines::is_built(routine),
        _ => false,
    }
}
