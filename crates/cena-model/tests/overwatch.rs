//! Creatures that hide, and the room they hid in.

use cena_model::GameState;
use cena_model::state::overwatch::{Sighting, classify};
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

fn line(wire: &str) -> cena_model::ChunkLine {
    let mut parser = Parser::new();
    let mut runs = Vec::new();
    for frame in parser.parse_line(wire) {
        if let cena_protocol::frame::Frame::Text(text) = frame {
            runs.push(text.as_run());
        }
    }
    cena_model::ChunkLine {
        runs: cena_protocol::runs::Runs { runs },
    }
}

/// A bolded creature, as the game sends one. Lich's patterns require exactly
/// this shape: `<pushBold/>`, a positive `exist` id, and `<popBold/>`.
fn creature(id: &str, noun: &str, name: &str, tail: &str) -> String {
    format!("<pushBold/>a <a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/>{tail}")
}

mod hiding {
    use super::{Sighting, classify, creature, line, state_after};

    #[test]
    fn a_creature_slipping_into_hiding_is_noticed() {
        assert_eq!(
            classify(&line(&creature(
                "123456",
                "kobold",
                "grimy kobold",
                " slips into hiding."
            ))),
            Some(Sighting::Hid)
        );
    }

    #[test]
    fn a_hiding_line_that_names_nobody_still_counts() {
        // Several of Lich's fifteen HIDING patterns name no creature at all,
        // which is why `Sighting::Hid` carries nobody: what matters is that
        // SOMETHING is in this room, not which thing.
        for wire in [
            "Something stirs in the shadows.",
            "A faint silvery light flickers from the shadows.",
            "The figure quickly disappears from view.",
        ] {
            assert_eq!(classify(&line(wire)), Some(Sighting::Hid), "{wire}");
        }
    }

    #[test]
    fn the_room_is_remembered() {
        let state = state_after(&[
            "<nav rm='7503251'/>",
            &creature("123456", "kobold", "grimy kobold", " slips into hiding."),
        ]);
        assert_eq!(state.overwatch.room(), Some("7503251"));
        assert!(state.overwatch.hiders_in(Some("7503251")));
    }

    #[test]
    fn another_room_has_no_hiders() {
        // The tracker holds ONE room (`overwatch.rb:19`), so walking away
        // answers false -- and walking back answers false too, until
        // something hides again. That is Lich's behaviour and the honest
        // one: nobody said the creature was still there.
        let state = state_after(&[
            "<nav rm='7503251'/>",
            &creature("123456", "kobold", "grimy kobold", " slips into hiding."),
            "<nav rm='7086'/>",
        ]);
        assert_eq!(state.overwatch.room(), Some("7503251"), "still recorded");
        assert!(!state.overwatch.hiders_in(Some("7086")), "but not here");
    }

    #[test]
    fn nothing_seen_means_no_hiders_anywhere() {
        let state = state_after(&["<nav rm='7503251'/>"]);
        assert!(!state.overwatch.hiders_in(Some("7503251")));
        assert!(!state.overwatch.hiders_in(None), "nor in an unknown room");
    }
}

mod revealing {
    use super::{Sighting, classify, creature, line, state_after};

    #[test]
    fn a_revealed_creature_carries_its_id_and_noun() {
        let found = classify(&line(&creature(
            "123456",
            "kobold",
            "grimy kobold",
            " comes out of hiding.",
        )));
        assert_eq!(
            found,
            Some(Sighting::Revealed {
                id: "123456".to_owned(),
                noun: "kobold".to_owned(),
                name: "grimy kobold".to_owned(),
                struck: false,
            })
        );
    }

    #[test]
    fn a_strike_from_hiding_is_marked_as_one() {
        // A creature that strikes from hiding may re-hide immediately
        // (`overwatch.rb:109`), so the flag is what a behavior reacts to.
        //
        // NOT tested here: that strikes are checked BEFORE reveals. VERIFIED
        // that no STRUCK phrase contains any REVEALED phrase, so no line can
        // reach both and the order is unreachable. A mutation swapping them
        // passes, and writing a test to kill it would mean inventing input
        // the game does not send.
        let found = classify(&line(&creature(
            "123456",
            "kobold",
            "grimy kobold",
            " leaps from hiding to attack!",
        )));
        assert!(
            matches!(found, Some(Sighting::Revealed { struck: true, .. })),
            "{found:?}"
        );
    }

    #[test]
    fn a_revealed_creature_goes_back_on_the_roster() {
        // **What the port is FOR.** Lich calls `GameObj.new_npc` and unshifts
        // the id onto `current_target_ids` (`overwatch.rb:111-120`); here
        // `Creatures::register` does both, and it is what makes the creature
        // targetable again.
        let state = state_after(&[
            "<nav rm='7503251'/>",
            &creature("123456", "kobold", "grimy kobold", " comes out of hiding."),
        ]);
        assert_eq!(
            state.creatures().get(123_456).map(|c| c.noun.as_deref()),
            Some(Some("kobold")),
            "registered by its noun"
        );
        assert!(
            state.creatures().in_room().any(|c| c.id == 123_456),
            "and present in the room"
        );
    }

    #[test]
    fn every_reveal_family_is_read() {
        // Lich needs one constant per message because each embeds the markup.
        // Here it is one phrase per message, so the 20 REVEALED_* and 6
        // SILENT_* families collapse -- but each still has to be READ, and a
        // family dropped in the collapse is a creature that stays invisible.
        for tail in [
            " is revealed from hiding.",
            " is forced from hiding!",
            " comes out of hiding.",
            ", who was hidden!",
            " explodes from the shadows!",
        ] {
            let found = classify(&line(&creature("1", "kobold", "a kobold", tail)));
            assert!(
                matches!(found, Some(Sighting::Revealed { .. })),
                "{tail}: {found:?}"
            );
        }
    }
}

mod the_creature_test {
    use super::{Sighting, classify, creature, line, state_after};

    #[test]
    fn a_player_coming_out_of_hiding_is_not_a_creature() {
        // **VERIFIED AGAINST THE CORPUS**, and this is the one narrowing the
        // port inherits deliberately rather than by accident.
        //
        // Lich's patterns require `<pushBold/>` AND a positive `exist` id.
        // Running its own REVEALED_COMES_OUT and SILENT_LEAP_ATTACK against
        // the 208 live logs: all 41 matching lines are PLAYERS -- negative
        // ids, no bold -- and not one matches.
        //
        // That is correct. Overwatch re-targets hidden CREATURES for a
        // hunting behavior; a player stepping out of hiding is not a hunt
        // target, and is already tracked by `room players`' own status.
        for wire in [
            r#"<a exist="-10319971" noun="Riend">Riend</a> comes out of hiding."#,
            r#"<a exist="-11182674" noun="Clairebie">Clairebie</a> leaps from hiding to attack!"#,
        ] {
            assert_eq!(classify(&line(wire)), None, "{wire}");
        }
    }

    #[test]
    fn a_player_reveal_does_not_register_a_creature() {
        let state = state_after(&[
            "<nav rm='7503251'/>",
            r#"<a exist="-10319971" noun="Riend">Riend</a> comes out of hiding."#,
        ]);
        assert!(
            state.creatures().is_empty(),
            "{:?}",
            state.creatures().all().collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_unbolded_creature_line_is_not_read() {
        // Bold is the test, not the id's sign. A line with a positive id and
        // no bold is not what the game sends for a creature, and reading it
        // would be inventing a shape.
        let wire = r#"<a exist="123456" noun="kobold">grimy kobold</a> comes out of hiding."#;
        assert_eq!(classify(&line(wire)), None);
    }

    #[test]
    fn a_bolded_creature_is_read() {
        // The guard for the test above: it must fail for the RIGHT reason.
        assert!(matches!(
            classify(&line(&creature(
                "1",
                "kobold",
                "a kobold",
                " comes out of hiding."
            ))),
            Some(Sighting::Revealed { .. })
        ));
    }
}

#[test]
fn an_ordinary_line_is_not_a_sighting() {
    assert_eq!(classify(&line("You see a large rock.")), None);
    assert_eq!(
        classify(&line("The kobold attacks you.")),
        None,
        "no hiding prose"
    );
}

#[test]
fn the_hiding_room_is_forgotten_on_a_reconnect() {
    // **Cleared, with the spell cooldowns and unlike the messages.** It is a
    // claim about NOW -- "something is hiding in room 7503251" -- and a
    // creature does not wait there while we are logged off.
    //
    // Lich reaches the same answer from the other side: `Overwatch.clear`
    // exists so the tracker can be reset when it may be stale.
    let mut state = state_after(&[
        "<nav rm='7503251'/>",
        &creature("123456", "kobold", "grimy kobold", " slips into hiding."),
    ]);
    assert!(state.overwatch.room().is_some(), "guard: known first");
    state.invalidate_for_reconnect();
    assert_eq!(state.overwatch.room(), None);
}

mod vanishing {
    use super::state_after;

    /// A `room objs` body listing bolded creatures.
    ///
    /// **Copied from real wire**, `2026-09-01`: the `<crtrStatus>` is
    /// self-closing, carries `exist=`, and sits INSIDE the component just
    /// before the link it vouches for. A first draft of this helper invented
    /// a `<crtrStatus id=...>hostile</crtrStatus>` after the component, and
    /// the creature never registered -- caught by a `guard:` assertion.
    ///
    /// ```text
    /// room objs'>  You also see<crtrStatus exist="203584689" flying="1"/><b>
    ///   <pushBold/>a <a exist="203584689" noun="snow-owl">pure white snow-owl</a><popBold/></b>
    /// ```
    fn room_with(creatures: &[(&str, &str, &str)]) -> String {
        let mut listed = String::new();
        for (id, noun, name) in creatures {
            use std::fmt::Write as _;
            let _ = write!(
                listed,
                "<crtrStatus exist=\"{id}\" hostile=\"1\"/><b>                  <pushBold/>a <a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/></b>"
            );
        }
        format!("<component id='room objs'>  You also see{listed}</component>")
    }

    #[test]
    fn gone_does_not_mean_hidden() {
        // **The correction that matters** (author, 2026-09-20):
        //
        //   "but gone just means not in the room, doesn't mean hid. We have
        //    creature arrival and leaving messaging though, which would get
        //    tagged somewhere along the way and get pushed to the creature."
        //
        // An earlier version of this test asserted the OPPOSITE -- that a
        // creature vanishing without dying is hiding -- and passed green,
        // because the code had been written to match the same mistake. The
        // author's inference has THREE conditions and I implemented two:
        // gone, not dead, and **not seen to leave**.
        //
        // The third needs a flee classifier. `creature_messages.tsv` already
        // holds 1,067 flee lines, but nothing matches against them and 802
        // carry `{direction}`/`{pronoun}` placeholders, so that is a port of
        // its own.
        //
        // Until then the tracker RECORDS NOTHING here: a creature that walked
        // out satisfies "not dead" exactly as one that hid does, and claiming
        // it is hiding would have a behavior search an empty room.
        let mut state = state_after(&[
            "<nav rm='7503251'/>",
            &room_with(&[("123456", "kobold", "grimy kobold")]),
        ]);
        assert!(
            state.creatures().in_room().any(|c| c.id == 123_456),
            "guard: in the room first"
        );

        let mut parser = cena_protocol::Parser::new();
        for frame in parser.parse_line(&room_with(&[])) {
            state.apply(&frame);
        }
        assert!(
            !state.overwatch.hiders_in(Some("7503251")),
            "gone is not hidden -- it may simply have walked away"
        );
    }

    #[test]
    fn the_hiding_prose_still_records_a_hider() {
        // What DOES record one: the game saying so. That signal is
        // unambiguous, which is why it is the only one acted on today.
        let state = state_after(&[
            "<nav rm='7503251'/>",
            &super::creature("123456", "kobold", "grimy kobold", " slips into hiding."),
        ]);
        assert!(state.overwatch.hiders_in(Some("7503251")));
    }

    #[test]
    fn a_creature_that_stays_is_not_hiding() {
        let mut state = state_after(&[
            "<nav rm='7503251'/>",
            &room_with(&[("123456", "kobold", "grimy kobold")]),
        ]);
        let mut parser = cena_protocol::Parser::new();
        for frame in parser.parse_line(&room_with(&[("123456", "kobold", "grimy kobold")])) {
            state.apply(&frame);
        }
        assert!(!state.overwatch.hiders_in(Some("7503251")));
    }
}
