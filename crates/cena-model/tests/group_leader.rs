//! "Am I the leader?" -- three answers, not two (`plan/39` §4 gap 2, §8).
//!
//! Lich's `@@leader` is `nil`, `:self` or a `GameObj`
//! (`gemstone/group.rb:21`, `:266-286`). This model spelled the first two
//! alike, as `None`, so a consumer asking "is it me?" could not tell "yes"
//! from "nobody has said" -- and every group role is read off this fact
//! (`plan/39` §8, questions 2 and 8).
//!
//! Wire lines are written from Lich's patterns and `@example` comments
//! (`group.rb:418-529`). The character is the committed fixtures' own:
//! `<playerID id='966483'/>` (`cena-protocol/tests/fixtures/login_burst.xml:3`),
//! and `Ashryn`, the name the scrubber gave him in `character_info.xml`.

use cena_model::GameState;
use cena_model::state::group::{Group, Leader, Member};
use cena_protocol::Parser;
use cena_protocol::frame::{Frame, LinkKind};

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    for frame in parser.parse_line("<prompt time=\"1\">&gt;</prompt>") {
        state.apply(&frame);
    }
    state
}

/// The leader, when it is someone else.
fn other(group: &Group) -> Option<&Member> {
    match group.leader() {
        Leader::Other(member) => Some(member),
        Leader::Unknown | Leader::You => None,
    }
}

/// This character's `<playerID>`, as the login burst sends it.
const I_AM: &str = "<playerID id='966483'/>";
/// The same character as the game's links name him.
const ME: &str = r#"<a exist="-10966483" noun="Ashryn">Ashryn</a>"#;
/// `group.rb:438`'s example.
const ETANAMIR_MAKES_YOU_LEADER: &str = concat!(
    r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> designates you "#,
    "as the new leader of the group.",
);
const JOIN_ETANAMIR: &str = r#"You join <a exist="-10488845" noun="Etanamir">Etanamir</a>."#;

#[test]
fn nobody_has_said_in_a_new_session() {
    let group = state_after(&[]).group;
    assert_eq!(
        *group.leader(),
        Leader::Unknown,
        "not `You`: nobody said so"
    );
}

#[test]
fn the_three_answers_are_three() {
    let unknown = state_after(&[]).group;
    let you = state_after(&[ETANAMIR_MAKES_YOU_LEADER]).group;
    let someone = state_after(&[JOIN_ETANAMIR]).group;
    assert_eq!(*you.leader(), Leader::You);
    assert_eq!(other(&someone).map(|m| m.noun.as_str()), Some("Etanamir"));
    assert_ne!(unknown.leader(), you.leader(), "the gap `plan/39` §4 named");
    assert_ne!(unknown.leader(), someone.leader());
}

mod reconnect {
    use super::{ETANAMIR_MAKES_YOU_LEADER, JOIN_ETANAMIR, Leader, state_after};

    #[test]
    fn leading_goes_back_to_unknown() {
        // Not to `You`: the group can change hands while we are gone, and
        // only the `group` command restates it.
        let mut state = state_after(&[ETANAMIR_MAKES_YOU_LEADER]);
        assert_eq!(*state.group.leader(), Leader::You, "guard: known first");
        state.invalidate_for_reconnect();
        assert_eq!(*state.group.leader(), Leader::Unknown);
    }

    #[test]
    fn someone_else_leading_goes_back_to_unknown() {
        let mut state = state_after(&[JOIN_ETANAMIR]);
        assert!(
            matches!(state.group.leader(), Leader::Other(_)),
            "guard: known first"
        );
        state.invalidate_for_reconnect();
        assert_eq!(*state.group.leader(), Leader::Unknown);
    }

    #[test]
    fn the_indicator_going_dark_afterwards_says_you() {
        // `GROUP_EMPTIED` (`group.rb:603-605`): the game saying you are in
        // no group is the game saying who leads.
        let mut state = state_after(&[JOIN_ETANAMIR]);
        state.invalidate_for_reconnect();
        let mut parser = cena_protocol::Parser::new();
        for frame in parser.parse_line(r#"<indicator id="IconJOINED" visible="n"/>"#) {
            state.apply(&frame);
        }
        assert_eq!(*state.group.leader(), Leader::You);
    }
}

mod your_own_id {
    use super::{Frame, I_AM, LinkKind, ME, state_after};

    #[test]
    fn the_login_burst_gives_it() {
        let character = state_after(&[I_AM]).character;
        assert_eq!(character.player_id.as_deref(), Some("966483"), "verbatim");
        assert_eq!(character.exist_id().as_deref(), Some("-10966483"));
        assert!(ME.contains("-10966483"), "guard: the tests' link is it");
    }

    #[test]
    fn it_is_the_id_the_same_session_links_the_character_by() {
        // The one VERIFIED pair behind `Character::exist_id`: both fixtures
        // are cut from one log (`cena-protocol/tests/FIXTURES.md`), and `info`
        // names the character by a link.
        let burst = fixture("login_burst.xml");
        let info = fixture("character_info.xml");
        let named = info
            .iter()
            .filter_map(|frame| match frame {
                Frame::Text(text) => text.link.as_ref(),
                _ => None,
            })
            .find_map(|link| match &link.kind {
                LinkKind::Exist { id, noun } if noun == "Ashryn" => Some(id.clone()),
                _ => None,
            });
        assert_eq!(named.as_deref(), Some("-10966483"), "guard: `Name:` link");

        let mut state = cena_model::GameState::default();
        for frame in &burst {
            state.apply(frame);
        }
        assert_eq!(state.character.exist_id(), named);
    }

    #[test]
    fn it_is_kept_across_a_reconnect() {
        // Who the character IS does not change; the burst re-sends it anyway.
        let mut state = state_after(&[I_AM]);
        state.invalidate_for_reconnect();
        assert_eq!(state.character.player_id.as_deref(), Some("966483"));
    }

    #[test]
    fn an_empty_or_unreadable_id_is_no_id() {
        let empty = state_after(&["<playerID id=''/>"]).character;
        assert_eq!(empty.player_id, None, "empty is not an id");
        let odd = state_after(&["<playerID id='abc'/>"]).character;
        assert_eq!(odd.player_id.as_deref(), Some("abc"), "kept verbatim");
        assert_eq!(odd.exist_id(), None, "but no link is named by it");
    }

    /// A committed fixture's frames. A helper returns rather than panics.
    fn fixture(name: &str) -> Vec<Frame> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cena-protocol/tests/fixtures")
            .join(name);
        let bytes = std::fs::read(&path).unwrap_or_default();
        let mut parser = cena_protocol::Parser::new();
        let mut frames = parser.push_bytes(&bytes);
        frames.extend(parser.push_bytes(b"\n"));
        frames
    }
}

/// A line that names YOU by a link: read as you, by the id `<playerID>` gave.
mod you_by_link {
    use super::{I_AM, JOIN_ETANAMIR, Leader, ME, other, state_after};

    fn grouped_with_me_first() -> String {
        format!(r#"You are grouped with {ME}, <a exist="-10974229" noun="Szan">Szan</a>."#)
    }

    #[test]
    fn a_roster_naming_you_first_names_you_leader() {
        let group = state_after(&[I_AM, &grouped_with_me_first()]).group;
        assert_eq!(*group.leader(), Leader::You);
    }

    #[test]
    fn you_are_never_your_own_member() {
        let group = state_after(&[I_AM, &grouped_with_me_first()]).group;
        let nouns: Vec<&str> = group.members().iter().map(|m| m.noun.as_str()).collect();
        assert_eq!(nouns, ["Szan"]);
    }

    #[test]
    fn without_the_id_the_same_link_is_someone_else() {
        // The control: it is `<playerID>` that makes the link you, so the
        // input above reaches the id comparison and not some other branch.
        let group = state_after(&[&grouped_with_me_first()]).group;
        assert_eq!(other(&group).map(|m| m.noun.as_str()), Some("Ashryn"));
        assert_eq!(group.members().len(), 2);
    }

    #[test]
    fn a_swap_naming_you_by_link_is_you() {
        let swap = format!(
            r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> designates {ME} as the new leader of the group."#
        );
        let group = state_after(&[I_AM, JOIN_ETANAMIR, &swap]).group;
        assert_eq!(*group.leader(), Leader::You);
    }

    #[test]
    fn a_roster_naming_nobody_names_no_leader() {
        // `grouped with` and no link: not you (it did not say `leading`),
        // and nobody the model can hold.
        let group = state_after(&[JOIN_ETANAMIR, "You are grouped with nobody."]).group;
        assert_eq!(*group.leader(), Leader::Unknown);
        assert!(group.is_empty());
    }
}
