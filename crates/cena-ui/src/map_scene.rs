//! One area of the map as the minimap draws it (`plan/53` §2, §7 step 2):
//! rooms at cells, the lines between them by kind, and a dot for each way
//! into a place not drawn. Plain data -- no engine type and no egui -- so
//! the painter is a pure function of it, and a web page could draw the same.
//!
//! Cells are the layout's sheet cells: one step between two street rooms is
//! several cells, so a building's rooms fit between them. A point is a cell
//! position that need not be a cell's centre: a bend, or a dot beside a room.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Where a character is on the map, as its minimap draws it.
#[derive(Clone, Debug, PartialEq)]
pub enum MinimapView {
    /// Its area, and the room it is in.
    Here {
        /// The area, laid out.
        scene: Arc<MapScene>,
        /// The room's number.
        room: u32,
        /// The room the player clicked to go to, if any.
        target: Option<u32>,
        /// The way there as travel would walk it, the room it is in first
        /// and the target last; empty with no target, or no way.
        route: Vec<u32>,
        /// The maps next door, each placed beside this one where a walk
        /// joins them (`plan/53` §8b).
        next_door: Vec<NextDoor>,
    },
    /// Nothing to draw yet, and why, to say where the map would be.
    Waiting(String),
}

/// A map next door: its sheet, and where its cells sit on this one's.
#[derive(Clone, Debug, PartialEq)]
pub struct NextDoor {
    /// The map, laid out.
    pub scene: Arc<MapScene>,
    /// Added to each of its cells and points to draw it beside this one.
    pub offset: (f32, f32),
    /// How many maps out it is: 1 beside this one, 2 beside one of those.
    pub ring: u8,
}

/// One laid-out area.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MapScene {
    /// The area's name, as baked into the map (`meta:area:`).
    pub area: String,
    /// Every room drawn.
    pub rooms: Vec<SceneRoom>,
    /// The lines between them.
    pub edges: Vec<SceneEdge>,
    /// A dot for each way into a place left off the sheet.
    pub doors: Vec<SceneDoor>,
    /// The buildings drawn with the streets: a room's `building` indexes
    /// here, so a map can show one building alone when you are in it.
    pub buildings: Vec<String>,
    /// The least cell across and down any room reaches.
    pub min: (i32, i32),
    /// The greatest.
    pub max: (i32, i32),
}

/// One room on the sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneRoom {
    /// The map's room number (`;go2` takes it).
    pub id: u32,
    /// Its cell.
    pub cell: (i32, i32),
    /// The room's title, `[Town Square, Center]`.
    pub title: String,
    /// The building it is in, by index into [`MapScene::buildings`];
    /// `None` for a street or the country between.
    pub building: Option<usize>,
}

/// How a line is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    /// An exit with a compass direction: a solid line.
    Directional,
    /// An exit with no direction (`go door`, `climb rope`): a dashed line,
    /// which may bend round what is in its way.
    Connector,
    /// A link too long to draw across the sheet: a short mark at each end,
    /// each labelled with the room at the other.
    Stub,
}

/// One line between two rooms.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneEdge {
    /// One end's room number.
    pub a: u32,
    /// The other's.
    pub b: u32,
    /// How it is drawn.
    pub kind: EdgeKind,
    /// The line from `a` to `b`, both ends and every bend, as points.
    pub path: Vec<(f32, f32)>,
    /// What a connector says, when it is worth saying (`dock`, `gate`).
    pub label: Option<String>,
    /// The building the line is inside, when both ends are in one: drawn
    /// with that building only.
    pub building: Option<usize>,
}

/// A way into a place not drawn: a dot beside the street room it is
/// entered from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneDoor {
    /// The street room.
    pub street: u32,
    /// The first room inside.
    pub inside: u32,
    /// Where the dot is, as a point.
    pub at: (f32, f32),
    /// The place behind it (`Angargreft`, `Trader's Bank`).
    pub place: String,
    /// Whether the place is big enough to name on the map.
    pub named: bool,
}

impl MapScene {
    /// The room with number `id`, if it is drawn here.
    #[must_use]
    pub fn room(&self, id: u32) -> Option<&SceneRoom> {
        self.rooms.iter().find(|r| r.id == id)
    }
}
