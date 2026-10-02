//! A box emptied by the loot planner, and how it came out (`plan/61` step
//! 5b): only a box known empty is the caller's to throw out. One whose
//! contents were never listed is kept (`box_loot`, `eloot.lic:5096`); one
//! still holding what no bag would take is kept too, the bags full or the
//! thing a gold ingot too heavy for any, which says nothing of the bag
//! (`single_drag`, `eloot.lic:4141-4146`).

use cena_behavior::loot::{Emptied, LootProfile, Memory, Outcome, Planner, Step};
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs};

/// One `<inv>` line of a container's contents, as the wire states it.
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
                link: Some(Link {
                    kind: LinkKind::Exist {
                        id: id.to_owned(),
                        noun: noun.to_owned(),
                    },
                    text: text.to_owned(),
                    coord: None,
                }),
                inner_link: None,
            }],
        },
    });
}

fn bag(id: &str, noun: &str) -> ItemRef {
    ItemRef {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: noun.to_owned(),
    }
}

/// A profile that takes gems and valuables.
fn profile() -> LootProfile {
    LootProfile {
        take: ["gem", "valuable"].into_iter().map(str::to_owned).collect(),
        ..LootProfile::default()
    }
}

/// A gem sack and a default backpack on the stow list.
fn state() -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.containers.apply(&ContainerEvent::StowListBegins);
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Gem,
        item: bag("901", "sack"),
    });
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Default,
        item: bag("902", "backpack"),
    });
    state
}

/// The box `40` opened and looked in: the planner at the box's contents.
fn opened(plan: &mut Planner, state: &GameState) {
    assert_eq!(plan.next(state), Step::Open("40".to_owned()));
    assert_eq!(plan.next(state), Step::LookIn("40".to_owned()));
}

/// The box `40` as the game lists it after `look in`.
fn listed(state: &mut GameState, things: &[(&str, &str, &str)]) {
    state.apply(&Frame::Container {
        id: "40".to_owned(),
        title: Some("Coffer".to_owned()),
        target: None,
        attrs: Vec::new(),
    });
    for (id, noun, text) in things {
        inside(state, "40", id, noun, text);
    }
}

/// A `look in` the game never answered with a listing: whatever is in the
/// box is unseen, and it is not the caller's to throw out.
#[test]
fn a_box_never_listed_is_unseen_not_empty() {
    let state = state();
    let mut plan = Planner::for_box(profile(), Memory::default(), "40", None);
    opened(&mut plan, &state);
    assert!(matches!(plan.next(&state), Step::Done(_)));
    assert_eq!(plan.emptied(), Emptied::Unseen);

    let mut empty = state.clone();
    listed(&mut empty, &[]);
    let mut plan = Planner::for_box(profile(), Memory::default(), "40", None);
    opened(&mut plan, &empty);
    assert!(matches!(plan.next(&empty), Step::Done(_)));
    assert_eq!(plan.emptied(), Emptied::Out, "listed, and nothing in it");
}

/// Every bag that would take the gem is full: it stays in the box, and the
/// box is not emptied.
#[test]
fn a_gem_no_bag_takes_leaves_the_box_not_emptied() {
    let mut state = state();
    listed(&mut state, &[("42", "emerald", "uncut emerald")]);
    let memory = Memory {
        full: ["901", "902"].into_iter().map(str::to_owned).collect(),
        ..Memory::default()
    };
    let mut plan = Planner::for_box(profile(), memory, "40", None);
    opened(&mut plan, &state);
    assert!(matches!(plan.next(&state), Step::Done(_)));
    assert_eq!(plan.emptied(), Emptied::ThingsLeft);
}

/// A gold ingot that will not fit is too heavy for any bag, and says nothing
/// of the bag: the next thing still goes in it. The ingot stays in the box,
/// and the box is not emptied.
#[test]
fn a_gold_ingot_that_will_not_fit_marks_no_bag_full() {
    let mut state = state();
    listed(
        &mut state,
        &[
            ("43", "ingot", "bright gold ingot"),
            ("44", "nugget", "gold nugget"),
        ],
    );
    let mut plan = Planner::for_box(profile(), Memory::default(), "40", None);
    opened(&mut plan, &state);
    let into_the_pack = |item: &str| Step::Drag {
        item: item.to_owned(),
        bag: "902".to_owned(),
    };
    assert_eq!(plan.next(&state), into_the_pack("43"));
    plan.outcome_in(&Outcome::WontFit, &state);
    assert_eq!(
        plan.next(&state),
        into_the_pack("44"),
        "the backpack is not full: an ingot fits in none"
    );
    plan.outcome_in(&Outcome::Stored, &state);
    state.apply(&Frame::ClearContainer {
        id: "40".to_owned(),
    });
    inside(&mut state, "40", "43", "ingot", "bright gold ingot");
    assert!(matches!(plan.next(&state), Step::Done(_)));
    assert!(plan.memory().full.is_empty(), "{:?}", plan.memory().full);
    assert_eq!(plan.emptied(), Emptied::ThingsLeft);
}
