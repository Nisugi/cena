//! The healer (`plan/36` Stage 2): eherbs' order of what to treat, the herb
//! found and eaten or drunk, what has no herb named, and the herbs put back.

use std::collections::BTreeSet;

use cena_behavior::heal::{HealProfile, Healed, Healer, Mode, Reply, Step, next_kind};
use cena_session::herbs::{Area, HerbKind, Hurt, Severity};
use cena_session::{Amount, Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs};

fn link(id: &str, noun: &str, text: &str) -> Link {
    Link {
        kind: LinkKind::Exist {
            id: id.to_owned(),
            noun: noun.to_owned(),
        },
        text: text.to_owned(),
        coord: None,
    }
}

#[expect(
    clippy::default_trait_access,
    reason = "the image's attributes type is not re-exported for behaviors"
)]
fn hurt(state: &mut GameState, part: &str, rank: &str) {
    state.apply(&Frame::InjuryImage {
        id: part.to_owned(),
        name: rank.to_owned(),
        dialog: Some("injuries".to_owned()),
        attrs: Default::default(),
    });
}

fn health(state: &mut GameState, current: i32, max: i32) {
    let percent = u32::try_from(current * 100 / max).unwrap_or(0);
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "health".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent,
        text: format!("health {current}/{max}"),
        amount: Some(Amount { current, max }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
fn inside(state: &mut GameState, container: &str, id: &str, noun: &str, text: &str) {
    state.apply(&Frame::ContainerItem {
        container_id: container.to_owned(),
        content: Runs {
            runs: vec![Run {
                text: text.to_owned(),
                style: Default::default(),
                link: Some(link(id, noun, text)),
                inner_link: None,
            }],
        },
    });
}

fn hand(state: &mut GameState, right: bool, item: Option<(&str, &str, &str)>) {
    let (text, link) = match item {
        Some((id, noun, text)) => (text.to_owned(), Some(link(id, noun, text))),
        None => ("Empty".to_owned(), None),
    };
    state.apply(&if right {
        Frame::RightHand { item: text, link }
    } else {
        Frame::LeftHand { item: text, link }
    });
}

/// A character at full health with a herb pouch (700) holding these.
fn setup(herbs: &[(&str, &str, &str)]) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    health(&mut state, 100, 100);
    state.apply(&Frame::Container {
        id: "700".to_owned(),
        title: Some("Herb Pouch".to_owned()),
        target: None,
    });
    for (id, noun, text) in herbs {
        inside(&mut state, "700", id, noun, text);
    }
    hand(&mut state, true, None);
    hand(&mut state, false, None);
    state
}

fn profile() -> HealProfile {
    HealProfile {
        container: "herb pouch".to_owned(),
        ..HealProfile::default()
    }
}

const fn wound(severity: Severity, area: Area) -> HerbKind {
    HerbKind::Injury {
        severity,
        area,
        hurt: Hurt::Wound,
    }
}

#[test]
fn eherbs_order_blood_then_major_wounds_then_minor_then_blood() {
    let mut skipped = BTreeSet::new();
    let mode = Mode::default();
    let mut state = setup(&[]);
    assert_eq!(next_kind(&state, mode, &skipped), None, "whole");
    hurt(&mut state, "leftArm", "Injury1");
    hurt(&mut state, "nsys", "Injury2");
    assert_eq!(
        next_kind(&state, mode, &skipped),
        Some(wound(Severity::Major, Area::Nerve)),
        "a major wound anywhere before a minor one"
    );
    health(&mut state, 40, 100);
    assert_eq!(
        next_kind(&state, mode, &skipped),
        Some(HerbKind::Blood),
        "under half health: blood first"
    );
    health(&mut state, 90, 100);
    skipped.insert("nerves");
    assert_eq!(
        next_kind(&state, mode, &skipped),
        Some(wound(Severity::Minor, Area::Limb))
    );
    skipped.insert("limbs");
    assert_eq!(
        next_kind(&state, mode, &skipped),
        Some(HerbKind::Blood),
        "then blood, ten short"
    );
}

#[test]
fn a_minor_scar_is_left_when_the_profile_skips_scars_and_the_eye_first() {
    let skipped = BTreeSet::new();
    let mut state = setup(&[]);
    hurt(&mut state, "chest", "Scar1");
    let scar = HerbKind::Injury {
        severity: Severity::Minor,
        area: Area::Organ,
        hurt: Hurt::Scar,
    };
    assert_eq!(next_kind(&state, Mode::default(), &skipped), Some(scar));
    let skipping = Mode {
        skip_scars: true,
        ..Mode::default()
    };
    assert_eq!(next_kind(&state, skipping, &skipped), None);
    hurt(&mut state, "rightEye", "Scar3");
    assert_eq!(
        next_kind(&state, skipping, &skipped),
        Some(HerbKind::MissingEye)
    );
}

#[test]
fn spellcast_treats_only_what_stops_a_cast() {
    let skipped = BTreeSet::new();
    let mut state = setup(&[]);
    hurt(&mut state, "leftLeg", "Injury2");
    let caster = Mode {
        spellcast: true,
        ..Mode::default()
    };
    assert_eq!(
        next_kind(&state, caster, &skipped),
        None,
        "a leg is no matter"
    );
    hurt(&mut state, "head", "Injury1");
    assert_eq!(
        next_kind(&state, caster, &skipped),
        Some(wound(Severity::Minor, Area::Head))
    );
    let archer = Mode {
        ranged: true,
        ..Mode::default()
    };
    assert_eq!(
        next_kind(&state, archer, &skipped),
        None,
        "nor is the head to an archer"
    );
}

#[test]
fn a_wound_is_healed_from_the_pouch_and_the_herb_put_back() {
    let mut state = setup(&[
        ("81", "leaf", "some ambrominas leaf"),
        ("82", "moss", "some ephlox moss"),
    ]);
    hurt(&mut state, "leftArm", "Injury1");
    let mut healer = Healer::new(profile(), false, false);
    assert_eq!(healer.next(&state), Step::Fetch("81".to_owned()));
    hand(
        &mut state,
        true,
        Some(("81", "leaf", "some ambrominas leaf")),
    );
    healer.outcome(&[]);
    assert_eq!(healer.next(&state), Step::Eat("leaf".to_owned()));
    healer.outcome(&[Reply::Used]);
    // Healed: the leaf goes back into the pouch.
    hurt(&mut state, "leftArm", "leftArm");
    assert_eq!(
        healer.next(&state),
        Step::Stow {
            item: "81".to_owned(),
            bag: "700".to_owned()
        }
    );
    hand(&mut state, true, None);
    assert_eq!(
        healer.next(&state),
        Step::Done(Healed::Done {
            missing: Vec::new()
        })
    );
}

#[test]
fn a_kind_with_no_herb_is_skipped_and_named_and_a_potion_is_drunk() {
    let mut state = setup(&[("83", "potion", "bolmara potion")]);
    hurt(&mut state, "nsys", "Injury2");
    hurt(&mut state, "head", "Injury1");
    let mut healer = Healer::new(profile(), false, false);
    // The nerve wound is major: first, and the pouch has bolmara for it.
    assert_eq!(healer.next(&state), Step::Fetch("83".to_owned()));
    hand(&mut state, true, Some(("83", "potion", "bolmara potion")));
    assert_eq!(healer.next(&state), Step::Drink("potion".to_owned()));
    hurt(&mut state, "nsys", "nsys");
    // The minor head wound has no herb here: skipped, named, and the potion
    // back in the pouch.
    assert_eq!(
        healer.next(&state),
        Step::Stow {
            item: "83".to_owned(),
            bag: "700".to_owned()
        }
    );
    hand(&mut state, true, None);
    assert_eq!(
        healer.next(&state),
        Step::Done(Healed::Done {
            missing: vec![wound(Severity::Minor, Area::Head)]
        })
    );
}

#[test]
fn yabathilium_first_for_blood_when_asked_and_edible_before_drinkable() {
    let mut state = setup(&[
        ("84", "potion", "green mushroom potion"),
        ("85", "leaf", "some acantha leaf"),
        ("86", "fruit", "yabathilium fruit"),
    ]);
    health(&mut state, 60, 100);
    let mut plain = Healer::new(profile(), false, false);
    assert_eq!(
        plain.next(&state),
        Step::Fetch("85".to_owned()),
        "edible before drinkable"
    );
    let yaba = HealProfile {
        yabathilium: true,
        ..profile()
    };
    let mut healer = Healer::new(yaba, false, false);
    assert_eq!(healer.next(&state), Step::Fetch("86".to_owned()));
    let potions = HealProfile {
        potions: true,
        ..profile()
    };
    let mut drinker = Healer::new(potions, false, false);
    assert_eq!(drinker.next(&state), Step::Fetch("84".to_owned()));
}

#[test]
fn no_container_and_a_public_herb_eaten_from_enough() {
    let mut state = setup(&[]);
    hurt(&mut state, "leftArm", "Injury1");
    let lost = HealProfile {
        container: "saddlebag".to_owned(),
        ..HealProfile::default()
    };
    let mut healer = Healer::new(lost, false, false);
    assert_eq!(healer.next(&state), Step::Look("my saddlebag".to_owned()));
    assert_eq!(healer.next(&state), Step::Done(Healed::NoContainer));

    let mut state = setup(&[("81", "leaf", "some ambrominas leaf")]);
    hurt(&mut state, "leftArm", "Injury1");
    hand(
        &mut state,
        true,
        Some(("81", "leaf", "some ambrominas leaf")),
    );
    let mut healer = Healer::new(profile(), false, false);
    assert_eq!(healer.next(&state), Step::Eat("leaf".to_owned()));
    healer.outcome(&[Reply::LeaveSome]);
    assert_eq!(healer.next(&state), Step::Done(Healed::Refused));
}
