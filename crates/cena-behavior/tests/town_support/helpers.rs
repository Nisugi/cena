//! The helpers themselves; `mod.rs` wires them.

pub use cena_behavior::town::{Reply, Seller, Shop, Step, Town};
pub use cena_map::RoomId;
pub use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
pub use cena_session::{Appraiser, Buyer, Frame, GameState, Link, LinkKind, LootFact, Run, Runs};

pub fn link(id: &str, noun: &str, text: &str) -> Link {
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
pub fn inside(state: &mut GameState, container: &str, id: &str, noun: &str, text: &str) {
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

pub fn hand(state: &mut GameState, right: bool, item: Option<(&str, &str, &str)>) {
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

/// A gem sack (901) and a backpack (902) on the stow list, both declared
/// containers, with these items inside each; the hands empty.
pub fn setup(gems: &[(&str, &str, &str)], pack: &[(&str, &str, &str)]) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    for (id, title) in [("901", "My Sack"), ("902", "My Backpack")] {
        state.apply(&Frame::Container {
            id: id.to_owned(),
            title: Some(title.to_owned()),
            target: None,
        });
    }
    for (id, noun, text) in gems {
        inside(&mut state, "901", id, noun, text);
    }
    for (id, noun, text) in pack {
        inside(&mut state, "902", id, noun, text);
    }
    state.containers.apply(&ContainerEvent::StowListBegins);
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Gem,
        item: ItemRef {
            id: "901".to_owned(),
            noun: "sack".to_owned(),
            text: "sack".to_owned(),
        },
    });
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Default,
        item: ItemRef {
            id: "902".to_owned(),
            noun: "backpack".to_owned(),
            text: "backpack".to_owned(),
        },
    });
    hand(&mut state, true, None);
    hand(&mut state, false, None);
    state
}

pub fn town() -> Town {
    Town {
        sell_types: ["gem", "weapon", "clothing", "jewelry"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        containers: ["default", "gem"].into_iter().map(str::to_owned).collect(),
        appraise_types: vec!["jewelry".to_owned()],
        appraise_gemshop: 14_999,
        appraise_pawnshop: 34_999,
        keep_transmogs: true,
        ..Town::default()
    }
}

pub const HOME: RoomId = RoomId(20);
pub const GEMSHOP: RoomId = RoomId(31);
pub const PAWNSHOP: RoomId = RoomId(32);

pub fn nearest(tag: &str) -> Option<RoomId> {
    match tag {
        "gemshop" => Some(GEMSHOP),
        "pawnshop" => Some(PAWNSHOP),
        _ => None,
    }
}

pub fn sold(silvers: u64) -> LootFact {
    LootFact::Sold {
        item: None,
        silvers,
        to: Buyer::Pawn,
        note: None,
    }
}

pub fn appraised(value: u64) -> LootFact {
    LootFact::Appraised {
        item: None,
        value: Some(value),
        by: Appraiser::Shop,
    }
}

pub fn town_4b() -> Town {
    Town {
        sell_types: ["gem", "skin"].into_iter().map(str::to_owned).collect(),
        containers: ["default", "gem"].into_iter().map(str::to_owned).collect(),
        collectibles: true,
        gold_rings: true,
        ..Town::default()
    }
}

pub const FURRIER: RoomId = RoomId(33);
pub const COUNTER: RoomId = RoomId(34);
pub const CHRONOMAGE: RoomId = RoomId(35);
pub const BANK: RoomId = RoomId(36);

pub fn every(tag: &str) -> Option<RoomId> {
    match tag {
        "gemshop" => Some(GEMSHOP),
        "pawnshop" => Some(PAWNSHOP),
        "furrier" => Some(FURRIER),
        "collectible" => Some(COUNTER),
        "chronomage" => Some(CHRONOMAGE),
        "bank" => Some(BANK),
        _ => None,
    }
}

/// The room's objects: one unbolded link, a clerk.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
pub fn with_clerk(state: &mut GameState) {
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![Run {
                text: "clerk".to_owned(),
                style: Default::default(),
                link: Some(link("77", "clerk", "clerk")),
                inner_link: None,
            }],
        },
    });
}

pub const POOL: RoomId = RoomId(37);

/// A bold NPC in the room: the pool's worker.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the bold depth matters"
)]
pub fn with_worker(state: &mut GameState) {
    let mut run = Run {
        text: "worker".to_owned(),
        style: Default::default(),
        link: Some(link("88", "worker", "worker")),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: vec![run] },
    });
}

pub fn pool_town() -> Town {
    Town {
        pool: true,
        pool_tip: 300,
        ..Town::default()
    }
}

pub fn at_pool(tag: &str) -> Option<RoomId> {
    (tag == "locksmith pool").then_some(POOL)
}

pub fn a_box() -> cena_session::containers::ItemRef {
    cena_session::containers::ItemRef {
        id: "5".to_owned(),
        noun: "coffer".to_owned(),
        text: "iron coffer".to_owned(),
    }
}
