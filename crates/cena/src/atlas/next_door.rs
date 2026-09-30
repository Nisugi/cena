//! The maps next door (`plan/53` §8b, the author: *"let's try the joins
//! too. We might remove them we might keep them, but might as well see"*).
//! Each map keeps its own sheet; one next door is drawn beside it where a
//! walk joins them: the two rooms of the first compass walk found one
//! street step apart in its direction, pushed on along it off anything
//! already drawn. A character crossing is then drawn on the other sheet,
//! the first placed by the same walk the other way, so little moves.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, PoisonError};

use cena_behavior::travel::{Map, RoomId};
use cena_ui::{MapScene, NextDoor};

use super::Atlas;

/// One street step, in cells.
const STEP: i32 = cena_map_layout::interior_shelf::TOWN_SCALE;

/// How many steps a map is pushed off another before it is left there.
const MOST_PUSHES: i32 = 12;

/// A walk from one map into another: the room either side, and the step
/// its direction takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Join {
    here: RoomId,
    there: RoomId,
    step: (i32, i32),
}

impl Atlas {
    /// The maps next door to the sheet `sheet` (an area, or a place opened
    /// on one), `rings` deep: each placed as its first walk from a map
    /// already placed says. A map not laid out yet is asked for and left
    /// out until it is.
    pub(crate) fn next_door(&self, map: &Map, sheet: &str, rings: usize) -> Vec<NextDoor> {
        let base = sheet.split('@').next().unwrap_or(sheet);
        let Some(home) = self.laid(base) else {
            return Vec::new();
        };
        let mut taken = cells(&home, (0, 0));
        let mut placed: Vec<(String, Arc<MapScene>, (i32, i32))> =
            vec![(base.to_owned(), home, (0, 0))];
        let mut ring = vec![0];
        for _ in 0..rings {
            let mut next = Vec::new();
            for from in ring {
                let (name, scene, offset) = placed[from].clone();
                for (other, join) in self.joins(map, &name) {
                    if placed.iter().any(|(placed, ..)| *placed == other) {
                        continue;
                    }
                    let Some(there) = self.laid(&other) else {
                        self.ask(&other);
                        continue;
                    };
                    let Some(at) = beside(&scene, offset, &there, join, &taken) else {
                        continue;
                    };
                    taken.extend(cells(&there, at));
                    next.push(placed.len());
                    placed.push((other, there, at));
                }
            }
            ring = next;
        }
        placed
            .into_iter()
            .skip(1)
            .map(|(_, scene, (x, y))| {
                #[allow(clippy::cast_precision_loss)]
                let offset = (x as f32, y as f32);
                NextDoor { scene, offset }
            })
            .collect()
    }

    /// Each map a walk leads into from `area`, by its first compass walk,
    /// least rooms first; kept once worked out.
    fn joins(&self, map: &Map, area: &str) -> Vec<(String, Join)> {
        if let Some(known) = self
            .joins
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(area)
        {
            return known.clone();
        }
        let dirs = self
            .dirs
            .get_or_init(|| cena_map_layout::DirectionMap::build(map));
        let mut first: BTreeMap<String, Join> = BTreeMap::new();
        for &here in self.rooms.get(area).into_iter().flatten() {
            for exit in map.room(here).into_iter().flat_map(|room| &room.exits) {
                let Some(other) = self.area_of.get(&exit.to.0).filter(|o| *o != area) else {
                    continue;
                };
                let Some(dir) = dirs.get(here, exit.to).filter(|d| d.is_compass()) else {
                    continue;
                };
                let join = Join {
                    here,
                    there: exit.to,
                    step: dir.offset(),
                };
                first
                    .entry(other.clone())
                    .and_modify(|kept| {
                        if (join.here, join.there) < (kept.here, kept.there) {
                            *kept = join;
                        }
                    })
                    .or_insert(join);
            }
        }
        let joins: Vec<(String, Join)> = first.into_iter().collect();
        self.joins
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(area.to_owned(), joins.clone());
        joins
    }
}

/// Where `there` goes beside `here` (drawn at `offset`) by `join`: its room
/// one step from `here`'s in the walk's direction, pushed on along it off
/// the cells `taken`. `None` when either room is not drawn.
fn beside(
    here: &MapScene,
    offset: (i32, i32),
    there: &MapScene,
    join: Join,
    taken: &HashSet<(i32, i32)>,
) -> Option<(i32, i32)> {
    let from = here.room(join.here.0)?.cell;
    let to = there.room(join.there.0)?.cell;
    let (dx, dy) = (join.step.0 * STEP, join.step.1 * STEP);
    let mut at = (from.0 + offset.0 + dx - to.0, from.1 + offset.1 + dy - to.1);
    for _ in 0..MOST_PUSHES {
        if !cells(there, at).iter().any(|c| near(taken, *c)) {
            break;
        }
        at = (at.0 + dx, at.1 + dy);
    }
    Some(at)
}

/// The cells `scene`'s rooms are drawn in, moved by `at`.
fn cells(scene: &MapScene, at: (i32, i32)) -> HashSet<(i32, i32)> {
    scene
        .rooms
        .iter()
        .map(|r| (r.cell.0 + at.0, r.cell.1 + at.1))
        .collect()
}

/// Whether a cell of `taken` is `cell` or beside it.
fn near(taken: &HashSet<(i32, i32)>, cell: (i32, i32)) -> bool {
    (-1..=1).any(|dx| (-1..=1).any(|dy| taken.contains(&(cell.0 + dx, cell.1 + dy))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(area: &str, rooms: &[(u32, (i32, i32))]) -> MapScene {
        MapScene {
            area: area.to_owned(),
            rooms: rooms
                .iter()
                .map(|&(id, cell)| cena_ui::SceneRoom {
                    id,
                    cell,
                    title: String::new(),
                    building: None,
                })
                .collect(),
            ..MapScene::default()
        }
    }

    /// Kraken Manor and its Wine Cellar, the smallest two maps a compass
    /// walk joins in the map Hydra ships: from the manor, the cellar is
    /// drawn beside it, no room of it on or beside one of the manor's.
    #[test]
    fn the_cellar_is_drawn_beside_the_manor() {
        let whole = cena_behavior::travel::read_map(cena_gs_map::GS_MAP).expect("decodes");
        let areas = cena_map_layout::areas::baked(&whole);
        let rooms = ["Kraken Manor", "Wine Cellar"]
            .iter()
            .flat_map(|area| &areas[*area])
            .filter_map(|&id| whole.room(id).cloned())
            .collect();
        let map = Arc::new(Map::from_rooms(rooms).expect("a subset"));
        let dir = std::env::temp_dir().join(format!("hydra-next-door-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let atlas = Atlas::start(&map, "5555555555555555", &dir);
        let deadline = std::time::Instant::now() + std::time::Duration::from_mins(1);
        let next = loop {
            let next = atlas.next_door(&map, "Kraken Manor", 1);
            if !next.is_empty() || std::time::Instant::now() > deadline {
                break next;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        assert_eq!(next.len(), 1, "the cellar next door");
        assert_eq!(next[0].scene.area, "Wine Cellar");
        let manor = cells(&atlas.laid("Kraken Manor").expect("laid"), (0, 0));
        #[allow(clippy::cast_possible_truncation)]
        let at = (next[0].offset.0 as i32, next[0].offset.1 as i32);
        assert!(
            cells(&next[0].scene, at).iter().all(|c| !near(&manor, *c)),
            "no room of the cellar on the manor's"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// East from room 1 to room 2: the map next door has room 2 one step
    /// east of room 1; where that is taken, it is pushed on east.
    #[test]
    fn a_map_next_door_is_a_step_along_its_walk() {
        let here = sheet("a", &[(1, (0, 0)), (3, (0, 4))]);
        let there = sheet("b", &[(2, (10, 10)), (4, (14, 10))]);
        let east = Join {
            here: RoomId(1),
            there: RoomId(2),
            step: (1, 0),
        };
        let taken = cells(&here, (0, 0));
        assert_eq!(beside(&here, (0, 0), &there, east, &taken), Some((-6, -10)));
        let crowded: HashSet<(i32, i32)> = taken.into_iter().chain([(4, 0)]).collect();
        assert_eq!(
            beside(&here, (0, 0), &there, east, &crowded),
            Some((-2, -10))
        );
    }
}
