//! Wound and scar accessors, and the three composites that are not what their
//! names suggest.
//!
//! Ports `lib/gemstone/wounds.rb` and `lib/gemstone/scars.rb`. The composites
//! get membership tests rather than result tests, because their surprise is
//! *which parts they cover* -- `limbs` excludes feet, `torso` includes the
//! eyes -- and a test that only checked a maximum would pass on the obvious
//! groupings too.

use std::collections::BTreeMap;

use cena_model::{Body, Injury, Track};

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

/// The wire carries **sixteen** parts; the wiki's table lists fourteen.
///
/// It omits `leftFoot` and `rightFoot`, which `wounds.rb:24-25` names and
/// `injured.rb`'s sneaking rule reads. The same incompleteness as its ranged
/// row (`inventory/10` §9b).
#[test]
fn the_wire_has_sixteen_parts_including_the_feet() {
    assert_eq!(cena_model::state::character::body::ALL_PARTS.len(), 16);
    for foot in ["leftFoot", "rightFoot"] {
        assert!(
            cena_model::state::character::body::ALL_PARTS.contains(&foot),
            "{foot} is on the wire though the wiki's table omits it"
        );
    }
}

/// One part, on either track.
#[test]
fn a_part_reads_its_own_wound_and_scar() {
    let parts = injuries(&[("head", 1, 2)]);
    let body = Body::new(&parts);
    assert_eq!(body.rank("head", Track::Wound), 1);
    assert_eq!(body.rank("head", Track::Scar), 2);
}

/// An unmentioned part is zero, not an error.
///
/// `apply_injury_image` removes a part that heals rather than storing a zero,
/// so absence and "no injury" are the same fact here.
#[test]
fn an_unmentioned_part_is_whole() {
    let parts = injuries(&[("head", 2, 0)]);
    let body = Body::new(&parts);
    assert_eq!(body.rank("leftFoot", Track::Wound), 0);
    assert_eq!(body.rank("leftFoot", Track::Scar), 0);
}

/// **The composites take a MAXIMUM, not a sum.**
#[test]
fn a_composite_takes_the_worst_not_the_total() {
    let parts = injuries(&[
        ("leftArm", 1, 0),
        ("rightArm", 1, 0),
        ("leftHand", 1, 0),
        ("rightHand", 1, 0),
    ]);
    assert_eq!(
        Body::new(&parts).arms(Track::Wound),
        1,
        "four rank-1 wounds give 1, not 4"
    );
}

/// **`arms` is four parts: both arms AND both hands.**
#[test]
fn arms_includes_the_hands() {
    let parts = injuries(&[("rightHand", 3, 0)]);
    assert_eq!(
        Body::new(&parts).arms(Track::Wound),
        3,
        "a hand injury shows up in `arms`"
    );
}

/// **`limbs` excludes the feet**, though the wire has them.
///
/// Load-bearing rather than an oversight: `eherbs.lic:3222` reads
/// `Scars.limbs == 3` as "severed limb", and a severed foot is not one for
/// that purpose.
#[test]
fn limbs_excludes_the_feet() {
    let parts = injuries(&[("leftFoot", 3, 0)]);
    assert_eq!(
        Body::new(&parts).limbs(Track::Wound),
        0,
        "a foot is not a limb here"
    );

    let parts = injuries(&[("leftLeg", 3, 0)]);
    assert_eq!(Body::new(&parts).limbs(Track::Wound), 3, "...but a leg is");
}

/// **`torso` includes both eyes**, which are not anatomically torso.
#[test]
fn torso_includes_the_eyes() {
    let parts = injuries(&[("rightEye", 2, 0)]);
    assert_eq!(Body::new(&parts).torso(Track::Wound), 2);

    let parts = injuries(&[("head", 3, 0)]);
    assert_eq!(
        Body::new(&parts).torso(Track::Wound),
        0,
        "...but the head is not in it"
    );
}

/// `all` lists every part, including the whole ones.
///
/// Unlike `Character::injuries`, which holds only hurt parts so `is_empty`
/// answers "unhurt" without a scan.
#[test]
fn all_lists_every_part_not_only_the_hurt_ones() {
    let parts = injuries(&[("head", 2, 0)]);
    let all = Body::new(&parts).all(Track::Wound);
    assert_eq!(all.len(), 16, "a complete picture, not a sparse one");
    assert_eq!(all.get("head").copied(), Some(2));
    assert_eq!(all.get("leftFoot").copied(), Some(0));
}

/// The two tracks are read independently.
#[test]
fn the_tracks_do_not_bleed_into_each_other() {
    let parts = injuries(&[("leftArm", 0, 2)]);
    let body = Body::new(&parts);
    assert_eq!(body.arms(Track::Wound), 0, "no wound");
    assert_eq!(body.arms(Track::Scar), 2, "but a scar");
    assert!(!body.any(Track::Wound));
    assert!(body.any(Track::Scar));
}

/// **A severed limb is a rank-3 limb SCAR**, `eherbs.lic:3222`.
///
/// The wiki: rank-3 arm scar is *"a missing right arm"*. A rank-3 *wound* is
/// *"a completely severed right arm"* -- so the two tracks describe the same
/// loss at different stages, and the herb check keys on the scar.
#[test]
fn a_severed_limb_is_a_rank_three_scar() {
    let parts = injuries(&[("leftArm", 0, 3)]);
    assert!(Body::new(&parts).has_severed_limb());

    let parts = injuries(&[("leftArm", 3, 0)]);
    assert!(
        !Body::new(&parts).has_severed_limb(),
        "a rank-3 WOUND is not yet the scar the herb check looks for"
    );

    let parts = injuries(&[("leftArm", 0, 2)]);
    assert!(
        !Body::new(&parts).has_severed_limb(),
        "rank 2 is mangled, not missing"
    );
}

/// **A missing eye reads the eyes directly**, not through `torso`.
///
/// Going through `torso` would match a rank-3 chest scar as a missing eye.
#[test]
fn a_missing_eye_does_not_match_a_chest_scar() {
    let parts = injuries(&[("rightEye", 0, 3)]);
    assert!(Body::new(&parts).has_missing_eye());

    let parts = injuries(&[("chest", 0, 3)]);
    assert!(
        !Body::new(&parts).has_missing_eye(),
        "a rank-3 chest scar is in `torso` but is not a missing eye"
    );
    assert_eq!(
        Body::new(&parts).torso(Track::Scar),
        3,
        "guard: it IS in torso, which is why reading through torso would fail"
    );
}

/// Reads real wire bytes, so the part spellings are under test.
///
/// A typo in `ALL_PARTS` -- `leftEye` for `lefteye` -- would make that part
/// permanently read zero, and every hand-written test above would still pass.
#[test]
fn the_part_names_match_the_wire() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = cena_model::GameState::default();
    for frame in parser.push_bytes(
        b"<dialogData id='injuries'>\
          <image id=\"leftEye\" name=\"Injury1\"/>\
          <image id=\"rightHand\" name=\"Injury2\"/>\
          <image id=\"nsys\" name=\"Scar2\"/>\
          <image id=\"leftFoot\" name=\"Injury3\"/>\
          </dialogData>\n",
    ) {
        state.apply(&frame);
    }

    let body = Body::new(&state.character.injuries);
    assert_eq!(body.rank("leftEye", Track::Wound), 1);
    assert_eq!(body.rank("rightHand", Track::Wound), 2);
    assert_eq!(body.rank("nsys", Track::Scar), 2);
    assert_eq!(
        body.rank("leftFoot", Track::Wound),
        3,
        "the feet are real parts on the wire"
    );

    assert_eq!(body.arms(Track::Wound), 2, "rightHand is in `arms`");
    assert_eq!(
        body.limbs(Track::Wound),
        2,
        "...and the foot is not in `limbs`"
    );
    assert_eq!(body.torso(Track::Wound), 1, "leftEye is in `torso`");
}
