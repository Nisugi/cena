//! `;room <number>`: a room of the map said in the story, as Lich's `;map`
//! says one on a shift-click (`respond room`, `Room#to_s` in
//! `lib/common/map/map_gs.rb:126-128`), for the minimap's Shift+click;
//! `;room <number> number` its number and title alone, for Ctrl+click
//! (`plan/53` §6 item 6, the author after the first run, 2026-09-29).

use cena_behavior::travel::{Map, RoomId};
use cena_session::SessionHandle;
use cena_session::{Body, Notice, NoticeKind};

use crate::commands::Took;

/// Answer `line` when it is `room`'s, and say the room; `None` for any
/// other word.
pub(crate) fn command(map: &Map, handle: &SessionHandle, line: &str) -> Option<Took> {
    let said = match parse(line)? {
        Ok((id, brief)) => described(map, id, brief).unwrap_or_else(|| {
            Notice::line(NoticeKind::Error, format!("The map has no room {id}."))
        }),
        Err(why) => Notice::line(NoticeKind::Error, why),
    };
    handle.say(said);
    Some(Took::Done)
}

/// `room 228` and `room 228 number`: the number, and whether only it is
/// asked for.
fn parse(line: &str) -> Option<Result<(u32, bool), String>> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("room") {
        return None;
    }
    let usage = || "room which? -- `;room 228`, or `;room 228 number`".to_owned();
    let Some(id) = words
        .next()
        .and_then(|w| w.trim_start_matches('#').parse().ok())
    else {
        return Some(Err(usage()));
    };
    Some(match (words.next(), words.next()) {
        (None, _) => Ok((id, false)),
        (Some(w), None) if w.eq_ignore_ascii_case("number") => Ok((id, true)),
        _ => Err(usage()),
    })
}

/// The room as `Room#to_s` gives it: its number and uid, its title and
/// where it is, what it looks like, and its ways out. `brief`: the number
/// and the title. `None` when the map has no such room.
fn described(map: &Map, id: u32, brief: bool) -> Option<Notice> {
    let room = map.room(RoomId(id))?;
    let title = room.title.last().map_or("", String::as_str);
    if brief {
        return Some(Notice::line(NoticeKind::Info, format!("#{id} {title}")));
    }
    let uid = room
        .uid
        .last()
        .map_or_else(String::new, |uid| format!(" (u{})", uid.0));
    let location = room
        .location
        .as_deref()
        .map_or_else(String::new, |l| format!(" ({l})"));
    let mut lines = vec![format!("#{id}{uid}:"), format!("{title}{location}")];
    lines.extend(room.description.last().cloned());
    lines.extend(room.paths.last().cloned());
    Some(Notice {
        kind: NoticeKind::Info,
        body: Body::Lines(lines),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_takes_a_number_and_perhaps_number() {
        assert_eq!(parse("room 228"), Some(Ok((228, false))));
        assert_eq!(parse("ROOM #228 number"), Some(Ok((228, true))));
        assert!(matches!(parse("room"), Some(Err(_))));
        assert!(matches!(parse("room bank"), Some(Err(_))));
        assert!(matches!(parse("room 228 please"), Some(Err(_))));
        assert_eq!(parse("go2 228"), None);
    }

    /// Rawknuckle's in the map Hydra ships, said as Lich says a room.
    #[test]
    fn a_room_is_said_as_lich_says_it() {
        let map = cena_behavior::travel::read_map(cena_gs_map::GS_MAP).expect("decodes");
        let Body::Lines(lines) = described(&map, 29877, false).expect("the tavern").body else {
            panic!("prose");
        };
        assert_eq!(lines[0], "#29877 (u7503251):");
        assert!(
            lines[1].starts_with("[Rawknuckle's, Watering Hole]"),
            "{}",
            lines[1]
        );
        assert!(
            lines.iter().any(|l| l.starts_with("Obvious exits")),
            "{lines:?}"
        );
        let Body::Lines(brief) = described(&map, 29877, true).expect("the tavern").body else {
            panic!("prose");
        };
        assert_eq!(brief, ["#29877 [Rawknuckle's, Watering Hole]"]);
    }
}
