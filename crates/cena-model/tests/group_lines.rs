//! The group lines Lich reads that the port did not (`plan/39` §4, gap 3),
//! and the eight `HOLD_*` lines it set aside -- which the dead member's
//! recovery needs, because taking their hand is how it groups them (`plan/39`
//! §8, question 10).
//!
//! Every wire line is written from Lich's patterns and `@example` comments
//! (`gemstone/group.rb:418-529`), names and ids included.

use cena_model::GameState;
use cena_model::state::group::{self, GroupEvent, GroupStatus, Leader, Member};
use cena_protocol::Parser;

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

/// What the classifier makes of one line, before any state.
fn event_of(line: &str) -> Option<GroupEvent> {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(format!("{line}\n").as_bytes()) {
        state.apply(&frame);
    }
    state.open_chunk().lines().first().and_then(group::classify)
}

/// The leader, when it is someone else.
fn other(state: &GameState) -> Option<&Member> {
    match state.group.leader() {
        Leader::Other(member) => Some(member),
        Leader::Unknown | Leader::You => None,
    }
}

const OPEN: &str = "Your group status is currently open.";
const JOIN_ETANAMIR: &str = r#"You join <a exist="-10488845" noun="Etanamir">Etanamir</a>."#;
const OREH_JOINS: &str = r#"<a exist="-10467645" noun="Oreh">Oreh</a> joins your group."#;

/// `STATUS`, `^Your group status is currently (?<status>open|closed)\.`
/// (`group.rb:525`), and Lich's `checked` flag, which it sets.
mod status {
    use super::{JOIN_ETANAMIR, OPEN, OREH_JOINS, state_after};
    use cena_model::state::group::GroupStatus;

    #[test]
    fn open_and_closed_are_read() {
        assert_eq!(state_after(&[OPEN]).group.status(), Some(GroupStatus::Open));
        let closed = state_after(&["Your group status is currently closed."]).group;
        assert_eq!(closed.status(), Some(GroupStatus::Closed));
    }

    #[test]
    fn unstated_until_the_game_says() {
        // Lich starts at `:closed` (`group.rb:23`): a default, not a fact.
        let group = state_after(&[]).group;
        assert_eq!(group.status(), None);
        assert!(!group.checked());
    }

    #[test]
    fn a_word_the_pattern_does_not_name_is_not_read() {
        let group = state_after(&["Your group status is currently ajar."]).group;
        assert_eq!(group.status(), None);
    }

    #[test]
    fn a_player_saying_it_is_not_the_game_saying_it() {
        let said =
            r#"<a exist="-5" noun="Bob">Bob</a> says, "Your group status is currently open.""#;
        assert_eq!(state_after(&[said]).group.status(), None);
    }

    #[test]
    fn it_ends_the_group_answer_so_the_roster_is_checked() {
        let answer = [
            r#"You are leading <a exist="-10467645" noun="Oreh">Oreh</a>."#,
            OPEN,
        ];
        assert!(state_after(&answer).group.checked());
    }

    #[test]
    fn joining_someone_puts_the_roster_in_doubt() {
        // `checked = false` (`group.rb:629`): the new group's other members
        // are not named.
        assert!(state_after(&[OPEN]).group.checked(), "guard: checked first");
        assert!(!state_after(&[OPEN, JOIN_ETANAMIR]).group.checked());
        let added = concat!(
            r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> adds you to "#,
            r#"<a exist="-10488845" noun="Etanamir">his</a> group."#,
        );
        assert!(!state_after(&[OPEN, added]).group.checked());
    }

    #[test]
    fn someone_joining_yours_does_not() {
        assert!(state_after(&[OPEN, OREH_JOINS]).group.checked());
    }

    #[test]
    fn forgotten_on_a_reconnect() {
        let mut state = state_after(&[OPEN]);
        state.invalidate_for_reconnect();
        assert_eq!(state.group.status(), None);
        assert!(!state.group.checked());
    }
}

/// `NO_GROUP_TO_DISBAND`, `^You have no group to disband\.` (`group.rb:516`).
mod no_group_to_disband {
    use super::{JOIN_ETANAMIR, OPEN, other, state_after};

    const NONE_TO_DISBAND: &str = "You have no group to disband.";

    #[test]
    fn a_follower_keeps_the_group_and_doubts_it() {
        // Lich's own comment (`group.rb:514-515`): a follower hears it too,
        // so it cannot clear membership, only the check (`:620-621`).
        let state = state_after(&[JOIN_ETANAMIR, OPEN, NONE_TO_DISBAND]);
        assert!(state.group.contains("-10488845"), "the members stay");
        assert_eq!(other(&state).map(|m| m.noun.as_str()), Some("Etanamir"));
        assert!(!state.group.checked());
    }

    #[test]
    fn a_player_saying_it_is_not_the_game_saying_it() {
        let said = r#"<a exist="-5" noun="Bob">Bob</a> says, "You have no group to disband.""#;
        assert!(state_after(&[OPEN, said]).group.checked());
    }
}

/// `<X>'s group status is closed`: the refusal `Group.add` and `Group.join`
/// read (`group.rb:307-309`, `:349-351`).
mod refused {
    use super::{GroupEvent, OREH_JOINS, event_of, state_after};

    #[test]
    fn whoever_refuses_is_not_in_the_group() {
        // `Group.delete(member)` on it (`group.rb:317-319`).
        for refusal in [
            r#"<a exist="-10467645" noun="Oreh">Oreh's</a> group status is closed."#,
            r#"<a exist="-10467645" noun="Oreh">Oreh</a>'s group status is closed."#,
        ] {
            let group = state_after(&[OREH_JOINS, refusal]).group;
            assert!(group.is_empty(), "{refusal}");
        }
    }

    #[test]
    fn the_name_is_not_possessive() {
        let event =
            event_of(r#"<a exist="-10467645" noun="Oreh">Oreh's</a> group status is closed."#);
        let Some(GroupEvent::Refused(member)) = event else {
            panic!("not a refusal: {event:?}");
        };
        assert_eq!(
            (member.id.as_str(), member.text.as_str()),
            ("-10467645", "Oreh")
        );
    }

    #[test]
    fn saying_it_is_not_the_game_saying_it() {
        // One link, as the refusal has, and the name not first.
        let said =
            r#"You say, "<a exist="-10467645" noun="Oreh">Oreh's</a> group status is closed.""#;
        assert_eq!(event_of(said), None);
    }
}

/// `OTHER_JOINED_GROUP`, `<X> joins <Y's> group.` (`group.rb:507-509`).
mod joins_another {
    use super::{GroupEvent, JOIN_ETANAMIR, event_of, state_after};

    const ZOLETA_JOINS_ETANAMIR: &str = concat!(
        r#"<a exist="-10154507" noun="Zoleta">Zoleta</a> joins "#,
        r#"<a exist="-10488845" noun="Etanamir">Etanamir's</a> group."#,
    );

    #[test]
    fn someone_joining_our_leader_is_a_member() {
        let group = state_after(&[JOIN_ETANAMIR, ZOLETA_JOINS_ETANAMIR]).group;
        assert!(group.contains("-10154507"));
    }

    #[test]
    fn someone_joining_a_stranger_is_not_ours() {
        // `Group.push(added) if Group.include?(leader)` (`group.rb:655-657`).
        assert!(state_after(&[ZOLETA_JOINS_ETANAMIR]).group.is_empty());
    }

    #[test]
    fn the_joiner_is_first_and_the_leader_second() {
        let event = event_of(ZOLETA_JOINS_ETANAMIR);
        let Some(GroupEvent::JoinedOther { member, leader }) = event else {
            panic!("not a join: {event:?}");
        };
        assert_eq!(member.noun, "Zoleta");
        assert_eq!(leader.noun, "Etanamir");
        assert_eq!(leader.text, "Etanamir", "not possessive");
    }

    #[test]
    fn more_words_are_another_line() {
        let longer = concat!(
            r#"<a exist="-10154507" noun="Zoleta">Zoleta</a> joins "#,
            r#"<a exist="-10488845" noun="Etanamir">Etanamir's</a> group, grinning."#,
        );
        assert_eq!(event_of(longer), None);
    }
}

/// The eight `HOLD_*_SECOND` and `_THIRD` (`group.rb:481-505`).
mod hands {
    use super::{JOIN_ETANAMIR, Leader, OPEN, OREH_JOINS, other, state_after};

    const ETANAMIR: &str = r#"<a exist="-10488845" noun="Etanamir">Etanamir</a>"#;
    const SZAN: &str = r#"<a exist="-10974229" noun="Szan">Szan's</a>"#;

    #[test]
    fn someone_taking_your_hand_leads_you_whatever_the_demeanor() {
        // `group.rb:648-651`: pushed, made leader, and the roster doubted.
        for tail in [
            " grabs your hand.",
            " reaches out and holds your hand.",
            " gently takes hold of your hand.",
            " clasps your hand tenderly.",
        ] {
            let state = state_after(&[OPEN, &format!("{ETANAMIR}{tail}")]);
            assert_eq!(
                other(&state).map(|m| m.noun.as_str()),
                Some("Etanamir"),
                "{tail}"
            );
            assert!(state.group.contains("-10488845"), "{tail}");
            assert!(!state.group.checked(), "{tail}");
        }
    }

    #[test]
    fn your_hand_taken_is_their_group_not_yours() {
        // As `adds you to his group` is: whoever was grouped with you before
        // is not named in the group you are in now.
        let state = state_after(&[OREH_JOINS, &format!("{ETANAMIR} grabs your hand.")]);
        assert!(!state.group.contains("-10467645"));
    }

    #[test]
    fn the_name_must_take_the_hand() {
        // Lich's pattern puts the link right before the verb; an emote that
        // ends the same way is someone else's sentence.
        let state = state_after(&[&format!("{ETANAMIR} smiles and grabs your hand.")]);
        assert_eq!(*state.group.leader(), Leader::Unknown);
    }

    #[test]
    fn our_leader_taking_a_hand_adds_its_owner_whatever_the_demeanor() {
        // `group.rb:652-654`: pushed if the taker is in our group.
        for (verb, end) in [
            (" grabs ", " hand."),
            (" reaches out and holds ", " hand."),
            (" gently takes hold of ", " hand."),
            (" clasps ", " hand tenderly."),
        ] {
            let line = format!("{ETANAMIR}{verb}{SZAN}{end}");
            let group = state_after(&[JOIN_ETANAMIR, &line]).group;
            let szan = group.members().iter().find(|m| m.id == "-10974229");
            assert_eq!(szan.map(|m| m.text.as_str()), Some("Szan"), "{verb}");
        }
    }

    #[test]
    fn the_taker_is_the_leader_not_the_held() {
        // Szan takes Etanamir's hand: Szan leads, and Szan is not ours.
        let line = concat!(
            r#"<a exist="-10974229" noun="Szan">Szan</a> grabs "#,
            r#"<a exist="-10488845" noun="Etanamir">Etanamir's</a> hand."#,
        );
        let group = state_after(&[JOIN_ETANAMIR, line]).group;
        assert!(!group.contains("-10974229"));
    }

    #[test]
    fn a_stranger_taking_a_hand_is_not_ours() {
        let line = format!("{ETANAMIR} grabs {SZAN} hand.");
        assert!(state_after(&[&line]).group.is_empty());
    }

    #[test]
    fn more_words_are_another_line() {
        // `^...$`, both ends (`group.rb:496`): the same verb and ending, with
        // words before the taker, is someone else's sentence.
        let line = format!("Laughing, {ETANAMIR} grabs {SZAN} hand.");
        let group = state_after(&[JOIN_ETANAMIR, &line]).group;
        assert!(!group.contains("-10974229"));
    }
}

#[test]
fn a_status_event_carries_which() {
    assert_eq!(
        event_of("Your group status is currently closed."),
        Some(GroupEvent::Status(GroupStatus::Closed))
    );
}
