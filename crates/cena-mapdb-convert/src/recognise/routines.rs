//! Arms whose crossing is a named search (`cena_map::routine`).

use cena_map::{Crossing, RoomId, Routine};

use super::holes;

/// Every exit of the Confluence, 3,233 of them, is two statements: name a
/// goal, then call the one script that holds the search
/// (`Room[23282].wayto['23282']`, 4 KB, itself an exit from a room to itself
/// and never routed). The goal is the exit's own destination, or the word
/// `tranquility` for the exits that lead out of the plane.
pub(super) fn confluence(script: &str, to: u32) -> Option<Crossing> {
    let [goal] = holes(
        script,
        &[
            ";e $mapdb_confluence_target = ",
            "; Room[23282].wayto['23282'].call",
        ],
    )?[..] else {
        return None;
    };
    let leave = match goal {
        "'tranquility'" => true,
        room if room.parse() == Ok(to) => false,
        _ => return None,
    };
    Some(Crossing::Routine(Routine::Confluence { leave }))
}

/// The minotaur maze: 497 exits whose whole configuration is a goal and a set
/// of rooms, followed by 1.2 KB of search that is **the same text on every
/// one** -- kept verbatim beside this file, so a change to it upstream stops
/// the arm matching like any other.
pub(super) fn minotaur_maze(script: &str, to: u32) -> Option<Crossing> {
    const SEARCH: &str = include_str!("../upstream_scripts/minotaur_maze.rb");
    let [target, rooms] = holes(
        script,
        &[
            ";e target_room_id = ",
            "; maze_rooms = [",
            &format!("]; {SEARCH}"),
        ],
    )?[..] else {
        return None;
    };
    (target.parse() == Ok(to)).then_some(())?;
    let rooms: Vec<RoomId> = rooms
        .split(',')
        .map(|room| room.trim().parse().map(RoomId))
        .collect::<Result<_, _>>()
        .ok()?;
    (!rooms.is_empty()).then_some(())?;
    Some(Crossing::Routine(Routine::MinotaurMaze { rooms }))
}
