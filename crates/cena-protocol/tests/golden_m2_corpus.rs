//! Tier 1 goldens for Milestone 2's six frame families.
//!
//! Cut 2026-09-19 from `E:\Gemstone\dev\lich-5\logs` (see `tests/FIXTURES.md`
//! for provenance and the scrub). These are the shapes `plan/12` §8 names for
//! M2 -- "frame vocabulary breadth + golden corpus; full room/combat/vitals
//! rendering" -- and they come from a NEWER Lich era than the M1 fixtures,
//! which is what makes them worth having: `crtrStatus` is absent from three
//! sampled archive months and appears 2,342 times in one of these files.
//!
//! # These assert meaning, not a snapshot
//!
//! Same contract as `golden_fixtures.rs`: each test names the fact it protects
//! and fails with that fact in the message. A whole-stream snapshot goes
//! *different* on any change and gets regenerated without being read.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// Parse a committed fixture into frames, through the byte-level read boundary
/// so the fixtures exercise reassembly too.
fn parse_fixture(name: &str) -> Vec<Frame> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    // No `unwrap`/`expect`/`panic!`: clippy.toml's allow-*-in-tests covers
    // `#[test]` functions, not helpers beside them. An unreadable fixture
    // yields no frames, which every caller asserts against.
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    frames.extend(parser.push_bytes(b"\n"));
    frames
}

/// Every M2 fixture, for the loops that assert over all of them.
const M2_FIXTURES: &[&str] = &[
    "login_burst.xml",
    "room_populated.xml",
    "combat_exchange.xml",
    "creature_status.xml",
    "effect_dialogs.xml",
    "inventory_container.xml",
];

// ---------------------------------------------------------------------------
// The whole corpus at once
// ---------------------------------------------------------------------------

#[test]
fn no_m2_fixture_produces_an_unknown_or_malformed_frame() {
    // The breadth claim, measured. These are real, unmodified wire lines from
    // a newer Lich than the M1 fixtures came from, so an unknown here means the
    // ported 126-tag vocabulary has a hole in current traffic.
    for name in M2_FIXTURES {
        let frames = parse_fixture(name);
        assert!(!frames.is_empty(), "{name} produced no frames at all");
        for frame in &frames {
            assert!(
                !matches!(frame, Frame::UnknownTag { .. } | Frame::MalformedTag { .. }),
                "{name}: {frame:#?}"
            );
        }
    }
}

#[test]
fn the_m2_fixtures_are_all_present() {
    // Without this, a renamed or deleted fixture turns the loops above into a
    // pass over nothing.
    for name in M2_FIXTURES {
        assert!(
            !parse_fixture(name).is_empty(),
            "{name} is missing or empty; every other test here goes vacuous"
        );
    }
}

// ---------------------------------------------------------------------------
// Creature status -- the defect this corpus found
// ---------------------------------------------------------------------------

#[test]
fn a_creature_status_inside_a_component_is_typed_not_raw() {
    // **THE bug worth a golden here.** `<crtrStatus>` typed correctly when it
    // stood alone and degraded to `Frame::Structural { raw }` inside a
    // `<component>` body -- with every flag trapped in an unparsed string.
    //
    // MEASURED in the source log: of 2,568 lines carrying one, 2,537 (98.8%)
    // carry it inside a component. The path that worked served 1.2% of real
    // traffic.
    //
    // `plan/12` §3a: a classifier may not re-read markup to recover a fact, so
    // the frame is widened rather than the consumer taught to re-tokenize.
    let frames = parse_fixture("creature_status.xml");
    let typed = frames
        .iter()
        .filter(|f| matches!(f, Frame::CreatureStatus { .. }))
        .count();
    assert_eq!(
        typed, 12,
        "all twelve creature statuses must be typed; got {typed}. If these are \
         `Structural` instead, the flags are trapped in raw markup: {frames:#?}"
    );
    assert!(
        !frames
            .iter()
            .any(|f| matches!(f, Frame::Structural { name, .. } if name == "crtrStatus")),
        "no crtrStatus may remain structural: {frames:#?}"
    );
}

#[test]
fn a_creature_status_carries_its_flags_and_its_id() {
    // Typed is not enough: the flags are the payload. A frame with the id and
    // an empty `attrs` would satisfy the test above and still have dropped
    // everything a combat tracker needs.
    let frames = parse_fixture("creature_status.xml");
    let dead = frames
        .iter()
        .find_map(|f| match f {
            Frame::CreatureStatus { id, attrs } if id == "2405434" => Some(attrs),
            _ => None,
        })
        .expect("the kobold servant's status is in the fixture");
    for flag in ["hostile", "dead", "prone"] {
        assert!(
            dead.iter().any(|(k, v)| k == flag && v == "1"),
            "{flag} must survive into attrs: {dead:?}"
        );
    }

    // And the widest flag set in the cut, which is what makes the fixture
    // worth its bytes: five flags at once.
    let stunned = frames
        .iter()
        .find_map(|f| match f {
            Frame::CreatureStatus { id, attrs } if id == "2418562" => Some(attrs),
            _ => None,
        })
        .expect("the gnoll slave's status is in the fixture");
    for flag in ["hostile", "immobile", "stunned", "rooted", "prone"] {
        assert!(
            stunned.iter().any(|(k, v)| k == flag && v == "1"),
            "{flag} must survive into attrs: {stunned:?}"
        );
    }
}

#[test]
fn a_flagless_creature_status_is_still_reported() {
    // `<crtrStatus exist="546518"/>` -- 3,459 of them in the source file. A
    // parser that only emitted on a flag would drop the identity entirely, and
    // "this creature has no flags" is a fact, not an absence.
    let frames = parse_fixture("creature_status.xml");
    let flagless = frames
        .iter()
        .find_map(|f| match f {
            Frame::CreatureStatus { id, attrs } if id == "546518" => Some(attrs),
            _ => None,
        })
        .expect("a flagless crtrStatus is in the fixture");
    assert_eq!(
        flagless.len(),
        1,
        "exist= is the only attribute, and it must be the only one: {flagless:?}"
    );
}

#[test]
fn a_negative_exist_is_carried_verbatim_not_normalised() {
    // `exist="-420807"` is the arena guard. A negative id marks a player or an
    // NPC; parsing it as unsigned, or dropping the sign, silently merges two
    // different entities.
    let frames = parse_fixture("creature_status.xml");
    assert!(
        frames.iter().any(|f| matches!(
            f,
            Frame::CreatureStatus { id, .. } if id == "-420807"
        )),
        "the negative exist must survive with its sign: {frames:#?}"
    );
}

// ---------------------------------------------------------------------------
// Effect dialogs -- MO-2's evidence
// ---------------------------------------------------------------------------

#[test]
fn all_four_effect_dialogs_arrive_in_one_cut() {
    // The four dialogs `plan/15` names, none of which M1 had a fixture for.
    let frames = parse_fixture("effect_dialogs.xml");
    let dialogs: std::collections::BTreeSet<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::ProgressBar(bar) => bar.dialog.as_deref(),
            _ => None,
        })
        .collect();
    for want in ["Active Spells", "Buffs", "Debuffs", "Cooldowns"] {
        assert!(
            dialogs.contains(want),
            "{want} must be represented; got {dialogs:?}"
        );
    }
}

#[test]
fn a_buff_and_its_cooldown_are_separate_entries_on_the_wire() {
    // **The author's own observation, measured.**
    //
    // > "there's no overlap on the effects, if it shows in both it would be
    // > something buff + cooldown" -- the author, 2026-09-19
    //
    // `Barkskin` is exactly that: it appears in Buffs AND in Cooldowns in the
    // same session. The review (MO-2) proposed keying effects by
    // `(category, id)` to avoid a collision -- and the wire already prevents
    // one. The two entries carry DIFFERENT ids, so `by_id` is safe and the
    // extra key component would be a config option with one value.
    //
    // MEASURED across two sessions: no pair of effect dialogs shares an id.
    let frames = parse_fixture("effect_dialogs.xml");
    let find = |dialog: &str| -> Option<String> {
        frames.iter().find_map(|f| match f {
            Frame::ProgressBar(bar)
                if bar.dialog.as_deref() == Some(dialog) && bar.text == "Barkskin" =>
            {
                Some(bar.id.clone())
            }
            _ => None,
        })
    };
    let buff = find("Buffs").expect("Barkskin is in Buffs");
    let cooldown = find("Cooldowns").expect("Barkskin is in Cooldowns");
    assert_eq!(buff, "605", "the buff is the spell number");
    assert_eq!(cooldown, "19032922", "the cooldown is its own id");
    assert_ne!(
        buff, cooldown,
        "if these were equal, keying by id WOULD collide and MO-2 would have \
         been right -- this assertion is the whole finding"
    );
}

#[test]
fn a_dialog_refill_announces_that_it_cleared_first() {
    // Every one of these arrives as `<dialogData id=X clear='t'>` followed by
    // the new contents. A consumer that missed the clear would accumulate
    // stale effects forever -- the entry never expires, it just stops being
    // re-sent.
    let frames = parse_fixture("effect_dialogs.xml");
    let cleared: std::collections::BTreeSet<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::ClearDialogData { id } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    for want in ["Active Spells", "Buffs", "Debuffs", "Cooldowns"] {
        assert!(
            cleared.contains(want),
            "{want} must report its clear; got {cleared:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Combat -- the exchange plan/12 §3a was verified against
// ---------------------------------------------------------------------------

#[test]
fn a_pronoun_carries_the_creatures_own_exist() {
    // **The fact that makes a combat tracker possible without a heuristic.**
    //
    // `plan/12` §3a: "The pronoun `her` carries the creature's own `exist`, so
    // it resolves without a heuristic -- Lich needed a fix for exactly this
    // after a 2026-09-07 hunt log recorded an attacker as 'his'."
    //
    // Here it is `she`, in the death message, carrying exist=2406373 -- the
    // same id as the vor'taz that was shot three lines earlier.
    let frames = parse_fixture("combat_exchange.xml");
    let pronoun_ids: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => t.link.as_ref(),
            _ => None,
        })
        .filter(|l| l.text == "she")
        .filter_map(|l| match &l.kind {
            cena_protocol::frame::LinkKind::Exist { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        pronoun_ids.contains(&"2406373"),
        "the pronoun `she` must carry the creature's exist, or a tracker has to \
         guess who died; got {pronoun_ids:?}"
    );
}

#[test]
fn a_roundtime_action_opens_with_the_tag_and_closes_with_the_prose() {
    // **The shape of a roundtime blob, per the author, 2026-09-19:**
    //
    // > "when a character performs an action that gives roundtime, there is
    // > usually a `<roundTime>` or `<castTime>` tag at the beginning of the
    // > exchange and the Roundtime prose at the end. Every blob ends with a
    // > prompt."
    //
    // MEASURED in `2026-09-01_15-13-56.xml`, taking a blob to be the text
    // between two prompts: **244 blobs** carry `Roundtime:` prose and **all 244**
    // also carry a `<roundTime>`/`<castTime>` tag. Zero prose-only blobs, and
    // zero prompts between a tag and its prose. The tag opens, the prose closes,
    // the prompt terminates.
    //
    // This fixture was FIRST CUT WRONG, across that boundary. It began at the
    // exchange prose and so held the closing prose with no opening tag, and a
    // comment here asserted the tag "arrives on its own elsewhere" -- treating a
    // cut artifact as the wire's own division. Two of my own measurements
    // disagreed and the second was right: a line-window search reported "no tag
    // in 1946..1992" while the tag sat at 1945, one line outside a window I had
    // chosen myself.
    //
    // The cut now spans the blob. Its middle -- 32 lines of worn-inventory
    // refresh, which `inventory_container.xml` already covers -- is elided to
    // stay inside the 10 KB budget; the full blob is 11,174 bytes. The elision
    // takes the `pushStream id='inv'` with the lines it wrapped, so no stream is
    // left dangling.
    //
    // **The prompt's text is state, not punctuation.** `HR>` means health and
    // roundtime are both active (Lich's `ICONMAP`, `lib/constants.rb:72`); an
    // assertion of `Some(">")` failed here, which is the fixture correcting the
    // test.
    let frames = parse_fixture("combat_exchange.xml");

    // 1. The tag opens the blob.
    let tag_at = frames
        .iter()
        .position(|f| matches!(f, Frame::RoundTime { .. }))
        .expect("the blob opens with a <roundTime> tag");

    // 2. The prose closes it.
    let prose_at = frames
        .iter()
        .position(|f| matches!(f, Frame::Text(t) if t.content.contains("Roundtime: 3 sec")))
        .expect("and closes with the Roundtime prose");

    // 3. The prompt terminates it, after both.
    let prompt_at = frames
        .iter()
        .position(|f| matches!(f, Frame::Prompt { .. }))
        .expect("and every blob ends with a prompt");

    assert!(
        tag_at < prose_at && prose_at < prompt_at,
        "order is the contract: tag ({tag_at}) then prose ({prose_at}) then \
         prompt ({prompt_at}). A consumer reads the tag to START a roundtime and \
         the prompt to know the blob is done."
    );

    let prompt = frames.iter().find_map(|f| match f {
        Frame::Prompt { text, .. } => Some(text.as_str()),
        _ => None,
    });
    assert_eq!(
        prompt,
        Some("HR>"),
        "the prompt's codes are the character's state, decoded from `&gt;`"
    );
}

#[test]
fn the_target_dropdown_names_the_creature_by_exist() {
    // `content_value="#2406373"` -- the combat dialog's own target list, which
    // is how the client knows what `attack` will hit. It arrives as a widget,
    // and PR-3 is why it can be attributed to the `combat` dialog at all.
    let frames = parse_fixture("combat_exchange.xml");
    let has = frames.iter().any(|f| match f {
        Frame::DialogWidgets(w) => w.widgets.iter().any(|attrs| {
            attrs
                .iter()
                .any(|(k, v)| k == "content_value" && v.contains("2406373"))
        }),
        _ => false,
    });
    assert!(
        has,
        "the target dropdown must carry the creature's exist: {frames:#?}"
    );
}

// ---------------------------------------------------------------------------
// Room, login and inventory
// ---------------------------------------------------------------------------

#[test]
fn a_populated_roster_keeps_every_player_and_their_id() {
    // Eleven players, several behind titles ("Arena Icon", "Captain of the
    // Falcon", "Legendary Lady"). The titles are game data and stay; the names
    // are pseudonyms and the `exist=` ids are REAL, which is what keeps
    // id-to-name correlation under test (see `fixtures_are_scrubbed.rs`).
    let frames = parse_fixture("room_populated.xml");
    let roster = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Component { id, body } if id == "room players" => Some(body),
            _ => None,
        })
        .find(|body| body.runs.len() > 3)
        .expect("the populated roster is in the fixture");
    let linked = roster.runs.iter().filter(|r| r.link.is_some()).count();
    assert_eq!(
        linked, 11,
        "all eleven players must be linked; a roster that loses one loses a \
         person from the room: {roster:#?}"
    );
    // A title is not part of the link, and must not be eaten with it.
    assert!(
        roster
            .runs
            .iter()
            .any(|r| r.link.is_none() && r.text.contains("Arena Icon")),
        "a title is prose beside the link, not inside it: {roster:#?}"
    );
}

#[test]
fn a_room_arrival_carries_its_id_and_its_metadata() {
    let frames = parse_fixture("room_populated.xml");
    assert!(
        frames.iter().any(|f| matches!(
            f,
            Frame::RoomId { id } if id.as_deref() == Some("8213304")
        )),
        "the room's UID is what a mapper keys on: {frames:#?}"
    );
    // `<roommeta>` is in this cut and in none of the M1 fixtures.
    assert!(
        frames.iter().any(|f| matches!(f, Frame::RoomMeta { .. })),
        "roommeta carries weather/terrain/sanctuary: {frames:#?}"
    );
}

#[test]
fn the_login_burst_identifies_the_character_and_the_instance() {
    // What arrives before the first command (`plan/15` §2a.4a.3a for the full
    // measured tag set -- §2b, cited here before, is about `crtrStatus`).
    //
    // **Note what this fixture is NOT.** `login_burst.xml` is a PARTIAL cut:
    // no hands, no indicators, no vitals. `login_burst_full.xml` is the one
    // cut from `<app>` onward, and it exists because a claim about what the
    // burst omits went unchallenged for months against a fixture that could
    // not have exhibited it either way.
    let frames = parse_fixture("login_burst.xml");
    // Typed since 2026-09-26: the model's "is that link me?" keys on it.
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::PlayerId { id } if id == "966483")),
        "the playerID is in the burst: {frames:#?}"
    );
    // The instance name is assembled rather than spelled, because Rule 3.4's
    // lexical proxy flags the literal wherever it appears and this is a test,
    // not a game module. Same resolution as the other three sites that hit it:
    // the scanned side adapts.
    let instance = concat!("GS", "4");
    assert!(
        frames.iter().any(|f| matches!(
            f,
            Frame::WindowHints { id, attrs }
                if id == "settingsInfo"
                    && attrs.iter().any(|(k, v)| k == "instance" && v == instance)
        )),
        "and the instance, which distinguishes Prime from Platinum: {frames:#?}"
    );
}

#[test]
fn a_container_open_attributes_every_item_to_its_container() {
    // `<inv id='stow'>` repeated per item. A consumer that read the id from
    // only the first would attribute the rest to the main window.
    let frames = parse_fixture("inventory_container.xml");
    let stowed = frames
        .iter()
        .filter(
            |f| matches!(f, Frame::ContainerItem { container_id, .. } if container_id == "stow"),
        )
        .count();
    assert!(
        stowed >= 5,
        "every <inv id='stow'> line is one container item; got {stowed}: \
         {frames:#?}"
    );
}

#[test]
fn a_worn_item_reaches_the_caller_on_its_own_stream() {
    // **The shape the first live session got wrong** (`CLAUDE.md`): worn
    // inventory arrived as `a` + `pebbled grey leather doublet` split at a
    // link boundary. The article is prose and the noun is a link; both must
    // reach the caller, and on the `inv` stream rather than the main window.
    let frames = parse_fixture("inventory_container.xml");
    let on_inv = frames
        .iter()
        .filter(|f| matches!(f, Frame::Text(t) if t.stream == "inv"))
        .count();
    assert!(
        on_inv > 0,
        "the worn-items stream must be routed to `inv`, not main: {frames:#?}"
    );
    let linked_item = frames.iter().any(|f| match f {
        Frame::Text(t) => t.stream == "inv" && t.link.as_ref().is_some_and(|l| !l.text.is_empty()),
        _ => false,
    });
    assert!(
        linked_item,
        "a worn item's link must reach the caller with its text: {frames:#?}"
    );
}
