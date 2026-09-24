//! The loot planner, driven step by step with no game (`plan/31` §3):
//! eloot's order, the game's own sorter, and what a reply teaches.

use cena_behavior::loot::{Left, LootProfile, Memory, Outcome, Planner, Step};
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{Frame, GameState, RoomItem};

fn profile() -> LootProfile {
    LootProfile {
        take: ["gem", "box", "magic", "wand"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        defensive: true,
        ..LootProfile::default()
    }
}

fn item(id: &str, noun: &str, text: &str) -> RoomItem {
    RoomItem {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: text.to_owned(),
        before: None,
        after: None,
        status: None,
    }
}

fn bag(id: &str, noun: &str) -> ItemRef {
    ItemRef {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: noun.to_owned(),
    }
}

/// A state standing defensive, with a stow list of a gem sack and a
/// default backpack, and these things on the floor.
fn state(floor: &[RoomItem], stow_list: bool) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.character.stance = Some("defensive (100%)".to_owned());
    if stow_list {
        state.containers.apply(&ContainerEvent::StowListBegins);
        state.containers.apply(&ContainerEvent::StowSet {
            slot: StowSlot::Gem,
            item: bag("901", "sack"),
        });
        state.containers.apply(&ContainerEvent::StowSet {
            slot: StowSlot::Default,
            item: bag("902", "backpack"),
        });
    }
    state.room.objects = floor.to_vec();
    state
}

#[test]
fn the_stance_first_then_each_corpse_then_the_floor() {
    let mut state = state(&[item("1", "emerald", "uncut emerald")], true);
    state.character.stance = Some("offensive (0%)".to_owned());
    let mut plan = Planner::new(profile(), Memory::default(), &[41, 42]);
    assert_eq!(
        plan.next(&state),
        Step::Stance("stance defensive".to_owned())
    );
    state.character.stance = Some("defensive (100%)".to_owned());
    assert_eq!(plan.next(&state), Step::Search(41));
    plan.outcome(&Outcome::Searched);
    assert_eq!(plan.next(&state), Step::Search(42));
    plan.outcome(&Outcome::Searched);
    assert_eq!(
        plan.next(&state),
        Step::LootRoom,
        "everything on the floor is wanted"
    );
    plan.outcome(&Outcome::Stored);
    // The game restates the floor without the emerald.
    state.room.objects.clear();
    assert_eq!(plan.next(&state), Step::Done(Left::Nothing));
}

#[test]
fn a_corpse_that_cannot_be_searched_is_tried_three_times_then_left() {
    let state = state(&[], true);
    let mut plan = Planner::new(profile(), Memory::default(), &[41]);
    for _ in 0..3 {
        assert_eq!(plan.next(&state), Step::Search(41));
        plan.outcome(&Outcome::NotInCondition);
    }
    assert_eq!(plan.next(&state), Step::Done(Left::Nothing));
}

#[test]
fn a_mixed_floor_goes_item_by_item_with_the_games_verb_where_it_stows_itself() {
    let floor = [
        item("1", "emerald", "uncut emerald"),
        item("2", "acantha", "acantha leaf"),
        item("3", "whatsit", "peculiar glowing whatsit"),
    ];
    let state = state(&floor, true);
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    // The leaf is not wanted, so no `loot room`: a gem goes by `loot #1`,
    // a thing of no kind by a drag into the default bag.
    assert_eq!(plan.next(&state), Step::LootItem("1".to_owned()));
    plan.outcome(&Outcome::Stored);
    let mut without_gem = state.clone();
    without_gem.room.objects.remove(0);
    assert_eq!(
        plan.next(&without_gem),
        Step::Drag {
            item: "3".to_owned(),
            bag: "902".to_owned()
        }
    );
}

#[test]
fn the_stow_list_is_asked_for_before_anything_is_dragged() {
    let floor = [
        item("2", "acantha", "acantha leaf"),
        item("3", "whatsit", "peculiar glowing whatsit"),
    ];
    let state = state(&floor, false);
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    assert_eq!(plan.next(&state), Step::Ask("stow list"));
}

#[test]
fn a_full_bag_is_remembered_and_the_next_one_tried_then_rest() {
    let floor = [
        item("2", "acantha", "acantha leaf"),
        item("3", "whatsit", "peculiar glowing whatsit"),
    ];
    let state = state(&floor, true);
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    assert_eq!(
        plan.next(&state),
        Step::Drag {
            item: "3".to_owned(),
            bag: "902".to_owned()
        }
    );
    plan.outcome(&Outcome::WontFit);
    assert!(
        plan.memory().full.contains("902"),
        "the backpack is full for the rest of the hunt"
    );
    assert_eq!(
        plan.next(&state),
        Step::Done(Left::BagsFull),
        "no other bag and no disk: too much loot, rest"
    );
}

#[test]
fn the_disk_is_the_last_bag_when_the_profile_uses_it() {
    let floor = [
        item("2", "acantha", "acantha leaf"),
        item("3", "whatsit", "peculiar glowing whatsit"),
        item("77", "disk", "Ashryn disk"),
    ];
    let mut state = state(&floor, true);
    state.character.name = Some("Ashryn".to_owned());
    let mut p = profile();
    p.disk = true;
    let mut memory = Memory::default();
    memory.full.insert("902".to_owned());
    let mut plan = Planner::new(p, memory, &[]);
    assert_eq!(
        plan.next(&state),
        Step::Drag {
            item: "3".to_owned(),
            bag: "77".to_owned()
        }
    );
}

#[test]
fn a_bag_that_closes_itself_is_opened_first_and_a_crumbly_name_is_learned() {
    let floor = [
        item("2", "acantha", "acantha leaf"),
        item("3", "whatsit", "peculiar glowing whatsit"),
    ];
    let state = state(&floor, true);
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    let drag = Step::Drag {
        item: "3".to_owned(),
        bag: "902".to_owned(),
    };
    assert_eq!(plan.next(&state), drag);
    plan.outcome(&Outcome::Closed);
    assert_eq!(plan.next(&state), Step::Open("902".to_owned()));
    assert_eq!(plan.next(&state), drag);
    plan.outcome(&Outcome::Crumbled);
    plan.learn(&Outcome::Crumbled, "peculiar glowing whatsit");
    assert!(plan.memory().crumbly.contains("peculiar glowing whatsit"));
    assert_eq!(
        plan.next(&state),
        Step::Done(Left::Nothing),
        "left where it lies"
    );
}

#[test]
fn a_box_left_in_hand_is_a_reason_to_rest() {
    let mut state = state(&[], true);
    state.apply(&Frame::RightHand {
        item: "enruned steel coffer".to_owned(),
        link: Some(cena_session::Link {
            kind: cena_session::LinkKind::Exist {
                id: "55".to_owned(),
                noun: "coffer".to_owned(),
            },
            text: "enruned steel coffer".to_owned(),
            coord: None,
        }),
    });
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    assert_eq!(plan.next(&state), Step::Done(Left::BoxInHand));
}
