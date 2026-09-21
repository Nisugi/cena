//! What must be known before a trip is priced (`plan/24` stage 5,
//! `plan/21` §4.0: *a cost never acts*). **Pure**: the map and the game's
//! answers in, facts out. The asking is the driver's (`drive::preflight`).
//!
//! Upstream finds these out inside its cost scripts, which send commands
//! while Dijkstra runs. Here a cost only ever reads a fact, so whatever a
//! fact needs asked is asked once, before the first plan.

use cena_map::{Map, RoomId};
use cena_session::ChunkLine;

/// What `urchin status` said (`go2.lic:975-987`): access until a date or for
/// good is access; the game does not say "until" of a day that has passed.
/// `None`: it said none of the three, and nothing is learned.
pub(super) fn urchin_access(answer: &[ChunkLine]) -> Option<bool> {
    answer.iter().find_map(|line| {
        let text = line.text();
        if text.contains("You will have access to the urchin guides")
            || text.contains("permanent access to the urchin guides")
        {
            Some(true)
        } else if text.contains("You currently have no access to the urchin guides") {
            Some(false)
        } else {
            None
        }
    })
}

/// The silver a walk asks for: each room's `silver-cost:<to>:<silver>` tag
/// for the room that follows it (`go2.lic:958-973`). `path` begins where the
/// walker stands. A cost upstream scripts is none of the 88 in this map
/// (MEASURED: `silver-cost` tags whose amount is not digits, 0), and one that
/// does not parse costs nothing here rather than guessing.
pub(super) fn silver_for(map: &Map, path: &[RoomId]) -> u64 {
    path.windows(2)
        .filter_map(|pair| {
            let (from, to) = (map.room(*pair.first()?)?, *pair.get(1)?);
            from.tags.iter().find_map(|tag| {
                let (room, silver) = tag.strip_prefix("silver-cost:")?.split_once(':')?;
                (room.parse() == Ok(to.0)).then_some(())?;
                silver.parse::<u64>().ok()
            })
        })
        .sum()
}

/// What to say to a banker for this much: Pinefar's has no `withdraw`, and
/// hands out no less than twenty (`go2.lic:2282-2286`).
pub(super) fn withdraw_command(room_title: Option<&str>, silver: u64) -> String {
    if room_title.is_some_and(|title| title.contains("Pinefar, Depository")) {
        format!("ask banker for {} silvers", silver.max(20))
    } else {
        format!("withdraw {silver} silvers")
    }
}

#[cfg(test)]
mod tests {
    use cena_map::Room;

    use super::*;

    #[test]
    fn access_is_what_the_game_says_it_is() {
        let said = |text: &str| urchin_access(&[ChunkLine::plain(text)]);
        assert_eq!(
            said("You will have access to the urchin guides until 10/1/2026 12:00:00 CDT."),
            Some(true)
        );
        assert_eq!(
            said("You have permanent access to the urchin guides."),
            Some(true)
        );
        assert_eq!(
            said("You currently have no access to the urchin guides."),
            Some(false)
        );
        assert_eq!(said("What?"), None);
    }

    #[test]
    fn a_walk_costs_what_its_own_exits_ask_and_no_others() {
        let rooms: Vec<Room> = serde_json::from_str(
            r#"[{"id":1,"tags":["bank","silver-cost:2:50","silver-cost:9:7000"]},
                {"id":2,"tags":["silver-cost:3:25"]},{"id":3},{"id":9}]"#,
        )
        .unwrap();
        let map = Map::from_rooms(rooms).unwrap();
        let path = [RoomId(1), RoomId(2), RoomId(3)];
        assert_eq!(silver_for(&map, &path), 75);
        assert_eq!(silver_for(&map, &path[1..]), 25);
        assert_eq!(silver_for(&map, &[RoomId(3)]), 0);
    }

    #[test]
    fn pinefars_banker_is_asked_and_for_no_less_than_twenty() {
        assert_eq!(
            withdraw_command(Some("[Bank, Teller]"), 5),
            "withdraw 5 silvers"
        );
        assert_eq!(
            withdraw_command(Some("[Pinefar, Depository]"), 5),
            "ask banker for 20 silvers"
        );
        assert_eq!(withdraw_command(None, 300), "withdraw 300 silvers");
    }
}
