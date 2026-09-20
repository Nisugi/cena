//! `<roommeta>` and `<objectives>` reach the model typed.
//!
//! Both were parsed into untyped attribute bags that **no consumer read**:
//! `Frame::RoomMeta { attrs }` and `Frame::ObjectivesUpdate { entries:
//! Vec<Attrs> }`, with zero `Frame::` arms in `cena-model`. The game was
//! handing us the room's environment and the character's whole quest list as
//! anonymous string pairs.
//!
//! Worse for objectives: `<objectives>` was not a paired tag, so each
//! `<objective>` tokenized separately and landed in `Frame::WindowHints` --
//! the *window placement* bag -- while the action that says what to do with
//! them arrived on a frame of its own.

use cena_model::GameState;
use cena_protocol::{ObjectivesAction, Parser};

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    state
}

/// A real `<roommeta>`, verbatim from the author's September logs. All eight
/// attributes, every time -- 2,043 occurrences, 2,043 of each.
const META: &str = r#"<roommeta weather="0" bonfire="1" inside="0" water="0" sanctuary="1" realm="15" climate="2" terrain="13"/>"#;

mod room_meta {
    use super::*;

    #[test]
    fn every_code_reaches_the_room() {
        let meta = state_after(&[META])
            .room
            .meta
            .expect("the room's environment");
        assert_eq!(
            (meta.weather, meta.bonfire, meta.inside, meta.water),
            (Some(0), Some(1), Some(0), Some(0))
        );
        assert_eq!(
            (meta.sanctuary, meta.realm, meta.climate, meta.terrain),
            (Some(1), Some(15), Some(2), Some(13))
        );
    }

    #[test]
    fn sanctuary_is_answerable_without_knowing_a_string_key() {
        // The one a hunting behavior asks first. It was
        // `attrs.iter().find(|(k, _)| k == "sanctuary")` against an untyped
        // bag in a frame nothing consumed.
        let state = state_after(&[META]);
        assert_eq!(state.room.meta.and_then(|m| m.sanctuary), Some(1));
    }

    #[test]
    fn unstated_is_none_rather_than_zero() {
        // §5.2: `Unknown` is not a fabricated zero. A tag that omits an
        // attribute has not said it is 0 -- and 0 is a meaningful value
        // here, since `sanctuary="0"` means "not a sanctuary".
        let meta = state_after(&[r#"<roommeta sanctuary="0"/>"#])
            .room
            .meta
            .expect("the tag arrived");
        assert_eq!(meta.sanctuary, Some(0), "stated, and false");
        assert_eq!(meta.terrain, None, "not stated at all");
    }

    #[test]
    fn a_room_that_never_stated_its_environment_says_so() {
        assert_eq!(GameState::default().room.meta, None);
    }

    #[test]
    fn the_environment_survives_a_reconnect() {
        // A room does not stop being a sanctuary because the socket dropped,
        // and the character was out of the world in between. Contrast
        // `room.creatures`, which IS cleared: other people keep moving.
        let mut state = state_after(&[META]);
        assert!(state.room.meta.is_some(), "guard: known first");
        state.invalidate_for_reconnect();
        assert_eq!(state.room.meta.and_then(|m| m.sanctuary), Some(1));
    }
}

mod objectives {
    use super::*;

    /// A real update, trimmed to three rows from the author's logs. The
    /// shapes that matter are all here: a bare `BOUNTY` with only an id and
    /// a type, and full `QUEST` rows with and without a location.
    pub(super) const REFRESH: &str = concat!(
        r"<objectives action='full-refresh'>",
        r"<objective id='17755' type='BOUNTY'/>",
        r#"<objective id='24352' type='QUEST' state='available' name="Into the Rift" "#,
        r#"description="A mysterious adventurer requests assistance." location="The Rift" cadence='monthly'>"#,
        r"</objective>",
        r#"<objective id='24330' type='QUEST' state='available' name="Shadow's Descent" "#,
        r#"description="Not over."></objective>"#,
        r"</objectives>",
    );

    #[test]
    fn a_full_refresh_is_the_whole_list() {
        let state = state_after(&[REFRESH]);
        assert_eq!(state.objectives.len(), 3);
        assert!(state.objectives.is_known());
        let quest = state.objectives.get("24352").expect("Into the Rift");
        assert_eq!(quest.kind, "QUEST");
        assert_eq!(quest.name.as_deref(), Some("Into the Rift"));
        assert_eq!(quest.location.as_deref(), Some("The Rift"));
        assert_eq!(quest.cadence.as_deref(), Some("monthly"));
    }

    #[test]
    fn a_bounty_row_carries_only_what_it_states() {
        let state = state_after(&[REFRESH]);
        let bounty = state.objectives.get("17755").expect("the bounty");
        assert_eq!(bounty.kind, "BOUNTY");
        assert_eq!(
            (bounty.name.as_deref(), bounty.state.as_deref()),
            (None, None),
            "an absent attribute is None, not an empty string"
        );
        assert_eq!(state.objectives.of_kind("QUEST").count(), 2);
    }

    #[test]
    fn a_refresh_replaces_rather_than_appends() {
        // The list, entire: anything absent is finished or abandoned. An
        // implementation that merged would never lose a completed quest.
        let state = state_after(&[
            REFRESH,
            r"<objectives action='full-refresh'><objective id='99' type='QUEST'/></objectives>",
        ]);
        assert_eq!(state.objectives.len(), 1);
        assert!(state.objectives.get("24352").is_none());
    }

    #[test]
    fn a_patch_updates_one_row_and_leaves_the_rest() {
        let state = state_after(&[
            REFRESH,
            r"<objectives action='patch-objective'><objective id='24330' type='QUEST' state='offered'/></objectives>",
        ]);
        assert_eq!(state.objectives.len(), 3, "a patch is not a snapshot");
        assert_eq!(
            state
                .objectives
                .get("24330")
                .and_then(|o| o.state.as_deref()),
            Some("offered")
        );
    }

    #[test]
    fn a_delete_removes_only_what_it_names() {
        let state = state_after(&[
            REFRESH,
            r"<objectives action='delete-objective'><objective id='17755' type='BOUNTY'/></objectives>",
        ]);
        assert_eq!(state.objectives.len(), 2);
        assert!(state.objectives.get("17755").is_none());
        assert!(state.objectives.get("24352").is_some());
    }

    #[test]
    fn an_unknown_action_changes_nothing() {
        // Guessing could delete a list the server meant to extend. The frame
        // still reached every observer, which is how the change stays
        // visible (Rule 2.2).
        assert_eq!(
            ObjectivesAction::parse("reticulate"),
            ObjectivesAction::Other("unrecognised")
        );
        let state = state_after(&[
            REFRESH,
            r"<objectives action='reticulate'><objective id='1' type='QUEST'/></objectives>",
        ]);
        assert_eq!(state.objectives.len(), 3);
        assert!(state.objectives.get("1").is_none());
    }

    #[test]
    fn an_empty_list_the_game_stated_is_not_an_unasked_one() {
        // §5.2 again, in the shape `Effects::active_in` needed: "the game
        // said you have no quests" and "nobody has asked" are different.
        let unasked = GameState::default();
        assert!(!unasked.objectives.is_known() && unasked.objectives.is_empty());

        let stated = state_after(&[r"<objectives action='full-refresh'></objectives>"]);
        assert!(stated.objectives.is_known() && stated.objectives.is_empty());
    }

    #[test]
    fn the_list_survives_a_reconnect() {
        // A logged-off character completes no quests, and the list is not in
        // the login burst -- clearing it would leave `is_known()` false with
        // no way back until something changed.
        let mut state = state_after(&[REFRESH]);
        state.invalidate_for_reconnect();
        assert_eq!(state.objectives.len(), 3);
        assert!(state.objectives.is_known());
    }
}

/// The two paths that do NOT go through `<objectives>`'s body. Both were
/// unasserted, and two mutants lived in them: removing `"objectives"` from
/// the paired-tag list, and hard-coding the action to `FullRefresh`.
mod without_an_envelope {
    use super::*;

    #[test]
    fn a_stray_objective_patches_rather_than_replacing() {
        // The wire is not known to send this. Treating it as a refresh
        // would let one stray row erase the whole list, which is the
        // expensive direction to be wrong in.
        let state = state_after(&[
            super::objectives::REFRESH,
            "<objective id='9' type='QUEST'/>",
        ]);
        assert_eq!(state.objectives.len(), 4, "added, not replaced");
        assert!(state.objectives.get("24352").is_some(), "the list survived");
        assert!(state.objectives.get("9").is_some());
    }

    #[test]
    fn an_envelope_with_no_body_still_carries_its_action() {
        // This is what fails when `"objectives"` leaves the paired-tag list:
        // the envelope parses alone, so the action must still be read from
        // it rather than assumed.
        let state = state_after(&[
            super::objectives::REFRESH,
            "<objectives action='delete-objective'/>",
        ]);
        assert_eq!(
            state.objectives.len(),
            3,
            "a delete naming nothing deletes nothing -- and must not be \
             mistaken for a refresh, which would empty the list"
        );
    }

    #[test]
    fn an_empty_refresh_envelope_does_empty_the_list() {
        // The other half of the pair above: same shape, opposite action,
        // opposite outcome. Together they pin that the action is READ.
        let state = state_after(&[
            super::objectives::REFRESH,
            "<objectives action='full-refresh'/>",
        ]);
        assert!(state.objectives.is_empty() && state.objectives.is_known());
    }
}
