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
    /// By room position: the position of the room it was reached from, and
    /// the index in that room's `exits` of the exit that was taken.
    came_from: Vec<Option<(usize, usize)>>,
    /// By room position: whether the search **settled** it -- took it off the
    /// frontier, which is the moment Dijkstra knows its distance is final.
    ///
    /// Distinct from "has a distance". A room one step past the frontier is
    /// *discovered* with a tentative distance that a later, cheaper path may
    /// still undercut; a search that stops at its target leaves such rooms
    /// behind, and answering for them would hand out a route that is not the
    /// shortest as though it were.
    settled: Vec<bool>,
    /// The target the search stopped at, if it was looking for one.
    stopped_at: Option<RoomId>,
}

impl Routes<'_> {
    /// Seconds of travel to a room; `None` when the search did not **settle**
    /// it.
    ///
    /// After a search that ran to exhaustion ([`Target::Everything`], or a
    /// target that cannot be reached), that is every reachable room. After one
    /// that stopped at its target, it is the target and the rooms nearer than
    /// it -- and **not** the rooms merely discovered on the way, whose distance
    /// was still tentative when the search stopped. `None` there means "not
    /// known", not "unreachable".
    #[must_use]
    pub fn seconds_to(&self, room: RoomId) -> Option<f64> {
        let at = self.settled_position(room)?;
        self.seconds.get(at).copied()?
    }

    /// The position of `room`, if the search settled it.
    fn settled_position(&self, room: RoomId) -> Option<usize> {
        let at = self.map.position(room)?;
        self.settled.get(at).copied()?.then_some(at)
    }

    /// The target the search reached: the room asked for, or the nearest of
    /// several. `None` when none can be reached, or none was asked for.
    #[must_use]
    pub fn reached(&self) -> Option<RoomId> {
        self.stopped_at
    }

    /// The rooms to pass through, in order, **excluding the start and
    /// including the destination**. Empty when already there; `None` when the
    /// search did not settle the destination -- see [`Self::seconds_to`] for
    /// which rooms that is.
    #[must_use]
    pub fn path_to(&self, destination: RoomId) -> Option<Vec<RoomId>> {
        Some(
            self.exits_to(destination)?
                .into_iter()
                .map(|(_, _, to)| to)
                .collect(),
        )
    }

    /// The exits to take, in order: for each, the room it leaves, its index in
    /// that room's [`Room::exits`], and the room it reaches. The last `to` is
    /// the destination. Empty when already there; `None` exactly when
    /// [`Self::path_to`] is.
    ///
    /// **The index is the answer [`Self::path_to`] cannot give.** Two exits
    /// from one room can reach the same room at different prices -- a gate
    /// and a wall -- and a caller holding only the next room has to re-price
    /// every parallel exit to find the one the search chose. This names it.
    #[must_use]
    pub fn exits_to(&self, destination: RoomId) -> Option<Vec<(RoomId, usize, RoomId)>> {
        let mut at = self.settled_position(destination)?;
        let rooms = self.map.rooms();
        let mut legs = Vec::new();
        while let Some((previous, exit)) = self.came_from.get(at).copied()? {
            legs.push((rooms.get(previous)?.id, exit, rooms.get(at)?.id));
            at = previous;
        }
        debug_assert_eq!(rooms.get(at).map(|room| room.id), Some(self.from));
        legs.reverse();
        Some(legs)
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
            settled: vec![false; count],
            stopped_at: None,
        };
        let Some(start) = self.position(from) else {
            return routes;
        };
        let mut frontier = BinaryHeap::new();
        routes.seconds[start] = Some(0.0);
        frontier.push(Reverse((Seconds(0.0), from, start)));

        while let Some(Reverse((Seconds(so_far), id, at))) = frontier.pop() {
            if std::mem::replace(&mut routes.settled[at], true) {
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
            for (index, exit) in room.exits.iter().enumerate() {
                let Some(next) = self.position(exit.to).filter(|next| !routes.settled[*next])
                else {
                    continue;
                };
                let Some(cost) = price(room, exit).filter(|c| c.is_finite() && *c >= 0.0) else {
                    continue;
                };
                let total = so_far + cost;
                if routes.seconds[next].is_none_or(|known| total < known) {
                    routes.seconds[next] = Some(total);
                    routes.came_from[next] = Some((at, index));
                    frontier.push(Reverse((Seconds(total), exit.to, next)));
                }
            }
        }
        routes
    }
}
