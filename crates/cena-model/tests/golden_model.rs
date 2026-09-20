//! **Milestone 2's golden corpus: real wire, folded all the way into the model.**
//!
//! `plan/12` §8 puts "frame vocabulary breadth + golden corpus" in M2. The
//! parser has had goldens since M1 (`cena-protocol/tests/golden_fixtures.rs`),
//! and they are good ones -- they assert *meaning* rather than a snapshot, so
//! they cannot be regenerated past a regression. What did not exist is a golden
//! that reaches **`GameState`**.
//!
//! # Why that gap mattered, measured rather than argued
//!
//! Every model test in this crate was written against hand-made wire snippets.
//! That is not a hypothetical weakness: `pbarStance` shipped in `vitals` through
//! **17 green tests in `character_dialogs.rs`**, because every one of them sent
//! the stance bar inside the `stance` dialog. The wire also sends it inside
//! `combat` -- 130 times each over 15 files, identical values -- and the `combat`
//! copy fell through to `vitals`, where a real capture showed `pbarStance: 0`
//! sitting beside health. It was found by an ad-hoc end-to-end check, not by the
//! suite.
//!
//! **A hand-written snippet tests the shape you remembered. A cut of the wire
//! tests the shape that exists.** This file is the second kind.
//!
//! # Provenance
//!
//! `tests/fixtures/m2_model.xml` (9,523 bytes) is cut from
//! `GSIV-Monstr/2025/09/xml/2025-09-07_16-15-48.xml`, lines 54, 102, 106-111,
//! 116-120, 192, 204-214, through `cena_protocol::scrub::Scrubber`
//! (`cargo run -p cena-protocol --example cut_fixtures -- <raw-dir>`), exactly as
//! the M1 fixtures were. Its provenance is in `cena-protocol/tests/FIXTURES.md`
//! and `fixtures_are_scrubbed.rs` proves the redaction held.
//!
//! That source file was chosen by scoring a 48-file sample across 8 characters
//! for how many M2 model families each carries. It is the **smallest** file
//! scoring 12/12 at 143 KB.
//!
//! # These assert facts, not a snapshot
//!
//! Following `golden_fixtures.rs`'s rule, which is `plan/05` §0's: a snapshot
//! goes *different* on any change and gets regenerated, so each test here names
//! the fact it protects and fails with that fact in the message.

use cena_model::GameState;
use cena_protocol::Parser;

/// Fold the golden fixture exactly as the session actor does.
fn golden() -> GameState {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/m2_model.xml");
    // No `unwrap`/`expect`/`panic!` in a helper: the workspace denies all three
    // and clippy.toml's test allowance covers `#[test]` functions only. An
    // unreadable fixture yields an empty state, which every assertion below
    // fails on loudly -- see `golden_fixtures.rs` for the same reasoning.
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(&bytes) {
        state.apply(&frame);
    }
    state
}

// ---------------------------------------------------------------------------
// The regression this file exists for
// ---------------------------------------------------------------------------

#[test]
fn vitals_holds_the_players_gauges_and_nothing_else() {
    // **THE `pbarStance` REGRESSION.** It reached `vitals` past 17 green tests
    // and was caught only by an ad-hoc check against a real capture. This is
    // that check, made permanent.
    //
    // Asserted as an exact set rather than "does not contain pbarStance":
    // naming the one known escapee would not catch the next one, and the set of
    // things that ARE the player's gauges is small and knowable.
    let state = golden();
    let ids: Vec<&str> = state.vitals.keys().map(String::as_str).collect();

    assert_eq!(
        ids,
        vec!["health", "health2", "mana", "spirit", "stamina"],
        "something that is not one of the player's gauges reached `vitals`. One \
         `<progressBar>` shape carries gauges, stance, encumbrance AND \
         advancement; only the enclosing dialog tells them apart."
    );
}

// ---------------------------------------------------------------------------
// Step 1 -- streams
// ---------------------------------------------------------------------------

#[test]
fn the_paired_spell_stream_is_routed_to_its_own_buffer() {
    // Three `<stream id="Spells">` rows in the cut. Before `</stream>` published
    // its pop, these accumulated in `main` forever and the frame stream stood at
    // depth 31 on the author's full capture.
    let state = golden();
    let spells: Vec<String> = state
        .stream("Spells")
        .iter()
        .map(cena_protocol::runs::Runs::plain)
        .collect();

    // Three rows, and the third is the wire's own blank spacer -- a paired
    // stream whose whole body is a single space. It is KEPT: the game chose that
    // layout, and a model that dropped whitespace-only rows would reflow
    // somebody else's formatting. The same reasoning as
    // `an_empty_line_inside_a_stream_is_kept` in `stream_routing.rs`.
    assert_eq!(spells, vec!["Minor Spiritual", "Minor Elemental", " "]);
}

#[test]
fn spell_rows_do_not_leak_into_the_main_window() {
    let state = golden();
    let main: String = state
        .stream("")
        .iter()
        .map(cena_protocol::runs::Runs::plain)
        .collect();
    assert!(
        !main.contains("Minor Spiritual"),
        "a routed stream's content also landed in main: {main:?}"
    );
}

// ---------------------------------------------------------------------------
// Step 2 -- room contents
// ---------------------------------------------------------------------------

#[test]
fn the_room_id_and_description_come_from_the_wire() {
    let state = golden();
    assert_eq!(state.room.id.as_deref(), Some("7503251"));
    assert!(
        state
            .room
            .description
            .as_ref()
            .is_some_and(|d| d.plain().starts_with("Low eaves, stained black with smoke")),
        "the room description was not folded: {:?}",
        state
            .room
            .description
            .as_ref()
            .map(cena_protocol::runs::Runs::plain)
    );
}

#[test]
fn the_creature_is_separated_from_the_room_by_its_boldness() {
    // One bold entry in `room objs`. There is no separate creature feed -- the
    // same component carries creatures and items, and `<pushBold/>` is the only
    // thing that tells them apart.
    let state = golden();
    let creatures: Vec<&str> = state
        .room
        .creatures
        .iter()
        .map(|c| c.noun.as_str())
        .collect();
    assert_eq!(creatures, vec!["artificer"]);
    assert_eq!(
        state.room.creatures[0].text, "gaunt masked artificer",
        "the creature's display text was lost"
    );
}

#[test]
fn an_empty_room_players_component_is_told_rather_than_absent() {
    // `<component id='room players'></component>` -- an EMPTY body is a real
    // update meaning "nobody here", which is distinct from never having been
    // told. 135 of these in the measured sample.
    let state = golden();
    assert!(state.room.players.is_empty());
    assert!(
        state.room.saw_players(),
        "an empty players component was indistinguishable from silence"
    );
}

#[test]
fn every_room_component_is_buffered_independently() {
    // The author's requirement: "they all need to save/buffer/update
    // independently ... it's the real time feed". Five components in one burst,
    // and none may drop another.
    let state = golden();
    let ids: Vec<&str> = state.room.components().map(|(id, _)| id).collect();
    assert_eq!(
        ids,
        vec![
            "room desc",
            "room exits",
            "room objs",
            "room players",
            "sprite"
        ]
    );
}

#[test]
fn the_empty_sprite_component_is_kept() {
    // Vellum's documented hazard: it exempts `sprite` from its unchanged-check
    // because "the game sends it EMPTY on every room change". Cena has no
    // unchanged-check, so this holds without an exception -- and this is what
    // catches one being added.
    let state = golden();
    assert!(
        state.room.component("sprite").is_some(),
        "the empty sprite component was skipped"
    );
}

// ---------------------------------------------------------------------------
// Step 3 -- the character dialogs
// ---------------------------------------------------------------------------

#[test]
fn the_expr_dialog_is_folded_from_real_traffic() {
    let state = golden();
    assert_eq!(
        state.character.experience.level.as_deref(),
        Some("Level 100")
    );
    assert_eq!(
        state.character.experience.mind_state.as_deref(),
        Some("clear as a bell")
    );
}

#[test]
fn the_stance_is_read_and_is_not_a_vital() {
    let state = golden();
    assert_eq!(
        state.character.stance.as_deref(),
        Some("defensive (100%)"),
        "the stance bar did not reach the character"
    );
    assert_eq!(state.character.stance_percent, Some(100));
}

#[test]
fn encumbrance_carries_both_the_level_and_the_prose() {
    let state = golden();
    assert_eq!(state.character.encumbrance.as_deref(), Some("None"));
    assert_eq!(
        state.character.encumbrance_detail.as_deref(),
        Some("You are not encumbered enough to notice."),
        "the `encumblurb` label was dropped"
    );
}

#[test]
fn scars_are_read_from_the_injuries_dialog_and_healthy_parts_are_not() {
    // Five scarred parts in this capture, and the rest of the body sent as
    // `name == id`. A model storing healthy parts as zeroes would report all
    // sixteen.
    let state = golden();
    let scarred: Vec<&str> = state
        .character
        .injuries
        .keys()
        .map(String::as_str)
        .collect();

    assert_eq!(
        scarred,
        vec!["back", "leftEye", "neck", "rightHand", "rightLeg"],
        "the injury map is not exactly the hurt parts"
    );
    assert!(
        state
            .character
            .injuries
            .values()
            .all(|i| i.scar == 1 && i.wound == 0),
        "a scar left a wound in place, double-counting one injury: {:?}",
        state.character.injuries
    );
}

#[test]
fn a_map_tile_in_this_capture_is_not_taken_for_an_injury() {
    // The cut includes `<dialogData id='mapViewMain'><image id='nomap' .../>`.
    // Before `Frame::InjuryImage` carried its dialog, that was reported as a
    // body part -- and 1,313 of 2,155 `<image>` tags in the corpus are these.
    let state = golden();
    assert!(
        !state.character.injuries.contains_key("nomap"),
        "a map tile was folded in as a body part"
    );
}

// ---------------------------------------------------------------------------
// Step 4 -- inventory
// ---------------------------------------------------------------------------

#[test]
fn the_container_listing_is_folded_with_its_title() {
    let state = golden();
    let stow = state
        .inventory
        .container("stow")
        .expect("the backpack is declared in the cut");

    assert_eq!(stow.title.as_deref(), Some("My Backpack"));
    assert_eq!(
        stow.items.len(),
        20,
        "the container's item count changed: {:?}",
        stow.items.iter().map(|i| &i.noun).collect::<Vec<_>>()
    );
}

#[test]
fn the_container_does_not_list_itself() {
    // `<inv id='stow'>In the <a exist="94358989" noun="backpack">backpack</a>:`
    // -- the header names the container. This capture is the `stow` case, where
    // the window id and the object's `exist` id DIFFER, so the exclusion has to
    // go through `<container target='#94358989'>`.
    let state = golden();
    let nouns: Vec<&str> = state
        .inventory
        .container("stow")
        .map(|c| c.items.iter().map(|i| i.noun.as_str()).collect())
        .unwrap_or_default();

    assert!(
        !nouns.contains(&"backpack"),
        "the container listed itself as its own contents: {nouns:?}"
    );
}

// ---------------------------------------------------------------------------
// Step 5 -- noun resolution
// ---------------------------------------------------------------------------

#[test]
fn a_noun_from_this_capture_resolves_to_the_thing_it_names() {
    let state = golden();
    let found = state.resolve_noun("artificer");

    assert_eq!(found.len(), 1, "expected exactly the one creature");
    assert_eq!(found[0].found_in, cena_model::Where::Creature);
    assert_eq!(found[0].item.text, "gaunt masked artificer");
}

#[test]
fn a_carried_item_resolves_to_its_container() {
    // Resolution spans the room and the inventory, and reports WHICH, because
    // where a thing is changes what a command can do with it.
    let state = golden();
    let carried: Vec<_> = state
        .inventory
        .container("stow")
        .map(|c| c.items.clone())
        .unwrap_or_default();
    let first = carried.first().expect("the backpack holds items");

    let found = state.find_by_id(&first.id).expect("it must be findable");
    assert_eq!(found.found_in, cena_model::Where::Container("stow"));
}

// ---------------------------------------------------------------------------
// The whole fold, across a reconnect
// ---------------------------------------------------------------------------

#[test]
fn a_reconnect_forgets_only_what_the_connection_owned() {
    // `plan/12` §5.2 on real traffic rather than on a snippet.
    //
    // **RENAMED and rewritten 2026-09-20.** This was
    // `a_reconnect_forgets_everything_this_capture_taught`, and asserted that
    // the room and the whole character model went to Unknown. Both are wrong:
    //
    // > **AUTHOR:** *"time stops for 99.9% of things when you're offline ...
    // > you can't really change rooms when you're logged off."*
    //
    // A logged-off character is out of the world, so what it knew is still
    // true. What a reconnect forgets is what belonged to the CONNECTION -- the
    // clock's extrapolation, the idle warning, half-assembled lines -- plus
    // container contents, which the burst does not re-send.
    let mut state = golden();
    assert!(state.room.id.is_some());
    assert!(!state.inventory.is_empty());
    assert!(!state.character.injuries.is_empty());

    state.invalidate_for_reconnect();

    assert!(
        state.room.id.is_some(),
        "the room survives: a logged-off character does not walk anywhere"
    );
    assert!(
        state.inventory.is_empty(),
        "container CONTENTS are the one inventory fact the burst does not          re-send, so a stale mirror would never be corrected"
    );
    assert!(
        !state.character.injuries.is_empty(),
        "wounds do not heal while the character is out of the world, and the          burst carries `dialogData id='injuries'` regardless"
    );
    assert!(state.streams().next().is_none());
    assert!(
        !state.vitals.is_empty(),
        "vitals are RETAINED -- the burst re-sends them, so clearing opens a \
         window where health reads Unknown for no reason"
    );
}
