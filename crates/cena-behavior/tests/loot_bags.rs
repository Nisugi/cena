//! Where loot goes (`plan/61` step 4b), as eloot puts it: a box on a disk
//! before any bag, the character's own and then, by the profile, its
//! group's, and nothing but a box on a disk (`single_drag_box`,
//! `eloot.lic:4035-4066`); the overflow containers after the stow list's
//! bags; with `keep_closed`, a bag opened before it is used and closed again
//! before the visit ends.

use cena_behavior::loot::{Left, LootProfile, Memory, Outcome, Planner, Step};
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::group::{GroupEvent, Member};
use cena_session::{Frame, GameState, Link, LinkKind, RoomItem};

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

fn drag(item: &str, bag: &str) -> Step {
    Step::Drag {
        item: item.to_owned(),
        bag: bag.to_owned(),
    }
}

fn profile() -> LootProfile {
    LootProfile {
        take: ["gem", "box", "magic", "wand"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ..LootProfile::default()
    }
}

/// Ashryn, with a stow list of a gem sack and a default backpack, and these
/// things on the floor.
fn state(floor: &[RoomItem]) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.character.name = Some("Ashryn".to_owned());
    state.containers.apply(&ContainerEvent::StowListBegins);
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Gem,
        item: bag("901", "sack"),
    });
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Default,
        item: bag("902", "backpack"),
    });
    state.room.objects = floor.to_vec();
    state
}

/// One line of the `inv` stream: `pieces` of text, each linked to
/// `(id, noun)` when given.
#[expect(
    clippy::default_trait_access,
    reason = "the text's style type is not re-exported for behaviors"
)]
fn inv_line(state: &mut GameState, pieces: &[(&str, Option<(&str, &str)>)]) {
    let last = pieces.len().saturating_sub(1);
    for (at, (text, object)) in pieces.iter().enumerate() {
        state.apply(&Frame::Text(cena_session::TextFrame {
            content: (*text).to_owned(),
            stream: "inv".to_owned(),
            style: Default::default(),
            link: object.map(|(id, noun)| Link {
                kind: LinkKind::Exist {
                    id: id.to_owned(),
                    noun: noun.to_owned(),
                },
                text: (*text).to_owned(),
                coord: None,
            }),
            inner_link: None,
            ends_line: at == last,
        }));
    }
}

/// The character wearing these `(id, noun, name)`, as the `inv` stream
/// lists them.
fn wearing(state: &mut GameState, items: &[(&str, &str, &str)]) {
    state.apply(&Frame::StreamPush { id: "inv".into() });
    inv_line(state, &[("Your worn items are:", None)]);
    for (id, noun, name) in items {
        inv_line(state, &[("  a ", None), (name, Some((id, noun)))]);
    }
    state.apply(&Frame::Prompt {
        time: "1001".into(),
        text: ">".into(),
    });
}

#[test]
fn a_box_goes_on_the_disk_first_and_nothing_else_does() {
    let floor = [
        item("3", "whatsit", "peculiar glowing whatsit"),
        item("4", "coffer", "enruned steel coffer"),
        item("77", "disk", "fiery red Ashryn disk"),
    ];
    let mut state = state(&floor);
    let mut p = profile();
    p.disk = true;
    let mut plan = Planner::new(p, Memory::default(), &[]);
    assert_eq!(
        plan.next(&state),
        drag("4", "77"),
        "the box onto the disk, though the backpack has room"
    );
    plan.outcome(&Outcome::Stored);
    state.room.objects.remove(1);
    assert_eq!(plan.next(&state), drag("3", "902"), "the rest into a bag");
    plan.outcome(&Outcome::WontFit);
    assert_eq!(
        plan.next(&state),
        Step::Done(Left::BagsFull),
        "a full backpack sends the rest nowhere, never onto the disk"
    );
}

#[test]
fn a_box_goes_on_a_group_disk_after_its_own_and_never_on_a_strangers() {
    let floor = [
        item("4", "coffer", "enruned steel coffer"),
        item("76", "disk", "Vasstryke disk"),
        item("78", "coffret", "four-toned Dicate coffret"),
        item("77", "disk", "Ashryn disk"),
    ];
    let mut state = state(&floor);
    state.group.apply(
        &GroupEvent::Joined(Member {
            id: "-5".to_owned(),
            noun: "Dicate".to_owned(),
            text: "Dicate".to_owned(),
        }),
        None,
    );
    let mut p = profile();
    p.disk_group = true;
    let mut plan = Planner::new(p, Memory::default(), &[]);
    assert_eq!(plan.next(&state), drag("4", "77"), "its own disk first");
    plan.outcome(&Outcome::WontFit);
    assert_eq!(plan.next(&state), drag("4", "78"), "then the group's");
    plan.outcome(&Outcome::WontFit);
    assert_eq!(
        plan.next(&state),
        drag("4", "902"),
        "then a bag: a stranger's disk is never one"
    );
}

/// Someone's disk is never loot, though its noun is a box's: `rusty iron
/// Duffield coffer` is Duffield's (`gemstone/disk.rb:7`).
#[test]
fn someones_disk_is_never_taken_for_a_box() {
    let state = state(&[item("76", "coffer", "rusty iron Duffield coffer")]);
    let mut plan = Planner::new(profile(), Memory::default(), &[]);
    assert_eq!(plan.next(&state), Step::Done(Left::Nothing));
}

/// The overflow containers, after the stow list's bags and in the profile's
/// order, found by name among what is worn (`ensure_items`,
/// `eloot.lic:2230`).
#[test]
fn the_overflow_bags_take_what_the_stow_bags_cannot() {
    // The leaf is not wanted, so the floor goes item by item.
    let floor = [
        item("3", "whatsit", "peculiar glowing whatsit"),
        item("2", "acantha", "acantha leaf"),
    ];
    let mut state = state(&floor);
    wearing(
        &mut state,
        &[
            ("903", "satchel", "leather satchel"),
            ("904", "cloak", "hooded grey cloak"),
        ],
    );
    let mut p = profile();
    p.overflow = vec!["cloak".to_owned(), "Satchel".to_owned()];
    let mut plan = Planner::new(p, Memory::default(), &[]);
    assert_eq!(plan.next(&state), drag("3", "902"));
    plan.outcome(&Outcome::WontFit);
    assert_eq!(plan.next(&state), drag("3", "904"), "the profile's first");
    plan.outcome(&Outcome::WontFit);
    assert_eq!(plan.next(&state), drag("3", "903"), "then its second");
    plan.outcome(&Outcome::WontFit);
    assert_eq!(plan.next(&state), Step::Done(Left::BagsFull));
}

/// With `keep_closed`, a bag whose contents are not listed is opened before
/// anything goes in, `loot room` too (`open_loot_containers`,
/// `eloot.lic:3869-3889`), and closed again before the visit ends
/// (`eloot.lic:2409`).
#[test]
fn a_kept_closed_bag_is_opened_before_loot_room_and_closed_after() {
    let floor = [item("1", "emerald", "uncut emerald")];
    let mut state = state(&floor);
    let mut p = profile();
    p.keep_closed = true;
    let mut plan = Planner::new(p, Memory::default(), &[]);
    assert_eq!(plan.next(&state), Step::Open("901".to_owned()));
    assert_eq!(plan.next(&state), Step::LootRoom);
    plan.outcome(&Outcome::Stored);
    state.room.objects.clear();
    assert_eq!(plan.next(&state), Step::Close("901".to_owned()));
    assert_eq!(plan.next(&state), Step::Done(Left::Nothing));
}

/// A disk is never opened or closed for `keep_closed`: it has no lid.
#[test]
fn a_kept_closed_visit_leaves_the_disk_alone() {
    let floor = [
        item("4", "coffer", "enruned steel coffer"),
        item("1", "emerald", "uncut emerald"),
        item("77", "disk", "Ashryn disk"),
    ];
    let mut state = state(&floor);
    let mut p = profile();
    p.disk = true;
    p.keep_closed = true;
    let mut plan = Planner::new(p, Memory::default(), &[]);
    assert_eq!(plan.next(&state), drag("4", "77"));
    plan.outcome(&Outcome::Stored);
    state.room.objects.remove(0);
    assert_eq!(plan.next(&state), Step::Open("901".to_owned()));
    assert_eq!(plan.next(&state), Step::LootItem("1".to_owned()));
    plan.outcome(&Outcome::Stored);
    state.room.objects.remove(0);
    assert_eq!(plan.next(&state), Step::Close("901".to_owned()));
    assert_eq!(plan.next(&state), Step::Done(Left::Nothing));
}

#[test]
fn the_importer_splits_the_overflow_and_carries_the_bag_switches() {
    let yaml = "---\n:loot_types: gem\n:overflow_containers: cloak, leather satchel ,\n:use_disk_group: true\n:keep_closed: true\n";
    let imported = cena_behavior::loot::import(yaml);
    let profile = imported.map(|brought| brought.profile).unwrap_or_default();
    assert_eq!(profile.overflow, ["cloak", "leather satchel"]);
    assert!(profile.disk_group && profile.keep_closed);
    assert!(
        profile.track_full,
        "on unless the file says otherwise, as eloot's is"
    );
    let off = cena_behavior::loot::import("---\n:track_full_sacks: false\n")
        .map(|brought| brought.profile.track_full);
    assert_eq!(off, Ok(false));
}
