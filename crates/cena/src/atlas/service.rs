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

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError, RwLock};

use cena_behavior::travel::{Map, RoomId};

/// hydra-mapper's revision Hydra is built with: part of the cache's name,
/// so an engine that lays out differently never reads another's areas.
/// `the_engine_revision_is_the_one_pinned` holds it to `Cargo.lock`.
const ENGINE: &str = "c3f8c989d41e2b560731fc0fc9a28e944776651e";

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
    scenes: RwLock<HashMap<String, Arc<cena_ui::MapScene>>>,
    /// Each room's own area.
    area_of: HashMap<u32, String>,
    queue: Mutex<Queue>,
    ready: Condvar,
}

/// The areas left to lay out: those asked for first.
#[derive(Default)]
struct Queue {
    asked: VecDeque<String>,
    rest: VecDeque<String>,
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
            area_of,
            queue: Mutex::new(Queue {
                asked: VecDeque::new(),
                rest: areas.keys().cloned().collect(),
            }),
            ready: Condvar::new(),
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

    /// The scene of `room`'s area, or why there is none yet. Asking for an
    /// area not laid out puts it first in the queue.
    pub(crate) fn scene_of(&self, room: u32) -> Result<Arc<cena_ui::MapScene>, Waiting> {
        let area = self.area_of.get(&room).ok_or(Waiting::NoArea)?;
        if let Some(scene) = self.laid(area) {
            return Ok(scene);
        }
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = queue.rest.iter().position(|a| a == area) {
            queue.rest.remove(at);
            queue.asked.push_back(area.clone());
        }
        Err(Waiting::Laying)
    }

    /// How many areas are laid out, of how many.
    pub(crate) fn progress(&self) -> (usize, usize) {
        let done = self
            .scenes
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        let queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        (done, done + queue.asked.len() + queue.rest.len())
    }

    fn laid(&self, area: &str) -> Option<Arc<cena_ui::MapScene>> {
        self.scenes
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(area)
            .cloned()
    }

    /// One worker: the next area, from the cache or laid out, until none
    /// is left.
    fn work(
        &self,
        map: &Map,
        areas: &std::collections::BTreeMap<String, Vec<RoomId>>,
        placeable: &HashSet<RoomId>,
        cache: Option<&Path>,
    ) {
        loop {
            let next = {
                let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
                queue.asked.pop_front().or_else(|| queue.rest.pop_front())
            };
            let Some(area) = next else { return };
            let file = cache.map(|dir| dir.join(file_name(&area)));
            let scene = file.as_deref().and_then(read).or_else(|| {
                let scene = super::scene::lay_out(map, &area, &areas[&area], placeable)?;
                if let Some(file) = &file {
                    write(file, &scene);
                }
                Some(scene)
            });
            if let Some(scene) = scene {
                self.scenes
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(area, Arc::new(scene));
                self.ready.notify_all();
            }
        }
    }
}

/// The cache for this map and this engine: `atlas/<map>-<engine>` in the
/// data folder, or none if it cannot be made.
fn cache_dir(data: &Path, sha256: &str) -> Option<PathBuf> {
    let dir = data.join("atlas").join(format!(
        "{}-{}",
        &sha256[..sha256.len().min(16)],
        &ENGINE[..12]
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

fn read(file: &Path) -> Option<cena_ui::MapScene> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

fn write(file: &Path, scene: &cena_ui::MapScene) {
    if let Ok(bytes) = serde_json::to_vec(scene) {
        let _ = std::fs::write(file, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The revision the cache is named by is the one `Cargo.lock` pins, so
    /// a new engine never reads the old engine's areas.
    #[test]
    fn the_engine_revision_is_the_one_pinned() {
        let lock =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"))
                .expect("the workspace's Cargo.lock");
        assert!(
            lock.contains(&format!("hydra-mapper?rev={ENGINE}#")),
            "atlas::service::ENGINE is not the hydra-mapper revision Cargo.lock pins"
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
        let scene = wait_for(&atlas, room);
        assert_eq!(scene.area, "icemule-trace-ranger-guild");
        let cached = cache_dir(&dir, "0123456789abcdef")
            .expect("made")
            .join(file_name("icemule-trace-ranger-guild"));
        assert_eq!(read(&cached).as_ref(), Some(scene.as_ref()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn wait_for(atlas: &Atlas, room: u32) -> Arc<cena_ui::MapScene> {
        let mut queue = atlas.queue.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            drop(queue);
            if let Ok(scene) = atlas.scene_of(room) {
                return scene;
            }
            queue = atlas.queue.lock().unwrap_or_else(PoisonError::into_inner);
            queue = atlas
                .ready
                .wait_timeout(queue, std::time::Duration::from_millis(200))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}
