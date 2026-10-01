//! The hands and the boxes (`plan/61` step 4c), as eloot works them: a
//! hand freed before a step that needs one and given back after
//! (`free_hand`, `return_hands`, `eloot.lic:3816-3944`), a creature that
//! hands its loot over (`search`, `:5709-5765`), and a box phased with 704
//! once it is in a bag (`box_phase`, `:2975-2984`).

use cena_behavior::loot::{Left, LootProfile, Memory, Outcome, Planner, Step};
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{Frame, GameState, Injury, Link, LinkKind, RoomItem, Run, Runs};

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

fn profile() -> LootProfile {
    LootProfile {
        take: ["gem", "box", "magic"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ..LootProfile::default()
    }
}

/// A stow list of a gem sack and a default backpack, empty hands, and these
/// things on the floor.
fn state(floor: &[RoomItem]) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.containers.apply(&ContainerEvent::StowListBegins);
    for (slot, id, noun) in [
        (StowSlot::Gem, "901", "sack"),
        (StowSlot::Default, "902", "backpack"),
    ] {
        state.containers.apply(&ContainerEvent::StowSet {
            slot,
            item: ItemRef {
                id: id.to_owned(),
                noun: noun.to_owned(),
                text: noun.to_owned(),
            },
        });
    }
    right(&mut state, None);
    left(&mut state, None);
    state.room.objects = floor.to_vec();
    state
}

/// What the right hand holds, `(id, noun, name)`, or nothing.
fn right(state: &mut GameState, held: Option<(&str, &str, &str)>) {
    state.apply(&match held {
        Some((id, noun, name)) => Frame::RightHand {
            item: name.to_owned(),
            link: Some(link(id, noun, name)),
        },
        None => Frame::RightHand {
            item: "Empty".to_owned(),
            link: None,
        },
    });
}

/// What the left hand holds, or nothing.
fn left(state: &mut GameState, held: Option<(&str, &str, &str)>) {
    state.apply(&match held {
        Some((id, noun, name)) => Frame::LeftHand {
            item: name.to_owned(),
            link: Some(link(id, noun, name)),
        },
        None => Frame::LeftHand {
            item: "Empty".to_owned(),
            link: None,
        },
    });
}

const SWORD: (&str, &str, &str) = ("11", "broadsword", "steel broadsword");
const TORCH: (&str, &str, &str) = ("12", "torch", "pine torch");

#[test]
fn both_hands_full_the_right_is_stored_and_given_back() {
    let mut state = state(&[item("1", "emerald", "uncut emerald")]);
    right(&mut state, Some(SWORD));
    left(&mut state, Some(TORCH));
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    assert_eq!(plan.next(&state), Step::Hand("store right".to_owned()));
    right(&mut state, None);
    assert_eq!(
        plan.next(&state),
        Step::LootRoom,
        "then what wanted the hand"
    );
    plan.outcome(&Outcome::Stored);
    state.room.objects.clear();
    assert_eq!(
        plan.next(&state),
        Step::Hand("get #11".to_owned()),
        "the sword back before the visit ends"
    );
    right(&mut state, Some(SWORD));
    assert_eq!(plan.next(&state), Step::Done(Left::Nothing));
}

#[test]
fn favor_left_frees_the_left_and_drags_it_back_into_it() {
    let mut state = state(&[item("1", "emerald", "uncut emerald")]);
    right(&mut state, Some(SWORD));
    left(&mut state, Some(TORCH));
    let mut p = profile();
    p.favor_left = true;
    let mut plan = Planner::new(p, Memory::default(), &[]);
    assert_eq!(
        plan.next(&state),
        Step::Drag {
            item: "12".to_owned(),
            bag: "902".to_owned()
        },
        "not an armament: into its bag"
    );
    plan.outcome(&Outcome::Stored);
    left(&mut state, None);
    assert_eq!(plan.next(&state), Step::LootRoom);
    state.room.objects.clear();
    assert_eq!(plan.next(&state), Step::Hand("_drag #12 left".to_owned()));
}

#[test]
fn a_right_arm_too_hurt_to_use_frees_the_left() {
    let mut state = state(&[item("1", "emerald", "uncut emerald")]);
    right(&mut state, Some(SWORD));
    left(&mut state, Some(SWORD_TWO));
    state
        .character
        .injuries
        .insert("rightArm".to_owned(), Injury { wound: 0, scar: 3 });
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    assert_eq!(plan.next(&state), Step::Hand("store left".to_owned()));
}

const SWORD_TWO: (&str, &str, &str) = ("13", "dagger", "steel dagger");

#[test]
fn neither_hand_fit_to_use_is_a_reason_to_rest() {
    let mut state = state(&[item("1", "emerald", "uncut emerald")]);
    for part in ["rightHand", "leftArm"] {
        state
            .character
            .injuries
            .insert(part.to_owned(), Injury { wound: 3, scar: 0 });
    }
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    assert_eq!(plan.next(&state), Step::Done(Left::NoHand));
}

#[test]
fn a_free_hand_frees_nothing() {
    let mut state = state(&[item("1", "emerald", "uncut emerald")]);
    right(&mut state, Some(SWORD));
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    assert_eq!(plan.next(&state), Step::LootRoom);
    state.room.objects.clear();
    assert_eq!(plan.next(&state), Step::Done(Left::Nothing));
}

/// The box being emptied is never the thing put away to free a hand.
#[test]
fn the_hand_holding_the_box_is_never_the_one_freed() {
    let mut state = state(&[]);
    right(&mut state, Some(("40", "coffer", "iron coffer")));
    left(&mut state, Some(SWORD));
    let mut plan = Planner::for_box(profile(), Memory::default(), "40", None);
    assert_eq!(plan.next(&state), Step::Open("40".to_owned()));
    assert_eq!(plan.next(&state), Step::LookIn("40".to_owned()));
    state.apply(&Frame::Container {
        id: "40".to_owned(),
        title: Some("Coffer".to_owned()),
        target: None,
        attrs: Vec::new(),
    });
    inside(&mut state, "40", "41", "emerald", "uncut emerald");
    assert_eq!(plan.next(&state), Step::Hand("store left".to_owned()));
}

/// One `<inv>` line of a container's contents.
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

/// A dead creature in the room, as the wire states it.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn dead(state: &mut GameState, id: i64, noun: &str, name: &str) {
    let mut run = Run {
        text: name.to_owned(),
        style: Default::default(),
        link: Some(link(&id.to_string(), noun, name)),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: vec![run] },
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

#[test]
fn a_bramble_hands_its_loot_to_the_left_which_is_freed_first() {
    let mut state = state(&[]);
    dead(&mut state, 41, "shrub", "ice-coated shrub");
    right(&mut state, Some(SWORD));
    left(&mut state, Some(("14", "shield", "kite shield")));
    let mut plan = Planner::new(profile(), Memory::default(), &[41]);
    assert_eq!(
        plan.next(&state),
        Step::Hand("store left".to_owned()),
        "the left, though the right is the one eloot would free"
    );
    left(&mut state, None);
    assert_eq!(plan.next(&state), Step::Search(41));
    plan.outcome(&Outcome::Searched);
    left(&mut state, Some(("77", "blossom", "frost blossom")));
    assert_eq!(
        plan.next(&state),
        Step::Drag {
            item: "77".to_owned(),
            bag: "902".to_owned()
        },
        "what it handed over, into its bag"
    );
    plan.outcome(&Outcome::Stored);
    left(&mut state, None);
    assert_eq!(plan.next(&state), Step::Hand("get #14".to_owned()));
}

#[test]
fn an_ordinary_corpse_is_searched_with_full_hands() {
    let mut state = state(&[]);
    dead(&mut state, 41, "kobold", "kobold");
    right(&mut state, Some(SWORD));
    left(&mut state, Some(TORCH));
    let mut plan = Planner::new(profile(), Memory::default(), &[41]);
    assert_eq!(plan.next(&state), Step::Search(41));
}

/// Phase in the character's spell list.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
fn knows_phase(state: &mut GameState) {
    state.known_spells.begin();
    state.known_spells.read_line(&Runs {
        runs: vec![Run {
            text: "Phase".to_owned(),
            style: Default::default(),
            link: Some(link("x", "704", "Phase")),
            inner_link: None,
        }],
    });
}

#[test]
fn a_box_dragged_into_a_bag_is_phased_and_cast_again_when_hindered() {
    let mut state = state(&[item("4", "coffer", "battered iron coffer")]);
    knows_phase(&mut state);
    let mut p = profile();
    p.phase_boxes = true;
    let mut plan = Planner::new(p, Memory::default(), &[]);
    assert_eq!(
        plan.next(&state),
        Step::Drag {
            item: "4".to_owned(),
            bag: "902".to_owned()
        }
    );
    plan.outcome(&Outcome::Stored);
    state.room.objects.clear();
    assert_eq!(plan.next(&state), Step::Cast("prepare 704".to_owned()));
    assert_eq!(plan.next(&state), Step::Cast("cast at #4".to_owned()));
    plan.outcome(&Outcome::Hindered);
    assert_eq!(plan.next(&state), Step::Cast("prepare 704".to_owned()));
    assert_eq!(plan.next(&state), Step::Cast("cast at #4".to_owned()));
    assert_eq!(plan.next(&state), Step::Done(Left::Nothing), "it went off");
}

#[test]
fn a_box_is_not_phased_on_a_disk_nor_of_mithril_nor_unknown_spell() {
    let cases: [(&str, bool, bool); 3] = [
        ("battered iron coffer", true, false),
        ("mithril coffer", false, true),
        ("battered iron coffer", false, false),
    ];
    for (name, disk, knows) in cases {
        let mut floor = vec![item("4", "coffer", name)];
        if disk {
            floor.push(item("77", "disk", "Ashryn disk"));
        }
        let mut state = state(&floor);
        state.character.name = Some("Ashryn".to_owned());
        if knows || disk {
            knows_phase(&mut state);
        }
        let mut p = profile();
        p.phase_boxes = true;
        p.disk = disk;
        let mut plan = Planner::new(p, Memory::default(), &[]);
        let step = plan.next(&state);
        assert!(matches!(step, Step::Drag { .. }), "{name}: {step:?}");
        plan.outcome(&Outcome::Stored);
        state.room.objects.retain(|thing| thing.id != "4");
        let after = plan.next(&state);
        assert!(
            !matches!(after, Step::Cast(_)),
            "{name}, disk {disk}, knows {knows}: {after:?}"
        );
    }
}

#[test]
fn the_importer_carries_favor_left() {
    let profile = cena_behavior::loot::import("---\n:favor_left: true\n").map(|brought| {
        (
            brought.profile.favor_left,
            brought.profile.town.contains_key("favor_left"),
        )
    });
    assert_eq!(profile, Ok((true, false)));
}
