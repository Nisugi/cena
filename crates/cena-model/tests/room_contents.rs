//! **M2 step 2: the room is several independent feeds, not one message.**
//!
//! > **AUTHOR, 2026-09-19:** *"they all need to save/buffer/update
//! > independently, because you go in a room and you will get constant room
//! > object updates as creatures come in and out, or player updates if a player
//! > comes in and out, it's the real time feed right, so lets not say room window
//! > feed is all one thing."*
//!
//! **This is a correctness requirement, not a display preference**, and the wire
//! says so. MEASURED over 18 files across 6 characters:
//!
//! | tag | count |
//! |---|---|
//! | `<component id='room objs'>` | 6,778 |
//! | `<component id='room players'>` | 3,870 |
//! | `<compDef id='room desc'>` | 3,008 |
//! | `<compDef id='room objs'>` | 3,002 |
//! | `<compDef id='room players'>` | 3,001 |
//! | `<compDef id='room exits'>` | 3,001 |
//! | `<compDef id='sprite'>` | 3,001 |
//!
//! `compDef` arrives ~3,000 times each -- the room-window snapshot on entry.
//! `component id='room objs'` arrives **6,778** times, more than twice as often:
//! that is the live feed, and it updates ALONE.
//!
//! MEASURED over 12 files: **4,445** standalone `<component>` updates arrive on
//! their own against 2,773 in groups. A real sequence from `GSIV-Monstr`:
//!
//! ```text
//! room players: Nisugi
//! room players: (empty)
//! room players: Sugiin
//! room players: Dicate, Sugiin
//! room players: Riend, Dicate, Sugiin
//! ```
//!
//! **191 of those bodies are EMPTY** (135 `room players`, 56 `room objs`) -- and
//! an empty component is a real update meaning "nobody here", which a blob model
//! cannot express distinctly from "not mentioned". That is the whole argument in
//! one measurement.
//!
//! # Creatures are derived from `room objs` by boldness
//!
//! Ported from `VellumFE` (`core/messages/component.rs:202-204`), and VERIFIED on
//! the wire:
//!
//! ```text
//! You also see<b> <pushBold/>a <a exist="412621" noun="lookout">wary-eyed
//! halfling lookout</a><popBold/></b>, a <a exist="18122889" noun="arrow">wooden
//! arrow</a> and a <a exist="-495692" noun="shop">squat shop</a>.
//! ```
//!
//! The lookout is bold; the arrow and the shop are not. **There is no separate
//! creature feed** -- one component carries both, and `<pushBold/>` is the only
//! thing that tells them apart. Our parser already keeps `bold_depth` on every
//! run, so this is a read rather than a new parse.
//!
//! # The hazard ported with it
//!
//! Vellum skips a component whose value is unchanged, but exempts `sprite`
//! because *"the game sends it EMPTY on every room change, so 'unchanged' would
//! short-circuit before room-art injection runs"* (`component.rs:165-171`). Cena
//! adds **no unchanged-check at all**: `room players` going empty and staying
//! empty is two real updates, and 191 empty bodies in the sample is exactly the
//! shape such a check would eat.

use cena_model::GameState;
use cena_protocol::Parser;

/// Fold a wire chunk the way the actor does.
fn fold(wire: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

/// The nouns of one typed collection, in wire order.
fn nouns(items: &[cena_model::RoomItem]) -> Vec<&str> {
    items.iter().map(|i| i.noun.as_str()).collect()
}

/// A real `room objs` body from `GSIV-Monstr`: one creature, two objects.
///
/// The shop's trailing prose is **shortened** from the capture's own wording,
/// which named a gem: Rule 3.4's architecture test flags the game's name
/// wherever it appears outside a game module, and it cannot tell a shop sign
/// from a product name. The trailing prose itself is load-bearing -- it is what
/// `the_raw_component_buffer_is_kept_beside_the_typed_collections` checks
/// survives -- so it is trimmed rather than dropped.
const OBJS: &[u8] = b"<component id='room objs'>  You also see<b> <pushBold/>a \
<a exist=\"412621\" noun=\"lookout\">wary-eyed halfling lookout</a><popBold/></b>, a \
<a exist=\"18122889\" noun=\"arrow\">wooden arrow</a> and a \
<a exist=\"-495692\" noun=\"shop\">squat shop</a> with a faded sign.</component>\n";

#[test]
fn a_players_update_does_not_disturb_the_objects() {
    // THE AUTHOR'S POINT, as a test. These arrive as separate components in the
    // same burst and separately thereafter; one must never clear the other.
    let mut wire = OBJS.to_vec();
    wire.extend_from_slice(
        b"<component id='room players'>Also here: <a exist=\"-10070682\" noun=\"Dicate\">Dicate</a></component>\n",
    );
    let state = fold(&wire);

    assert_eq!(nouns(&state.room.players), vec!["Dicate"]);
    assert_eq!(
        nouns(&state.room.objects),
        vec!["arrow", "shop"],
        "a players update wiped the objects"
    );
    assert_eq!(
        nouns(&state.room.creatures),
        vec!["lookout"],
        "a players update wiped the creatures"
    );
}

#[test]
fn a_component_update_leaves_every_other_raw_buffer_alone() {
    // **The test the first thirteen did not have.** They all assert on the TYPED
    // collections, which are separate fields -- so a blob-model `components` map
    // that cleared itself on every update passed all thirteen. Review pattern (C)
    // in `plan/19` for the second time in this milestone, caught by falsifying
    // rather than by trusting green.
    //
    // The raw buffers are what a renderer draws, so their independence is the
    // author's requirement just as much as the typed ones'.
    let mut wire = OBJS.to_vec();
    wire.extend_from_slice(
        b"<compDef id='room exits'>Obvious paths: <d>north</d></compDef>
",
    );
    wire.extend_from_slice(
        b"<compDef id='sprite'></compDef>
",
    );
    wire.extend_from_slice(
        b"<component id='room players'>Also here: <a exist=\"1\" noun=\"Dicate\">Dicate</a></component>
",
    );
    let state = fold(&wire);

    let ids: Vec<&str> = state.room.components().map(|(id, _)| id).collect();
    assert_eq!(
        ids,
        vec!["room exits", "room objs", "room players", "sprite"],
        "a component update dropped raw buffers it did not name"
    );
    assert!(
        state
            .room
            .component("room objs")
            .is_some_and(|b| b.plain().contains("You also see")),
        "the objects' raw body was replaced by a later, unrelated component"
    );
}

#[test]
fn an_empty_component_is_a_real_update_meaning_nobody_here() {
    // 135 empty `room players` bodies in the sample. "Nobody here" is a FACT,
    // distinct from "not mentioned", and a blob model cannot say it.
    let state = fold(
        b"<component id='room players'>Also here: <a exist=\"1\" noun=\"Dicate\">Dicate</a></component>\n\
          <component id='room players'></component>\n",
    );

    assert!(
        state.room.players.is_empty(),
        "an empty update left the previous occupants in the room: {:?}",
        nouns(&state.room.players)
    );
    assert!(
        state.room.saw_players(),
        "and it must be distinguishable from never having been told"
    );
}

#[test]
fn never_being_told_is_not_the_same_as_being_told_nobody_is_here() {
    let state = GameState::default();
    assert!(state.room.players.is_empty());
    assert!(
        !state.room.saw_players(),
        "an unobserved room claimed to know it was empty"
    );
}

#[test]
fn creatures_are_the_bold_entries_and_objects_are_the_rest() {
    // VERIFIED on the wire: one component carries both, and `<pushBold/>` is the
    // only thing separating them. There is no separate creature feed.
    let state = fold(OBJS);

    assert_eq!(nouns(&state.room.creatures), vec!["lookout"]);
    assert_eq!(nouns(&state.room.objects), vec!["arrow", "shop"]);
}

#[test]
fn a_creature_keeps_its_name_and_its_exist_id() {
    // The noun is what a command targets; the id is what the noun registry will
    // key on (`plan/18` step 5); the text is what a player reads. All three come
    // off the same link, so none of them may be dropped here.
    let state = fold(OBJS);
    let creature = &state.room.creatures[0];

    assert_eq!(creature.noun, "lookout");
    assert_eq!(creature.id, "412621");
    assert_eq!(creature.text, "wary-eyed halfling lookout");
}

#[test]
fn a_negative_exist_id_is_kept_verbatim() {
    // `exist="-495692"`. Ids are negative for room fixtures and for players, and
    // parsing them as a number would either lose the sign or force a type on the
    // model that the wire does not promise.
    let state = fold(OBJS);
    let shop = state
        .room
        .objects
        .iter()
        .find(|o| o.noun == "shop")
        .expect("the shop is an object");
    assert_eq!(shop.id, "-495692");
}

#[test]
fn the_raw_component_buffer_is_kept_beside_the_typed_collections() {
    // Both, as Vellum does: `room_components` keyed by id AND typed collections
    // derived from them (`core/app_core/state.rs:282`, `core/state.rs:127-151`).
    // The typed view answers "what can I attack"; the raw view is what a renderer
    // draws, prose and punctuation included.
    let state = fold(OBJS);
    let raw = state
        .room
        .component("room objs")
        .expect("the raw body must be kept");

    assert!(
        raw.plain().contains("You also see"),
        "the prose a renderer needs was thrown away: {:?}",
        raw.plain()
    );
    assert!(raw.plain().contains("with a faded sign."));
}

#[test]
fn an_unseen_component_is_absent_rather_than_empty() {
    // Unlike a stream buffer, where empty and absent are the same thing, here
    // they differ: "the game has not told us about the sprite" and "the sprite is
    // empty" are both real, and `sprite` is EMPTY on every room change.
    let state = fold(OBJS);
    assert!(state.room.component("sprite").is_none());
    assert!(state.room.component("room objs").is_some());
}

#[test]
fn an_empty_sprite_is_recorded_rather_than_skipped() {
    // Vellum's documented hazard, ported as a test rather than as a special case:
    // its unchanged-check had to exempt `sprite` because "the game sends it EMPTY
    // on every room change". Cena adds no unchanged-check, so this holds without
    // an exception -- and this test is what would catch one being added.
    let state = fold(b"<compDef id='sprite'></compDef>\n");
    assert!(
        state.room.component("sprite").is_some(),
        "an empty component was skipped as 'unchanged'"
    );
}

#[test]
fn comp_def_and_component_feed_the_same_collections() {
    // MEASURED: `compDef` is the room-entry snapshot (~3,000 each) and
    // `component` is the live update (6,778 for `room objs`). They carry the same
    // id and the same body shape, so they are one feed with two spellings; the
    // parser already emits `Frame::Component` for both.
    let state = fold(
        b"<compDef id='room players'>Also here: <a exist=\"1\" noun=\"Dicate\">Dicate</a></compDef>\n\
          <component id='room players'>Also here: <a exist=\"2\" noun=\"Riend\">Riend</a></component>\n",
    );
    assert_eq!(
        nouns(&state.room.players),
        vec!["Riend"],
        "the live update must replace the snapshot, not append to it"
    );
}

#[test]
fn entering_a_new_room_clears_every_component() {
    // `Frame::RoomId` already replaces the whole `Room`, because "a description
    // without its id describes somewhere the character no longer is". The
    // per-component buffers must go with it -- creatures from the last room are
    // the most dangerous thing to keep, since a behavior would attack them.
    let mut wire = OBJS.to_vec();
    wire.extend_from_slice(b"<nav rm='4128018'/>\n");
    let state = fold(&wire);

    assert!(
        state.room.creatures.is_empty(),
        "creatures from the previous room survived the move: {:?}",
        nouns(&state.room.creatures)
    );
    assert!(state.room.objects.is_empty());
    assert!(state.room.component("room objs").is_none());
}

#[test]
fn the_room_description_still_lands_where_criterion_2_expects_it() {
    // The existing contract. `room desc` is the one component with a dedicated
    // field, because criterion 2 reads it, and step 2 must not move it.
    let state = fold(b"<compDef id='room desc'>A wide, clear space.</compDef>\n");
    assert_eq!(
        state.room.description.as_ref().map(Runs::plain),
        Some("A wide, clear space.".to_owned())
    );
    assert!(
        state.room.component("room desc").is_some(),
        "and it is also a component, so a renderer drawing the room window by id \
         does not need a special case for the one component that is different"
    );
}

#[test]
fn a_reconnect_forgets_the_room_contents() {
    // `plan/12` §5.2. The login burst re-sends the room, so anything kept would
    // be a stale belief about who is standing next to the character.
    let mut state = fold(OBJS);
    assert!(!state.room.creatures.is_empty());

    state.invalidate_for_reconnect();

    assert!(state.room.creatures.is_empty());
    assert!(state.room.component("room objs").is_none());
    assert!(!state.room.saw_players());
}

use cena_protocol::runs::Runs;
