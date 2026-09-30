//! Every area laid out at launch (the author, 2026-09-29: *"it just loads
//! 1 map for all characters yeah? So why not just load the entire thing
//! right then and there"*; `plan/53` §7 step 3).
//!
//! One [`Atlas`] per Hydra, shared by every character. Started with the map,
//! it lays out every area on the spare cores in the background, and keeps
//! each on disk in Hydra's data folder, under the map's hash and the
//! engine's revision: a later launch on the same map reads them back and
//! lays out nothing. An area a character is in and asks for goes to the
//! front of the queue.
//!
//! A room inside a place its area's sheet leaves off (a tavern, a shop, the
//! pits) is shown on that place's own sheet, laid out when first asked for
//! and kept the same way ([`super::scene::places`]).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError, RwLock};

use cena_behavior::travel::{Map, RoomId};

/// The hydra-mapper commit Hydra is built with, read from `Cargo.lock` by
/// the build (`build.rs`): part of the cache's name, so a newer engine
/// never reads an older one's areas.
const ENGINE: &str = env!("HYDRA_MAPPER_COMMIT");

/// Why a room has no scene yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Waiting {
    /// The room is in no area the map was laid out by.
    NoArea,
    /// Its area is still being laid out.
    Laying,
}

/// The areas, as they are laid out.
pub(crate) struct Atlas {
    /// Every sheet laid out, an area's or a place's, by its name.
    scenes: RwLock<HashMap<String, Arc<cena_ui::MapScene>>>,
    /// Each area as the engine laid it out, which a place is opened on.
    engines: RwLock<HashMap<String, Arc<cena_map_layout::MapScene>>>,
    /// Each room's own area.
    pub(super) area_of: HashMap<u32, String>,
    /// Each area's rooms.
    pub(super) rooms: std::collections::BTreeMap<String, Vec<RoomId>>,
    /// Which way each walk goes, for the maps next door; worked out the
    /// first time they are.
    pub(super) dirs: std::sync::OnceLock<cena_map_layout::DirectionMap>,
    /// Each area's walks into the areas next door (`next_door.rs`).
    pub(super) joins: RwLock<HashMap<String, Vec<(String, super::next_door::Join)>>>,
    /// The areas' names, to count them apart from the places.
    areas: HashSet<String>,
    /// The hidden places of the areas laid out.
    places: RwLock<Places>,
    queue: Mutex<Queue>,
    /// A sheet laid out.
    ready: Condvar,
    /// A sheet asked for: an idle worker takes it.
    asked: Condvar,
}

/// The places left off the areas' sheets: each one's rooms by its name,
/// and each room's place.
#[derive(Default)]
struct Places {
    rooms: HashMap<String, Vec<RoomId>>,
    of: HashMap<u32, String>,
}

/// The sheets left to lay out: those asked for first, then every area.
#[derive(Default)]
struct Queue {
    asked: VecDeque<String>,
    rest: VecDeque<String>,
    /// Taken by a worker: not to be queued again while it is laid out.
    taken: HashSet<String>,
}

impl Atlas {
    /// Start laying out every area of `map`, whose bytes hash to `sha256`,
    /// on a worker per spare core; cached under `data`.
    pub(crate) fn start(map: &Arc<Map>, sha256: &str, data: &Path) -> Arc<Self> {
        let areas = cena_map_layout::areas::baked(map);
        let area_of = areas
            .iter()
            .flat_map(|(name, rooms)| rooms.iter().map(move |r| (r.0, name.clone())))
            .collect();
        let atlas = Arc::new(Self {
            scenes: RwLock::default(),
            engines: RwLock::default(),
            area_of,
            areas: areas.keys().cloned().collect(),
            rooms: areas.clone(),
            dirs: std::sync::OnceLock::new(),
            joins: RwLock::default(),
            places: RwLock::default(),
            queue: Mutex::new(Queue {
                rest: areas.keys().cloned().collect(),
                ..Queue::default()
            }),
            ready: Condvar::new(),
            asked: Condvar::new(),
        });
        // Which way each walk goes, for the maps next door, worked out off
        // the window's thread before a character asks.
        let (ahead, whole) = (Arc::clone(&atlas), Arc::clone(map));
        let _ = std::thread::Builder::new()
            .name("atlas-dirs".to_owned())
            .spawn(move || {
                ahead
                    .dirs
                    .get_or_init(|| cena_map_layout::DirectionMap::build(&whole));
            });
        let cache = cache_dir(data, sha256);
        forget_other_maps(data, cache.as_deref());
        let placeable = Arc::new(cena_map_layout::regions::placeable_rooms(map));
        let areas = Arc::new(areas);
        let workers = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .saturating_sub(1)
            .max(1);
        for n in 0..workers {
            let (atlas, map, areas, placeable, cache) = (
                Arc::clone(&atlas),
                Arc::clone(map),
                Arc::clone(&areas),
                Arc::clone(&placeable),
                cache.clone(),
            );
            let spawned = std::thread::Builder::new()
                .name(format!("atlas-{n}"))
                .spawn(move || atlas.work(&map, &areas, &placeable, cache.as_deref()));
            if let Err(error) = spawned {
                eprintln!("[map] cannot start a layout worker: {error}");
            }
        }
        atlas
    }

    /// The sheet `room` is drawn on, or why there is none yet: its area's,
    /// or the place's its area leaves it off. Asking for a sheet not laid
    /// out puts it first in the queue.
    pub(crate) fn scene_of(&self, room: u32) -> Result<Arc<cena_ui::MapScene>, Waiting> {
        let area = self.area_of.get(&room).ok_or(Waiting::NoArea)?;
        let Some(scene) = self.laid(area) else {
            self.ask(area);
            return Err(Waiting::Laying);
        };
        if scene.room(room).is_some() {
            return Ok(scene);
        }
        let place = self
            .places
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .of
            .get(&room)
            .cloned();
        // A room neither drawn nor in a place: the area's sheet, which
        // says so.
        let Some(place) = place else {
            return Ok(scene);
        };
        self.laid(&place).ok_or_else(|| {
            self.ask(&place);
            Waiting::Laying
        })
    }

    /// Put the sheet `name` first in the queue, unless it is there already
    /// or being laid out.
    pub(super) fn ask(&self, name: &str) {
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = queue.rest.iter().position(|a| a == name) {
            queue.rest.remove(at);
        } else if queue.taken.contains(name) || queue.asked.iter().any(|a| a == name) {
            return;
        }
        queue.asked.push_back(name.to_owned());
        self.asked.notify_one();
    }

    /// How many areas are laid out, of how many.
    pub(crate) fn progress(&self) -> (usize, usize) {
        let done = self
            .scenes
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .filter(|name| self.areas.contains(*name))
            .count();
        (done, self.areas.len())
    }

    pub(super) fn laid(&self, area: &str) -> Option<Arc<cena_ui::MapScene>> {
        self.scenes
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(area)
            .cloned()
    }

    /// One worker: the next sheet, from the cache or laid out, and when none
    /// is left, waiting for one to be asked for: a place is laid out only
    /// when someone walks in, long after the areas are done. An area's
    /// places are known before its sheet is, so a room left off it always
    /// finds its place.
    fn work(
        &self,
        map: &Map,
        areas: &std::collections::BTreeMap<String, Vec<RoomId>>,
        placeable: &HashSet<RoomId>,
        cache: Option<&Path>,
    ) {
        loop {
            let name = {
                let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
                loop {
                    if let Some(name) = queue.asked.pop_front().or_else(|| queue.rest.pop_front()) {
                        queue.taken.insert(name.clone());
                        break name;
                    }
                    queue = self
                        .asked
                        .wait(queue)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            };
            let file = cache.map(|dir| dir.join(file_name(&name)));
            let scene = if let Some(rooms) = areas.get(&name) {
                self.remember_places(&name, super::scene::places(map, rooms, placeable));
                // An area is kept as the engine laid it out, which a place
                // is opened on, and drawn as the window's scene.
                let engine = file.as_deref().and_then(read).or_else(|| {
                    let engine = super::scene::lay_out(map, &name, rooms, placeable)?;
                    if let Some(file) = &file {
                        write(file, &engine);
                    }
                    Some(engine)
                });
                engine.map(|engine| {
                    let scene = super::scene::convert(&name, &engine);
                    self.engines
                        .write()
                        .unwrap_or_else(PoisonError::into_inner)
                        .insert(name.clone(), Arc::new(engine));
                    scene
                })
            } else {
                self.opened(map, &name, file.as_deref())
            };
            if let Some(scene) = scene {
                self.scenes
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(name.clone(), Arc::new(scene));
                self.ready.notify_all();
            }
            self.queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .taken
                .remove(&name);
        }
    }

    /// The place `name` (`area@room`) opened on its area's sheet, from the
    /// cache or opened now.
    fn opened(&self, map: &Map, name: &str, file: Option<&Path>) -> Option<cena_ui::MapScene> {
        if let Some(scene) = file.and_then(read) {
            return Some(scene);
        }
        let (area, _) = name.split_once('@')?;
        let engine = self
            .engines
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(area)
            .cloned()?;
        let rooms = self
            .places
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .rooms
            .get(name)
            .cloned()?;
        let scene = super::scene::open_place(map, name, &engine, &rooms)?;
        if let Some(file) = file {
            write(file, &scene);
        }
        Some(scene)
    }

    /// Keep `area`'s places, each named by the area and its least room:
    /// `the-hinterwilds@29877`.
    fn remember_places(&self, area: &str, places: Vec<Vec<RoomId>>) {
        let mut known = self.places.write().unwrap_or_else(PoisonError::into_inner);
        for rooms in places {
            let Some(least) = rooms.iter().min() else {
                continue;
            };
            let name = format!("{area}@{}", least.0);
            for room in &rooms {
                known.of.insert(room.0, name.clone());
            }
            known.rooms.insert(name, rooms);
        }
    }
}

/// The cache for this map and this engine: `atlas/<map>-<engine>` in the
/// data folder, or none if it cannot be made.
fn cache_dir(data: &Path, sha256: &str) -> Option<PathBuf> {
    let dir = data.join("atlas").join(format!(
        "{}-{}",
        &sha256[..sha256.len().min(16)],
        ENGINE.get(..12).unwrap_or(ENGINE)
    ));
    std::fs::create_dir_all(&dir)
        .map_err(|error| eprintln!("[map] no layout cache at {}: {error}", dir.display()))
        .ok()?;
    Some(dir)
}

/// Remove the caches of other maps and engines: each is a whole map's
/// layout that will never be read again.
fn forget_other_maps(data: &Path, keep: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(data.join("atlas")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if keep != Some(path.as_path()) && path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// An area's file name: its name, which is already a slug.
fn file_name(area: &str) -> String {
    let safe: String = area
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}.json")
}

fn read<T: serde::de::DeserializeOwned>(file: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

fn write<T: serde::Serialize>(file: &Path, scene: &T) {
    if let Ok(bytes) = serde_json::to_vec(scene) {
        let _ = std::fs::write(file, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The build found the mapper in `Cargo.lock`: a commit, not the
    /// fallback, so the cache is named by the engine it was laid out by.
    #[test]
    fn the_engine_is_named_by_its_commit() {
        assert!(
            ENGINE.len() == 40 && ENGINE.chars().all(|c| c.is_ascii_hexdigit()),
            "HYDRA_MAPPER_COMMIT is {ENGINE}"
        );
    }

    /// An area a character asks for is laid out first, then read back from
    /// the cache by a second atlas on the same map without laying it out.
    #[test]
    fn an_asked_area_comes_first_and_is_kept() {
        let bytes = cena_gs_map::GS_MAP;
        let map = Arc::new(cena_behavior::travel::read_map(bytes).expect("decodes"));
        let dir = std::env::temp_dir().join(format!("hydra-atlas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // A room of the ranger guild, one of the smallest areas.
        let areas = cena_map_layout::areas::baked(&map);
        let room = areas["icemule-trace-ranger-guild"][0].0;

        let atlas = Atlas::start(&map, "0123456789abcdef", &dir);
        assert_eq!(atlas.scene_of(u32::MAX).err(), Some(Waiting::NoArea));
        let scene = wait_for(&atlas, room).expect("laid out");
        assert_eq!(scene.area, "icemule-trace-ranger-guild");
        let cached = cache_dir(&dir, "0123456789abcdef")
            .expect("made")
            .join(file_name("icemule-trace-ranger-guild"));
        let engine: cena_map_layout::MapScene = read(&cached).expect("the engine's scene kept");
        assert_eq!(
            &super::super::scene::convert("icemule-trace-ranger-guild", &engine),
            scene.as_ref()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A room its area's sheet leaves off is shown with its place opened on
    /// the area's sheet where its dot was, the street still there
    /// (`plan/53` §8a): Rawknuckle's tavern, entered off Cold River's
    /// thoroughfare, where the author stood and saw no map (2026-09-29),
    /// then only the tavern (*"I don't want a 2 room minimap"*). Asked for once
    /// every area is laid out and the workers have nothing left, as the
    /// author walked in, it is laid out still: they waited for the ask,
    /// where they had gone and "206 of 206" stood for good.
    #[test]
    fn a_room_inside_is_on_its_place_s_own_sheet() {
        let whole = cena_behavior::travel::read_map(cena_gs_map::GS_MAP).expect("decodes");
        let rooms = cena_map_layout::areas::baked(&whole)["the-hinterwilds"]
            .iter()
            .filter_map(|&id| whole.room(id).cloned())
            .collect();
        let map = Arc::new(Map::from_rooms(rooms).expect("a subset"));
        let dir = std::env::temp_dir().join(format!("hydra-atlas-in-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (street, tavern) = (29869, 29877);

        let atlas = Atlas::start(&map, "fedcba9876543210", &dir);
        wait_for(&atlas, street).expect("the Hinterwilds laid out");
        idle(&atlas);
        let scene = wait_for(&atlas, tavern).expect("the tavern laid out once asked");
        assert!(
            scene.area.contains('@'),
            "the area's sheet, not the place's: {}",
            scene.area
        );
        let inside = scene.room(tavern).expect("the tavern is drawn");
        assert!(
            inside.building.is_some(),
            "the tavern is a building of the sheet"
        );
        let (thoroughfare, at) = (
            scene.room(street).expect("the street is still drawn"),
            inside.cell,
        );
        let steps = (at.0 - thoroughfare.cell.0)
            .abs()
            .max((at.1 - thoroughfare.cell.1).abs());
        assert!(
            steps <= 8,
            "the tavern is beside its door, {steps} cells off"
        );
        assert!(
            scene.doors.iter().all(|d| d.inside != tavern),
            "the tavern's dot gives way to the tavern"
        );
        let area = &atlas.area_of[&tavern];
        let sheet = atlas.laid(area).expect("the area laid out first");
        assert!(
            sheet.room(tavern).is_none(),
            "the tavern is left off the area's own sheet"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Cold River is on the Hinterwilds' map (the author, 2026-09-29), and
    /// the Issenflow's current, drawn only as dots at the banks it is
    /// entered from, gives that map with those dots.
    #[test]
    fn cold_river_is_on_the_hinterwilds_and_the_current_is_its_dots() {
        let map = Arc::new(cena_behavior::travel::read_map(cena_gs_map::GS_MAP).expect("decodes"));
        let dir = std::env::temp_dir().join(format!("hydra-atlas-hw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (thoroughfare, current) = (29869, 30115);

        let atlas = Atlas::start(&map, "0f0f0f0f0f0f0f0f", &dir);
        let scene = wait_for(&atlas, current).expect("laid out");
        assert_eq!(scene.area, "the-hinterwilds");
        assert!(scene.room(thoroughfare).is_some(), "Cold River on the map");
        assert!(scene.room(current).is_none());
        assert!(
            scene.doors.iter().filter(|d| d.inside == current).count() >= 3,
            "the current is dots at its banks"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `room`'s sheet, asked for until it comes, or `None` after a minute.
    fn wait_for(atlas: &Atlas, room: u32) -> Option<Arc<cena_ui::MapScene>> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_mins(1);
        let mut queue = atlas.queue.lock().unwrap_or_else(PoisonError::into_inner);
        while std::time::Instant::now() < deadline {
            drop(queue);
            if let Ok(scene) = atlas.scene_of(room) {
                return Some(scene);
            }
            queue = atlas.queue.lock().unwrap_or_else(PoisonError::into_inner);
            queue = atlas
                .ready
                .wait_timeout(queue, std::time::Duration::from_millis(200))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        None
    }

    /// Until nothing is queued or being laid out.
    fn idle(atlas: &Atlas) {
        loop {
            {
                let queue = atlas.queue.lock().unwrap_or_else(PoisonError::into_inner);
                if queue.asked.is_empty() && queue.rest.is_empty() && queue.taken.is_empty() {
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
