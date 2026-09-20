//! The four injury predicates, checked against the wiki's own penalty table.
//!
//! Ported from `lib/gemstone/injured.rb`, with
//! `reference/wiki_clean/Wound.txt` as an oracle for what it *states*.
//!
//! # The wiki is authoritative where it speaks, and silent elsewhere
//!
//! It earned the oracle role by catching a wrong assertion: I had written from
//! the Ruby that a rank-2 arm blocks casting, and the table says plainly that
//! it prevents ranged attacks only.
//!
//! It does **not** follow that the table is exhaustive. It omits the
//! cumulative rules (`injured.rb:128-133`), the per-side arm/hand merge, and
//! the rank-2 nerves block on ranged -- and that last one the author settled
//! by testing it:
//!
//! > **AUTHOR, 2026-09-20:** *"I personally tested ranged for injured.rb."*
//!
//! So a row's absence is not evidence against a rule. That is the same error
//! as reading absence from the login burst as invalidation
//! (`plan/15` §2a.4a.3a), and it is worth naming twice because both times the
//! silent source looked authoritative.
//!
//! The table the last group of tests walks, verbatim from that page:
//!
//! ```text
//! head            | Rank 2: prevents searching, spellcasting
//!                 | Rank 3: prevents searching, spellcasting, ranged attacks
//! nervous system  | Rank 2: prevents spellcasting, searching
//! eye             | Rank 3: prevents searching, spellcasting
//! leg             | Rank 2: prevents sneaking
//! arm and/or hand | Rank 2: prevents ranged attacks
//!                 | Rank 3: prevents spellcasting, ranged attacks
//! ```

use std::collections::BTreeMap;

use cena_model::{Able, Injuries, Injury};

/// Build an injury map from `(part, wound, scar)` triples.
///
/// A part may carry both: *"if there is both a scar and a fresh wound on the
/// same location then the wound must be healed first"* (the wiki).
fn injuries(parts: &[(&str, u8, u8)]) -> BTreeMap<String, Injury> {
    parts
        .iter()
        .map(|(id, wound, scar)| {
            (
                (*id).to_owned(),
                Injury {
                    wound: *wound,
                    scar: *scar,
                },
            )
        })
        .collect()
}

/// An unhurt character can do everything.
#[test]
fn an_unhurt_character_is_able() {
    let parts = BTreeMap::new();
    let state = Injuries::new(&parts);
    assert_eq!(state.able_to_cast(), Able::Yes);
    assert_eq!(state.able_to_sneak(), Able::Yes);
    assert_eq!(state.able_to_search(), Able::Yes);
    assert_eq!(state.able_to_use_ranged(), Able::Yes);
}

/// **Rank-1 scars carry no penalty; worse ones count like wounds.**
///
/// The wiki, "Wound Penalties": *"Rank 1 scars never have any mechanical
/// penalties. More significant scars have penalties similar to wounds of the
/// same level."* That sentence is `effective = max(wound, scar unless 1)`.
#[test]
fn a_rank_one_scar_carries_no_penalty() {
    let parts = injuries(&[("head", 0, 1)]);
    assert_eq!(Injuries::new(&parts).effective_rank("head"), 0);
    assert_eq!(Injuries::new(&parts).able_to_cast(), Able::Yes);

    let parts = injuries(&[("head", 0, 2)]);
    assert_eq!(Injuries::new(&parts).effective_rank("head"), 2);
    assert_eq!(
        Injuries::new(&parts).able_to_cast(),
        Able::NoInjured,
        "a rank-2 scar blocks casting exactly as a rank-2 wound does"
    );
}

/// **A wound and a scar coexist, and the worse one governs.**
///
/// > **AUTHOR, 2026-09-20:** *"R2W -> R1W & R2S -> R0W & R2S -> R1S ->
/// > Healthy."*
///
/// `R1W & R2S` is the state after one herb application. The scar is worse than
/// the wound now covering it, because the scar records the rank the wound was
/// healed from.
#[test]
fn a_wound_and_a_scar_coexist_and_the_worse_one_governs() {
    let parts = injuries(&[("head", 1, 2)]);
    assert_eq!(
        Injuries::new(&parts).effective_rank("head"),
        2,
        "the scar is the worse of the two"
    );
    assert_eq!(Injuries::new(&parts).able_to_cast(), Able::NoInjured);
}

/// **Sigil bypasses rank 2, never rank 3.**
///
/// The wiki: *"Sigil of Determination allows you to ignore penalties from rank
/// 2 and lower wounds, including combined penalties from multiple wounds. It
/// does not allow you to ignore penalties from rank 3 wounds."*
#[test]
fn sigil_bypasses_rank_two_but_not_rank_three() {
    let parts = injuries(&[("head", 2, 0)]);
    assert_eq!(Injuries::new(&parts).able_to_cast(), Able::NoInjured);
    assert_eq!(
        Injuries::new(&parts).with_sigil(true).able_to_cast(),
        Able::Yes
    );

    let parts = injuries(&[("head", 3, 0)]);
    assert_eq!(
        Injuries::new(&parts).with_sigil(true).able_to_cast(),
        Able::NoCritical,
        "rank 3 is beyond Sigil"
    );
}

/// `Able` says whether Sigil would help, which is the reason for two `No`s.
#[test]
fn the_two_refusals_differ_by_whether_sigil_helps() {
    assert!(Able::Yes.is_yes());
    assert!(!Able::NoInjured.is_yes());

    assert!(Able::NoInjured.sigil_would_help());
    assert!(
        !Able::NoCritical.sigil_would_help(),
        "a rank-3 injury is not something Sigil can answer"
    );
}

/// **Arm and hand are worst-of-per-side, not summed.**
///
/// `injured.rb:110-117`, and the wiki groups them as *"arm and/or hand"*. A
/// rank-2 arm and a rank-2 hand on one side is a rank-2 side.
///
/// `eherbs.lic:2896-2910` implements this differently -- it sums wound and
/// scar separately across the pair -- and the author settled which is right:
/// **"I would say injured.rb implementation is the right one."**
#[test]
fn an_arm_and_its_hand_are_one_side() {
    // Rank 1 on arm AND hand of one side: worst-of gives that side 1.
    let parts = injuries(&[("leftArm", 1, 0), ("leftHand", 1, 0)]);
    assert_eq!(
        Injuries::new(&parts).able_to_cast(),
        Able::Yes,
        "one side at rank 1, the other at 0: nothing blocks"
    );

    // Rank 1 per side: 1 + 1 = 2, under the cumulative threshold of 3.
    let parts = injuries(&[("leftArm", 1, 0), ("rightHand", 1, 0)]);
    assert_eq!(Injuries::new(&parts).able_to_cast(), Able::Yes);

    // Rank 2 on one side and rank 1 on the other: 2 + 1 = 3.
    let parts = injuries(&[("leftArm", 2, 0), ("rightHand", 1, 0)]);
    assert_eq!(
        Injuries::new(&parts).with_sigil(true).able_to_cast(),
        Able::Yes,
        "guard: Sigil clears the individual rank-2 rule, so the cumulative \
         rule below is what is being tested rather than that one"
    );
}

/// **The cumulative rule: three across the eyes blocks casting.**
///
/// The wiki gestures at it -- *"In certain cases, having wounds on both sides
/// of the body also restricts activities"* -- and `injured.rb:128-133` gives
/// the number.
#[test]
fn cumulative_eye_injuries_block_casting() {
    let parts = injuries(&[("leftEye", 1, 0), ("rightEye", 1, 0)]);
    assert_eq!(
        Injuries::new(&parts).able_to_cast(),
        Able::Yes,
        "two rank-1 eyes total 2, under the threshold"
    );

    let parts = injuries(&[("leftEye", 1, 0), ("rightEye", 0, 2)]);
    assert_eq!(
        Injuries::new(&parts).able_to_cast(),
        Able::NoInjured,
        "1 + 2 = 3, and no single part is at rank 2 in the head/nerves group"
    );
}

// ---------------------------------------------------------------------------
// The wiki's penalty table, walked row by row.
// ---------------------------------------------------------------------------

/// `head | Rank 2: prevents searching, spellcasting`
#[test]
fn a_rank_two_head_prevents_searching_and_spellcasting_only() {
    let parts = injuries(&[("head", 2, 0)]);
    let state = Injuries::new(&parts);
    assert_eq!(state.able_to_search(), Able::NoInjured);
    assert_eq!(state.able_to_cast(), Able::NoInjured);
    assert_eq!(
        state.able_to_use_ranged(),
        Able::Yes,
        "ranged is a RANK 3 head penalty, not rank 2"
    );
    assert_eq!(state.able_to_sneak(), Able::Yes);
}

/// `head | Rank 3: prevents searching, spellcasting, ranged attacks`
#[test]
fn a_rank_three_head_adds_ranged() {
    let parts = injuries(&[("head", 3, 0)]);
    let state = Injuries::new(&parts);
    assert_eq!(state.able_to_search(), Able::NoCritical);
    assert_eq!(state.able_to_cast(), Able::NoCritical);
    assert_eq!(state.able_to_use_ranged(), Able::NoCritical);
    assert_eq!(state.able_to_sneak(), Able::Yes, "still not sneaking");
}

/// `nervous system | Rank 2: prevents spellcasting, searching` -- **and ranged.**
///
/// The wiki's table omits ranged here. `injured.rb:187` blocks it, and the
/// author settled which is right by testing it in the game:
///
/// > **AUTHOR, 2026-09-20:** *"I personally tested ranged for injured.rb."*
///
/// So the table is incomplete rather than contradictory, which is consistent
/// with it also omitting the cumulative rules and the per-side arm/hand merge.
/// A primary source's silence is not evidence (`inventory/10` §9b).
#[test]
fn a_rank_two_nervous_system_prevents_casting_and_searching() {
    let parts = injuries(&[("nsys", 2, 0)]);
    let state = Injuries::new(&parts);
    assert_eq!(state.able_to_cast(), Able::NoInjured);
    assert_eq!(state.able_to_search(), Able::NoInjured);
    assert_eq!(
        state.able_to_use_ranged(),
        Able::NoInjured,
        "`injured.rb:187` includes nsys in the rank-2 ranged block; the \
         wiki's table does not mention ranged for nerves. Ported as Lich has \
         it, per the author's call, and flagged in inventory/10."
    );
}

/// `eye | Rank 3: prevents searching, spellcasting`
#[test]
fn a_rank_three_eye_prevents_searching_and_spellcasting() {
    let parts = injuries(&[("leftEye", 3, 0)]);
    let state = Injuries::new(&parts);
    assert_eq!(state.able_to_search(), Able::NoCritical);
    assert_eq!(state.able_to_cast(), Able::NoCritical);
    assert_eq!(
        state.able_to_sneak(),
        Able::Yes,
        "a missing eye does not stop sneaking"
    );
}

/// `leg | Rank 2: prevents sneaking`
#[test]
fn a_rank_two_leg_prevents_sneaking_only() {
    let parts = injuries(&[("leftLeg", 2, 0)]);
    let state = Injuries::new(&parts);
    assert_eq!(state.able_to_sneak(), Able::NoInjured);
    assert_eq!(state.able_to_cast(), Able::Yes);
    assert_eq!(state.able_to_search(), Able::Yes);
    assert_eq!(state.able_to_use_ranged(), Able::Yes);
}

/// A rank-3 leg blocks sneaking beyond Sigil's help.
///
/// `injured.rb:143` calls these "NOT critical" while giving them exactly
/// critical behaviour. **AUTHOR: "Lich quirk -- flag it."** Ported as
/// [`Able::NoCritical`], because that is what it does.
#[test]
fn a_rank_three_leg_is_beyond_sigil() {
    let parts = injuries(&[("rightFoot", 3, 0)]);
    assert_eq!(
        Injuries::new(&parts).with_sigil(true).able_to_sneak(),
        Able::NoCritical
    );
}

/// `arm and/or hand | Rank 2: prevents ranged attacks`
#[test]
fn a_rank_two_arm_prevents_ranged() {
    let parts = injuries(&[("leftArm", 2, 0)]);
    let state = Injuries::new(&parts);
    assert_eq!(state.able_to_use_ranged(), Able::NoInjured);
    assert_eq!(
        state.able_to_cast(),
        Able::Yes,
        "casting does NOT block: one side at rank 2 totals 2, under the          cumulative threshold of 3. The wiki agrees -- a rank-2 arm prevents          ranged attacks only. This assertion said NoInjured, guessed from          reading the Ruby rather than the table."
    );
    assert_eq!(state.able_to_sneak(), Able::Yes);
    assert_eq!(state.able_to_search(), Able::Yes);
}

/// `arm and/or hand | Rank 3: prevents spellcasting, ranged attacks`
#[test]
fn a_rank_three_arm_prevents_casting_and_ranged() {
    let parts = injuries(&[("leftArm", 3, 0)]);
    let state = Injuries::new(&parts);
    assert_eq!(state.able_to_cast(), Able::NoCritical);
    assert_eq!(state.able_to_use_ranged(), Able::NoCritical);
    assert_eq!(state.able_to_search(), Able::Yes);
    assert_eq!(state.able_to_sneak(), Able::Yes);
}

/// **The parser retains a scar when a new wound arrives.**
///
/// The defect this port found: `apply_injury_image` wrote `scar: 0` on every
/// `Injury<n>`, so a known scar was erased the moment the part took a fresh
/// wound. Lich sets the wound alone and leaves the scar
/// (`xmlparser.rb:811-815`), which is what makes `R1W & R2S` representable.
#[test]
fn a_new_wound_does_not_erase_a_known_scar() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = cena_model::GameState::default();

    // A rank-2 scar, then a rank-1 wound over it.
    for frame in parser
        .push_bytes(b"<dialogData id='injuries'><image id=\"head\" name=\"Scar2\"/></dialogData>\n")
    {
        state.apply(&frame);
    }
    assert_eq!(
        state.character.injuries.get("head").map(|i| i.scar),
        Some(2),
        "guard: the scar was recorded"
    );

    for frame in parser.push_bytes(
        b"<dialogData id='injuries'><image id=\"head\" name=\"Injury1\"/></dialogData>\n",
    ) {
        state.apply(&frame);
    }

    let head = state.character.injuries.get("head").copied();
    assert_eq!(head.map(|i| i.wound), Some(1));
    assert_eq!(
        head.map(|i| i.scar),
        Some(2),
        "a wound image is not evidence the scar healed"
    );
    assert_eq!(
        Injuries::new(&state.character.injuries).effective_rank("head"),
        2
    );
}

/// A scar image clears the wound over it.
///
/// The other half of Lich's asymmetry: a scar is only visible once no wound
/// covers it, so a `Scar<n>` image does mean the wound is gone.
#[test]
fn a_scar_image_clears_the_wound() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = cena_model::GameState::default();
    for frame in parser.push_bytes(
        b"<dialogData id='injuries'><image id=\"head\" name=\"Injury2\"/></dialogData>\n",
    ) {
        state.apply(&frame);
    }
    assert_eq!(
        state.character.injuries.get("head").map(|i| i.wound),
        Some(2),
        "guard"
    );

    for frame in parser
        .push_bytes(b"<dialogData id='injuries'><image id=\"head\" name=\"Scar2\"/></dialogData>\n")
    {
        state.apply(&frame);
    }

    let head = state.character.injuries.get("head").copied();
    assert_eq!(head.map(|i| i.wound), Some(0));
    assert_eq!(head.map(|i| i.scar), Some(2));
}
