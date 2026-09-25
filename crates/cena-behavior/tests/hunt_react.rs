//! The hunt's answers to incidents (`hunt/react.rs`).

use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::containers::ItemRef;
use cena_session::incident::{Disarm, Incident};
use cena_session::{Frame, GameState, Runs};

const PROFILE: &str = r#"
targets = [{ any = true, routine = "a" }]

[routines]
a = ["attack"]
"#;

fn standing(second: u32) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state.room.id = Some("10".to_owned());
    state.status.set("standing", true);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state
}

fn here() -> Here<'static> {
    Here {
        room: Some(RoomId(10)),
        exits: &[],
    }
}

fn send(line: &str) -> Said {
    Said::Send {
        line: line.to_owned(),
        target: None,
    }
}

fn knocked() -> Incident {
    Incident::Disarmed {
        how: Disarm::Knocked,
        weapon: Some(ItemRef {
            id: "5".to_owned(),
            noun: "broadsword".to_owned(),
            text: "steel broadsword".to_owned(),
        }),
    }
}

#[test]
fn a_knocked_weapon_is_knelt_for_and_recovered_and_the_kneel_is_kept() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let mut state = standing(1_000);
    hunt.incidents(&[knocked()]);
    assert_eq!(hunt.tick(&state, here(), Some(1_000)), send("kneel"));
    state.status.set("standing", false);
    state.status.set("kneeling", true);
    assert_eq!(
        hunt.tick(&state, here(), Some(1_001)),
        send("recover item"),
        "kneeling for a recovery is not stood up from"
    );
    hunt.replied(["You spy a steel broadsword and recover it!"], Some(1_002));
    assert_eq!(
        hunt.tick(&state, here(), Some(1_003)),
        send("stand"),
        "recovered: stand again"
    );
}

#[test]
fn ten_failed_searches_end_the_hunt() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let mut state = standing(1_000);
    state.status.set("standing", false);
    state.status.set("kneeling", true);
    hunt.incidents(&[knocked()]);
    for second in 0..10 {
        assert_eq!(
            hunt.tick(&state, here(), Some(1_000 + second)),
            send("recover item")
        );
    }
    assert_eq!(
        hunt.tick(&state, here(), Some(1_020)),
        Said::Done(Ending::Disarmed)
    );
}

#[test]
fn a_weapon_reaction_is_taken_unless_switched_off() {
    let state = standing(1_000);
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    hunt.incidents(&[Incident::WeaponReaction("TACKLE #77".to_owned())]);
    assert_eq!(
        hunt.tick(&state, here(), Some(1_000)),
        send("weapon TACKLE #77")
    );

    let off = format!("{PROFILE}\n[react]\nweapon_reaction = false\n");
    let mut hunt = Hunt::new(Profile::parse(&off).unwrap(), 1);
    hunt.incidents(&[Incident::WeaponReaction("TACKLE #77".to_owned())]);
    assert_ne!(
        hunt.tick(&state, here(), Some(1_000)),
        send("weapon TACKLE #77")
    );
}

#[test]
fn swallowed_the_hunt_fights_its_way_out() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let mut state = standing(1_000);
    state.room.title = Some("The Belly of the Beast".to_owned());
    assert_eq!(hunt.tick(&state, here(), Some(1_000)), send("attack wall"));
    assert_eq!(hunt.tick(&state, here(), Some(1_001)), send("attack wall"));
    state.room.title = Some("Ooze, Innards".to_owned());
    assert_eq!(hunt.tick(&state, here(), Some(1_002)), send("kill organ"));
    state.room.title = Some("Snowy Ridge".to_owned());
    assert_ne!(hunt.tick(&state, here(), Some(1_003)), send("kill organ"));
}

/// One kobold, `#42`, targeted, with this routine.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors"
)]
fn kobold(state: &mut GameState) {
    let mut run = cena_session::Run {
        text: "kobold".to_owned(),
        style: Default::default(),
        link: Some(cena_session::Link {
            kind: cena_session::LinkKind::Exist {
                id: "42".to_owned(),
                noun: "kobold".to_owned(),
            },
            text: "kobold".to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: vec![run] },
    });
    state.apply(&Frame::CreatureStatus {
        id: "42".to_owned(),
        attrs: vec![
            ("exist".to_owned(), "42".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
    state.targeting.read("#42", None);
}

fn at(line: &str) -> Said {
    Said::Send {
        line: line.to_owned(),
        target: Some(42),
    }
}

#[test]
fn an_ambush_walks_the_part_list_on_refusals() {
    let profile = "targets = [{ any = true, routine = \"a\" }]\n[aim]\nambush = [\"head\", \"neck\"]\n[routines]\na = [\"ambush\"]\n";
    let mut hunt = Hunt::new(Profile::parse(profile).unwrap(), 1);
    let mut state = standing(1_000);
    kobold(&mut state);
    assert_eq!(
        hunt.tick(&state, here(), Some(1_000)),
        at("attack #42 head")
    );
    hunt.replied(["The kobold does not have a head!"], Some(1_000));
    state.status.set("hidden", true);
    assert_eq!(
        hunt.tick(&state, here(), Some(1_001)),
        at("ambush #42 neck")
    );
    hunt.replied(["You cannot aim that high!"], Some(1_001));
    assert_eq!(
        hunt.tick(&state, here(), Some(1_002)),
        at("ambush #42 chest")
    );
}

#[test]
fn a_fire_step_aims_first_and_skips_where_an_arrow_is_stuck() {
    let profile = "targets = [{ any = true, routine = \"a\" }]\n[aim]\narchery = [\"right eye\", \"left arm\"]\n[routines]\na = [\"fire\"]\n";
    let mut hunt = Hunt::new(Profile::parse(profile).unwrap(), 1);
    let mut state = standing(1_000);
    kobold(&mut state);
    assert_eq!(hunt.tick(&state, here(), Some(1_000)), at("aim right eye"));
    hunt.incidents(&[Incident::Aiming(Some("right eye".to_owned()))]);
    assert_eq!(hunt.tick(&state, here(), Some(1_001)), at("fire"));
    hunt.incidents(&[Incident::ArrowStuck {
        creature: None,
        at: "eye".to_owned(),
    }]);
    assert_eq!(hunt.tick(&state, here(), Some(1_002)), at("aim left arm"));
}

#[test]
fn a_bless_gone_is_renewed_or_the_hunt_ends_when_nothing_can_bless() {
    let gone = Incident::BlessExpired(Some(ItemRef {
        id: "5".to_owned(),
        noun: "broadsword".to_owned(),
        text: "steel broadsword".to_owned(),
    }));
    let state = standing(1_000);
    let on = format!("{PROFILE}\n[react]\nbless = true\n");
    let mut hunt = Hunt::new(Profile::parse(&on).unwrap(), 1);
    hunt.incidents(std::slice::from_ref(&gone));
    assert_eq!(
        hunt.tick(&state, here(), Some(1_000)),
        Said::Done(Ending::Unblessed),
        "neither 304 nor the symbol is known"
    );
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    hunt.incidents(&[gone]);
    assert_ne!(
        hunt.tick(&state, here(), Some(1_000)),
        Said::Done(Ending::Unblessed),
        "bless off: said, not acted on"
    );
}

#[test]
fn a_wand_step_gets_a_fresh_wand_waves_it_and_ends_when_none_are_left() {
    let profile = "targets = [{ any = true, routine = \"a\" }]\n[wand]\nnames = [\"oaken wand\", \"iron wand\"]\nfresh = \"satchel\"\n[routines]\na = [\"wand\"]\n";
    let mut hunt = Hunt::new(Profile::parse(profile).unwrap(), 1);
    let mut state = standing(1_000);
    kobold(&mut state);
    assert_eq!(
        hunt.tick(&state, here(), Some(1_000)),
        at("get oaken wand from my satchel")
    );
    hunt.replied(["Get what?"], Some(1_000));
    assert_eq!(
        hunt.tick(&state, here(), Some(1_001)),
        at("get iron wand from my satchel")
    );
    state.apply(&Frame::RightHand {
        item: "polished iron wand".to_owned(),
        link: None,
    });
    assert_eq!(
        hunt.tick(&state, here(), Some(1_002)),
        at("wave my iron wand at #42")
    );
    hunt.replied(
        ["You wave your wand at the kobold, but nothing happens."],
        Some(1_002),
    );
    assert_eq!(
        hunt.tick(&state, here(), Some(1_003)),
        at("drop my iron wand"),
        "spent, and no dead-wand container"
    );
}

/// The room's roster: Bob, with this clause after his name.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors"
)]
fn bob(state: &mut GameState, clause: &str) {
    let text = |t: &str| cena_session::Run {
        text: t.to_owned(),
        style: Default::default(),
        link: None,
        inner_link: None,
    };
    let mut name = text("Bob");
    name.link = Some(cena_session::Link {
        kind: cena_session::LinkKind::Exist {
            id: "-9".to_owned(),
            noun: "Bob".to_owned(),
        },
        text: "Bob".to_owned(),
        coord: None,
    });
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs {
            runs: vec![text("Also here: "), name, text(clause)],
        },
    });
}

#[test]
fn a_fallen_player_is_pulled_while_something_hostile_is_here_and_a_dead_one_ends_it() {
    let mut state = standing(1_000);
    kobold(&mut state);
    bob(&mut state, " who is lying down.");
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    // Bob was here first, so the room is not the hunt's; the pull is still owed.
    assert_eq!(hunt.tick(&state, here(), Some(1_000)), send("pull Bob"));
    assert_ne!(
        hunt.tick(&state, here(), Some(1_001)),
        send("pull Bob"),
        "not again at once"
    );
    bob(&mut state, " who appears dead.");
    let deader = format!("{PROFILE}\n[react]\ndeader = true\n");
    let mut hunt = Hunt::new(Profile::parse(&deader).unwrap(), 1);
    assert_eq!(
        hunt.tick(&state, here(), Some(1_000)),
        Said::Done(Ending::Deader)
    );
}

/// A flickering cougar, `#42`, targeted: a boon creature by the object
/// table.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors"
)]
fn flickering_cougar(state: &mut GameState) {
    let mut run = cena_session::Run {
        text: "flickering cougar".to_owned(),
        style: Default::default(),
        link: Some(cena_session::Link {
            kind: cena_session::LinkKind::Exist {
                id: "42".to_owned(),
                noun: "cougar".to_owned(),
            },
            text: "flickering cougar".to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: vec![run] },
    });
    state.apply(&Frame::CreatureStatus {
        id: "42".to_owned(),
        attrs: vec![
            ("exist".to_owned(), "42".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
    state.targeting.read("#42", None);
}

#[test]
fn a_boon_creature_is_assessed_once_and_one_with_an_ignored_trait_is_left_alone() {
    let profile = format!("{PROFILE}\n[boons]\nignore = [\"blink\"]\n");
    let mut hunt = Hunt::new(Profile::parse(&profile).unwrap(), 1);
    let mut state = standing(1_000);
    flickering_cougar(&mut state);
    assert_eq!(hunt.tick(&state, here(), Some(1_000)), send("assess #42"));
    hunt.replied(["The cougar appears to be flickering."], Some(1_000));
    assert_ne!(hunt.tick(&state, here(), Some(1_001)), at("attack"));

    let plain = format!("{PROFILE}\n[boons]\nignore = [\"regen\"]\n");
    let mut hunt = Hunt::new(Profile::parse(&plain).unwrap(), 1);
    assert_eq!(hunt.tick(&state, here(), Some(1_000)), send("assess #42"));
    hunt.replied(["The cougar appears to be flickering."], Some(1_000));
    assert_eq!(hunt.tick(&state, here(), Some(1_001)), at("attack"));
}
