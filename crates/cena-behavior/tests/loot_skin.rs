//! Skinning in the planner (`plan/31` §5): eloot's order, the skinner in
//! and out of hand, and what a reply teaches.

use cena_behavior::loot::{LootProfile, Memory, Outcome, Planner, Skin, Step};
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs};

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
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
fn run(id: &str, noun: &str, text: &str, bold: bool) -> Run {
    let mut run = Run {
        text: text.to_owned(),
        style: Default::default(),
        link: Some(link(id, noun, text)),
        inner_link: None,
    };
    if bold {
        run.style.bold_depth = 1;
    }
    run
}

/// A dead creature in the room, in the registry by name.
fn corpse(state: &mut GameState, id: i64, noun: &str, name: &str) {
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![run(&id.to_string(), noun, name, true)],
        },
    });
    state.apply(&Frame::CreatureStatus {
        id: id.to_string(),
        attrs: vec![
            ("exist".to_owned(), id.to_string()),
            ("hostile".to_owned(), "1".to_owned()),
            ("dead".to_owned(), "1".to_owned()),
        ],
    });
}

/// A backpack holding a dagger and a sheath, a right hand, a stow list.
fn state(right_hand: Option<(&str, &str, &str)>) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.apply(&Frame::Container {
        id: "902".to_owned(),
        title: Some("My Backpack".to_owned()),
        target: None,
    });
    for (id, noun, text) in [
        ("77", "dagger", "curved skinning dagger"),
        ("78", "sheath", "leather skinning sheath"),
        ("79", "cudgel", "iron cudgel"),
    ] {
        state.apply(&Frame::ContainerItem {
            container_id: "902".to_owned(),
            content: Runs {
                runs: vec![run(id, noun, text, false)],
            },
        });
    }
    state.containers.apply(&ContainerEvent::StowListBegins);
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Default,
        item: ItemRef {
            id: "902".to_owned(),
            noun: "backpack".to_owned(),
            text: "backpack".to_owned(),
        },
    });
    if let Some((id, noun, text)) = right_hand {
        state.apply(&Frame::RightHand {
            item: text.to_owned(),
            link: Some(link(id, noun, text)),
        });
    }
    state
}

fn profile(skin: Skin) -> LootProfile {
    LootProfile {
        skin,
        ..LootProfile::default()
    }
}

fn skinning() -> Skin {
    Skin {
        enable: true,
        weapon: "dagger".to_owned(),
        sheath: "sheath".to_owned(),
        ..Skin::default()
    }
}

#[test]
fn skinning_comes_before_the_search_with_the_dagger_fetched_and_sheathed() {
    let mut state = state(None);
    corpse(&mut state, 41, "troll", "cave troll");
    let mut plan = Planner::new(profile(skinning()), Memory::default(), &[41]);
    assert_eq!(
        plan.next(&state),
        Step::Wield("77".to_owned()),
        "the dagger from the backpack"
    );
    // The game puts it in the right hand.
    state.apply(&Frame::RightHand {
        item: "curved skinning dagger".to_owned(),
        link: Some(link("77", "dagger", "curved skinning dagger")),
    });
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 41,
            hand: "right"
        }
    );
    plan.outcome_in(&Outcome::Skinned, &state);
    assert_eq!(
        plan.next(&state),
        Step::Drag {
            item: "77".to_owned(),
            bag: "78".to_owned()
        },
        "back into its sheath"
    );
    plan.outcome_in(&Outcome::Stored, &state);
    assert_eq!(plan.next(&state), Step::Search(41), "then the search");
}

#[test]
fn a_skinner_already_in_hand_is_neither_fetched_nor_put_away() {
    let mut state = state(Some(("77", "dagger", "curved skinning dagger")));
    corpse(&mut state, 41, "troll", "cave troll");
    let mut plan = Planner::new(profile(skinning()), Memory::default(), &[41]);
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 41,
            hand: "right"
        }
    );
    plan.outcome_in(&Outcome::Skinned, &state);
    assert_eq!(plan.next(&state), Step::Search(41));
}

#[test]
fn with_no_weapon_named_the_right_hands_item_serves() {
    let mut state = state(Some(("55", "falchion", "steel falchion")));
    corpse(&mut state, 41, "troll", "cave troll");
    let mut skin = skinning();
    skin.weapon = String::new();
    let mut plan = Planner::new(profile(skin), Memory::default(), &[41]);
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 41,
            hand: "right"
        }
    );
}

#[test]
fn kneel_and_the_sigil_come_first_and_the_stand_after() {
    let mut state = state(Some(("77", "dagger", "curved skinning dagger")));
    corpse(&mut state, 41, "troll", "cave troll");
    let mut skin = skinning();
    skin.kneel = true;
    skin.resolve = true;
    let mut plan = Planner::new(profile(skin), Memory::default(), &[41]);
    assert_eq!(plan.next(&state), Step::Kneel);
    plan.outcome_in(&Outcome::Kneeled, &state);
    assert_eq!(plan.next(&state), Step::Cast("incant 9704".to_owned()));
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 41,
            hand: "right"
        }
    );
    plan.outcome_in(&Outcome::Skinned, &state);
    assert_eq!(plan.next(&state), Step::Stand);
    assert_eq!(plan.next(&state), Step::Search(41));
}

#[test]
fn you_cannot_skin_teaches_the_creature_and_moves_on() {
    let mut state = state(Some(("77", "dagger", "curved skinning dagger")));
    corpse(&mut state, 41, "troll", "cave troll");
    corpse(&mut state, 42, "golem", "stone golem");
    let mut plan = Planner::new(profile(skinning()), Memory::default(), &[41, 42]);
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 41,
            hand: "right"
        }
    );
    plan.outcome_in(&Outcome::CannotSkin, &state);
    assert!(plan.memory().unskinnable.contains("cave troll"));
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 42,
            hand: "right"
        }
    );
    // Next visit, a cave troll is not even tried.
    let mut later = Planner::new(profile(skinning()), plan.memory().clone(), &[41]);
    assert_eq!(later.next(&state), Step::Search(41));
}

#[test]
fn a_refusal_ends_skinning_for_the_visit() {
    let mut state = state(Some(("77", "dagger", "curved skinning dagger")));
    corpse(&mut state, 41, "troll", "cave troll");
    corpse(&mut state, 42, "golem", "stone golem");
    let mut plan = Planner::new(profile(skinning()), Memory::default(), &[41, 42]);
    plan.next(&state);
    plan.outcome_in(&Outcome::SkinNotAllowed, &state);
    assert_eq!(
        plan.next(&state),
        Step::Search(41),
        "no more skinning; the searches"
    );
}

#[test]
fn blunt_creatures_wait_for_a_blunt_weapon_and_the_excluded_are_left() {
    let mut state = state(Some(("77", "dagger", "curved skinning dagger")));
    corpse(&mut state, 41, "krynch", "grizzled krynch");
    corpse(&mut state, 42, "troll", "cave troll");
    corpse(&mut state, 43, "wraith", "ghostly wraith");
    // No blunt weapon: the krynch is left; the wraith is never skinned.
    let mut plan = Planner::new(profile(skinning()), Memory::default(), &[41, 42, 43]);
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 42,
            hand: "right"
        }
    );
    plan.outcome_in(&Outcome::Skinned, &state);
    assert_eq!(plan.next(&state), Step::Search(41));

    // With one named: the edged group first, then the cudgel for the krynch.
    let mut skin = skinning();
    skin.weapon_blunt = "cudgel".to_owned();
    let mut plan = Planner::new(profile(skin), Memory::default(), &[41, 42]);
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 42,
            hand: "right"
        }
    );
    plan.outcome_in(&Outcome::Skinned, &state);
    assert_eq!(plan.next(&state), Step::Wield("79".to_owned()));
    state.apply(&Frame::LeftHand {
        item: "iron cudgel".to_owned(),
        link: Some(link("79", "cudgel", "iron cudgel")),
    });
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 41,
            hand: "left"
        }
    );
    plan.outcome_in(&Outcome::Skinned, &state);
    assert_eq!(
        plan.next(&state),
        Step::Drag {
            item: "79".to_owned(),
            bag: "902".to_owned()
        },
        "no blunt sheath named: the default bag"
    );
}

#[test]
fn a_gem_that_breaks_out_is_stowed_at_once() {
    let mut state = state(Some(("77", "dagger", "curved skinning dagger")));
    corpse(&mut state, 41, "golem", "crystal golem");
    let mut plan = Planner::new(profile(skinning()), Memory::default(), &[41]);
    plan.next(&state);
    state.apply(&Frame::LeftHand {
        item: "uncut emerald".to_owned(),
        link: Some(link("88", "emerald", "uncut emerald")),
    });
    plan.outcome_in(&Outcome::BrokeThrough, &state);
    assert_eq!(plan.next(&state), Step::StowGem("88".to_owned()));
    plan.outcome_in(&Outcome::Stored, &state);
    // The corpse is not skinned again for the gem: it counts as done only
    // when the game says so.
    assert_eq!(
        plan.next(&state),
        Step::Skin {
            corpse: 41,
            hand: "right"
        }
    );
}

#[test]
fn skinning_off_means_no_skinning_steps() {
    let mut state = state(Some(("77", "dagger", "curved skinning dagger")));
    corpse(&mut state, 41, "troll", "cave troll");
    let mut plan = Planner::new(LootProfile::default(), Memory::default(), &[41]);
    assert_eq!(plan.next(&state), Step::Search(41));
}
