//! The route, shown and not walked: the port of `route2.lic`.
//!
//! > *"It's like go2 but instead of sending you on the route, it displays the
//! > route to you."* -- the author, 2026-09-21. Its own header says what it is
//! > for: **troubleshooting mapDB issues.**
//!
//! # Where this can see and route2 cannot
//!
//! route2 prints `(StringProc)` for a scripted exit and `(proc)` for a
//! scripted cost, and leaves that cost out of the total with a `+P` beside
//! it (`route2.lic:136`, `:151-168`). Those are exactly the exits that go
//! wrong. Here a crossing is a list of steps and a cost is a question asked
//! of the walker, so both are shown, and the total is the real one.
//!
//! It also says **what it did not take**. At each room on the way, every exit
//! that is shut to *this* walker is listed with the reason -- a fact the
//! game has not told us yet, a fact that says no, or a crossing this build
//! cannot run. A route that goes the long way round says why.
//!
//! # The same pricing as the trip
//!
//! [`Trip::pricing`](super::Trip) is what this asks, so the route shown is
//! the route walked. A view priced any other way would be a second opinion.
//!
//! **Pure**: a map and a walker in, rows out. [`table`] lays the rows out as
//! text for a `Notice`; a frontend that wants to draw them takes the rows.

use cena_map::{Cond, Cost, Crossing, Exit, Map, Room, RoomId, Target, Uid, Walker};

use super::{Trip, can_cross};

/// One exit taken.
#[derive(Debug, Clone, PartialEq)]
pub struct Leg {
    pub to: RoomId,
    /// What crosses it: a command, or the steps in order.
    pub how: String,
    /// What this walker pays for it, in seconds.
    pub seconds: f64,
    /// And for everything up to and including it.
    pub so_far: f64,
    pub title: Option<String>,
    pub location: Option<String>,
    /// The exits out of the room this leg *leaves* that are shut to this
    /// walker. Not every exit it passed by -- only the ones it could not have
    /// taken, which is what explains a long way round.
    pub shut: Vec<Shut>,
}

/// An exit this walker cannot take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shut {
    pub to: RoomId,
    pub how: String,
    pub why: ShutWhy,
}

/// Why an exit is shut to this walker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutWhy {
    /// Its price hangs on a fact nobody has said yet. The question, as the
    /// map asks it.
    NotKnown(String),
    /// Every fact is known, and they say no.
    SaysNo,
    /// The walker could pay; this build cannot run the crossing yet.
    NotBuiltYet,
    /// The map gives it no cost at all, as upstream's `timeto` of `nil`.
    NoCost,
}

/// The room a player means by `what`, standing in `from`. go2's order, as
/// `route2.lic:33-76` has it, for the two kinds built:
///
/// 1. **A number**: the map's own id, or the game's with a `u` before it
///    (`u7120`), when it names one room.
/// 2. **A tag** (`bank`, `town`): the *nearest* room that has it, by what
///    this walker would pay to get there -- so a bard's nearest bank may not
///    be a warrior's.
///
/// **Not built:** go2's custom targets, which want a home in the travel file
/// first, and its match on a room's title or description, whose rules in
/// `Room[]` are their own port.
#[must_use]
pub fn destination(map: &Map, walker: &Walker, from: RoomId, what: &str) -> Option<RoomId> {
    let what = what.trim();
    if let Some(uid) = what
        .strip_prefix('u')
        .and_then(|digits| digits.parse().ok())
    {
        let [room] = map.ids_for_uid(Uid(uid)) else {
            return None;
        };
        return Some(*room);
    }
    if let Ok(id) = what.parse() {
        return map.room(RoomId(id)).map(|room| room.id);
    }
    let tagged: Vec<RoomId> = map
        .rooms()
        .iter()
        .filter(|room| room.tags.iter().any(|tag| tag.eq_ignore_ascii_case(what)))
        .map(|room| room.id)
        .collect();
    let trip = Trip::to(from);
    map.routes(from, Target::Nearest(&tagged), trip.pricing(walker))
        .reached()
}

/// The route from `from` to `goal` as this walker would walk it now, or
/// `None` when there is no way.
#[must_use]
pub fn itinerary(map: &Map, walker: &Walker, from: RoomId, goal: RoomId) -> Option<Vec<Leg>> {
    let trip = Trip::to(goal);
    let price = trip.pricing(walker);
    let path = map.routes(from, Target::Room(goal), &price).path_to(goal)?;
    let mut legs = Vec::with_capacity(path.len());
    let (mut here, mut so_far) = (from, 0.0);
    for to in path {
        let room = map.room(here)?;
        let (seconds, exit) = room
            .exits
            .iter()
            .filter(|exit| exit.to == to)
            .filter_map(|exit| Some((price(room, exit)?, exit)))
            .min_by(|(a, _), (b, _)| a.total_cmp(b))?;
        so_far += seconds;
        let arrived = map.room(to);
        legs.push(Leg {
            to,
            how: how(&exit.crossing),
            seconds,
            so_far,
            title: arrived.and_then(|room| room.title.first().cloned()),
            location: arrived.and_then(|room| room.location.clone()),
            shut: shut_from(room, walker, &price),
        });
        here = to;
    }
    Some(legs)
}

fn shut_from(
    room: &Room,
    walker: &Walker,
    price: impl Fn(&Room, &Exit) -> Option<f64>,
) -> Vec<Shut> {
    room.exits
        .iter()
        .filter(|exit| price(room, exit).is_none())
        .map(|exit| Shut {
            to: exit.to,
            how: how(&exit.crossing),
            why: why_shut(exit, walker),
        })
        .collect()
}

fn why_shut(exit: &Exit, walker: &Walker) -> ShutWhy {
    let Some(cost) = &exit.cost else {
        return ShutWhy::NoCost;
    };
    if cost.price(walker).is_some() {
        // Payable, and still shut: it is the crossing this build lacks (or
        // the trip banned it, which a fresh trip has not).
        debug_assert!(!can_cross(&exit.crossing));
        return ShutWhy::NotBuiltYet;
    }
    let asked: Vec<&Cond> = match cost {
        Cost::Gated { when, .. } => vec![when],
        Cost::Ladder { ladder, .. } => ladder.iter().map(|rung| &rung.when).collect(),
        _ => Vec::new(),
    };
    match asked.into_iter().find_map(|cond| not_known(cond, walker)) {
        Some(question) => ShutWhy::NotKnown(format!("{question:?}")),
        None if matches!(cost, Cost::Table { .. }) => ShutWhy::NotKnown(format!("{cost:?}")),
        None => ShutWhy::SaysNo,
    }
}

/// The first question inside `cond` that has no answer yet.
fn not_known<'c>(cond: &'c Cond, walker: &Walker) -> Option<&'c Cond> {
    match cond {
        Cond::All(parts) | Cond::Any(parts) => {
            parts.iter().find_map(|part| not_known(part, walker))
        }
        Cond::Not(inner) | Cond::Otherwise(inner) => not_known(inner, walker),
        leaf => leaf.ask(walker).is_none().then_some(leaf),
    }
}

fn how(crossing: &Crossing) -> String {
    match crossing {
        Crossing::Command(command) => command.clone(),
        Crossing::Steps(steps) => steps
            .iter()
            .map(|step| format!("{:?}", step.action))
            .collect::<Vec<_>>()
            .join("; "),
        other => format!("{other:?}"),
    }
}

/// The rows as a table of text, route2's columns: step, trip time, this
/// step's time, the move, the room, its name and where it is. A rule every
/// ten rows, as route2 has it, and the shut exits under the row that left
/// them behind.
#[must_use]
pub fn table(map: &Map, from: RoomId, legs: &[Leg]) -> Vec<String> {
    const MOVE_WIDTH: usize = 28;
    let clip = |text: &str| {
        if text.chars().count() > MOVE_WIDTH {
            let kept: String = text.chars().take(MOVE_WIDTH - 3).collect();
            format!("{kept}...")
        } else {
            text.to_owned()
        }
    };
    let row = |step: usize, trip: &str, time: &str, how: &str, room: RoomId, name: &str| {
        format!(
            "{step:>4}  {trip:>8}  {time:>6}  {:<MOVE_WIDTH$}  {:>6}  {name}",
            clip(how),
            room.0
        )
    };
    let name_of = |title: Option<&String>, location: Option<&String>| match (title, location) {
        (Some(title), Some(location)) => format!("{title} ({location})"),
        (Some(title), None) => title.clone(),
        (None, _) => String::new(),
    };
    let start = map.room(from);
    let mut lines = vec![
        format!(
            "STEP      TRIP    TIME  {:<MOVE_WIDTH$}    ROOM  NAME",
            "MOVE"
        ),
        row(
            0,
            "",
            "",
            "",
            from,
            &name_of(
                start.and_then(|room| room.title.first()),
                start.and_then(|room| room.location.as_ref()),
            ),
        ),
    ];
    for (index, leg) in legs.iter().enumerate() {
        let step = index + 1;
        if step % 10 == 0 {
            lines.push("-".repeat(72));
        }
        for shut in &leg.shut {
            let why = match &shut.why {
                ShutWhy::NotKnown(question) => format!("not known yet: {question}"),
                ShutWhy::SaysNo => "not for this character".to_owned(),
                ShutWhy::NotBuiltYet => "Hydra cannot cross this yet".to_owned(),
                ShutWhy::NoCost => "the map gives it no cost".to_owned(),
            };
            lines.push(format!(
                "      shut: {} -> {}: {why}",
                clip(&shut.how),
                shut.to.0
            ));
        }
        lines.push(row(
            step,
            &format!("{:.1}", leg.so_far),
            &format!("{:.1}", leg.seconds),
            &leg.how,
            leg.to,
            &name_of(leg.title.as_ref(), leg.location.as_ref()),
        ));
    }
    lines
}
