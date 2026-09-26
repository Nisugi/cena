//! Multi-Strike and unarmed combat (`hunt/verbs/ucs.rs`): bigshot's
//! `cmd_mstrike` and `cmd_unarmed`. The positioning lines are cut from the
//! committed fixture `crates/cena-model/tests/fixtures/combat/jab.txt`, the
//! creature and tier changed; the follow-up line is the model's own pattern
//! (`crates/cena-model/src/state/combat/ucs.rs`).

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{
    Amount, Effect, Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs, SkillKind, SkillLine,
    TextFrame,
};

fn link(id: &str, noun: &str, name: &str) -> Link {
    Link {
        kind: LinkKind::Exist {
            id: id.to_owned(),
            noun: noun.to_owned(),
        },
        text: name.to_owned(),
        coord: None,
    }
}

/// Room 10 at second 1000 with these `(id, noun, name)` creatures, the
/// first targeted, and 100 of 100 stamina.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn fighting(creatures: &[(&str, &str, &str)]) -> GameState {
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
    let runs = creatures
        .iter()
        .map(|(id, noun, name)| {
            let mut run = Run {
                text: (*name).to_owned(),
                style: Default::default(),
                link: Some(link(id, noun, name)),
                inner_link: None,
            };
            run.style.bold_depth = 1;
            run
        })
        .collect();
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs },
    });
    for (id, _, _) in creatures {
        state.apply(&Frame::CreatureStatus {
            id: (*id).into(),
            attrs: vec![
                ("exist".to_owned(), (*id).to_owned()),
                ("hostile".to_owned(), "1".to_owned()),
            ],
        });
    }
    state.targeting.read(&format!("#{}", creatures[0].0), None);
    state.effects.clear_category("Cooldowns");
    state.effects.clear_category("Debuffs");
    stamina(&mut state, 100);
    state
}

fn stamina(state: &mut GameState, points: i32) {
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "stamina".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 100,
        text: format!("stamina {points}/100"),
        amount: Some(Amount {
            current: points,
            max: 100,
        }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

fn kobold() -> GameState {
    fighting(&[("42", "kobold", "kobold")])
}

fn two() -> GameState {
    fighting(&[("42", "kobold", "kobold"), ("43", "rat", "giant rat")])
}

/// `state` with Multi-Opponent Combat at `ranks`, the skills table read.
fn moc(mut state: GameState, ranks: u16) -> GameState {
    state.character.skills.replace(&[(
        SkillLine::Skill {
            kind: SkillKind::MultiOpponentCombat,
            bonus: 0,
            ranks,
        },
        false,
    )]);
    state
}

/// A creature as a line links it: `(id, noun, name)`.
type Named<'a> = (&'a str, &'a str, &'a str);

/// A line: the text before a creature, the creature, and the text after.
type Said3<'a> = (&'a str, Option<Named<'a>>, &'a str);

/// Main-window lines, each `(before, creature, after)`, then a prompt.
#[expect(
    clippy::default_trait_access,
    reason = "the text's style type is not re-exported for behaviors"
)]
fn say(state: &mut GameState, lines: &[Said3<'_>]) {
    for (before, creature, after) in lines {
        let mut parts = vec![((*before).to_owned(), None)];
        if let Some((id, noun, name)) = creature {
            parts.push(((*name).to_owned(), Some(link(id, noun, name))));
        }
        parts.push(((*after).to_owned(), None));
        let last = parts.len() - 1;
        for (at, (text, link)) in parts.into_iter().enumerate() {
            let mut frame = TextFrame {
                content: text,
                stream: String::new(),
                style: Default::default(),
                link,
                inner_link: None,
                ends_line: at == last,
            };
            if frame.link.is_some() {
                frame.style.bold_depth = 1;
            }
            state.apply(&Frame::Text(frame));
        }
    }
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
}

/// `kobold()` after a jab that found `tier` positioning, and `followup`
/// offered when given.
fn after_jab(tier: &str, followup: Option<&str>) -> GameState {
    let mut state = kobold();
    let it = Some(("42", "kobold", "kobold"));
    let mut lines = vec![
        ("You attempt to jab ", it, "!"),
        ("You have ", None, ""),
        (
            "  UAF: 681 vs UDF: 918 = 0.741 * MM: 81 + d100: 18 = 78",
            None,
            "",
        ),
    ];
    let positioning = format!("You have {tier} positioning against ");
    lines[1] = (positioning.as_str(), it, ".");
    let offered = followup
        .map(|attack| format!("Strike leaves foe vulnerable to a followup {attack} attack!"));
    if let Some(text) = &offered {
        lines.push((text.as_str(), None, ""));
    }
    lines.push(("Roundtime: 2 sec.", None, ""));
    say(&mut state, &lines);
    state
}

fn first(step: &str, extra: &str, state: &GameState) -> String {
    let text = format!(
        "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\n[routines]\na = [\"{step}\"]\n{extra}"
    );
    let Ok(profile) = Profile::parse(&text) else {
        return "profile".to_owned();
    };
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    match Hunt::new(profile, 1).tick(state, here, state.game_time_now()) {
        Said::Send { line, .. } => line,
        Said::Wait(n) => format!("wait {n}"),
        _ => "nothing".to_owned(),
    }
}

#[test]
fn mstrike_goes_by_the_characters_multi_opponent_combat() {
    assert_eq!(
        first("mstrike", "", &kobold()),
        "wait 1",
        "never read: 0 ranks"
    );
    assert_eq!(first("mstrike", "", &moc(kobold(), 40)), "mstrike #42");
    assert_eq!(
        first("mstrike", "", &moc(two(), 40)),
        "mstrike",
        "unfocused at the mob"
    );
    assert_eq!(first("mstrike", "", &moc(kobold(), 10)), "wait 1");
    assert_eq!(first("mstrike", "", &moc(two(), 10)), "mstrike");
    assert_eq!(
        first("mstrike", "", &moc(two(), 3)),
        "wait 1",
        "under 5 ranks, not even unfocused"
    );
    let nest = moc(
        fighting(&[("42", "kobold", "kobold"), ("44", "nest", "wasp nest")]),
        40,
    );
    assert_eq!(
        first("mstrike", "", &nest),
        "wait 1",
        "not with a nest here"
    );
}

#[test]
fn mstrike_waits_out_its_cooldown_unless_told_and_quickstrikes_when_set() {
    let mut cooling = moc(kobold(), 40);
    cooling.effects.insert(
        "c".to_owned(),
        Effect {
            category: "Cooldowns".to_owned(),
            text: "Multi-Strike".to_owned(),
            ends_at: Some(1_030),
            percent: 50,
        },
    );
    assert_eq!(first("mstrike", "", &cooling), "wait 1");
    let during = "[mstrike]\ncooldown = true\nstamina_cooldown = 50\n";
    assert_eq!(first("mstrike", during, &cooling), "mstrike #42");
    let quick = "[mstrike]\nquickstrike = true\nstamina_quickstrike = 50\n";
    assert_eq!(
        first("mstrike", quick, &moc(kobold(), 40)),
        "quickstrike 1 mstrike #42"
    );
    let mut tired = moc(kobold(), 40);
    stamina(&mut tired, 40);
    assert_eq!(
        first("mstrike", quick, &tired),
        "mstrike #42",
        "short of it"
    );
}

#[test]
fn unarmed_attacks_by_the_creatures_positioning_and_aims() {
    assert_eq!(first("unarmed jab", "", &kobold()), "jab #42");
    assert_eq!(
        first("unarmed jab", "[unarmed]\naim = [\"head\"]\n", &kobold()),
        "jab #42 head"
    );
    assert_eq!(first("unarmed jab neck", "", &kobold()), "jab #42 neck");
    let tier3 = after_jab("excellent", None);
    assert_eq!(
        first("unarmed jab", "", &tier3),
        "punch #42",
        "tier 3: the tier 3 attack"
    );
    assert_eq!(
        first("unarmed jab", "[unarmed]\ntier3 = \"kick\"\n", &tier3),
        "kick #42"
    );
    let offered = after_jab("good", Some("grapple"));
    assert_eq!(
        first("unarmed jab", "", &offered),
        "grapple #42",
        "the follow-up"
    );
    let decent = after_jab("decent", None);
    assert_eq!(first("unarmed jab", "", &decent), "jab #42");
}

#[test]
fn unarmed_multi_strikes_with_the_tier3_attack_unless_told_not_to() {
    let trained = moc(kobold(), 40);
    assert_eq!(first("unarmed jab", "", &trained), "mstrike punch #42");
    assert_eq!(
        first("unarmed jab", "[unarmed]\nno_mstrike = true\n", &trained),
        "jab #42"
    );
}

#[test]
fn bigshots_uac_and_mstrike_tabs_come_across() {
    let yaml = "tier3: kick\naim: head, neck\nuac_smite: true\nuac_mstrike: true\n\
                mstrike_cooldown: true\nmstrike_quickstrike: true\n\
                mstrike_stamina_cooldown: 50\nmstrike_stamina_quickstrike: 60\nmstrike_mob: 3\n";
    let brought = cena_behavior::hunt::import("t", yaml).unwrap();
    let p = &brought.profile;
    assert_eq!(p.unarmed.tier3, "kick");
    assert_eq!(p.unarmed.aim, ["head", "neck"]);
    assert!(p.unarmed.smite && p.unarmed.no_mstrike);
    assert!(p.mstrike.cooldown && p.mstrike.quickstrike);
    assert_eq!(
        (
            p.mstrike.stamina_cooldown,
            p.mstrike.stamina_quickstrike,
            p.mstrike.mob
        ),
        (Some(50), Some(60), 3)
    );
    let notes = brought.notes.join("\n");
    assert!(
        !notes.contains("tier3") && !notes.contains("mstrike_"),
        "{notes}"
    );
    let blank = cena_behavior::hunt::import("t", "tier3: \nmstrike_mob: \n").unwrap();
    assert_eq!(blank.profile.unarmed.tier3, "punch", "bigshot's default");
    assert_eq!(blank.profile.mstrike.mob, 2, "bigshot's default");
}

#[test]
fn a_paladin_surges_before_a_multi_strike_once_in_301_seconds() {
    let mut paladin = moc(kobold(), 40);
    paladin.character.identity.profession = Some("Paladin".to_owned());
    paladin.known_spells.begin();
    paladin.known_spells.read_line(&Runs {
        runs: vec![spell_run("Adrenal Surge", "1107")],
    });
    paladin.effects.clear_category("Active Spells");
    let mut h = Hunt::new(
        Profile::parse(
            "targets = [{ any = true, routine = \"a\" }]\n[rooms]\nhunting = 10\n[routines]\na = [\"mstrike\"]\n",
        )
        .unwrap(),
        1,
    );
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    let mut tick = |state: &GameState| match h.tick(state, here, state.game_time_now()) {
        Said::Send { line, .. } => line,
        other => format!("{other:?}"),
    };
    assert_eq!(tick(&paladin), "incant 1107");
    assert_eq!(tick(&paladin), "mstrike #42");
    assert_eq!(tick(&paladin), "mstrike #42", "not again inside 301 s");
    let mut tired = paladin.clone();
    stamina(&mut tired, 60);
    let mut fresh = Hunt::new(
        Profile::parse(
            "targets = [{ any = true, routine = \"a\" }]\n[rooms]\nhunting = 10\n[routines]\na = [\"mstrike\"]\n",
        )
        .unwrap(),
        1,
    );
    let said = fresh.tick(&tired, here, tired.game_time_now());
    assert!(
        matches!(&said, Said::Send { line, .. } if line == "mstrike #42"),
        "60 of 100, no blessings: the surge would not reach it: {said:?}"
    );
}

/// A spell list's line for spell `number`, as the list links it.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors"
)]
fn spell_run(name: &str, number: &str) -> Run {
    Run {
        text: name.to_owned(),
        style: Default::default(),
        link: Some(link("x", number, name)),
        inner_link: None,
    }
}
