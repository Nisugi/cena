//! The named routines (`cena_map::Routine`, `plan/24` stage 6): crossings
//! that are a search, a puzzle or an errand, which no list of steps can say.
//!
//! # A routine is a pure solver, like the trip
//!
//! Each is a [`Solver`]: told what the walker [`Seen`] -- the room, what is in
//! it, the game's answer to the last thing sent -- it answers with the
//! [`Next`] thing to do. No socket, no clock, no sleeping, so each is tested
//! by table and replays the same way (`plan/12` §7.2 criterion 7). One loop
//! in the driver (`drive::solve`) runs them all.
//!
//! # What a solver may ask for
//!
//! The same things upstream's scripts do. `fput` is [`Next::Put`]: a command
//! and the game's answer. `move` is [`Next::Go`], which goes back through the
//! trip and so has the whole ladder of remedies a plain exit has.
//! `Script.run('go2', …)` is [`Next::WalkTo`]: a trip inside the trip.
//! Whatever must be owed back -- hands, stance -- is asked for as
//! [`Next::Steps`], so the trip that owes it knows.
//!
//! # How one ends
//!
//! [`Next::Done`] says only that the routine has no more to do. **Where the
//! walker landed is the trip's to look at**, as after any crossing: at the
//! exit's destination it walks on, anywhere else it plans again, and if it
//! never moved the exit is given up for the trip. [`Next::Failed`] gives the
//! exit up outright.
//!
//! Each solver is written from the upstream script it is pinned to
//! (`cena-mapdb-convert/src/upstream_scripts/`), which is the reference for
//! what it must do.

mod guild_password;
mod patrol;
mod signposts;

use cena_map::{RoomId, Routine, Step, Walker};
use cena_session::{ChunkLine, GameState};

/// Turns of any routine's own loop: the walker's stop, not an estimate.
pub(super) const MAX_TURNS: u32 = 200;

/// What the walker knows when a solver is asked what is next.
pub(super) struct Seen<'a> {
    /// The located room; `None` when the map cannot say.
    pub here: Option<RoomId>,
    /// The exit's destination. **Always the goal** (`cena_map::Routine`).
    pub goal: RoomId,
    /// The walker's facts, fresh: exits, what the room shows, the profile.
    pub walker: &'a Walker,
    /// The model, for what the walker's facts leave out: ids, hands, the
    /// room's title.
    pub state: &'a GameState,
    /// The lines that answered the last [`Next::Put`] or ended the last
    /// [`Next::Await`]. Empty after anything else.
    pub answer: &'a [ChunkLine],
    /// Whether the last [`Next::Go`], [`Next::Steps`], [`Next::WalkTo`] or
    /// [`Next::Await`] did what it was for. `true` before the first.
    pub ok: bool,
    /// A number from the trip's seed, fresh each time.
    pub random: u64,
}

impl Seen<'_> {
    /// Whether the room shows a thing by this noun. Upstream's `checkloot`.
    pub fn sees_noun(&self, noun: &str) -> bool {
        self.state
            .room
            .objects
            .iter()
            .any(|thing| thing.noun == noun)
    }

    /// Whether any answering line holds this.
    pub fn answered(&self, text: &str) -> bool {
        self.answer.iter().any(|line| line.text().contains(text))
    }
}

/// What a solver asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Next {
    /// Send this and show me the game's answer. Roundtime is waited out.
    Put(String),
    /// A move: send this and arrive somewhere else, with every remedy a
    /// plain exit has.
    Go(String),
    /// Run these as a crossing's steps, from here.
    Steps(Vec<Step>),
    /// Walk there by the map, and come back to me.
    WalkTo(RoomId),
    /// Wait until the game says one of these, or this many milliseconds.
    Await(Vec<String>, u64),
    /// Wait this many milliseconds.
    Pause(u64),
    /// Nothing more to do. See the module docs.
    Done,
    /// This exit cannot be crossed by this walker today.
    Failed,
}

/// One routine, part-way through.
pub(super) trait Solver {
    fn next(&mut self, seen: &Seen<'_>) -> Next;
}

/// Whether this build runs the routine. What it does not is priced shut, so
/// the pathfinder goes round it rather than the trip failing at it.
pub(super) fn is_built(routine: &Routine) -> bool {
    matches!(
        routine,
        Routine::Signposts { .. } | Routine::GuildPassword | Routine::Patrol { .. }
    )
}

/// The solver for a routine; `None` for one this build does not run.
pub(super) fn solver_for(routine: &Routine) -> Option<Box<dyn Solver + Send>> {
    Some(match routine.clone() {
        Routine::Signposts {
            verb,
            dirs,
            hands_free_in,
        } => Box::new(signposts::Signposts::new(verb, dirs, hands_free_in)),
        Routine::GuildPassword => Box::new(guild_password::GuildPassword::default()),
        Routine::Patrol {
            starts,
            dirs,
            landmarks,
            after,
        } => Box::new(patrol::Patrol::new(starts, dirs, landmarks, after)),
        _ => return None,
    })
}

#[cfg(test)]
pub(super) mod testing {
    //! A table for a solver: what it sees, turn by turn.

    use super::*;

    /// What a test shows a solver. Everything defaults to "nothing known".
    #[derive(Default)]
    pub struct Scene {
        pub here: Option<u32>,
        pub goal: u32,
        pub walker: Walker,
        pub state: GameState,
        pub answer: Vec<ChunkLine>,
        pub failed: bool,
        pub random: u64,
    }

    impl Scene {
        pub fn at(here: u32, goal: u32) -> Scene {
            Scene {
                here: Some(here),
                goal,
                ..Scene::default()
            }
        }

        pub fn answered(mut self, lines: &[&str]) -> Scene {
            self.answer = lines.iter().map(|line| ChunkLine::plain(line)).collect();
            self
        }

        /// A thing in the room, by noun.
        pub fn showing(mut self, noun: &str) -> Scene {
            self.state.room.objects.push(cena_session::RoomItem {
                id: "1".into(),
                noun: noun.into(),
                text: noun.into(),
                before: None,
                after: None,
                status: None,
            });
            self
        }

        pub fn ask(&self, solver: &mut dyn Solver) -> Next {
            solver.next(&Seen {
                here: self.here.map(RoomId),
                goal: RoomId(self.goal),
                walker: &self.walker,
                state: &self.state,
                answer: &self.answer,
                ok: !self.failed,
                random: self.random,
            })
        }
    }
}
