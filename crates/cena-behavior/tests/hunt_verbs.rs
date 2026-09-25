//! bigshot's routine verbs through the engine (`hunt/verbs.rs`): what each
//! step puts on the wire, aimed at the creature by id as bigshot's handlers
//! aim it.

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Amount, Effect, Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs};

/// A link to `id`, bold for a creature.
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

/// Standing in room 10 at second 1000 with mastodon #42 targeted, and
/// `also` among the room's objects.
fn fighting(also: &[Run]) -> GameState {
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
    let mut runs = vec![linked("mastodon", "42", "mastodon", true)];
    runs.extend_from_slice(also);
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs },
    });
    state.apply(&Frame::CreatureStatus {
        id: "42".into(),
        attrs: vec![
            ("exist".to_owned(), "42".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
    state.targeting.read("#42", None);
    state
}

/// The first `ticks` things a hunt whose routine is `step` does against
/// `state`: a line sent, `wait N`, or nothing.
fn run(step: &str, state: &GameState, ticks: usize) -> Result<(Vec<String>, Vec<String>), String> {
    let profile = Profile::parse(&format!(
        "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\n[routines]\na = [\"{step}\"]\n"
    ))?;
    let mut hunt = Hunt::new(profile, 1);
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    let mut out = Vec::new();
    for _ in 0..ticks {
        out.push(match hunt.tick(state, here, state.game_time_now()) {
            Said::Send { line, .. } => line,
            Said::Wait(n) => format!("wait {n}"),
            _ => "nothing".to_owned(),
        });
    }
    Ok((out, hunt.take_notes()))
}

/// The first line a routine of `step` sends.
fn first(step: &str) -> String {
    run(step, &fighting(&[]), 1)
        .map(|(lines, _)| lines.concat())
        .unwrap_or_default()
}

#[test]
fn maneuvers_techniques_and_feats_go_through_their_command_at_the_creature() {
    for (step, sent) in [
        ("coupdegrace", "cman coupdegrace #42"),
        ("sweep", "cman sweep #42"),
        ("bearhug", "cman bearhug #42"),
        ("volley", "weapon volley #42"),
        ("gthrusts", "weapon gthrusts #42"),
        ("chastise", "feat chastise #42"),
        ("shield bash", "shield bash #42"),
        ("burst", "cman burst"),
        ("dislodge left arm", "cman dislodge #42 left arm"),
        ("fire", "fire #42"),
        ("throw", "throw #42"),
        ("smite", "smite #42"),
        ("depress", "renew 1015"),
        ("dhurl head", "hurl #42 head"),
    ] {
        assert_eq!(first(step), sent, "`{step}`");
    }
}

#[test]
fn a_game_command_goes_as_written() {
    for step in [
        "weapon volley",
        "store weapon",
        "ready 2weapon",
        "stance offensive",
        "hide",
        "shield foo",
        "berserk",
    ] {
        assert_eq!(first(step), step, "`{step}`");
    }
}

#[test]
fn warcries_are_warcries_and_three_are_aimed() {
    let mut rested = fighting(&[]);
    rested.apply(&Frame::ProgressBar(ProgressBar {
        id: "stamina".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 100,
        text: "stamina 100/100".to_owned(),
        amount: Some(Amount {
            current: 100,
            max: 100,
        }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
    for (step, sent) in [
        ("shout", "warcry shout"),
        ("bellow", "warcry bellow #42"),
        ("cry all", "warcry cry all"),
        ("growl", "warcry growl #42"),
    ] {
        let (lines, _) = run(step, &rested, 1).unwrap();
        assert_eq!(lines, [sent], "`{step}`");
    }
    let mut tired = rested;
    tired.apply(&Frame::ProgressBar(ProgressBar {
        id: "stamina".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 20,
        text: "stamina 20/100".to_owned(),
        amount: Some(Amount {
            current: 20,
            max: 100,
        }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
    let (lines, _) = run("shout", &tired, 1).unwrap();
    assert_eq!(lines, ["wait 1"], "a shout needs 25 stamina");
}

#[test]
fn a_spell_is_prepared_and_cast_at_the_creature_unless_it_is_cast_on_oneself() {
    let (lines, _) = run("incant 611", &fighting(&[]), 2).unwrap();
    assert_eq!(lines, ["prepare 611", "cast #42"]);
    let (lines, _) = run("611 evoke", &fighting(&[]), 2).unwrap();
    assert_eq!(lines, ["prepare 611", "evoke #42"]);
    assert_eq!(first("incant 608"), "incant 608", "Camouflage is self-cast");
    let (lines, _) = run("curse hex", &fighting(&[]), 2).unwrap();
    assert_eq!(lines, ["prep 715", "curse #42 hex"]);
    let (lines, _) = run("caststop 1030", &fighting(&[]), 3).unwrap();
    assert_eq!(lines, ["prepare 1030", "cast #42", "stop 1030"]);
}

#[test]
fn kweed_is_tangleweed_evoked_unless_a_plant_is_here() {
    let (lines, _) = run("kweed", &fighting(&[]), 2).unwrap();
    assert_eq!(lines, ["prepare 610", "evoke #42"]);
    let (lines, _) = run("weed", &fighting(&[]), 2).unwrap();
    assert_eq!(lines, ["prepare 610", "cast #42"]);
    let vine = fighting(&[linked("a thorny vine", "77", "vine", false)]);
    let (lines, _) = run("kweed", &vine, 1).unwrap();
    assert_eq!(lines, ["wait 1"], "a vine is already here");
    let ivy_ish = fighting(&[linked("a wooden privy door", "78", "door", false)]);
    let (lines, _) = run("kweed", &ivy_ish, 1).unwrap();
    assert_eq!(lines, ["prepare 610"], "a privy is not ivy");
}

#[test]
fn a_maneuver_cooling_is_not_sent() {
    let mut state = fighting(&[]);
    state.effects.clear_category("Cooldowns");
    state.effects.insert(
        "c".to_owned(),
        Effect {
            category: "Cooldowns".to_owned(),
            text: "Coup de Grace".to_owned(),
            ends_at: Some(1_060),
            percent: 50,
        },
    );
    let (lines, _) = run("coupdegrace", &state, 1).unwrap();
    assert_eq!(lines, ["wait 1"]);
    let (lines, _) = run("volley", &state, 1).unwrap();
    assert_eq!(
        lines,
        ["weapon volley #42"],
        "another's cooldown is its own"
    );
}

#[test]
fn sleep_waits_and_an_unported_verb_is_skipped_and_named_once() {
    let (lines, _) = run("sleep 3", &fighting(&[]), 1).unwrap();
    assert_eq!(lines, ["wait 3"]);
    let (lines, notes) = run("eachtarget fire", &fighting(&[]), 3).unwrap();
    assert_eq!(lines, ["wait 1", "wait 1", "wait 1"]);
    assert_eq!(notes.len(), 1, "said once: {notes:?}");
    assert!(notes[0].contains("eachtarget"), "{notes:?}");
}

#[test]
fn a_jewel_is_activated_unless_cooling_or_unknown() {
    // The command itself is pinned beside it, in the game module
    // (`src/gemstone/jewel.rs`): its verb is the game's name (Rule 3.4).
    let (lines, _) = run("jewel spellblade", &fighting(&[]), 1).unwrap();
    assert!(lines[0].ends_with(" activate spellblade"), "{lines:?}");
    let mut cooling = fighting(&[]);
    cooling.effects.clear_category("Cooldowns");
    cooling.effects.insert(
        "j".to_owned(),
        Effect {
            category: "Cooldowns".to_owned(),
            text: "Spellblade's Fury".to_owned(),
            ends_at: Some(1_060),
            percent: 50,
        },
    );
    let (lines, _) = run("jewel spellblade", &cooling, 1).unwrap();
    assert_eq!(lines, ["wait 1"]);
    let (lines, notes) = run("jewel sparkle", &fighting(&[]), 1).unwrap();
    assert_eq!(lines, ["wait 1"]);
    assert!(
        notes.iter().any(|n| n.contains("does not know")),
        "{notes:?}"
    );
}

/// A hunt firing at #42 with this `ammo_container`, and an arrow in hand.
fn archer(container: &str) -> Result<(Hunt, GameState), String> {
    let aim = if container.is_empty() {
        String::new()
    } else {
        format!("[aim]\nammo_container = \"{container}\"\n")
    };
    let profile = Profile::parse(&format!(
        "targets = [{{ any = true, routine = \"a\" }}]\n{aim}[rooms]\nhunting = 10\n[routines]\na = [\"fire\"]\n"
    ))?;
    let mut state = fighting(&[]);
    state.apply(&Frame::RightHand {
        item: "iron-tipped arrow".to_owned(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "901".to_owned(),
                noun: "arrow".to_owned(),
            },
            text: "iron-tipped arrow".to_owned(),
            coord: None,
        }),
    });
    Ok((Hunt::new(profile, 1), state))
}

/// What a tick sends, or "nothing".
fn tick(hunt: &mut Hunt, state: &GameState) -> String {
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    match hunt.tick(state, here, state.game_time_now()) {
        Said::Send { line, .. } => line,
        _ => "nothing".to_owned(),
    }
}

#[test]
fn an_arrow_the_game_will_not_fire_is_stowed_and_a_closed_quiver_opened() {
    let (mut hunt, state) = archer("quiver").unwrap();
    assert_eq!(tick(&mut hunt, &state), "fire #42");
    hunt.replied(
        ["You cannot fire an arrow that is not nocked."],
        Some(1_000),
    );
    assert_eq!(tick(&mut hunt, &state), "stow #901");
    hunt.replied(["That is closed."], Some(1_000));
    assert_eq!(tick(&mut hunt, &state), "open my quiver");
    assert_eq!(tick(&mut hunt, &state), "put #901 in my quiver");
    assert_eq!(
        tick(&mut hunt, &state),
        "fire #42",
        "and back to the routine"
    );

    let (mut plain, state) = archer("").unwrap();
    assert_eq!(tick(&mut plain, &state), "fire #42");
    plain.replied(["You cannot fire that."], Some(1_000));
    assert_eq!(tick(&mut plain, &state), "stow #901");
    plain.replied(["That is closed."], Some(1_000));
    assert_eq!(
        tick(&mut plain, &state),
        "fire #42",
        "no ammo_container: nothing to open"
    );
}
