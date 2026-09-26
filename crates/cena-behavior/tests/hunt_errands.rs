//! bigshot's errand verbs: `wield` and `briar` on the worn list, `wandolier`
//! on the reserve, `nudgeweapons` out of the room and back, and `throw` with
//! its hands emptied and filled (`hunt/verbs/gated.rs`, `hunt/follow.rs`).
//! Every reply line is synthetic, from the handler's own pattern in
//! `bigshot.lic`.

mod gated_support;

use cena_behavior::hunt::{Hunt, Profile};
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs};
use gated_support::{fighting, first, hunt, kobold, linked, tick, ticks, with};

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

/// `kobold(1_000)`, wearing these `(id, noun, name)`.
fn wearing(items: &[(&str, &str, &str)]) -> GameState {
    let mut state = kobold(1_000);
    state.apply(&Frame::StreamPush { id: "inv".into() });
    inv_line(&mut state, &[("Your worn items are:", None)]);
    for (id, noun, name) in items {
        inv_line(&mut state, &[("  ", None), (name, Some((id, noun)))]);
    }
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state
}

#[test]
fn wield_takes_off_what_is_worn_and_gets_the_rest() {
    let worn = wearing(&[("9", "shield", "a kite shield")]);
    assert_eq!(
        worn.worn.wears("shield"),
        Some(true),
        "the input reaches the check"
    );
    let mut h = hunt(&["wield shield left"]).unwrap();
    assert_eq!(ticks(&mut h, &worn, 2), ["store left", "remove my shield"]);
    let mut h = hunt(&["wield sword"]).unwrap();
    assert_eq!(ticks(&mut h, &worn, 2), ["store right", "get my sword"]);
    let mut holding = kobold(1_000);
    holding.apply(&Frame::RightHand {
        item: "steel sword".to_owned(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "5".to_owned(),
                noun: "sword".to_owned(),
            },
            text: "steel sword".to_owned(),
            coord: None,
        }),
    });
    assert_eq!(first("wield sword", &holding), "wait 1", "already wielded");
}

#[test]
fn briar_measures_each_weapon_and_raises_a_full_one() {
    let mut state = wearing(&[("7", "gauntlet", "a briar gauntlet")]);
    state.apply(&Frame::RightHand {
        item: "briar gauntlet".to_owned(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "8".to_owned(),
                noun: "gauntlet".to_owned(),
            },
            text: "briar gauntlet".to_owned(),
            coord: None,
        }),
    });
    let mut h = hunt(&["briar gauntlet", "kick"]).unwrap();
    assert_eq!(tick(&mut h, &state), "measure #8", "the hand first");
    h.replied(
        ["You gaze intently at the gauntlet and judge its thorns to be about 100 percent."],
        Some(1_000),
    );
    assert_eq!(ticks(&mut h, &state, 2), ["raise #8", "measure #7"]);
    h.replied(
        ["You gaze intently at the gauntlet and judge its thorns to be about 40 percent."],
        Some(1_001),
    );
    assert_eq!(tick(&mut h, &state), "kick", "not full: not raised");
    // The same gauntlet in hand, so the input reaches the Briar check.
    let up = with(state, "Active Spells", "9105", "Briar", 1_060);
    assert_eq!(first("briar gauntlet", &up), "wait 1", "Briar is up");
}

/// `kobold(1_000)` with the wandolier's reserve listed as these
/// `(id, noun, name)`.
fn reserving(items: &[(&str, &str, &str)]) -> GameState {
    let mut state = kobold(1_000);
    state.apply(&Frame::StreamPush {
        id: "reserve".into(),
    });
    for (id, noun, name) in items {
        state.apply(&Frame::Text(cena_session::TextFrame {
            content: (*name).to_owned(),
            stream: "reserve".to_owned(),
            style: kobold_style().style,
            link: Some(Link {
                kind: LinkKind::Exist {
                    id: (*id).to_owned(),
                    noun: (*noun).to_owned(),
                },
                text: (*name).to_owned(),
                coord: None,
            }),
            inner_link: None,
            ends_line: true,
        }));
    }
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state
}

#[expect(
    clippy::default_trait_access,
    reason = "the text's style type is not re-exported for behaviors"
)]
fn kobold_style() -> cena_session::TextFrame {
    cena_session::TextFrame {
        content: String::new(),
        stream: String::new(),
        style: Default::default(),
        link: None,
        inner_link: None,
        ends_line: false,
    }
}

fn wandolier(step: &str) -> Result<Hunt, String> {
    Ok(Hunt::new(
        Profile::parse(&format!(
            "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\n[wand]\nnames = [\"oaken wand\"]\nfresh = \"wandolier\"\n[routines]\na = [\"{step}\"]\n"
        ))?,
        1,
    ))
}

#[test]
fn wandolier_asks_for_the_reserve_once_then_waves_from_it() {
    let mut unknown = wandolier("wandolier").unwrap();
    assert_eq!(tick(&mut unknown, &kobold(1_000)), "reserve list");
    assert_eq!(
        tick(&mut unknown, &kobold(1_000)),
        "wait 1",
        "asked once, not every tick"
    );

    let stocked = reserving(&[("77", "wand", "oaken wand")]);
    assert_eq!(
        stocked.reserve.items().map(<[_]>::len),
        Some(1),
        "the input reaches the check"
    );
    let mut h = wandolier("wandolier").unwrap();
    assert_eq!(ticks(&mut h, &stocked, 2), ["stance offensive", "wave #77"]);
    let mut guarded = wandolier("wandolier guarded").unwrap();
    assert_eq!(
        ticks(&mut guarded, &stocked, 2),
        ["stance guarded", "wave #77"]
    );

    let empty = reserving(&[]);
    let mut h = wandolier("wandolier").unwrap();
    assert_eq!(tick(&mut h, &empty), "get oaken wand from my wandolier");
    h.replied(["Get what?"], Some(1_000));
    assert_eq!(tick(&mut h, &empty), "rub my wandolier");
}

#[test]
fn wandolier_reserves_a_wand_in_hand_unless_told_not_to() {
    let mut state = reserving(&[]);
    state.apply(&Frame::RightHand {
        item: "oaken wand".to_owned(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "78".to_owned(),
                noun: "wand".to_owned(),
            },
            text: "oaken wand".to_owned(),
            coord: None,
        }),
    });
    let mut h = wandolier("wandolier").unwrap();
    assert_eq!(
        ticks(&mut h, &state, 3),
        ["reserve #78", "stance offensive", "wave #78"]
    );
    let mut kept = wandolier("wandolier noreserve").unwrap();
    assert_eq!(
        ticks(&mut kept, &state, 2),
        ["stance offensive", "wave #78"]
    );
    kept.replied(["What were you referring to?"], Some(1_000));
    assert_eq!(tick(&mut kept, &state), "reserve list");
}

/// `kobold(1_000)` with exits `n` and `e`, and these `(id, noun, name)`
/// lying on the ground.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors"
)]
fn littered(items: &[(&str, &str, &str)]) -> GameState {
    let mut state = kobold(1_000);
    state.apply(&Frame::Compass {
        directions: vec!["n".to_owned(), "e".to_owned()],
    });
    let mut runs = vec![linked("kobold")];
    runs.extend(items.iter().map(|(id, noun, name)| Run {
        text: (*name).to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: (*id).to_owned(),
                noun: (*noun).to_owned(),
            },
            text: (*name).to_owned(),
            coord: None,
        }),
        inner_link: None,
    }));
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs },
    });
    state
}

#[test]
fn nudgeweapons_carries_each_weapon_out_and_walks_back() {
    let state = littered(&[("7", "longsword", "a longsword"), ("8", "rock", "a rock")]);
    let mut h = hunt(&["nudgeweapons", "kick"]).unwrap();
    assert_eq!(
        ticks(&mut h, &state, 5),
        ["get #7", "n", "drop #7", "s", "kick"],
        "the sword out by the first exit and back; the rock left"
    );
    let bare = littered(&[("8", "rock", "a rock")]);
    let mut nothing = hunt(&["nudgeweapons"]).unwrap();
    assert_eq!(tick(&mut nothing, &bare), "wait 1", "no weapon here");
    let mut full = littered(&[("7", "longsword", "a longsword")]);
    for (item, frame) in [("shield", true), ("sword", false)] {
        let hand = cena_session::Frame::RightHand {
            item: item.to_owned(),
            link: None,
        };
        let left = cena_session::Frame::LeftHand {
            item: item.to_owned(),
            link: None,
        };
        full.apply(if frame { &left } else { &hand });
    }
    let mut h = hunt(&["nudgeweapons"]).unwrap();
    assert_eq!(
        ticks(&mut h, &full, 6),
        ["sheath", "get #7", "n", "drop #7", "s", "gird"],
        "both hands full: sheathed first, girded after"
    );
}

#[test]
fn a_nudge_goes_on_in_the_next_room_and_the_hunt_resumes_where_it_was() {
    let here = littered(&[("7", "longsword", "a longsword")]);
    let mut next_door = kobold(1_001);
    next_door.room.id = Some("11".to_owned());
    let mut h = hunt(&["nudgeweapons", "kick"]).unwrap();
    assert_eq!(ticks(&mut h, &here, 2), ["get #7", "n"]);
    assert_eq!(
        ticks(&mut h, &next_door, 2),
        ["drop #7", "s"],
        "a room change does not drop the errand"
    );
    assert_eq!(tick(&mut h, &here), "kick", "back, and hunting");
}

/// `kobold(1_000)` holding a steel sword (#5) and a kite shield (#6).
fn armed() -> GameState {
    let mut state = kobold(1_000);
    let held = |id: &str, noun: &str, name: &str| {
        Some(Link {
            kind: LinkKind::Exist {
                id: id.to_owned(),
                noun: noun.to_owned(),
            },
            text: name.to_owned(),
            coord: None,
        })
    };
    state.apply(&Frame::RightHand {
        item: "steel sword".to_owned(),
        link: held("5", "sword", "steel sword"),
    });
    state.apply(&Frame::LeftHand {
        item: "kite shield".to_owned(),
        link: held("6", "shield", "kite shield"),
    });
    state
}

#[test]
fn throw_empties_the_hands_first_and_takes_them_back_after() {
    let state = armed();
    let mut h = hunt(&["throw", "kick"]).unwrap();
    let lines = ticks(&mut h, &state, 3);
    assert_eq!(lines[2], "throw #42", "{lines:?}");
    h.replied(["You attempt to throw a kobold!"], Some(1_000));
    let back = ticks(&mut h, &state, 3);
    assert_eq!(
        (lines[..2].to_vec(), back),
        (
            vec!["store right".to_owned(), "store left".to_owned()],
            vec!["get #6".to_owned(), "get #5".to_owned(), "kick".to_owned()]
        ),
        "stored right then left, taken back left then right"
    );
}

#[test]
fn a_throw_refused_for_a_creature_gone_still_takes_the_hands_back() {
    let state = armed();
    let mut h = hunt(&["throw"]).unwrap();
    assert_eq!(ticks(&mut h, &state, 3)[2], "throw #42");
    // The gate refused it: the creature left. The driver says so, then
    // hands on whatever the game answered (nothing).
    h.target_gone();
    h.replied([], Some(1_000));
    let mut gone = fighting(1_001, "kobold", &[]);
    gone.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: Vec::new() },
    });
    assert_eq!(ticks(&mut h, &gone, 2), ["get #6", "get #5"]);
}
