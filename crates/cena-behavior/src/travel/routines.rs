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
//! (`upstream_scripts/` in `Nisugi/hydra-mapdb`, the mapdb pipeline's own
//! repo -- moved out of this workspace, `plan/21` records the move), which
//! is the reference for what it must do.

mod altar_levers;
mod bridge_wheel;
mod bronze_gate;
mod casting;
mod colour_barrier;
mod confluence;
mod crown_door;
mod cutter;
pub(super) mod day_pass;
mod eye_spy_runes;
mod familiar_doors;
mod flight_of_steps;
mod giant;
mod guild_password;
mod labyrinth_entry;
mod minotaur_maze;
mod mirror;
mod mural_of_deities;
mod patrol;
mod ring_wedges;
mod rolaren_gate;
mod rune_staircase;
mod search_rooms;
mod seeking;
mod shopping;
mod signposts;
mod sword_gorge;
mod three_pillars;
mod trinket;
mod vaalorn_door;
mod workshop_pillars;

use cena_map::{Errand, Map, Puzzle, RoomId, Routine, Step, Walker};
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
    /// Walk to the nearest room with this tag -- `bank`, `alchemist` -- as
    /// `go2 bank` does, and come back to me. "Nearest" needs the walker's
    /// place and the map, which only the driver has.
    WalkToTag(String),
    /// Wait until the game says one of these, or this many milliseconds.
    Await(Vec<String>, u64),
    /// Wait this many milliseconds.
    Pause(u64),
    /// Only the player can settle this -- a gem to hold, a helper to find.
    /// **The whole trip stops**, and tells them this. Upstream pauses the
    /// script and waits; a walker that waits for a person is a walker nobody
    /// can stop cleanly, so this one stops and is started again.
    Stop(String),
    /// Nothing more to do. See the module docs.
    Done,
    /// This exit cannot be crossed by this walker today.
    Failed,
}

/// One routine, part-way through.
pub(super) trait Solver {
    fn next(&mut self, seen: &Seen<'_>) -> Next;

    /// Hand back whatever should outlive this crossing ([`Kept`]).
    fn keep(self: Box<Self>, _kept: &mut Kept) {}
}

/// What routines learn that the next crossing should start with. The driver
/// keeps it for the trip and lends it to each solver in turn.
#[derive(Debug, Default)]
pub(super) struct Kept {
    confluence: confluence::Learned,
    maze: minotaur_maze::Learned,
}

/// The solver for a routine. **Every routine the map can name has one**: a
/// new one upstream is a name this build cannot parse, which loads as
/// `Crossing::Unknown` and is gone round (`cena_map::routine`).
///
/// The map and the goal are for what a routine must know before it starts --
/// its destination's titles, rooms it names by the game's numbers -- resolved
/// here once, since a solver is not shown the map afterwards.
pub(super) fn solver_for(
    routine: &Routine,
    map: &Map,
    goal: RoomId,
    kept: &mut Kept,
) -> Box<dyn Solver + Send> {
    let goal = map.room(goal);
    match routine.clone() {
        Routine::Confluence { leave } => Box::new(confluence::Confluence::new(
            leave,
            std::mem::take(&mut kept.confluence),
        )),
        Routine::Seeking { remember } => Box::new(seeking::Seeking::new(
            goal.map(|room| room.title.clone()).unwrap_or_default(),
            remember,
        )),
        // Upstream's `/Isle of Four Winds|Mist Harbor/`.
        Routine::Trinket => Box::new(trinket::Trinket::new(
            goal.and_then(|room| room.location.as_deref())
                .is_some_and(|at| at.contains("Isle of Four Winds") || at.contains("Mist Harbor")),
        )),
        Routine::MinotaurMaze { rooms } => Box::new(minotaur_maze::MinotaurMaze::new(
            rooms,
            std::mem::take(&mut kept.maze),
        )),
        Routine::SearchRooms {
            rooms,
            by_uid,
            sees,
            enter,
        } => Box::new(search_rooms::SearchRooms::new(
            search_rooms::resolve(&rooms, by_uid, map),
            sees,
            enter,
        )),
        Routine::FlightOfSteps { wall } => Box::new(flight_of_steps::FlightOfSteps::new(wall)),
        Routine::DayPass { route } => Box::new(day_pass::DayPass::new(&route)),
        Routine::BronzeGate { batter } => Box::new(bronze_gate::BronzeGate::new(batter)),
        Routine::Mirror => Box::new(mirror::Mirror::default()),
        Routine::RingWedges => Box::new(ring_wedges::RingWedges::default()),
        Routine::ColourBarrier => Box::new(colour_barrier::ColourBarrier::default()),
        Routine::Errand { errand } => match errand {
            Errand::GiantToRiversRest => Box::new(giant::Giant::new(giant::Way::ToRiversRest)),
            Errand::GiantFromRiversRest => Box::new(giant::Giant::new(giant::Way::FromRiversRest)),
            Errand::SwordInTheGorge => Box::new(sword_gorge::SwordGorge::new()),
            Errand::CutterFromMarshtown => Box::new(cutter::Cutter::marshtown()),
            Errand::CutterFromRiversRest => Box::new(cutter::Cutter::rivers_rest()),
        },
        Routine::Puzzle { puzzle } => match puzzle {
            Puzzle::RolarenGate => Box::new(rolaren_gate::RolarenGate::default()),
            Puzzle::ThreePillars => Box::new(three_pillars::ThreePillars::default()),
            Puzzle::WorkshopPillars => Box::new(workshop_pillars::WorkshopPillars::default()),
            Puzzle::EyeSpyRunes => Box::new(eye_spy_runes::EyeSpyRunes::default()),
            Puzzle::FamiliarDoors => Box::new(familiar_doors::FamiliarDoors::default()),
            Puzzle::CrownDoor => Box::new(crown_door::CrownDoor::default()),
            Puzzle::LabyrinthEntry => Box::new(labyrinth_entry::LabyrinthEntry::default()),
            Puzzle::VaalornDoor => Box::new(vaalorn_door::VaalornDoor::default()),
            Puzzle::BridgeWheel => Box::new(bridge_wheel::BridgeWheel::default()),
            Puzzle::RuneStaircase => Box::new(rune_staircase::RuneStaircase::default()),
            Puzzle::AltarLevers => Box::new(altar_levers::AltarLevers::default()),
            Puzzle::MuralOfDeities => Box::new(mural_of_deities::MuralOfDeities::default()),
        },
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
    }
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
