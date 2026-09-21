//! What the driver reads for the deeds that keep something to give back: the
//! language spoken, the key taken out, and the game's id for a named thing
//! (`plan/24` stage 3's last four steps). **Pure**: the model and the game's
//! answer in, a command or a fact out.

use cena_session::{ChunkLine, GameState, LinkKind};

/// What the game calls the tongue `speak <this>` selects, where the two
/// differ. Upstream's guild door sends `speak wizard` unless the answer to
/// `speak` was `Guildspeak`.
const TONGUES: [(&str, &str); 1] = [("wizard", "Guildspeak")];

/// Whether `speaking`, as the game names it, is what `speak <language>` sets.
pub(super) fn is_spoken(language: &str, speaking: &str) -> bool {
    let named = TONGUES
        .iter()
        .find(|(word, _)| word.eq_ignore_ascii_case(language))
        .map_or(language, |(_, named)| named);
    named.eq_ignore_ascii_case(speaking)
}

/// The language in `You are currently speaking Common.`
pub(super) fn language_in(answer: &[ChunkLine]) -> Option<String> {
    answer.iter().find_map(|line| {
        let text = line.text();
        let (_, rest) = text.split_once("You are currently speaking ")?;
        let language = rest.trim_end().trim_end_matches('.');
        (!language.is_empty()).then(|| language.to_owned())
    })
}

/// What `get my heavy key` took, and out of what, as the game's ids: the
/// first two things linked in `You remove a heavy iron key from in your
/// cloak.` (`heavy_key.rb`, `lockpick_shed.rb`). `None` for `Get what?`.
pub(super) fn taken_from(answer: &[ChunkLine]) -> Option<(String, String)> {
    answer.iter().find_map(|line| {
        let text = line.text();
        (text.starts_with("You remove") || text.contains("detach")).then_some(())?;
        let mut ids = line.objects().filter_map(|link| match &link.kind {
            LinkKind::Exist { id, .. } => Some(id.clone()),
            _ => None,
        });
        Some((ids.next()?, ids.next()?))
    })
}

/// `#id` for the thing with exactly this name: on the walker first, then in
/// the room -- its objects, and what its description links (a building with
/// a long name). Names are Lich's `GameObj` names: no article.
pub(super) fn item_id(state: &GameState, name: &str) -> Option<String> {
    let carried = state.inventory_snapshot.on_person().find_map(|item| {
        let named = format!("{} {}", item.adjective, item.noun);
        (named.trim() == name || item.name == name).then(|| item.id.clone())
    });
    let room = &state.room;
    let shown = || {
        room.objects
            .iter()
            .find(|thing| thing.text == name)
            .map(|thing| thing.id.clone())
    };
    let described = || {
        room.description
            .as_ref()?
            .objects()
            .find_map(|link| match &link.kind {
                LinkKind::Exist { id, .. } if link.text == name => Some(id.clone()),
                _ => None,
            })
    };
    carried.or_else(shown).or_else(described)
}

/// A command with every `{item:name}` made `#id`. `None` when the walker has
/// no such thing and the room shows none.
pub(super) fn with_items(command: &str, state: &GameState) -> Option<String> {
    let mut out = String::new();
    let mut rest = command;
    while let Some((before, after)) = rest.split_once("{item:") {
        let (name, tail) = after.split_once('}')?;
        out.push_str(before);
        out.push('#');
        out.push_str(&item_id(state, name)?);
        rest = tail;
    }
    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guildspeak_is_what_speak_wizard_sets() {
        assert!(is_spoken("wizard", "Guildspeak"));
        assert!(!is_spoken("wizard", "Common"));
        assert!(is_spoken("common", "Common"));
    }

    #[test]
    fn the_language_is_read_without_its_full_stop() {
        let answer = [ChunkLine::plain("You are currently speaking Common.")];
        assert_eq!(language_in(&answer).as_deref(), Some("Common"));
        assert_eq!(language_in(&[ChunkLine::plain("Get what?")]), None);
    }

    #[test]
    fn a_command_with_no_item_in_it_is_sent_as_written() {
        let state = GameState::default();
        assert_eq!(with_items("go door", &state).as_deref(), Some("go door"));
        assert_eq!(with_items("remove {item:brass key}", &state), None);
    }
}
