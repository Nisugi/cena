//! Offline presentation adapter: selected rooms in, native scenes out.
//! Does not alter the map, its exits, or the layout engine.
use cena_map::{Map, RoomId};
use cena_map_layout::{build_scene, generate_layout};
use serde_json::json;
use std::{collections::BTreeMap, env, fs, io::BufWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let map = cena_map::binary::decode(&fs::read(args.get(1).ok_or("map path required")?)?)?;
    if args.get(2).map(String::as_str) == Some("--graph") {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(args.get(3).ok_or("new output JSON required")?)?;
        serde_json::to_writer(BufWriter::new(file), &json!({"rooms":map.rooms()}))?;
        return Ok(());
    }
    let selections: BTreeMap<String, Vec<u32>> =
        serde_json::from_slice(&fs::read(args.get(2).ok_or("selection JSON required")?)?)?;
    let mut scenes = BTreeMap::new();
    for (name, ids) in selections {
        let records = ids
            .iter()
            .map(|id| {
                map.room(RoomId(*id))
                    .cloned()
                    .ok_or_else(|| format!("missing selected room {id}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let selected = Map::from_rooms(records)?;
        let layout = generate_layout(&selected);
        let scene = build_scene(&name, &layout, &selected);
        eprintln!(
            "{name}: {} rooms, {} units",
            scene.sheet.rooms.len(),
            scene.units.len()
        );
        scenes.insert(name, json!({"scene":scene,"rooms":selected.rooms()}));
    }
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(args.get(3).ok_or("new output JSON required")?)?;
    serde_json::to_writer(BufWriter::new(file), &scenes)?;
    Ok(())
}
