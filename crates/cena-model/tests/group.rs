//! Who is grouped with you.
//!
//! Every wire line here is from Lich's own `@example` comments
//! (`gemstone/group.rb:418-470`), which carry real captures rather than
//! invented ones.

use cena_model::GameState;
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

/// `group.rb:420`'s example.
const OREH_JOINS: &str = r#"<a exist="-10467645" noun="Oreh">Oreh</a> joins your group."#;
const OREH_LEAVES: &str = r#"<a exist="-10467645" noun="Oreh">Oreh</a> leaves your group."#;

#[test]
fn someone_joining_is_a_member() {
    let group = state_after(&[OREH_JOINS]).group;
    assert_eq!(group.members().len(), 1);
    assert_eq!(group.members()[0].id, "-10467645", "the id is the identity");
    assert_eq!(group.members()[0].noun, "Oreh");
}

#[test]
fn someone_leaving_is_removed() {
    let group = state_after(&[OREH_JOINS, OREH_LEAVES]).group;
    assert!(group.is_empty());
}

#[test]
fn joining_twice_is_one_member() {
    // `AlreadyMember` is a push in Lich too (`group.rb:642`): the game is
    // stating membership, which is worth believing even though the command
    // failed.
    let group = state_after(&[
        OREH_JOINS,
        r#"But <a exist="-10467645" noun="Oreh">Oreh</a> is already a member of your group!"#,
    ])
    .group;
    assert_eq!(group.members().len(), 1);
}

#[test]
fn you_adding_and_removing_are_read() {
    let group =
        state_after(&[r#"You add <a exist="-10467645" noun="Oreh">Oreh</a> to your group."#]).group;
    assert_eq!(group.members().len(), 1);

    let group = state_after(&[
        r#"You add <a exist="-10467645" noun="Oreh">Oreh</a> to your group."#,
        r#"You remove <a exist="-10467645" noun="Oreh">Oreh</a> from the group."#,
    ])
    .group;
    assert!(group.is_empty());
}

/// `group.rb:469`'s example, and the other three demeanors of the same line.
#[test]
fn taking_a_hand_adds_them_whatever_the_demeanor() {
    const DICATE: &str = r#"<a exist="-10070682" noun="Dicate">Dicate's</a>"#;
    for (start, end) in [
        ("You grab ", " hand."),
        ("You reach out and hold ", " hand."),
        ("You gently take hold of ", " hand."),
        ("You clasp ", " hand tenderly."),
    ] {
        let group = state_after(&[&format!("{start}{DICATE}{end}")]).group;
        assert_eq!(group.members().len(), 1, "{start}");
        assert_eq!(group.members()[0].id, "-10070682");
        assert_eq!(
            group.members()[0].text,
            "Dicate",
            "a name, not a possessive"
        );
    }
    // Someone else's hand being taken is not yours: two people, not one.
    let watched =
        r#"<a exist="-1" noun="Oreh">Oreh</a> grabs <a exist="-2" noun="Szan">Szan's</a> hand."#;
    assert!(state_after(&[watched]).group.is_empty());
}

#[test]
fn disbanding_empties_the_group() {
    let group = state_after(&[OREH_JOINS, "You disband your group."]).group;
    assert!(group.is_empty());
    assert_eq!(group.leader(), None);
}

mod leadership {
    use super::{OREH_JOINS, state_after};

    /// `group.rb:438`'s example.
    const ETANAMIR_MAKES_YOU_LEADER: &str = concat!(
        r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> designates you "#,
        "as the new leader of the group.",
    );

    #[test]
    fn being_made_leader_leaves_the_leader_unknown() {
        // "You" is not a link, so there is no id to store. `None` is honest:
        // this model cannot answer "is it me" without knowing its own id, and
        // setting the leader to the person who GAVE it away would be wrong.
        let group = state_after(&[OREH_JOINS, ETANAMIR_MAKES_YOU_LEADER]).group;
        assert_eq!(group.leader(), None);
        assert_eq!(group.members().len(), 1, "membership is untouched");
    }

    #[test]
    fn a_swap_names_the_new_leader_not_the_old() {
        // `group.rb:442`'s example -- the pattern whose three duplicate
        // capture-group names are harmless because Lich reads the LINKS, not
        // the captures (`group.rb:608`).
        let group = state_after(&[concat!(
            r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> designates "#,
            r#"<a exist="-10778599" noun="Ondreian">Ondreian</a> as the new leader of the group."#,
        )])
        .group;
        assert_eq!(
            group.leader().map(|m| m.id.as_str()),
            Some("-10778599"),
            "the second link, not the first"
        );
    }

    #[test]
    fn giving_leadership_away_names_who_has_it() {
        let group = state_after(&[concat!(
            r#"You designate <a exist="-10778599" noun="Ondreian">Ondreian</a> "#,
            "as the new leader of the group.",
        )])
        .group;
        assert_eq!(group.leader().map(|m| m.noun.as_str()), Some("Ondreian"));
    }
}

mod joining_someone_elses {
    use super::state_after;

    #[test]
    fn being_added_replaces_whatever_was_held() {
        // You are in THEIR group now, and its other members are unknown until
        // the game names them. Keeping the old roster would show members of a
        // group you left.
        let group = state_after(&[
            r#"<a exist="-1" noun="Oreh">Oreh</a> joins your group."#,
            concat!(
                r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> adds you to "#,
                r#"<a exist="-10488845" noun="Etanamir">his</a> group."#,
            ),
        ])
        .group;
        assert_eq!(group.members().len(), 1, "Oreh's group is gone");
        assert_eq!(group.leader().map(|m| m.noun.as_str()), Some("Etanamir"));
    }

    #[test]
    fn you_joining_someone_names_them_leader() {
        let group =
            state_after(&[r#"You join <a exist="-10488845" noun="Etanamir">Etanamir</a>."#]).group;
        assert_eq!(group.leader().map(|m| m.noun.as_str()), Some("Etanamir"));
        assert!(group.contains("-10488845"));
    }

    #[test]
    fn the_possessive_pronoun_is_a_link_and_is_not_a_third_member() {
        // **The distinction the link COUNT makes.** `<X> adds you to <his>
        // group.` is two links and `<X> adds <Y> to <his> group.` is three,
        // and the prose either side of the middle name is identical -- so the
        // count is what tells them apart.
        //
        // In Etanamir's group first: Lich adds only when the leader is ours
        // (`group.rb:636-638`), which the next test pins.
        let group = state_after(&[
            ETANAMIR_JOINED,
            concat!(
                r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> adds "#,
                r#"<a exist="-10974229" noun="Szan">Szan</a> to "#,
                r#"<a exist="-10488845" noun="Etanamir">his</a> group."#,
            ),
        ])
        .group;
        assert_eq!(
            group.members().len(),
            2,
            "Etanamir and Szan, not a third member called `his`"
        );
        assert!(group.contains("-10974229"));
    }

    /// You join Etanamir's group: he is its leader and its one known member.
    const ETANAMIR_JOINED: &str = r#"You join <a exist="-10488845" noun="Etanamir">Etanamir</a>."#;

    #[test]
    fn a_stranger_adding_someone_to_their_own_group_is_not_ours() {
        // Review finding. `LEADER_ADDED_MEMBER` is `Group.push(added) if
        // Group.include?(leader)` (`group.rb:636-638`): the line reaches
        // everyone in the room, so a leader we are not grouped with adding
        // someone is THEIR news. Pushing unconditionally put strangers on
        // our roster.
        let group = state_after(&[concat!(
            r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> adds "#,
            r#"<a exist="-10974229" noun="Szan">Szan</a> to "#,
            r#"<a exist="-10488845" noun="Etanamir">his</a> group."#,
        )])
        .group;
        assert!(group.is_empty(), "neither of them is in our group");
    }

    #[test]
    fn a_leader_removing_someone_removes_them() {
        let group = state_after(&[
            ETANAMIR_JOINED,
            concat!(
                r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> adds "#,
                r#"<a exist="-10974229" noun="Szan">Szan</a> to "#,
                r#"<a exist="-10488845" noun="Etanamir">his</a> group."#,
            ),
            concat!(
                r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> removes "#,
                r#"<a exist="-10974229" noun="Szan">Szan</a> from the group."#,
            ),
        ])
        .group;
        assert!(!group.contains("-10974229"));
    }
}

#[test]
fn an_unrelated_line_with_a_link_is_not_a_group_event() {
    let group = state_after(&[r#"<a exist="-10467645" noun="Oreh">Oreh</a> waves at you."#]).group;
    assert!(group.is_empty());
}

#[test]
fn the_group_is_forgotten_on_a_reconnect() {
    // Its members are other people, each held by an `exist` id a behavior can
    // target, and they leave while we are gone -- the same reason
    // `room.players` is cleared. Lich reaches it from the other side:
    // `Group.check` CLEARS and re-runs the command (`group.rb:152-157`).
    let mut state = state_after(&[OREH_JOINS]);
    assert_eq!(state.group.members().len(), 1, "guard: known first");
    state.invalidate_for_reconnect();
    assert!(state.group.is_empty());
}

mod emptied {
    //! Review finding: nothing but `disband` or a reconnect ever emptied the
    //! group. Lich clears it on three more signals (`group.rb:603-605`,
    //! `:617-619`) and replaces it on the `group` command's roster (`:644-645`).

    use super::{OREH_JOINS, state_after};

    #[test]
    fn the_joined_indicator_going_dark_empties_the_group() {
        // `GROUP_EMPTIED`: `<indicator id='IconJOINED' visible='n'/>`.
        let group = state_after(&[OREH_JOINS, r#"<indicator id="IconJOINED" visible="n"/>"#]).group;
        assert!(group.is_empty());
        assert_eq!(group.leader(), None, "the leader is you again");
    }

    #[test]
    fn the_indicator_lighting_up_changes_nothing() {
        let group = state_after(&[OREH_JOINS, r#"<indicator id="IconJOINED" visible="y"/>"#]).group;
        assert_eq!(group.members().len(), 1);
    }

    #[test]
    fn not_being_in_a_group_empties_it() {
        // `NO_GROUP`, `^You are not currently in a group` (`group.rb:512`).
        let group = state_after(&[OREH_JOINS, "You are not currently in a group."]).group;
        assert!(group.is_empty());
    }

    #[test]
    fn a_player_saying_it_is_not_the_game_saying_it() {
        // Anchored, as Lich anchors it.
        let group = state_after(&[
            OREH_JOINS,
            r#"<a exist="-5" noun="Bob">Bob</a> says, "You are not currently in a group.""#,
        ])
        .group;
        assert_eq!(group.members().len(), 1);
    }

    #[test]
    fn the_group_roster_replaces_the_members() {
        // `MEMBER` (`group.rb:521`) -> `Group.refresh(*people)`: a complete
        // list, so Oreh -- held from before and not named -- has gone.
        let group = state_after(&[
            OREH_JOINS,
            concat!(
                r#"You are grouped with <a exist="-10488845" noun="Etanamir">Etanamir</a>, "#,
                r#"<a exist="-10974229" noun="Szan">Szan</a>."#,
            ),
        ])
        .group;
        let nouns: Vec<&str> = group.members().iter().map(|m| m.noun.as_str()).collect();
        assert_eq!(nouns, ["Etanamir", "Szan"]);
        assert_eq!(
            group.leader().map(|m| m.noun.as_str()),
            Some("Etanamir"),
            "`grouped with` names the leader first (`group.rb:610-614`)"
        );
    }

    #[test]
    fn leading_the_roster_makes_the_leader_you() {
        let group = state_after(&[concat!(
            r#"You are leading <a exist="-10467645" noun="Oreh">Oreh</a>, "#,
            r#"<a exist="-10974229" noun="Szan">Szan</a>."#,
        )])
        .group;
        assert_eq!(group.members().len(), 2);
        assert_eq!(group.leader(), None, "you, which is not a link");
    }
}
