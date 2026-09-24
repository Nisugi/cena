//! One immutable map per process, shared by travel and presentation.
//! No alternate browser-side location resolver and no live game requests.

use cena_behavior::travel::{Map, Whence, read_map, room_of};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Owned by the application/session table, never a process global.
pub(crate) type ConfiguredMap = Result<Arc<MapContext>, String>;

pub(crate) struct MapContext {
    pub(crate) map: Arc<Map>,
    sha256: String,
}

impl MapContext {
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let map = read_map(bytes).map_err(|error| format!("Map cannot be used: {error}"))?;
        Ok(Self {
            map: Arc::new(map),
            sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }

    pub(crate) fn location(&self, state: &cena_session::GameState) -> cena_ui::MapLocationView {
        cena_ui::MapLocationView {
            map_sha256: self.sha256.clone(),
            // No motion history is invented from coalesced snapshots. If the
            // native resolver needs that history, show unknown for now.
            room: room_of(&self.map, state, Whence::Nowhere).map(|room| room.0),
        }
    }
}

/// Freeze both successful and failed loads until restart. All characters and
/// travel desks use these same bytes, even if the file is replaced on disk.
pub(crate) fn load() -> ConfiguredMap {
    let path = std::env::var_os(crate::travel::MAP_ENV)
        .ok_or_else(|| "No map. Set CENA_MAP to a combined map file and restart.".to_owned())?;
    let bytes = std::fs::read(path).map_err(|error| format!("Cannot read CENA_MAP: {error}"))?;
    MapContext::decode(&bytes).map(Arc::new)
}

pub(crate) fn projection(configured: &ConfiguredMap) -> Option<cena_web::MapProjection> {
    match configured {
        Ok(context) => {
            let context = Arc::clone(context);
            Some(Arc::new(move |snapshot| context.location(&snapshot.state)))
        }
        Err(error) => {
            eprintln!("[map] Minimap unavailable: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_uid_is_not_the_map_id_and_ambiguity_is_not_guessed() {
        let rooms = serde_json::from_str(
            r#"[
            {"id":228,"uid":[7000],"title":["[Town Square]"]},
            {"id":10,"uid":[8000]}, {"id":11,"uid":[8000]}
        ]"#,
        )
        .unwrap();
        let context = MapContext {
            map: Arc::new(Map::from_rooms(rooms).unwrap()),
            sha256: "a".repeat(64),
        };
        let mut state = cena_session::GameState::default();
        state.room.id = Some("7000".into());
        assert_eq!(context.location(&state).room, Some(228));
        assert_eq!(context.location(&state).map_sha256, context.sha256);
        state.room.id = Some("8000".into());
        assert_eq!(context.location(&state).room, None);
        state.room.id = Some("999999".into());
        assert_eq!(context.location(&state).room, None);
        // Login text can identify a room before the UID arrives, using the
        // exact same native ladder as travel.
        state.room.id = None;
        state.room.title = Some("Town Square".into());
        assert_eq!(context.location(&state).room, Some(228));
    }

    #[test]
    fn bad_binary_is_refused_and_fingerprint_is_of_input_bytes() {
        assert!(MapContext::decode(b"not a map").is_err());
        let bytes = [
            b"HYDRAMAP".as_slice(),
            &1_u32.to_le_bytes(),
            &0_u32.to_le_bytes(),
            &0_u32.to_le_bytes(),
        ]
        .concat();
        let context = MapContext::decode(&bytes).unwrap();
        assert!(context.map.rooms().is_empty());
        assert_eq!(context.sha256, format!("{:x}", Sha256::digest(&bytes)));
    }
}
