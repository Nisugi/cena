//! The six statuses that arrive only as text, never as an `<indicator>`.
//!
//! **Found by censusing Lich's parsers rather than a log**, at the author's
//! instruction:
//!
//! > *"This is also why I expected you to sleuth through the lich parser,
//! > xmlparser, and all of it's related things to see what else is missing
//! > rather than just looking at what my character only sees"*.
//!
//! MEASURED: every `Infomon.set` key in `infomon/parser.rb` and
//! `infomon/xmlparser.rb` -- 38 distinct -- against what `cena-model` reads.
//! All were modelled except `status.bound`, `status.calmed`,
//! `status.cutthroat`, `status.silenced`, `status.sleeping` and
//! `status.thorned`. **No indicator exists for any of them**, so nothing that
//! reads `<indicator>` could ever know.
//!
//! Every line below is Lich's own pattern text (`parser.rb:87-102`).

use cena_model::GameState;
use cena_model::state::afflictions::{Affliction, classify};
use cena_protocol::Parser;

fn chunk(text: &str) -> GameState {
    let mut state = GameState::default();
    let wire = format!("{text}\n<prompt time=\"1\">&gt;</prompt>\n");
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

#[test]
fn each_affliction_is_recognised_both_ways() {
    let cases = [
        (
            Affliction::Sleeping,
            "Your mind goes completely blank.",
            "You wake up from your slumber.",
        ),
        (
            Affliction::Bound,
            "An unseen force envelops you, restricting all movement!",
            "The restricting force enveloping you fades away.",
        ),
        (
            Affliction::Silenced,
            "A pall of silence settles over you.",
            "The pall of silence leaves you.",
        ),
        (
            Affliction::Calmed,
            "A calm washes over you.",
            "The feeling of calm leaves you.",
        ),
        (
            Affliction::Cutthroat,
            "All you manage to do is cough up some blood.",
            "That tingles, but there are no head injuries to repair.",
        ),
        (
            Affliction::Thorned,
            "One of the vines surrounding the thicket lashes out at you, driving a thorn into your skin!  You feel poison coursing through your veins.",
            "Your body begins to respond normally again.",
        ),
    ];
    for (affliction, on, off) in cases {
        assert_eq!(classify(on), Some((affliction, true)), "start: {on}");
        assert_eq!(classify(off), Some((affliction, false)), "end: {off}");
    }
}

#[test]
fn an_affliction_reaches_the_status_map_from_the_wire() {
    // They land in `StatusInfo` beside the indicators: one home for "what is
    // true of this character right now", whatever told us.
    let state = chunk("A pall of silence settles over you.");
    assert_eq!(state.status.known().silenced(), Some(true));
    assert!(state.status.silenced(), "and the bool accessor agrees");
}

#[test]
fn not_told_is_not_false() {
    // §5.2, and the reason `known` exists beside the bool accessors.
    assert_eq!(GameState::default().status.known().silenced(), None);
}

#[test]
fn an_end_line_clears_it() {
    let mut state = chunk("A pall of silence settles over you.");
    assert_eq!(state.status.known().silenced(), Some(true), "guard");
    for frame in Parser::new()
        .push_bytes(b"The pall of silence leaves you.\n<prompt time=\"2\">&gt;</prompt>\n")
    {
        state.apply(&frame);
    }
    assert_eq!(state.status.known().silenced(), Some(false));
}

#[test]
fn a_thorn_deprogression_is_still_poisoned() {
    // **The grouping is the fact**, and Lich's `# TODO: refactor / streamline?`
    // is inviting the bug: `ThornPoisonDeprogression` lines mean the poison is
    // WEAKENING, not gone. Only `ThornPoisonEnd` clears it.
    let easing = "With a shaky gasp and trembling muscles, you regain at least some small ability to move, however slowly.";
    assert_eq!(classify(easing), Some((Affliction::Thorned, true)));

    let mut state = chunk(easing);
    assert_eq!(state.status.known().thorned(), Some(true));
    for frame in Parser::new()
        .push_bytes(b"Your skin takes on a more pinkish tint.\n<prompt time=\"2\">&gt;</prompt>\n")
    {
        state.apply(&frame);
    }
    assert_eq!(state.status.known().thorned(), Some(false), "the end line");
}

#[test]
fn cutthroat_is_the_one_pattern_that_is_not_anchored() {
    // Lich marks it in its own source: `CutthroatActiveMid` is "mid-line:
    // cannot be part of the anchored fast path", because the attacker comes
    // first on the wire.
    assert_eq!(
        classify("A kobold slices deep into your vocal cords!"),
        Some((Affliction::Cutthroat, true))
    );
}

#[test]
fn every_other_pattern_is_anchored_so_a_player_cannot_say_it() {
    // The anchors in `parser.rb` are load-bearing: a player can type any of
    // these into a channel, and a speech line puts the speaker first.
    for said in [
        "Someone says, \"A calm washes over you.\"",
        "Someone says, \"Your mind goes completely blank.\"",
        "Someone whispers, \"A pall of silence settles over you.\"",
    ] {
        assert_eq!(classify(said), None, "a player said it: {said}");
    }
}

#[test]
fn an_ordinary_line_says_nothing() {
    assert_eq!(classify("You see nothing unusual."), None);
    assert_eq!(classify(""), None);
    let state = chunk("You see nothing unusual.");
    let k = state.status.known();
    assert_eq!(
        [
            k.bound(),
            k.calmed(),
            k.cutthroat(),
            k.silenced(),
            k.sleeping(),
            k.thorned()
        ],
        [None; 6]
    );
}

#[test]
fn the_ids_are_lichs_keys_without_the_prefix() {
    // `status.silenced` -> `silenced`, the same shape `StatusInfo` normalises
    // `IconSTUNNED` to. A rename here silently stops matching Lich's data.
    let ids: Vec<&str> = Affliction::ALL.iter().map(|a| a.id()).collect();
    assert_eq!(
        ids,
        [
            "bound",
            "calmed",
            "cutthroat",
            "silenced",
            "sleeping",
            "thorned"
        ]
    );
}

#[test]
fn a_reconnect_forgets_them_with_the_rest_of_the_status_map() {
    let mut state = chunk("A calm washes over you.");
    assert_eq!(state.status.known().calmed(), Some(true), "guard");
    state.invalidate_for_reconnect();
    assert_eq!(
        state.status.known().calmed(),
        None,
        "a calm cannot be believed across a disconnect"
    );
}

#[test]
fn a_leading_space_does_not_hide_an_affliction() {
    // **MEASURED by mutation**: removing the `trim_start` left every test
    // green, because none of them fed an indented line. The wire indents --
    // Lich's own `CutthroatNoActive` pattern begins `^\s*`, which is that
    // source saying the leading space is real -- and `society_line` in this
    // same crate treats indentation as load-bearing for the opposite reason.
    assert_eq!(
        classify(
            "   The horrible pain in your vocal cords subsides as you spit out the last of the blood clogging your throat."
        ),
        Some((Affliction::Cutthroat, false))
    );
    assert_eq!(
        classify("  A pall of silence settles over you."),
        Some((Affliction::Silenced, true))
    );
}
