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

mod recovery;

use std::collections::{HashSet, VecDeque};

use cena_map::{Cond, Crossing, Exit, Map, Room, RoomId, Target, Walker, priced_for};
use cena_session::MoveFeedback;

use recovery::{Attempt, Reaction};
pub use recovery::{MAX_REMEDIES, MAX_ROLLS};

/// How many times one trip may plan again before it gives up. A walker that
/// is carried somewhere unexpected replans; one that is carried somewhere
/// unexpected *forever* has met something this machine does not understand,
/// and says so rather than walking in circles.
pub const MAX_REPLANS: u32 = 20;

/// How long a move may go unanswered before it is sent again. Vellum's
/// `STEP_TIMEOUT_MS`; Lich gives up after ten seconds.
pub const STEP_TIMEOUT_MS: u64 = 8000;

/// How many times an unanswered move is sent again. Vellum's
/// `MAX_EDGE_RETRIES`.
pub const MAX_RESENDS: u32 = 2;

/// After giving an exit up, how long lines are ignored: they are about the
/// move just abandoned. Vellum's orphan window.
pub const ORPHAN_MS: u64 = 1500;

/// How many times the trip will send `stand` before a move. Vellum's
/// `MAX_STAND_ATTEMPTS`.
pub const MAX_STANDS: u32 = 5;

/// How often a stunned walker is looked at again.
const STUN_POLL_MS: u64 = 500;

/// What the trip asks of whoever drives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// Send this, and tick again.
    Send(String),
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

/// A move that has been sent and not yet seen to land.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    /// Where the walker stood when it was sent.
    from: RoomId,
    /// The room the exit leaves in the map: `from`, unless the path passed
    /// through a room only the map has. It is what a ban names.
    leaving: RoomId,
    /// Where it should land.
    expected: RoomId,
    attempt: Attempt,
    sent_at: u64,
    resends: u32,
    /// Nothing is sent before this.
    hold_until: u64,
    /// The move is to be sent again once the hold runs out.
    again: bool,
    /// ...and not while the walker is stunned.
    unstunned: bool,
}

/// One journey to one room.
#[derive(Debug, Clone)]
pub struct Trip {
    goal: RoomId,
    /// The rooms still to pass through, the next first and the goal last.
    /// Empty until planned, and again once there.
    ahead: Vec<RoomId>,
    pending: Option<Pending>,
    /// Exits this trip will not try again, as `(from, to)`.
    banned: HashSet<(RoomId, RoomId)>,
    /// Of those, the ones the game said do not exist: facts about the map.
    wrong: Vec<(RoomId, RoomId)>,
    /// How many times the trip has planned, the first included.
    plans: u32,
    /// Set once the trip has ended short; the answer it goes on giving.
    ended: Option<Why>,
    /// Lines heard since the last tick.
    inbox: Vec<MoveFeedback>,
    /// Remedies to send before anything else.
    outbox: VecDeque<String>,
    deaf_until: u64,
    /// The game said a move landed where no room change could show it (pitch
    /// dark, a long swim): while the walker still seems to be in `.0` it is
    /// taken to be in `.1`.
    believed: Option<(RoomId, RoomId)>,
    first_room: Option<RoomId>,
    left_first_room: bool,
    stands: u32,
}

impl Trip {
    /// A trip to `goal`. Nothing is planned until the first [`Self::tick`],
    /// because where it starts is not known until then.
    #[must_use]
    pub fn to(goal: RoomId) -> Trip {
        Trip {
            goal,
            ahead: Vec::new(),
            pending: None,
            banned: HashSet::new(),
            wrong: Vec::new(),
            plans: 0,
            ended: None,
            inbox: Vec::new(),
            outbox: VecDeque::new(),
            deaf_until: 0,
            believed: None,
            first_room: None,
            left_first_room: false,
            stands: 0,
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

    /// A line the game sent that says something about a move. Kept for the
    /// next [`Self::tick`]; see the module docs for why nothing happens here.
    pub fn heard(&mut self, feedback: MoveFeedback) {
        self.inbox.push(feedback);
    }

    /// Where the walker is, what time it is, and what it knows of itself.
    /// Answers with what to do.
    ///
    /// Safe to call as often as the driver likes: a move under way answers
    /// [`Said::Hold`] and is not sent twice, and a finished trip goes on
    /// giving the answer it finished with.
    pub fn tick(&mut self, map: &Map, walker: &Walker, now: Now) -> Said {
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
        if here == self.goal {
            self.pending = None;
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
        if let Some(pending) = &self.pending {
            if here == pending.from {
                return self.still_there(map, walker, now.ms);
            }
            // The room changed: whatever was heard was about a move that
            // worked, or about nothing.
            if here != pending.expected {
                self.ahead.clear();
            }
            self.pending = None;
            self.outbox.clear();
        }
        self.inbox.clear();
        self.step_from(map, walker, here, now.ms)
    }

    /// A move is under way and the walker has not left.
    fn still_there(&mut self, map: &Map, walker: &Walker, ms: u64) -> Said {
        let standing = walker.posture.as_deref().is_none_or(|is| is == "standing");
        let heard = std::mem::take(&mut self.inbox);
        if ms >= self.deaf_until {
            for feedback in heard {
                let Some(pending) = self.pending.as_mut() else {
                    break;
                };
                match pending.attempt.react(feedback, standing) {
                    Reaction::Again {
                        first,
                        after_ms,
                        unstunned,
                    } => {
                        self.outbox.extend(first);
                        pending.hold_until = ms + after_ms;
                        pending.again = true;
                        pending.unstunned = unstunned;
                    }
                    Reaction::GiveUp { wrong_for_map } => {
                        return self.give_up(map, walker, ms, true, wrong_for_map);
                    }
                    Reaction::Landed => {
                        let (from, landed) = (pending.from, pending.expected);
                        self.believed = Some((from, landed));
                        self.pending = None;
                        self.outbox.clear();
                        if landed == self.goal {
                            return Said::Arrived;
                        }
                        return self.step_from(map, walker, landed, ms);
                    }
                }
            }
        }
        if let Some(remedy) = self.outbox.pop_front() {
            return Said::Send(remedy);
        }
        let Some(pending) = self.pending.as_mut() else {
            return Said::Hold;
        };
        if ms < pending.hold_until {
            return Said::Hold;
        }
        if pending.again {
            if pending.unstunned && is_stunned(walker) {
                pending.hold_until = ms + STUN_POLL_MS;
                return Said::Hold;
            }
            pending.again = false;
            pending.sent_at = ms;
            return Said::Send(pending.attempt.sent.clone());
        }
        if ms.saturating_sub(pending.sent_at) < STEP_TIMEOUT_MS {
            return Said::Hold;
        }
        // Silence. Not a failure: the game may only be slow.
        if pending.resends < MAX_RESENDS {
            pending.resends += 1;
            pending.sent_at = ms;
            return Said::Send(pending.attempt.sent.clone());
        }
        let ban = !self.left_first_room;
        self.give_up(map, walker, ms, ban, false)
    }

    /// Stop trying the exit under way, and plan again from here.
    fn give_up(&mut self, map: &Map, walker: &Walker, ms: u64, ban: bool, wrong: bool) -> Said {
        let Some(pending) = self.pending.take() else {
            return Said::Hold;
        };
        self.outbox.clear();
        self.inbox.clear();
        self.deaf_until = ms + ORPHAN_MS;
        if ban {
            self.banned.insert((pending.leaving, pending.expected));
        }
        if wrong {
            self.wrong.push((pending.leaving, pending.expected));
        }
        self.replan(map, walker, pending.from, ms)
    }

    fn step_from(&mut self, map: &Map, walker: &Walker, here: RoomId, ms: u64) -> Said {
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
            match &exit.crossing {
                Crossing::PassThrough(_) => {
                    self.ahead.remove(0);
                    leaving = next;
                }
                Crossing::Command(command) => {
                    if is_stunned(walker) {
                        return Said::Hold;
                    }
                    if self.must_stand_first(walker, command) {
                        return Said::Send("stand".to_owned());
                    }
                    self.ahead.remove(0);
                    self.stands = 0;
                    self.pending = Some(Pending {
                        from: here,
                        leaving,
                        expected: next,
                        attempt: Attempt::new(command),
                        sent_at: ms,
                        resends: 0,
                        hold_until: 0,
                        again: false,
                        unstunned: false,
                    });
                    return Said::Send(command.clone());
                }
                _ => return self.replan(map, walker, here, ms),
            }
        }
    }

    /// Vellum stands before it moves, unless the move is a swim or a pedal
    /// (`tick_prepare`). A walker whose posture is not known is left alone:
    /// the game will say "you must be standing" if it matters, and that has
    /// its own remedy.
    fn must_stand_first(&mut self, walker: &Walker, command: &str) -> bool {
        let down = walker.posture.as_deref().is_some_and(|is| is != "standing");
        let afloat = command.contains("swim") || command.contains("pedal");
        if down && !afloat && self.stands < MAX_STANDS {
            self.stands += 1;
            return true;
        }
        false
    }

    fn replan(&mut self, map: &Map, walker: &Walker, here: RoomId, ms: u64) -> Said {
        self.ahead.clear();
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
    fn pricing<'a>(&'a self, walker: &'a Walker) -> impl Fn(&Room, &Exit) -> Option<f64> + 'a {
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
        self.pending = None;
        self.ahead.clear();
        Said::Failed(why)
    }
}

fn is_stunned(walker: &Walker) -> bool {
    Cond::Flag("stunned".to_owned()).holds(walker)
}

/// What this stage of the walker can cross. The rest is priced shut, so the
/// pathfinder goes round it rather than the trip failing at it.
fn can_cross(crossing: &Crossing) -> bool {
    matches!(crossing, Crossing::Command(_) | Crossing::PassThrough(_))
}
