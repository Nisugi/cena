//! What bigshot does around a spell step and every command (`cmd`,
//! `cmd_spell`, `stand`): the rest on an unaffordable spell, the spells it
//! will not cast now, Soothe first while calmed, and standing in the stand
//! stance, not for a crossbow archer kneeling to fire.

use cena_behavior::hunt::engine::{Phase, Why};
use cena_behavior::hunt::{Here, Hunt, Profile, Said, import};
use cena_map::RoomId;
use cena_session::{Amount, Effect, Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs};

/// A link, bold for a creature.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn linked(text: &str, id: &str, noun: &str, bold: bool) -> Run {
    let mut run = Run {
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
    };
    if bold {
        run.style.bold_depth = 1;
    }
    run
}

/// Room 10 at second 1000, kobold #42 targeted, `mana` of 200, the spell
/// list read with `known` in it and the Active Spells list seen.
fn fighting(mana: i32, known: &[(&str, &str)]) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some("10".to_owned());
    state.status.set("standing", true);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![linked("kobold", "42", "kobold", true)],
        },
    });
    state.apply(&Frame::CreatureStatus {
        id: "42".into(),
        attrs: vec![
            ("exist".to_owned(), "42".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
    state.targeting.read("#42", None);
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "mana".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 50,
        text: format!("mana {mana}/200"),
        amount: Some(Amount {
            current: mana,
            max: 200,
        }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
    state.known_spells.begin();
    for (name, number) in known {
        state.known_spells.read_line(&Runs {
            runs: vec![linked(name, "x", number, false)],
        });
    }
    state.effects.clear_category("Active Spells");
    state
}

/// `state` with effect `id` (`text`) up in `dialog` for a minute.
fn with(mut state: GameState, dialog: &str, id: &str, text: &str) -> GameState {
    state.effects.insert(
        id.to_owned(),
        Effect {
            category: dialog.to_owned(),
            text: text.to_owned(),
            ends_at: Some(1_060),
            percent: 50,
        },
    );
    state
}

fn hunt(step: &str, extra: &str) -> Result<Hunt, String> {
    Ok(Hunt::new(
        Profile::parse(&format!(
            "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\nresting = 20\n[routines]\na = [\"{step}\"]\n{extra}"
        ))?,
        1,
    ))
}

fn tick(hunt: &mut Hunt, state: &GameState) -> String {
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    match hunt.tick(state, here, state.game_time_now()) {
        Said::Send { line, .. } => line,
        Said::Wait(n) => format!("wait {n}"),
        _ => "nothing".to_owned(),
    }
}

const UNAFFORDABLE: &str = "[rest.when]\nunaffordable = true\n";

/// Every spell these tests cast, known: an empty list read is "none known",
/// and a spell not known is skipped before any rule here is reached.
const KNOWN: &[(&str, &str)] = &[
    ("Minor Water", "903"),
    ("Tangleweed", "610"),
    ("Celerity", "506"),
    ("Camouflage", "608"),
    ("Wizard's Shield", "919"),
    ("Surge of Strength", "9605"),
    ("Soothe", "1201"),
];

#[test]
fn a_routine_spell_the_character_cannot_afford_sends_the_hunt_to_rest() {
    let broke = fighting(0, KNOWN);
    let mut h = hunt("903", UNAFFORDABLE).unwrap();
    assert_eq!(tick(&mut h, &broke), "wait 1", "not cast");
    tick(&mut h, &broke);
    assert_eq!(h.phase(), Phase::ToRest(Why::Mana));

    let mut off = hunt("903", "").unwrap();
    tick(&mut off, &broke);
    tick(&mut off, &broke);
    assert_eq!(off.phase(), Phase::Hunting, "oom negative: no rest");

    // Celerity's cost is not one bigshot rests for, and a handler that checks
    // its own spell (`kweed`) only skips.
    for step in ["506", "kweed"] {
        let mut h = hunt(step, UNAFFORDABLE).unwrap();
        tick(&mut h, &broke);
        tick(&mut h, &broke);
        assert_eq!(h.phase(), Phase::Hunting, "`{step}`");
    }
}

#[test]
fn bigshots_oom_becomes_the_switch_and_a_blank_one_is_on() {
    let on =
        |oom: &str| import("t", &format!("oom: {oom}\n")).map(|b| b.profile.rest.when.unaffordable);
    assert_eq!(on(""), Ok(true), "bigshot reads a blank as 0");
    assert_eq!(on("20"), Ok(true));
    assert_eq!(on("-1"), Ok(false));
}

#[test]
fn spells_cmd_spell_will_not_cast_now_are_skipped() {
    let first = |step: &str, state: &GameState| {
        hunt(step, "").map_or_else(|e| e, |mut h| tick(&mut h, state))
    };
    let rich = || fighting(200, KNOWN);
    assert_eq!(first("506", &rich()), "incant 506");
    assert_eq!(
        first("506", &with(rich(), "Active Spells", "506", "Celerity")),
        "wait 1",
        "Celerity is up"
    );
    let mut hidden = rich();
    hidden.status.set("hidden", true);
    assert_eq!(first("608", &hidden), "wait 1", "hidden already");
    assert_eq!(first("608", &rich()), "incant 608");
    let surge = with(rich(), "Cooldowns", "c", "Surge of Strength");
    assert_eq!(first("9605", &surge), "wait 1");
    let shield = with(rich(), "Cooldowns", "c", "Wizard's Shield");
    assert_eq!(first("919", &shield), "wait 1", "a short buff's cooldown");
    assert_eq!(first("919", &rich()), "incant 919");
}

#[test]
fn soothe_goes_first_while_a_calming_spell_is_on_the_character() {
    let calmed = with(fighting(200, KNOWN), "Active Spells", "201", "Calm");
    let mut h = hunt("kick", "").unwrap();
    assert_eq!(tick(&mut h, &calmed), "incant 1201");
    assert_eq!(tick(&mut h, &calmed), "kick");
    let mut clear = hunt("kick", "").unwrap();
    assert_eq!(tick(&mut clear, &fighting(200, KNOWN)), "kick");
    let mut unknown = hunt("kick", "").unwrap();
    let calmed_unknown = with(fighting(200, &[]), "Active Spells", "201", "Calm");
    assert_eq!(
        tick(&mut unknown, &calmed_unknown),
        "kick",
        "1201 not known"
    );
    // A list never read is no proof it is known: bigshot's `known?` is false.
    let mut unread = with(fighting(200, KNOWN), "Active Spells", "201", "Calm");
    unread.known_spells.clear();
    let mut h = hunt("kick", "").unwrap();
    assert_eq!(tick(&mut h, &unread), "kick", "the list never read");
}

#[test]
fn the_hunter_stands_in_the_stand_stance_but_a_crossbow_archer_kneels_to_fire() {
    let mut kneeling = fighting(200, &[]);
    kneeling.status.set("standing", false);
    kneeling.status.set("kneeling", true);
    kneeling.character.stance_percent = Some(100);
    let mut h = hunt("kick", "[stance]\nstand = \"offensive\"\n").unwrap();
    assert_eq!(tick(&mut h, &kneeling), "stance offensive");
    let mut plain = hunt("kick", "").unwrap();
    assert_eq!(tick(&mut plain, &kneeling), "stand");

    let mut archer = kneeling.clone();
    archer.apply(&Frame::LeftHand {
        item: "heavy crossbow".to_owned(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "9".to_owned(),
                noun: "crossbow".to_owned(),
            },
            text: "heavy crossbow".to_owned(),
            coord: None,
        }),
    });
    let mut fire = hunt("fire", "").unwrap();
    assert_eq!(tick(&mut fire, &archer), "fire #42", "kneeling to fire");
    let mut kick = hunt("kick", "").unwrap();
    assert_eq!(tick(&mut kick, &archer), "stand", "but not to kick");
}
