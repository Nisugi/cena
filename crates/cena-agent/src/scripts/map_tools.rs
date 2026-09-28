//! The map's questions a runner asks (`plan/46` §11 step 8; `SCRIPTS.md`):
//! the shortest ways out of a room, what a path takes, every room, and which
//! rooms carry a tag or one of the game's numbers. Each is answered from
//! Hydra's map, and a way is priced for the character as travel prices its
//! own walk ([`super::local::Atlas::walker`]), so what a script is told is
//! what the character's walk would do.

use std::collections::BTreeSet;

use axum::http::request::Parts;
use cena_map::{RoomId, Target, Uid, Walker};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use super::Seat;
use super::local::Atlas;
use super::tools::{Scripting, json, seat};

/// The most rooms `rooms` answers at once.
pub const MAX_ROOMS: usize = 5_000;

/// Seconds Lich counts for a step it cannot price (`map_base.rb:253`,
/// `estimate_time`).
const UNPRICED_STEP: f64 = 0.2;

/// `route`'s question.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RouteAsked {
    /// The room to go from, by the map's number.
    pub from: u32,
    /// Where to: one room, the nearest of several, or absent for everywhere.
    pub to: Option<Vec<u32>>,
}

/// `seconds`'s question.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PathAsked {
    /// The rooms walked through, in order, by the map's numbers.
    pub path: Vec<u32>,
}

/// `rooms`'s question.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RoomsAsked {
    /// The rooms after this number; absent for the first.
    pub after: Option<u32>,
    /// How many at most; absent, or more than 5000, is 5000.
    pub limit: Option<usize>,
}

/// `find`'s question.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindAsked {
    /// The rooms carrying this tag.
    pub tag: Option<String>,
    /// The rooms the game numbers so.
    pub uid: Option<i64>,
}

#[tool_router(router = map_router, vis = "pub(super)")]
impl Scripting {
    #[tool(
        description = "The shortest ways out of a room, priced for the character as its own walk would be: `previous` (each room reached, and the room it was reached from) and `seconds` (each room reached, and the seconds to it), keyed by the map's numbers. `to` one room stops there; several, at the nearest of them; absent, every room that can be reached. `map` false when Hydra has no map."
    )]
    async fn route(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(RouteAsked { from, to }): Parameters<RouteAsked>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        let Some(atlas) = &seat.atlas else {
            return json(&serde_json::json!({ "map": false }));
        };
        let walker = walker(&seat, atlas).await?;
        let to: Vec<RoomId> = to.unwrap_or_default().into_iter().map(RoomId).collect();
        let target = match to.as_slice() {
            [] => Target::Everything,
            [one] => Target::Room(*one),
            several => Target::Nearest(several),
        };
        let routes = atlas
            .map
            .routes(RoomId(from), target, cena_map::priced_for(&walker));
        let (mut previous, mut seconds) = (serde_json::Map::new(), serde_json::Map::new());
        for (room, taken, came_from) in routes.settled() {
            seconds.insert(room.0.to_string(), serde_json::json!(taken));
            if let Some(came_from) = came_from {
                previous.insert(room.0.to_string(), serde_json::json!(came_from.0));
            }
        }
        json(&serde_json::json!({ "map": true, "previous": previous, "seconds": seconds }))
    }

    #[tool(
        description = "What a path takes, in seconds: each step priced for the character as its own walk would be, a step with no priced exit counted as 0.2 seconds, as Lich counts it. `map` false when Hydra has no map."
    )]
    async fn seconds(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(PathAsked { path }): Parameters<PathAsked>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        let Some(atlas) = &seat.atlas else {
            return json(&serde_json::json!({ "map": false }));
        };
        let walker = walker(&seat, atlas).await?;
        let price = cena_map::priced_for(&walker);
        let total: f64 = path
            .windows(2)
            .map(|step| {
                atlas
                    .map
                    .room(RoomId(step[0]))
                    .and_then(|room| {
                        room.exits
                            .iter()
                            .filter(|exit| exit.to == RoomId(step[1]))
                            .filter_map(|exit| price(room, exit))
                            .reduce(f64::min)
                    })
                    .unwrap_or(UNPRICED_STEP)
            })
            .sum();
        json(&serde_json::json!({ "map": true, "seconds": total }))
    }

    #[tool(
        description = "Every room of Hydra's map, a page at a time, in the order of their numbers: each room's `id`, `title`, `location`, `tags` and `uid` (the game's numbers). Ask the next page `after` the answer's `next`, which is null after the last. The rest of a room is `room`'s. `map` false when Hydra has no map."
    )]
    async fn rooms(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(RoomsAsked { after, limit }): Parameters<RoomsAsked>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        let Some(atlas) = &seat.atlas else {
            return json(&serde_json::json!({ "map": false }));
        };
        let limit = limit.unwrap_or(MAX_ROOMS).clamp(1, MAX_ROOMS);
        let mut rooms: Vec<&cena_map::Room> = atlas
            .map
            .rooms()
            .iter()
            .filter(|room| after.is_none_or(|after| room.id.0 > after))
            .collect();
        rooms.sort_by_key(|room| room.id);
        let more = rooms.len() > limit;
        rooms.truncate(limit);
        let next = more.then(|| rooms.last().map(|room| room.id.0)).flatten();
        let rooms: Vec<serde_json::Value> = rooms
            .into_iter()
            .map(|room| {
                serde_json::json!({
                    "id": room.id.0,
                    "title": room.title,
                    "location": room.location,
                    "tags": room.tags,
                    "uid": room.uid,
                })
            })
            .collect();
        json(&serde_json::json!({ "map": true, "rooms": rooms, "next": next }))
    }

    #[tool(
        description = "The rooms of Hydra's map that carry a `tag`, or that the game numbers `uid`, by the map's numbers in order. `map` false when Hydra has no map."
    )]
    async fn find(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(FindAsked { tag, uid }): Parameters<FindAsked>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        let Some(atlas) = &seat.atlas else {
            return json(&serde_json::json!({ "map": false }));
        };
        let ids: BTreeSet<u32> = match (tag, uid) {
            (Some(tag), _) => atlas
                .map
                .rooms()
                .iter()
                .filter(|room| room.tags.contains(&tag))
                .map(|room| room.id.0)
                .collect(),
            (None, Some(uid)) => atlas
                .map
                .ids_for_uid(Uid(uid))
                .iter()
                .map(|id| id.0)
                .collect(),
            (None, None) => return Err(ErrorData::invalid_params("a tag or a uid", None)),
        };
        json(&serde_json::json!({ "map": true, "ids": ids }))
    }

    #[tool(
        description = "Every tag any room of Hydra's map carries, in order. `map` false when Hydra has no map."
    )]
    async fn tags(&self, Extension(parts): Extension<Parts>) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        let Some(atlas) = &seat.atlas else {
            return json(&serde_json::json!({ "map": false }));
        };
        let tags: BTreeSet<&str> = atlas
            .map
            .rooms()
            .iter()
            .flat_map(|room| room.tags.iter().map(String::as_str))
            .collect();
        json(&serde_json::json!({ "map": true, "tags": tags }))
    }
}

/// The character as a walker, now.
async fn walker(seat: &Seat, atlas: &Atlas) -> Result<Walker, ErrorData> {
    let stop = tokio_util::sync::CancellationToken::new();
    let (snapshot, _) = crate::characters::subscribe(&seat.observer, &stop)
        .await
        .ok_or_else(|| ErrorData::internal_error("the character has no session", None))?;
    Ok((atlas.walker)(&seat.character, &snapshot.state))
}
