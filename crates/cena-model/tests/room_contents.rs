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

// ---------------------------------------------------------------------------
// `plan/19` §1c -- exits has THREE states, not two
// ---------------------------------------------------------------------------

/// An empty compass is a FACT, not an absence.
///
/// `plan/19` pattern A: a collection read as two states where three exist. The
/// review left this OPEN as §1c, "the same shape" as the roundtime bug that
/// made a gated send fire early.
///
/// The author settled the game fact (2026-09-19): both empty cases are real,
/// and they are different from each other.
///
/// - a room whose only way out is a **portal, door or teleport** rather than a
///   cardinal direction -- the compass is empty and the room IS exitable
/// - the **consultation lounge**, with no exits at all
///
/// So "the server sent an empty compass" and "we have not seen a compass" are
/// different facts, and anything that would route on them must act differently:
/// one says look for a door, the other says look.
#[test]
fn an_empty_compass_is_observed_rather_than_unknown() {
    // The trailing newline is REQUIRED: `push_bytes` emits on a line boundary,
    // so a fixture without one yields no frames at all and this test would pass
    // vacuously against a model that never saw a compass. Dropped it on the
    // first cut and the test failed for that reason rather than the real one.
    let state = fold(
        b"<compass></compass>
",
    );

    assert_eq!(
        state.room.exits,
        Some(Vec::new()),
        "an empty compass must record that the server SAID there are no \
         cardinal exits -- a room reached only by a portal looks exactly like \
         this, and it is not the same as never having looked"
    );
}

/// Before any compass arrives, exits are genuinely unknown.
#[test]
fn exits_are_unknown_until_a_compass_arrives() {
    let state = GameState::default();
    assert_eq!(
        state.room.exits, None,
        "a fresh state has not observed a compass, and must not claim the \
         room has no exits"
    );
}

/// The three states are distinguishable from one another.
///
/// Stated as one assertion because the point is the DISTINCTION: any two of
/// these collapsing is the defect, and a test of each state alone cannot show
/// they differ.
#[test]
fn the_three_exit_states_are_all_different() {
    let unknown = GameState::default().room.exits;
    let observed_empty = fold(
        b"<compass></compass>
",
    )
    .room
    .exits;
    let observed_some = fold(
        b"<compass><dir value='n'/></compass>
",
    )
    .room
    .exits;

    assert_ne!(
        unknown, observed_empty,
        "not-yet-observed collapsed into observed-with-no-exits"
    );
    assert_ne!(unknown, observed_some);
    assert_ne!(observed_empty, observed_some);
    assert_eq!(observed_some, Some(vec!["n".to_owned()]));
}

/// **A `<nav>` for the room we are already in is a re-declaration, not an
/// arrival.**
///
/// Review finding 4. The login burst sends `<nav rm='7086'/>` for the room the
/// character is already in and carries **no `compass`** to replace what a full
/// reset throws away -- so a reconnect that had just been taught to keep the
/// exits lost them to the burst one frame later.
#[test]
fn re_declaring_the_same_room_keeps_its_exits() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = cena_model::GameState::default();
    for frame in
        parser.push_bytes(b"<nav rm='7086'/><compass><dir value='n'/><dir value='e'/></compass>\n")
    {
        state.apply(&frame);
    }
    assert!(state.room.exits.is_some(), "guard: exits were observed");

    for frame in parser.push_bytes(b"<nav rm='7086'/>\n") {
        state.apply(&frame);
    }

    assert_eq!(
        state.room.exits.as_deref(),
        Some(["n".to_owned(), "e".to_owned()].as_slice()),
        "the same room id is the same room; nothing was replaced"
    );
}

/// A DIFFERENT room still resets everything.
///
/// The other half, and the one that must not regress: creatures from the last
/// room are the most dangerous thing to keep, because a behavior would attack
/// them.
#[test]
fn a_different_room_still_resets_whole() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = cena_model::GameState::default();
    for frame in parser.push_bytes(b"<nav rm='7086'/><compass><dir value='n'/></compass>\n") {
        state.apply(&frame);
    }
    assert!(state.room.exits.is_some(), "guard");

    for frame in parser.push_bytes(b"<nav rm='9999'/>\n") {
        state.apply(&frame);
    }

    assert_eq!(state.room.id.as_deref(), Some("9999"));
    assert_eq!(
        state.room.exits, None,
        "a new room invalidates the old exits"
    );
}

/// Two bare `<nav/>`s are two arrivals, not one.
///
/// `id.is_some()` guards the comparison because unknown is not equal to
/// unknown: treating two id-less arrivals as the same room would keep the
/// previous room's creatures.
#[test]
fn two_id_less_arrivals_are_not_the_same_room() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = cena_model::GameState::default();
    for frame in parser.push_bytes(b"<nav/><compass><dir value='n'/></compass>\n") {
        state.apply(&frame);
    }
    assert!(state.room.exits.is_some(), "guard");

    for frame in parser.push_bytes(b"<nav/>\n") {
        state.apply(&frame);
    }

    assert_eq!(
        state.room.exits, None,
        "unknown is not equal to unknown -- this is a second arrival"
    );
}

/// **A player's status is prose OUTSIDE the link, and it is read.**
///
/// `GameObj#status` for PCs (`gameobj.rb:311-317`), filled from
/// `xmlparser.rb:1152-1155`. The clause follows the `<a>` element, so it is not
/// an attribute and cannot be read off the link:
///
/// ```text
/// Also here: <a exist="-1" noun="Demandred">Demandred</a> who is hiding, ...
/// ```
///
/// The line is the author's own (2026-09-20). MEASURED through the parser
/// before the field existed: it yielded two ordinary players and dropped the
/// clause into the raw component, which is what this now fixes.
#[test]
fn a_player_status_is_read_from_the_prose_after_the_link() {
    let state = fold(
        b"<component id='room players'>Also here: \
<a exist=\"-1\" noun=\"Demandred\">Demandred</a> who is hiding, \
<a exist=\"-2\" noun=\"Kiyna\">Kiyna</a>.</component>\n",
    );

    let players = &state.room.players;
    assert_eq!(players.len(), 2);

    assert_eq!(players[0].noun, "Demandred");
    assert_eq!(
        players[0]
            .status
            .as_ref()
            .map(cena_model::PlayerStatus::as_str),
        Some("hiding"),
        "the clause after the link is this player's status"
    );
    assert!(
        players[0]
            .status
            .as_ref()
            .is_some_and(cena_model::PlayerStatus::is_hiding)
    );

    // **A player the room said nothing about has no status**, which is the
    // ordinary case and must not inherit the previous player's.
    assert_eq!(players[1].noun, "Kiyna");
    assert_eq!(
        players[1].status, None,
        "a status must not leak from the player before"
    );
}

/// Every status form the wire uses.
///
/// `who is X`, `who appears X`, a multi-word status, and the parenthesised
/// form -- which may carry two, joined as Lich joins them
/// (`xmlparser.rb:1153-1154`).
#[test]
fn every_status_form_is_read() {
    let cases: &[(&str, &str)] = &[
        (" who is hiding,", "hiding"),
        (" who is sitting,", "sitting"),
        (" who is lying down,", "lying down"),
        (" who appears dead.", "dead"),
        (" (hiding)", "hiding"),
        (" (hiding) (stunned)", "hiding stunned"),
    ];
    for (suffix, expected) in cases {
        let wire = format!(
            "<component id='room players'>Also here: \
<a exist=\"-1\" noun=\"Demandred\">Demandred</a>{suffix}</component>\n"
        );
        let state = fold(wire.as_bytes());
        assert_eq!(
            state.room.players[0]
                .status
                .as_ref()
                .map(cena_model::PlayerStatus::as_str),
            Some(*expected),
            "reading {suffix:?}"
        );
    }
}

/// Ordinary separators are not statuses.
///
/// Most of what follows a link is `, ` or `.`, and reading those as a status
/// would give every player in a busy room a nonsense condition.
#[test]
fn a_separator_is_not_a_status() {
    for suffix in [", ", ".", " and ", ""] {
        let wire = format!(
            "<component id='room players'>Also here: \
<a exist=\"-1\" noun=\"Demandred\">Demandred</a>{suffix}</component>\n"
        );
        let state = fold(wire.as_bytes());
        assert_eq!(
            state.room.players[0].status, None,
            "{suffix:?} is a separator, not a status"
        );
    }
}

/// **Objects and creatures never carry a status.**
///
/// The clause belongs to a person. Reading it for `room objs` would let a
/// hiding player's clause attach to whatever object followed them in the list.
#[test]
fn only_players_carry_a_status() {
    let state = fold(
        b"<component id='room objs'>  You also see<b> <pushBold/>a \
<a exist=\"1\" noun=\"kobold\">kobold</a><popBold/></b> who is hiding, a \
<a exist=\"2\" noun=\"disk\">disk</a>.</component>\n",
    );
    assert!(
        state.room.creatures.iter().all(|c| c.status.is_none()),
        "a creature has no player status"
    );
    assert!(
        state.room.objects.iter().all(|o| o.status.is_none()),
        "nor does an object"
    );
}
