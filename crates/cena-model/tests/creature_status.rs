//! `<crtrStatus>`, classified from real wire traffic.
//!
//! The frames below are folded through the real parser rather than hand-built,
//! and the attribute vocabulary is taken from the committed fixture
//! `crates/cena-protocol/tests/fixtures/creature_status.xml`, which was cut
//! from live traffic. MEASURED there:
//!
//! ```text
//! 12 exist=   7 inferior=   2 prone=   2 hostile=
//!  2 flying=  1 stunned=    1 rooted=  1 immobile=   1 dead=
//! ```
//!
//! **Every one of those is in Lich's two flag tables**, including `immobile`,
//! which Lich renames to `immobilized` on the way in.

use cena_model::state::creature::status::{Classification, CreatureStatus, Status};
use cena_protocol::{Frame, Parser};

/// Classify every `<crtrStatus>` in some wire bytes.
fn classify(wire: &[u8]) -> Vec<CreatureStatus> {
    let mut parser = Parser::new();
    parser
        .push_bytes(wire)
        .into_iter()
        .filter_map(|frame| match frame {
            Frame::CreatureStatus { id, attrs } => Some(CreatureStatus::from_attrs(
                &id,
                attrs.iter().map(|(k, v)| (k.as_str(), v.as_str())),
            )),
            _ => None,
        })
        .collect()
}

/// A real frame from the fixture classifies.
#[test]
fn a_real_frame_classifies() {
    let statuses = classify(b"<crtrStatus exist=\"1094219\" flying=\"1\" inferior=\"1\"/>\n");
    assert_eq!(statuses.len(), 1);
    let status = &statuses[0];

    assert_eq!(status.id, "1094219");
    assert!(status.is(Status::Flying));
    assert!(status.is_classified(Classification::Inferior));
    assert!(
        status.unknown.is_empty(),
        "every attribute should be known: {:?}",
        status.unknown
    );
}

/// **`immobile` on the wire becomes `Immobilized`.**
///
/// `creature_base.rb:113` maps it, because Lich's message parser produces the
/// longer word and the two detections have to reconcile into one status.
/// MEASURED: the fixture carries `immobile` and never `immobilized`.
#[test]
fn the_wire_says_immobile_and_the_model_says_immobilized() {
    let statuses = classify(b"<crtrStatus exist=\"1\" immobile=\"1\"/>\n");
    assert!(statuses[0].is(Status::Immobilized));

    assert_eq!(Status::Immobilized.wire_name(), "immobile");
    assert_eq!(Status::Immobilized.as_str(), "immobilized");
    assert_eq!(Status::parse("immobile"), Some(Status::Immobilized));
    assert_eq!(
        Status::parse("immobilized"),
        None,
        "the canonical spelling is not what the wire sends"
    );

    // `calmed` -> `calm` is the other rename.
    assert_eq!(Status::Calm.wire_name(), "calmed");
    assert_eq!(Status::Calm.as_str(), "calm");
}

/// Every status and classification round-trips through its wire name.
#[test]
fn every_flag_round_trips() {
    for status in Status::ALL {
        assert_eq!(Status::parse(status.wire_name()), Some(status));
    }
    for class in Classification::ALL {
        assert_eq!(Classification::parse(class.wire_name()), Some(class));
    }
    assert_eq!(Status::ALL.len(), 13, "13 transient statuses");
    assert_eq!(Classification::ALL.len(), 11, "11 classifications");
}

/// **Two classifications are camel-cased on the wire.**
///
/// `AscensionBoss` and `MiniBoss`, where every other flag is lowercase.
/// Matching is case-sensitive so a normalising lowercase here would silently
/// stop recognising the two flags that mark the hardest creatures in the game.
#[test]
fn the_camel_cased_flags_are_matched_exactly() {
    let statuses = classify(b"<crtrStatus exist=\"1\" MiniBoss=\"1\" AscensionBoss=\"1\"/>\n");
    assert!(statuses[0].is_classified(Classification::MiniBoss));
    assert!(statuses[0].is_classified(Classification::AscensionBoss));
    assert!(statuses[0].unknown.is_empty());

    assert_eq!(Classification::parse("miniboss"), None, "case matters");
    assert_eq!(
        Classification::parse("MiniBoss"),
        Some(Classification::MiniBoss)
    );
}

/// **Statuses and classifications are separate vocabularies.**
///
/// Lich keeps them in two hashes read through different predicates, and the
/// distinction is what makes a target check legible: `stunned` is worth
/// attacking, `dead` is not.
#[test]
fn a_status_is_not_a_classification() {
    let statuses = classify(b"<crtrStatus exist=\"1\" stunned=\"1\" hostile=\"1\"/>\n");
    let status = &statuses[0];

    assert!(status.is(Status::Stunned));
    assert!(status.is_classified(Classification::Hostile));

    assert_eq!(status.statuses.len(), 1, "one transient status");
    assert_eq!(status.classifications.len(), 1, "one classification");
    assert_eq!(Status::parse("hostile"), None);
    assert_eq!(Classification::parse("stunned"), None);
}

/// **`stunned="0"` is a statement, not silence.**
///
/// A `<crtrStatus>` is a delta. "The game said it is not stunned" and "the
/// game did not mention stunned" are different facts, and a consumer
/// accumulating across frames must not read the second as the first --
/// `plan/12` §5.2 applied to combat.
#[test]
fn a_cleared_flag_is_stated_rather_than_absent() {
    let statuses = classify(b"<crtrStatus exist=\"1\" stunned=\"0\"/>\n");
    let status = &statuses[0];

    assert!(!status.is(Status::Stunned), "it is not stunned");
    assert_eq!(
        status.stated(Status::Stunned),
        Some(false),
        "and the game SAID so"
    );
    assert_eq!(
        status.stated(Status::Webbed),
        None,
        "whereas webbed was never mentioned"
    );
    assert!(
        !status.is(Status::Webbed),
        "is() collapses the two, which is why stated() exists"
    );
}

/// **A muckle stops the creature acting; a posture does not.**
///
/// Lich's `muckled?` (`creature.rb:658`). A prone creature still attacks, so
/// `Prone` is not a muckle -- and neither are `Kneeling`, `Sitting`, `Flying`,
/// `Hovering` or `Hidden`.
#[test]
fn only_a_real_muckle_stops_a_creature() {
    for wire_name in [
        "stunned", "webbed", "sleeping", "immobile", "rooted", "calmed",
    ] {
        let wire = format!("<crtrStatus exist=\"1\" {wire_name}=\"1\"/>\n");
        assert!(
            classify(wire.as_bytes())[0].is_muckled(),
            "{wire_name} should be a muckle"
        );
    }
    for wire_name in [
        "prone", "kneeling", "sitting", "flying", "hovering", "hidden",
    ] {
        let wire = format!("<crtrStatus exist=\"1\" {wire_name}=\"1\"/>\n");
        assert!(
            !classify(wire.as_bytes())[0].is_muckled(),
            "{wire_name} is a posture, not a muckle -- it still attacks"
        );
    }

    // A muckle that was explicitly CLEARED does not count.
    let cleared = classify(b"<crtrStatus exist=\"1\" stunned=\"0\"/>\n");
    assert!(!cleared[0].is_muckled());
}

/// Attackable means hostile and not dead.
#[test]
fn a_dead_creature_is_not_attackable() {
    let hostile = classify(b"<crtrStatus exist=\"1\" hostile=\"1\"/>\n");
    assert!(hostile[0].is_attackable());

    let dead = classify(b"<crtrStatus exist=\"1\" hostile=\"1\" dead=\"1\"/>\n");
    assert!(
        !dead[0].is_attackable(),
        "a corpse is hostile and not worth attacking"
    );

    let neutral = classify(b"<crtrStatus exist=\"1\" inferior=\"1\"/>\n");
    assert!(
        !neutral[0].is_attackable(),
        "nothing said it was hostile, so do not attack it"
    );
}

/// **An unrecognised flag is kept and reported, not dropped.**
///
/// Rule 2.2. A silently ignored attribute is how a client stops noticing that
/// creatures can now be, say, `feared` -- the table was cut at one moment and
/// the game keeps moving.
#[test]
fn an_unknown_flag_is_kept() {
    let statuses = classify(b"<crtrStatus exist=\"1\" stunned=\"1\" feared=\"1\"/>\n");
    let status = &statuses[0];

    assert!(status.is(Status::Stunned), "the known flag still works");
    assert_eq!(
        status.unknown.get("feared").map(String::as_str),
        Some("1"),
        "and the unknown one survives: {:?}",
        status.unknown
    );
    assert!(
        !status.unknown.contains_key("exist"),
        "`exist` is the id, not a flag"
    );
}

/// A frame with no flags is a real statement about a creature with no flags.
#[test]
fn a_bare_frame_classifies_to_nothing() {
    let statuses = classify(b"<crtrStatus exist=\"546518\"/>\n");
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].id, "546518");
    assert!(statuses[0].statuses.is_empty());
    assert!(statuses[0].classifications.is_empty());
    assert!(statuses[0].unknown.is_empty());
    assert!(!statuses[0].is_muckled());
    assert!(!statuses[0].is_attackable());
}

/// The fixture's own traffic classifies with nothing unknown.
///
/// The strongest check here: real bytes, every attribute recognised. A flag
/// the tables missed would show up as a non-empty `unknown`.
#[test]
fn the_committed_fixture_classifies_cleanly() {
    // `include_str!`, not `include_bytes!`: the latter is banned because the
    // arch-test harness reads sources as UTF-8, so a non-UTF-8 payload would
    // be a file no scan can see (`file_rules.rs:386`).
    let wire = include_str!("../../cena-protocol/tests/fixtures/creature_status.xml");
    let statuses = classify(wire.as_bytes());

    assert!(
        statuses.len() >= 12,
        "the fixture carries 12 crtrStatus frames, got {}",
        statuses.len()
    );
    for status in &statuses {
        assert!(!status.id.is_empty(), "every frame carries an exist id");
        assert!(
            status.unknown.is_empty(),
            "unrecognised flag in real traffic: {:?}",
            status.unknown
        );
    }
    assert!(
        statuses
            .iter()
            .any(|s| s.is_classified(Classification::Inferior)),
        "`inferior` appears 7 times in the fixture"
    );
}
