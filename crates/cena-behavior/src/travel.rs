//! Travel: the walk, as a **pure state machine** (`plan/24`, `plan/21` §4.0).
//!
//! [`Trip::tick`] is told where the walker is and answers with what to do. It
//! owns no socket, reads no clock and draws no random numbers, so a replay
//! walks the same way (`plan/12` §7.2 criterion 7) and every rule below can be
//! tested by table. The driver that feeds it frames and sends what it asks
//! for is a later stage (`plan/24` §3, stage 4), and is thin by design.
//!
//! # Stage 1: plain exits
//!
//! What is here crosses [`Crossing::Command`] and [`Crossing::PassThrough`] --
//! 83,270 of the map's 84,867 exits -- and **routes around everything else**:
//! an exit this build of the walker cannot cross yet is priced shut by the
//! trip's own pricing, exactly as a banned one is, so the pathfinder never
//! offers it. Steps and routines arrive with the stages that can run them.
//!
//! # Arrival is where the walker *is*, not what the game said
//!
//! A move has arrived when the located room is the one expected (Vellum,
//! `executor.rs:1035`). Landing anywhere else is not an error to explain but
//! a fact to plan from: the trip replans from wherever it now is.

use std::collections::HashSet;

use cena_map::{Crossing, Exit, Map, Room, RoomId, Target, Walker, priced_for};

/// How many times one trip may plan again before it gives up. A walker that
/// is carried somewhere unexpected replans; one that is carried somewhere
/// unexpected *forever* has met something this machine does not understand,
/// and says so rather than walking in circles.
pub const MAX_REPLANS: u32 = 20;

/// What the trip asks of whoever drives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// Send this, and tick again when the walker's room is next known.
    Send(String),
    /// Nothing to do yet: the room is not known, or a move is still under way.
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

/// A move that has been sent and not yet seen to land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pending {
    /// Where the walker stood when it was sent.
    from: RoomId,
    /// Where it should land.
    expected: RoomId,
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
    /// How many times the trip has planned, the first included.
    plans: u32,
    /// Set once the trip has ended short; the answer it goes on giving.
    ended: Option<Why>,
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
            plans: 0,
            ended: None,
        }
    }

    /// Never offer this exit to the trip again. For whoever learns that an
    /// exit cannot be crossed -- stage 2's move recovery, which bans only when
    /// the walker never left the first room, never on lag (`plan/21` §2c).
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

    /// Where the walker is now -- `None` while the room has not been
    /// identified -- and what it knows of itself. Answers with what to do.
    ///
    /// Safe to call as often as the driver likes: a move under way answers
    /// [`Said::Hold`] and is not sent twice, and a finished trip goes on
    /// giving the answer it finished with.
    pub fn tick(&mut self, map: &Map, walker: &Walker, here: Option<RoomId>) -> Said {
        let Some(here) = here else {
            return Said::Hold;
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
        if let Some(pending) = self.pending {
            if here == pending.from {
                return Said::Hold;
            }
            self.pending = None;
            if here != pending.expected {
                // Carried somewhere else. Not a failure to explain: a fact to
                // plan from.
                self.ahead.clear();
            }
        }
        self.step_from(map, walker, here)
    }

    fn step_from(&mut self, map: &Map, walker: &Walker, here: RoomId) -> Said {
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
                return self.replan(map, walker, here);
            };
            match &exit.crossing {
                Crossing::PassThrough(_) => {
                    self.ahead.remove(0);
                    leaving = next;
                }
                Crossing::Command(command) => {
                    self.ahead.remove(0);
                    self.pending = Some(Pending {
                        from: here,
                        expected: next,
                    });
                    return Said::Send(command.clone());
                }
                _ => return self.replan(map, walker, here),
            }
        }
    }

    fn replan(&mut self, map: &Map, walker: &Walker, here: RoomId) -> Said {
        self.ahead.clear();
        match self.plan(map, walker, here) {
            Ok(()) => self.step_from(map, walker, here),
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

/// What this stage of the walker can cross. The rest is priced shut, so the
/// pathfinder goes round it rather than the trip failing at it.
fn can_cross(crossing: &Crossing) -> bool {
    matches!(crossing, Crossing::Command(_) | Crossing::PassThrough(_))
}
