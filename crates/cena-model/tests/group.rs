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
        let group = state_after(&[concat!(
            r#"<a exist="-10488845" noun="Etanamir">Etanamir</a> adds "#,
            r#"<a exist="-10974229" noun="Szan">Szan</a> to "#,
            r#"<a exist="-10488845" noun="Etanamir">his</a> group."#,
        )])
        .group;
        assert_eq!(group.members().len(), 1, "Szan, not Szan and `his`");
        assert!(group.contains("-10974229"));
    }

    #[test]
    fn a_leader_removing_someone_removes_them() {
        let group = state_after(&[
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
