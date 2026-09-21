//! `Routine::Confluence`: the Elemental Confluence (3,234 exits).
//!
//! Upstream is one 4 KB script every exit of the plane calls
//! (`upstream_scripts/confluence_engine.rb`). The plane's rooms keep their
//! numbers and **reshuffle their exits**, so no map of it stays true; the
//! script explores instead:
//!
//! - it writes down where each exit of each room led ([`Learned::wayto`]),
//!   and **forgets all of it** the moment a room's exits are not the ones it
//!   wrote down -- the plane has shifted;
//! - to reach a room it searches what it has learned backwards from the
//!   target, thirty rooms deep ([`Learned::dir_to`]); knowing no way, it
//!   heads for the nearest exit it has never taken; with none of those, for
//!   the neighbour it visited longest ago ([`Learned::wander`]);
//! - the plane is two halves, hot and cold, joined only by a *gaping
//!   bottomless pit* that wanders, and left only by a *point of elemental
//!   tranquility* that wanders too. Each is remembered where last seen and
//!   forgotten when it is seen to be gone.
//!
//! A move that fails is answered upstream with `look` and a random exit; here
//! the exits are already known, so it is the random exit alone.
//!
//! What upstream keeps in globals for the Lich session is [`Learned`], which
//! the driver keeps for the trip and lends to each crossing in turn: one
//! walk through the plane is many of these exits, and each would otherwise
//! start knowing nothing.
//!
//! Not copied: waiting for an escorted child to catch up (`bounty?`), which
//! is the bounty behaviour's business when there is one.

use std::collections::HashMap;

use cena_map::RoomId;

use super::{Next, Seen, Solver};

/// Moves one crossing may make. Upstream has no bound; the plane has 53
/// rooms, and this is room to learn it several times over.
const MAX_MOVES: u32 = 500;
/// How deep the search back from a target goes. Upstream's `30.times`.
const MAX_DEPTH: usize = 30;

const TRANQUILITY: &str = "point of elemental tranquility";
const PIT: &str = "gaping bottomless pit";

/// Which half of the plane a room is in. Upstream's `hot_rooms` and
/// `cold_rooms`, which are these runs of ids.
fn is_hot(room: RoomId) -> Option<bool> {
    match room.0 {
        23282..=23303 | 23329..=23334 => Some(true),
        23304..=23328 => Some(false),
        _ => None,
    }
}

/// What has been learned of the plane. Kept between crossings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::travel) struct Learned {
    /// Each known room's exits, in the game's order, and where each led.
    wayto: HashMap<RoomId, Vec<(String, Option<RoomId>)>>,
    /// Rooms visited, the most recent last.
    wander: Vec<RoomId>,
    /// Where the way out and the way across were last seen: `[cold, hot]`.
    tranquility: [Option<RoomId>; 2],
    pit: [Option<RoomId>; 2],
}

impl Learned {
    /// An exit of `from` that leads towards any of `targets`, searching
    /// backwards through what is known. `None` among the targets means "an
    /// exit never taken".
    fn dir_to(&self, from: RoomId, targets: &[Option<RoomId>]) -> Option<String> {
        let exits = self.wayto.get(&from)?;
        let mut targets = targets.to_vec();
        let mut tried: Vec<Option<RoomId>> = Vec::new();
        for _ in 0..MAX_DEPTH {
            if let Some((dir, _)) = exits.iter().find(|(_, to)| targets.contains(to)) {
                return Some(dir.clone());
            }
            for target in &targets {
                if !tried.contains(target) {
                    tried.push(*target);
                }
            }
            // The rooms known to lead to a target are the next targets.
            let mut nearer: Vec<Option<RoomId>> = self
                .wayto
                .iter()
                .filter(|(room, exits)| {
                    exits.iter().any(|(_, to)| targets.contains(to))
                        && !tried.contains(&Some(**room))
                })
                .map(|(room, _)| Some(*room))
                .collect();
            if nearer.is_empty() {
                return None;
            }
            // A `HashMap` has no order, and a replay must choose the same.
            nearer.sort_unstable();
            targets = nearer;
        }
        None
    }

    /// Note whether `thing` is here, or is gone from where it was.
    fn note(seen_at: &mut Option<RoomId>, here: RoomId, is_here: bool) {
        if is_here {
            *seen_at = Some(here);
        } else if *seen_at == Some(here) {
            *seen_at = None;
        }
    }

    /// Make sure this room's exits are written down, forgetting everything
    /// if they are not the ones that were.
    fn look_at(&mut self, here: RoomId, exits: &[String]) {
        let same = self
            .wayto
            .get(&here)
            .is_none_or(|known| known.iter().map(|(dir, _)| dir).eq(exits.iter()));
        if !same {
            self.wayto.clear();
        }
        self.wayto
            .entry(here)
            .or_insert_with(|| exits.iter().map(|dir| (dir.clone(), None)).collect());
    }

    fn landed(&mut self, from: RoomId, dir: &str, at: RoomId) {
        if let Some(exit) = self
            .wayto
            .get_mut(&from)
            .and_then(|exits| exits.iter_mut().find(|(known, _)| known == dir))
        {
            exit.1 = Some(at);
        }
        self.wander.retain(|room| *room != at);
        self.wander.push(at);
    }

    /// Upstream's last two resorts: an exit to somewhere not yet wandered
    /// through, and then the neighbour visited longest ago.
    fn wander_from(&self, here: RoomId) -> Option<String> {
        let exits = self.wayto.get(&here)?;
        let fresh = exits
            .iter()
            .find(|(_, to)| to.is_none_or(|to| !self.wander.contains(&to)));
        let stale = || {
            let oldest = self
                .wander
                .iter()
                .find(|room| exits.iter().any(|(_, to)| *to == Some(**room)))?;
            exits.iter().find(|(_, to)| *to == Some(*oldest))
        };
        fresh.or_else(stale).map(|(dir, _)| dir.clone())
    }
}

pub(in crate::travel) struct Confluence {
    /// Upstream's `'tranquility'` goal: leave the plane.
    leave: bool,
    learned: Learned,
    /// The move under way: the room it left, and by which exit.
    moving: Option<(RoomId, String)>,
    moves: u32,
    /// `go tranquility` has been sent.
    left: bool,
}

impl Confluence {
    pub fn new(leave: bool, learned: Learned) -> Self {
        Confluence {
            leave,
            learned,
            moving: None,
            moves: 0,
            left: false,
        }
    }

    fn go(&mut self, from: RoomId, dir: String) -> Next {
        self.moves += 1;
        self.moving = Some((from, dir.clone()));
        Next::Go(dir)
    }

    /// The exit to take from `here`, by everything upstream tries in turn.
    fn choose(&self, here: RoomId, hot: bool, goal: RoomId) -> Option<String> {
        let learned = &self.learned;
        let half = usize::from(hot);
        let known = if self.leave {
            learned.tranquility[half].and_then(|at| learned.dir_to(here, &[Some(at)]))
        } else if is_hot(goal).is_some_and(|goal_hot| goal_hot != hot) {
            learned.pit[half].and_then(|at| learned.dir_to(here, &[Some(at)]))
        } else {
            learned.dir_to(here, &[Some(goal)])
        };
        known
            .or_else(|| learned.dir_to(here, &[None]))
            .or_else(|| learned.wander_from(here))
    }
}

impl Solver for Confluence {
    fn keep(self: Box<Self>, kept: &mut super::Kept) {
        kept.confluence = self.learned;
    }

    fn next(&mut self, seen: &Seen<'_>) -> Next {
        let (Some(here), false) = (seen.here, self.left) else {
            return Next::Done;
        };
        let exits = seen.walker.exits.as_deref().unwrap_or(&[]);
        if let Some((from, dir)) = self.moving.take() {
            if !seen.ok {
                // Upstream looks and takes any exit; the exits are known.
                let Some(any) = pick(exits, seen.random) else {
                    return Next::Failed;
                };
                self.moves += 1;
                return Next::Go(any.clone());
            }
            if here != from {
                self.learned.landed(from, &dir, here);
            }
        }
        if !self.leave && here == seen.goal {
            return Next::Done;
        }
        // Out of the plane: fallen, carried, or never in it. Plan again.
        let Some(hot) = is_hot(here) else {
            return Next::Done;
        };
        if self.moves >= MAX_MOVES || exits.is_empty() {
            return Next::Failed;
        }
        let shows = |name: &str| {
            seen.state
                .room
                .objects
                .iter()
                .any(|thing| thing.text == name)
        };
        let half = usize::from(hot);
        Learned::note(
            &mut self.learned.tranquility[half],
            here,
            shows(TRANQUILITY),
        );
        Learned::note(&mut self.learned.pit[half], here, shows(PIT));
        self.learned.look_at(here, exits);

        if self.leave && shows(TRANQUILITY) {
            self.left = true;
            return Next::Go("go tranquility".to_owned());
        }
        let across = !self.leave && is_hot(seen.goal).is_some_and(|goal_hot| goal_hot != hot);
        if across && shows(PIT) {
            self.moves += 1;
            return Next::Go("go pit".to_owned());
        }
        match self.choose(here, hot, seen.goal) {
            Some(dir) => self.go(here, dir),
            None => Next::Failed,
        }
    }
}

fn pick(exits: &[String], random: u64) -> Option<&String> {
    let len = u64::try_from(exits.len()).ok().filter(|len| *len > 0)?;
    exits.get(usize::try_from(random % len).ok()?)
}

#[cfg(test)]
mod tests {
    use cena_session::RoomItem;

    use super::super::testing::Scene;
    use super::*;

    const A: u32 = 23282; // hot
    const B: u32 = 23283; // hot
    const C: u32 = 23284; // hot
    const COLD: u32 = 23304;

    fn room(here: u32, goal: u32, exits: &[&str]) -> Scene {
        let mut scene = Scene::at(here, goal);
        scene.walker.exits = Some(exits.iter().map(|dir| (*dir).to_owned()).collect());
        scene
    }

    fn showing(mut scene: Scene, name: &str) -> Scene {
        scene.state.room.objects.push(RoomItem {
            id: "9".into(),
            noun: "x".into(),
            text: name.into(),
            before: None,
            after: None,
            status: None,
        });
        scene
    }

    #[test]
    fn the_halves_are_upstreams_lists() {
        let hot = (23000..24000).filter(|id| is_hot(RoomId(*id)) == Some(true));
        let cold = (23000..24000).filter(|id| is_hot(RoomId(*id)) == Some(false));
        assert_eq!((hot.count(), cold.count()), (28, 25));
        assert_eq!(is_hot(RoomId(23328)), Some(false));
        assert_eq!(is_hot(RoomId(23329)), Some(true));
        assert_eq!(is_hot(RoomId(1)), None);
    }

    #[test]
    fn it_explores_what_it_has_never_taken_and_then_knows_the_way() {
        let mut plane = Confluence::new(false, Learned::default());
        // Nothing known: the first exit never taken.
        assert_eq!(
            room(A, C, &["n", "e"]).ask(&mut plane),
            Next::Go("n".into())
        );
        // `n` led to B, which is a dead end but for the way back.
        assert_eq!(room(B, C, &["s"]).ask(&mut plane), Next::Go("s".into()));
        // Back in A: `n` is known now, so the exit never taken is `e`.
        assert_eq!(
            room(A, C, &["n", "e"]).ask(&mut plane),
            Next::Go("e".into())
        );
        assert_eq!(room(C, C, &["w"]).ask(&mut plane), Next::Done);

        // The next crossing is lent what this one learned, and goes straight.
        let mut again = Confluence::new(false, plane.learned);
        assert_eq!(
            room(A, C, &["n", "e"]).ask(&mut again),
            Next::Go("e".into())
        );
    }

    #[test]
    fn the_way_is_found_through_rooms_in_between() {
        let mut learned = Learned::default();
        learned.look_at(RoomId(A), &["n".into()]);
        learned.landed(RoomId(A), "n", RoomId(B));
        learned.look_at(RoomId(B), &["s".into(), "e".into()]);
        learned.landed(RoomId(B), "e", RoomId(C));
        assert_eq!(
            learned.dir_to(RoomId(A), &[Some(RoomId(C))]).as_deref(),
            Some("n")
        );
        assert_eq!(learned.dir_to(RoomId(A), &[Some(RoomId(COLD))]), None);
    }

    #[test]
    fn a_room_whose_exits_changed_means_everything_is_forgotten() {
        let mut plane = Confluence::new(false, Learned::default());
        room(A, C, &["n", "e"]).ask(&mut plane);
        room(B, C, &["s"]).ask(&mut plane);
        assert_eq!(plane.learned.wayto.len(), 2);
        // A again, and its exits are not what was written down.
        room(A, C, &["w", "e"]).ask(&mut plane);
        assert_eq!(plane.learned.wayto.len(), 1);
        assert_eq!(plane.learned.wayto[&RoomId(A)][0], ("w".to_owned(), None));
    }

    #[test]
    fn the_other_half_is_reached_by_the_pit_and_the_plane_left_by_tranquility() {
        let mut across = Confluence::new(false, Learned::default());
        assert_eq!(
            showing(room(A, COLD, &["n"]), PIT).ask(&mut across),
            Next::Go("go pit".into())
        );
        // The same pit means nothing to a walker staying in this half.
        let mut staying = Confluence::new(false, Learned::default());
        assert_eq!(
            showing(room(A, C, &["n"]), PIT).ask(&mut staying),
            Next::Go("n".into())
        );
        let mut leaving = Confluence::new(true, Learned::default());
        assert_eq!(
            showing(room(A, 1, &["n"]), TRANQUILITY).ask(&mut leaving),
            Next::Go("go tranquility".into())
        );
        assert_eq!(room(1, 1, &["out"]).ask(&mut leaving), Next::Done);
    }

    #[test]
    fn a_pit_seen_is_headed_for_and_a_pit_gone_is_forgotten() {
        let mut plane = Confluence::new(false, Learned::default());
        // B shows the pit, but the walker is bound for C: it is only noted.
        room(A, C, &["n", "e"]).ask(&mut plane);
        showing(room(B, C, &["s"]), PIT).ask(&mut plane);
        assert_eq!(plane.learned.pit[1], Some(RoomId(B)));
        // Bound for the cold half from A, the way to the pit is `n`.
        let mut across = Confluence::new(false, plane.learned);
        assert_eq!(
            room(A, COLD, &["n", "e"]).ask(&mut across),
            Next::Go("n".into())
        );
        // ...and it has gone.
        room(B, COLD, &["s"]).ask(&mut across);
        assert_eq!(across.learned.pit[1], None);
    }

    #[test]
    fn a_move_that_failed_takes_any_exit_and_teaches_nothing() {
        let mut plane = Confluence::new(false, Learned::default());
        room(A, C, &["n", "e"]).ask(&mut plane);
        let mut failed = room(A, C, &["n", "e"]);
        failed.failed = true;
        failed.random = 1;
        assert_eq!(failed.ask(&mut plane), Next::Go("e".into()));
        room(B, C, &["s"]).ask(&mut plane);
        assert_eq!(plane.learned.wayto[&RoomId(A)][1], ("e".to_owned(), None));
    }

    #[test]
    fn outside_the_plane_there_is_nothing_to_do_but_plan_again() {
        let mut plane = Confluence::new(false, Learned::default());
        assert_eq!(room(5, C, &["n"]).ask(&mut plane), Next::Done);
    }
}
