//! The shortest way between rooms (`plan/21` §5 step 5).
//!
//! Dijkstra over the exits, weighted by seconds of travel.
//!
//! # The cost is the caller's
//!
//! What an exit costs depends on who is walking: a day pass, a profession, a
//! society, silver in hand, an exit that failed twice this trip. So the search
//! takes the pricing as a function and knows none of it. `None` is impassable.
//! The reference client keeps those facts in process-wide state, which cannot
//! be right when several characters plan routes at once; here they arrive as a
//! value and the map stays shared and immutable.
//!
//! [`as_converted`] is the pricing that needs no character: a plain command at
//! a constant cost, and nothing else.
//!
//! # No default cost
//!
//! An exit with no cost is impassable, as upstream (`map_base.rb:829`, `next
//! unless edge_weight`). A price that is negative or not a number is refused
//! the same way rather than being allowed to corrupt the search.
//!
//! # Not ported: the twenty-second rule
//!
//! Upstream's nearest-of-several search accepts the first target it reaches
//! only if that is under twenty seconds away, and otherwise explores the whole
//! map. Dijkstra settles rooms in distance order, so the first target settled
//! *is* the nearest at any distance; the rule buys nothing and is left behind.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::cond::Walker;
use crate::exit::{Cost, Exit};
use crate::map::Map;
use crate::room::{Room, RoomId};

/// When a search may stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    Room(RoomId),
    /// The nearest of these. The starting room counts, at distance zero.
    Nearest(&'a [RoomId]),
    /// Every room that can be reached.
    Everything,
}

/// The pricing that needs no character: see the module docs.
#[must_use]
pub fn as_converted(_from: &Room, exit: &Exit) -> Option<f64> {
    match (&exit.cost, exit.is_routable()) {
        (Some(Cost::Fixed(seconds)), true) => Some(*seconds),
        _ => None,
    }
}

/// The pricing for one walker: every exit this build can cross, at what it
/// costs *them*. The starting point for a trip's own pricing, which adds what
/// only the trip knows -- the exits it has banned.
pub fn priced_for(walker: &Walker) -> impl Fn(&Room, &Exit) -> Option<f64> + '_ {
    |_from, exit| {
        exit.crossing
            .is_crossable()
            .then(|| exit.cost.as_ref()?.price(walker))
            .flatten()
    }
}

/// Seconds, ordered. Only finite, non-negative values are ever constructed.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seconds(f64);

impl Eq for Seconds {}

impl PartialOrd for Seconds {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Seconds {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// What one search found. Holds the map it searched, so a path is read out
/// without a second search -- one search answers "how far is every bank".
#[derive(Debug)]
pub struct Routes<'m> {
    map: &'m Map,
    from: RoomId,
    /// By room position in the map: seconds from the start, if reached.
    seconds: Vec<Option<f64>>,
    /// By room position: the position of the room it was reached from.
    came_from: Vec<Option<usize>>,
    /// The target the search stopped at, if it was looking for one.
    stopped_at: Option<RoomId>,
}

impl Routes<'_> {
    /// Seconds of travel to a room; `None` when it was not reached. After a
    /// search that stopped early, only rooms nearer than the target are sure
    /// to have been.
    #[must_use]
    pub fn seconds_to(&self, room: RoomId) -> Option<f64> {
        self.seconds.get(self.map.position(room)?).copied()?
    }

    /// The target the search reached: the room asked for, or the nearest of
    /// several. `None` when none can be reached, or none was asked for.
    #[must_use]
    pub fn reached(&self) -> Option<RoomId> {
        self.stopped_at
    }

    /// The rooms to pass through, in order, **excluding the start and
    /// including the destination**. Empty when already there; `None` when the
    /// destination was not reached.
    #[must_use]
    pub fn path_to(&self, destination: RoomId) -> Option<Vec<RoomId>> {
        let mut at = self.map.position(destination)?;
        self.seconds.get(at).copied()??;
        let mut path = Vec::new();
        while let Some(previous) = self.came_from.get(at).copied()? {
            path.push(self.map.rooms().get(at)?.id);
            at = previous;
        }
        debug_assert_eq!(
            self.map.rooms().get(at).map(|room| room.id),
            Some(self.from)
        );
        path.reverse();
        Some(path)
    }
}

impl Map {
    /// Search outward from `from` until `target` is settled.
    ///
    /// `price` is asked once per exit considered; see the module docs. Ties
    /// are broken by room id, so the same question has the same answer.
    #[must_use]
    pub fn routes(
        &self,
        from: RoomId,
        target: Target<'_>,
        price: impl Fn(&Room, &Exit) -> Option<f64>,
    ) -> Routes<'_> {
        let count = self.rooms().len();
        let mut routes = Routes {
            map: self,
            from,
            seconds: vec![None; count],
            came_from: vec![None; count],
            stopped_at: None,
        };
        let Some(start) = self.position(from) else {
            return routes;
        };
        let mut settled = vec![false; count];
        let mut frontier = BinaryHeap::new();
        routes.seconds[start] = Some(0.0);
        frontier.push(Reverse((Seconds(0.0), from, start)));

        while let Some(Reverse((Seconds(so_far), id, at))) = frontier.pop() {
            if std::mem::replace(&mut settled[at], true) {
                continue;
            }
            let wanted = match target {
                Target::Room(room) => room == id,
                Target::Nearest(rooms) => rooms.contains(&id),
                Target::Everything => false,
            };
            if wanted {
                routes.stopped_at = Some(id);
                break;
            }
            let Some(room) = self.rooms().get(at) else {
                continue;
            };
            for exit in &room.exits {
                let Some(next) = self.position(exit.to).filter(|next| !settled[*next]) else {
                    continue;
                };
                let Some(cost) = price(room, exit).filter(|c| c.is_finite() && *c >= 0.0) else {
                    continue;
                };
                let total = so_far + cost;
                if routes.seconds[next].is_none_or(|known| total < known) {
                    routes.seconds[next] = Some(total);
                    routes.came_from[next] = Some(at);
                    frontier.push(Reverse((Seconds(total), exit.to, next)));
                }
            }
        }
        routes
    }
}
