//! What the injury window's mode hides, the model keeps; and the nerves'
//! rank, which the window gives without saying wound or scar (`plan/55`
//! §1a, step 0).
//!
//! The wire shapes are the author's captures
//! (`E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi`, 2026-09): the radios arrive in
//! the window's `openDialog`, and a hand wounded and scarred shows `Injury1`
//! under Both and `Scar1` under Scars.

use cena_model::{GameState, Injury};
use cena_protocol::Parser;

/// The injury window opened with its radio on `mode`, as captured.
fn window(mode: &str) -> String {
    let on = |id: &str| if id == mode { "1" } else { "0" };
    format!(
        "<openDialog type=\"dynamic\" id=\"injuries\" title=\"Injuries\" target=\"injuries\" \
         location=\"right\" height=\"180\" width=\"190\" resident=\"true\"><dialogData id=\"injuries\">\
         <radio id=\"injrRad\" value=\"{}\" text=\"Injuries\" cmd=\"_injury 0\" group=\"injureMode\"/>\
         <radio id=\"scarRad\" value=\"{}\" text=\"Scars\" cmd=\"_injury 1\" group=\"injureMode\"/>\
         <radio id=\"bothRad\" value=\"{}\" text=\"Both\" cmd=\"_injury 2\" group=\"injureMode\"/>\
         </dialogData></openDialog>\n",
        on("injrRad"),
        on("scarRad"),
        on("bothRad"),
    )
}

/// One injury image, in its dialog.
fn image(part: &str, name: &str) -> String {
    format!("<dialogData id='injuries'><image id='{part}' name='{name}'/></dialogData>\n")
}

fn feed(parser: &mut Parser, state: &mut GameState, bytes: &str) {
    for frame in parser.push_bytes(bytes.as_bytes()) {
        state.apply(&frame);
    }
}

fn injury(state: &GameState, part: &str) -> Injury {
    state
        .character
        .injuries
        .get(part)
        .copied()
        .unwrap_or_default()
}

#[test]
fn switching_to_scars_and_back_keeps_both_tracks() {
    let (mut parser, mut state) = (Parser::new(), GameState::default());
    feed(&mut parser, &mut state, &window("bothRad"));
    // Both: the wound covers the scar. Scar learned earlier.
    feed(&mut parser, &mut state, &image("leftHand", "Scar1"));
    feed(&mut parser, &mut state, &image("leftHand", "Injury1"));
    assert_eq!(injury(&state, "leftHand"), Injury { wound: 1, scar: 1 });

    feed(&mut parser, &mut state, &window("scarRad"));
    feed(&mut parser, &mut state, &image("leftHand", "Scar1"));
    assert_eq!(
        injury(&state, "leftHand"),
        Injury { wound: 1, scar: 1 },
        "showing scars only, a scar says nothing about the wound"
    );

    feed(&mut parser, &mut state, &window("bothRad"));
    feed(&mut parser, &mut state, &image("leftHand", "Injury1"));
    assert_eq!(injury(&state, "leftHand"), Injury { wound: 1, scar: 1 });
}

#[test]
fn whole_clears_only_what_the_mode_shows() {
    let (mut parser, mut state) = (Parser::new(), GameState::default());
    feed(&mut parser, &mut state, &window("bothRad"));
    feed(&mut parser, &mut state, &image("rightArm", "Injury1"));
    feed(&mut parser, &mut state, &image("head", "Scar2"));

    // Captured: a wounded arm shows whole under Scars.
    feed(&mut parser, &mut state, &window("scarRad"));
    feed(&mut parser, &mut state, &image("rightArm", "rightArm"));
    assert_eq!(injury(&state, "rightArm"), Injury { wound: 1, scar: 0 });

    // Under Wounds, a scarred head shows whole.
    feed(&mut parser, &mut state, &window("injrRad"));
    feed(&mut parser, &mut state, &image("head", "head"));
    assert_eq!(injury(&state, "head"), Injury { wound: 0, scar: 2 });

    // Shown both ways, whole is whole.
    feed(&mut parser, &mut state, &window("bothRad"));
    feed(&mut parser, &mut state, &image("head", "head"));
    assert!(!state.character.injuries.contains_key("head"));
}

/// A chunk: its lines, then its prompt, where the nerves are read.
fn chunk(parser: &mut Parser, state: &mut GameState, body: &str) {
    feed(
        parser,
        state,
        &format!("{body}<prompt time=\"1\">&gt;</prompt>\n"),
    );
}

fn nsys(state: &GameState) -> Injury {
    injury(state, "nsys")
}

/// The author's sequence (`nerves.rs`): damage makes a wound, each herb a
/// step down, the scar showing once the wound is gone; nothing asked.
#[test]
fn a_nerve_wound_healed_by_herbs_is_worked_out_step_by_step() {
    let (mut parser, mut state) = (Parser::new(), GameState::default());
    let bite = "You take a bite of your wolifrew lichen.\n";
    chunk(&mut parser, &mut state, &image("nsys", "nsys"));
    chunk(&mut parser, &mut state, &image("nsys", "Nsys2"));
    assert_eq!(
        nsys(&state),
        Injury { wound: 2, scar: 0 },
        "damage: a wound"
    );
    for (rank, wound, scar) in [(1, 1, 2), (2, 0, 2), (1, 0, 1)] {
        let shown = image("nsys", &format!("Nsys{rank}"));
        chunk(&mut parser, &mut state, &format!("{bite}{shown}"));
        assert_eq!(nsys(&state), Injury { wound, scar }, "Nsys{rank}");
        assert!(state.character.nerves.settled);
    }
    let whole = image("nsys", "Nsys0");
    chunk(&mut parser, &mut state, &format!("{bite}{whole}"));
    assert!(!state.character.injuries.contains_key("nsys"), "whole");
    assert!(
        !state.character.take_nerve_question(),
        "worked out, never asked"
    );
}

/// The first rank after logging in may be an old scar: shown as a wound,
/// asked once, and settled by `health`.
#[test]
fn a_first_sighting_asks_and_health_settles_it() {
    let (mut parser, mut state) = (Parser::new(), GameState::default());
    chunk(&mut parser, &mut state, &image("nsys", "Nsys2"));
    assert_eq!(nsys(&state), Injury { wound: 2, scar: 0 });
    assert!(!state.character.nerves.settled);
    assert!(state.character.take_nerve_question(), "confused: asks");
    assert!(!state.character.take_nerve_question(), "once");

    chunk(&mut parser, &mut state, &image("nsys", "Nsys2"));
    assert!(
        !state.character.take_nerve_question(),
        "the same rank again asks nothing"
    );

    let report = "<output class=\"mono\"/>\nYou have constant muscle spasms.\n\n     \
                  Maximum Health Points:   158       193\n<output class=\"\"/>\n";
    chunk(&mut parser, &mut state, report);
    assert_eq!(nsys(&state), Injury { wound: 0, scar: 2 });
    assert!(state.character.nerves.settled);
}

/// A fall with no herb is not worked out: an empath or a spell may have made
/// it, and whether it left a scar is not known.
#[test]
fn a_fall_with_no_herb_asks() {
    let (mut parser, mut state) = (Parser::new(), GameState::default());
    chunk(&mut parser, &mut state, &image("nsys", "nsys"));
    chunk(&mut parser, &mut state, &image("nsys", "Nsys2"));
    assert!(!state.character.take_nerve_question());
    chunk(&mut parser, &mut state, &image("nsys", "Nsys1"));
    assert!(state.character.take_nerve_question());
}

#[test]
fn another_characters_convulsions_settle_nothing() {
    let (mut parser, mut state) = (Parser::new(), GameState::default());
    chunk(&mut parser, &mut state, &image("nsys", "Nsys2"));
    chunk(
        &mut parser,
        &mut state,
        "Ada is wracked by a case of sporadic convulsions.\n",
    );
    assert!(!state.character.nerves.settled);
}
