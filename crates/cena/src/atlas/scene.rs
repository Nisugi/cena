//! One area laid out by the engine, as the window's [`cena_ui::MapScene`].

use std::collections::{HashMap, HashSet};

use cena_behavior::travel::{Map, RoomId};
use cena_map_layout::scene::{SceneEdgeKind, UnitKind};

/// Lay out one area: its own rooms, plus the neighbours a stranded room
/// needs to stay attached (`cena_map_layout::areas::layout_rooms`), as the
/// mapper's window and gate lay it out. `None` when nothing in it is a
/// place.
pub(crate) fn lay_out(
    map: &Map,
    area: &str,
    rooms: &[RoomId],
    placeable: &HashSet<RoomId>,
) -> Option<cena_ui::MapScene> {
    sheet(
        area,
        &Map::from_rooms(cena_map_layout::areas::layout_rooms(rooms, map, placeable)).ok()?,
    )
}

/// The places `area`'s sheet leaves off: its hidden rooms
/// (`cena_map_layout::hidden`), joined by walks either way round, each
/// place's rooms. Each is laid out on a sheet of its own by [`lay_out_place`]:
/// the engine's indoor sheet, and `plan/53` §7 step 6's *"indoors, the
/// building alone"*. Rawknuckle's in the Hinterwilds is one.
pub(crate) fn places(map: &Map, rooms: &[RoomId], placeable: &HashSet<RoomId>) -> Vec<Vec<RoomId>> {
    let Ok(subset) = Map::from_rooms(cena_map_layout::areas::layout_rooms(rooms, map, placeable))
    else {
        return Vec::new();
    };
    let hidden = cena_map_layout::hidden::hidden_rooms(&subset);
    let mut next: HashMap<RoomId, Vec<RoomId>> = HashMap::new();
    for room in subset.rooms().iter().filter(|r| hidden.contains(&r.id)) {
        for exit in room
            .exits
            .iter()
            .filter(|e| cena_map_layout::regions::is_passage(e) && hidden.contains(&e.to))
        {
            next.entry(room.id).or_default().push(exit.to);
            next.entry(exit.to).or_default().push(room.id);
        }
    }
    let mut ids: Vec<RoomId> = hidden.iter().copied().collect();
    ids.sort_unstable();
    let mut seen: HashSet<RoomId> = HashSet::new();
    let mut places = Vec::new();
    for start in ids {
        if !seen.insert(start) {
            continue;
        }
        let mut place = vec![start];
        let mut i = 0;
        while i < place.len() {
            for &n in next.get(&place[i]).into_iter().flatten() {
                if seen.insert(n) {
                    place.push(n);
                }
            }
            i += 1;
        }
        places.push(place);
    }
    places
}

/// Lay out one hidden place on its own sheet, named `key`. All indoors, so
/// the engine hides none of it.
pub(crate) fn lay_out_place(map: &Map, key: &str, rooms: &[RoomId]) -> Option<cena_ui::MapScene> {
    let rooms = rooms
        .iter()
        .filter_map(|&id| map.room(id).cloned())
        .collect();
    sheet(key, &Map::from_rooms(rooms).ok()?)
}

/// Lay out `subset` as the sheet named `name`; `None` when it has no room.
fn sheet(name: &str, subset: &Map) -> Option<cena_ui::MapScene> {
    if subset.rooms().is_empty() {
        return None;
    }
    let layout = cena_map_layout::generate_layout(subset);
    Some(convert(
        name,
        &cena_map_layout::build_scene(name, &layout, subset),
    ))
}

/// The engine's scene as the window's: rooms, lines with their bends, the
/// way-in dots, and which building each room and line is in.
fn convert(area: &str, scene: &cena_map_layout::MapScene) -> cena_ui::MapScene {
    let sheet = &scene.sheet;
    // Units are the streets and each building; the window only needs to
    // know the buildings, numbered from 0.
    let mut buildings: Vec<String> = Vec::new();
    let mut building_of: HashMap<usize, usize> = HashMap::new();
    for (unit, u) in scene.units.iter().enumerate() {
        if u.kind == UnitKind::Building {
            building_of.insert(unit, buildings.len());
            buildings.push(u.name.clone());
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let point = |x: i32, y: i32| (x as f32, y as f32);
    let rooms = sheet
        .rooms
        .iter()
        .map(|r| cena_ui::SceneRoom {
            id: r.id.0,
            cell: (r.cell.x, r.cell.y),
            title: r.title.clone(),
            building: building_of.get(&r.unit).copied(),
        })
        .collect();
    let edges = sheet
        .edges
        .iter()
        .map(|e| {
            let mut path = vec![point(e.a.x, e.a.y)];
            path.extend(e.via.iter().map(|p| (p.x, p.y)));
            path.push(point(e.b.x, e.b.y));
            cena_ui::SceneEdge {
                a: e.a_room.0,
                b: e.b_room.0,
                kind: match e.kind {
                    SceneEdgeKind::Directional => cena_ui::EdgeKind::Directional,
                    SceneEdgeKind::Connector => cena_ui::EdgeKind::Connector,
                    SceneEdgeKind::Stub => cena_ui::EdgeKind::Stub,
                },
                path,
                label: e.label.clone(),
                building: e.unit.and_then(|u| building_of.get(&u).copied()),
            }
        })
        .collect();
    let doors = sheet
        .doors
        .iter()
        .map(|d| cena_ui::SceneDoor {
            street: d.street.0,
            inside: d.inside.0,
            at: (d.at.x, d.at.y),
            place: d.place.clone(),
            named: d.named(),
        })
        .collect();
    cena_ui::MapScene {
        area: area.to_owned(),
        rooms,
        edges,
        doors,
        buildings,
        min: (sheet.min.x, sheet.min.y),
        max: (sheet.max.x, sheet.max.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The map Hydra ships, decoded.
    fn shipped() -> Map {
        cena_behavior::travel::read_map(cena_gs_map::GS_MAP).expect("the shipped map decodes")
    }

    /// A small area of the shipped map lays out whole: every room once,
    /// every line between two drawn rooms along its own path.
    #[test]
    fn an_area_of_the_shipped_map_lays_out() {
        let map = shipped();
        let areas = cena_map_layout::areas::baked(&map);
        let placeable = cena_map_layout::regions::placeable_rooms(&map);
        let rooms = &areas["icemule-trace-ranger-guild"];
        let scene = lay_out(&map, "icemule-trace-ranger-guild", rooms, &placeable)
            .expect("the guild is a place");

        assert_eq!(scene.area, "icemule-trace-ranger-guild");
        assert!(!scene.rooms.is_empty());
        let ids: HashSet<u32> = scene.rooms.iter().map(|r| r.id).collect();
        assert_eq!(ids.len(), scene.rooms.len(), "a room drawn twice");
        for edge in &scene.edges {
            assert!(ids.contains(&edge.a) && ids.contains(&edge.b));
            assert!(edge.path.len() >= 2);
            let start = scene.room(edge.a).expect("drawn").cell;
            #[allow(clippy::cast_precision_loss)]
            let start = (start.0 as f32, start.1 as f32);
            assert_eq!(edge.path[0], start, "a line starts at its room");
        }
        for room in &scene.rooms {
            assert!(room.cell.0 >= scene.min.0 && room.cell.0 <= scene.max.0);
            assert!(room.cell.1 >= scene.min.1 && room.cell.1 <= scene.max.1);
        }
    }
}
