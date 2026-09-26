//! bigshot's handlers that gate what they send, hold after it, or read the
//! answer (`hunt/verbs/gated.rs`, `hunt/follow.rs`). Every reply line is
//! synthetic, from the handler's own pattern in `bigshot.lic`.

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::incident::Incident;
use cena_session::{
    Amount, Effect, Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs, gameobj,
};

/// A bold creature link.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn linked(noun: &str) -> Run {
    let mut run = Run {
        text: noun.to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "42".to_owned(),
                noun: noun.to_owned(),
            },
            text: noun.to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    run
}

/// Room 10 at `second`, creature #42 (`noun`) targeted, with `attrs` on
/// its status.
fn fighting(second: u32, noun: &str, attrs: &[(&str, &str)]) -> GameState {
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
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![linked(noun)],
        },
    });
    let mut all = vec![
        ("exist".to_owned(), "42".to_owned()),
        ("hostile".to_owned(), "1".to_owned()),
    ];
    all.extend(
        attrs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
    );
    state.apply(&Frame::CreatureStatus {
        id: "42".into(),
        attrs: all,
    });
    state.targeting.read("#42", None);
    state
}

fn kobold(second: u32) -> GameState {
    fighting(second, "kobold", &[])
}

/// `state` with the stamina bar at `points` of 100.
fn stamina(mut state: GameState, points: i32) -> GameState {
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
    state
}

/// `state` with an effect `text` (id `id`) up in `dialog` until `ends`.
fn with(mut state: GameState, dialog: &str, id: &str, text: &str, ends: u32) -> GameState {
    state.effects.clear_category(dialog);
    state.effects.insert(
        id.to_owned(),
        Effect {
            category: dialog.to_owned(),
            text: text.to_owned(),
            ends_at: Some(ends),
            percent: 50,
        },
    );
    state
}

fn hunt(routine: &[&str]) -> Result<Hunt, String> {
    let steps: Vec<String> = routine.iter().map(|s| format!("{s:?}")).collect();
    Ok(Hunt::new(
        Profile::parse(&format!(
            "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\n[routines]\na = [{}]\n",
            steps.join(", ")
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

fn ticks(hunt: &mut Hunt, state: &GameState, n: usize) -> Vec<String> {
    (0..n).map(|_| tick(hunt, state)).collect()
}

/// The first line a routine of `step` sends in `state`.
fn first(step: &str, state: &GameState) -> String {
    hunt(&[step]).map_or_else(|e| e, |mut h| tick(&mut h, state))
}

#[test]
fn stomp_channels_tremors_when_it_is_down_and_stomps_while_it_is_up() {
    let down = with(kobold(1_000), "Active Spells", "x", "", 1_100);
    let mut h = hunt(&["stomp"]).unwrap();
    assert_eq!(ticks(&mut h, &down, 3), ["prepare 909", "channel", "stomp"]);
    let up = with(kobold(1_000), "Active Spells", "909", "Tremors", 1_100);
    assert_eq!(first("stomp", &up), "stomp");
}

#[test]
fn leech_waits_for_its_cooldown_and_rapid_for_its_buff() {
    let cooling = |left: u32| with(kobold(1_000), "Cooldowns", "c", "Mana Leech", 1_000 + left);
    assert_eq!(first("leech", &cooling(20)), "wait 1");
    assert_eq!(first("leech", &cooling(10)), "incant 516");
    let fire = |left: u32| with(kobold(1_000), "Buffs", "b", "Rapid Fire", 1_000 + left);
    assert_eq!(first("rapid", &fire(10)), "wait 1");
    assert_eq!(first("rapid", &fire(2)), "incant 515", "lapsing");
    let recovering = with(
        kobold(1_000),
        "Cooldowns",
        "c",
        "Rapid Fire Recovery",
        1_030,
    );
    assert_eq!(first("rapid", &recovering), "wait 1");
    assert_eq!(first("rapid ignore", &recovering), "incant 515");
}

#[test]
fn burst_needs_its_stamina_and_not_its_enhancement() {
    assert_eq!(first("burst", &stamina(kobold(1_000), 25)), "wait 1");
    assert_eq!(first("burst", &stamina(kobold(1_000), 40)), "cman burst");
    let cooling = with(
        stamina(kobold(1_000), 40),
        "Cooldowns",
        "c",
        "Burst of Swiftness",
        1_100,
    );
    assert_eq!(first("burst", &cooling), "wait 1", "60 while it cools");
    let enhanced = with(
        stamina(kobold(1_000), 90),
        "Buffs",
        "b",
        "Enh. Dexterity (+10)",
        1_100,
    );
    assert_eq!(first("burst", &enhanced), "wait 1");
    assert_eq!(first("surge", &enhanced), "cman surge");
}

#[test]
fn smite_only_an_undead_creature_and_throw_only_one_standing() {
    assert!(
        gameobj::classify("zombie", "zombie").is("undead"),
        "the input reaches the check"
    );
    assert_eq!(first("smite", &fighting(1_000, "zombie", &[])), "smite #42");
    assert_eq!(first("smite", &kobold(1_000)), "wait 1");
    assert_eq!(first("throw", &kobold(1_000)), "throw #42");
    let down = fighting(1_000, "kobold", &[("prone", "1")]);
    assert_eq!(first("throw", &down), "wait 1");
}

#[test]
fn curse_is_prepared_once_and_the_star_waits_for_its_bonus() {
    let mut h = hunt(&["curse hex"]).unwrap();
    assert_eq!(
        ticks(&mut h, &kobold(1_000), 2),
        ["prep 715", "curse #42 hex"]
    );
    let mut prepared = kobold(1_000);
    prepared.prepared = Some("Curse".to_owned());
    assert_eq!(first("curse hex", &prepared), "curse #42 hex");
    let mut other = kobold(1_000);
    other.prepared = Some("Minor Water".to_owned());
    let mut h = hunt(&["curse hex"]).unwrap();
    assert_eq!(
        ticks(&mut h, &other, 3),
        ["release", "prep 715", "curse #42 hex"]
    );
    let star = with(
        kobold(1_000),
        "Active Spells",
        "9053",
        "Curse of the Star (bonus)",
        1_060,
    );
    assert_eq!(first("curse star", &star), "wait 1");
    assert_eq!(first("curse whatever", &kobold(1_000)), "curse whatever");
}

#[test]
fn store_a_hand_only_when_it_holds_something_and_stance_only_when_not_taken() {
    let mut empty = kobold(1_000);
    empty.apply(&Frame::RightHand {
        item: "Empty".to_owned(),
        link: None,
    });
    assert_eq!(first("store right", &empty), "wait 1");
    let mut holding = kobold(1_000);
    holding.apply(&Frame::RightHand {
        item: "steel broadsword".to_owned(),
        link: None,
    });
    assert_eq!(first("store right", &holding), "store right");
    assert_eq!(first("store weapon", &empty), "store weapon");

    let mut defensive = kobold(1_000);
    defensive.character.stance_percent = Some(100);
    assert_eq!(first("stance defensive", &defensive), "wait 1");
    assert_eq!(first("stance offensive", &defensive), "stance offensive");
}

#[test]
fn sacrifice_appraises_first_and_sacrifices_only_the_frail() {
    let state = kobold(1_000);
    let mut h = hunt(&["sacrifice"]).unwrap();
    assert_eq!(tick(&mut h, &state), "appraise #42");
    h.replied(
        ["The kobold is small in size and looks enticingly frail."],
        Some(1_000),
    );
    assert_eq!(tick(&mut h, &state), "sacrifice #42");
    let mut sturdy = hunt(&["sacrifice"]).unwrap();
    assert_eq!(tick(&mut sturdy, &state), "appraise #42");
    sturdy.replied(["The kobold is large in size."], Some(1_000));
    assert_eq!(tick(&mut sturdy, &state), "appraise #42", "not sacrificed");
    let cooling = with(kobold(1_000), "Cooldowns", "c", "Sacrifice", 1_100);
    assert_eq!(first("sacrifice", &cooling), "wait 1");
}

#[test]
fn depress_begins_the_song_when_it_is_not_being_sung() {
    let state = kobold(1_000);
    let mut h = hunt(&["depress"]).unwrap();
    assert_eq!(tick(&mut h, &state), "renew 1015");
    h.replied(["But you are not singing that spellsong."], Some(1_000));
    assert_eq!(tick(&mut h, &state), "incant 1015");
}

#[test]
fn unravel_stops_the_song_as_the_answer_asks() {
    let state = kobold(1_000);
    // `kick` after it, so a cast again is the answer's, not the routine's.
    let mut h = hunt(&["unravel", "kick"]).unwrap();
    assert_eq!(ticks(&mut h, &state, 2), ["prepare 1013", "cast #42"]);
    h.replied(["You are already singing that spellsong."], Some(1_000));
    assert_eq!(
        ticks(&mut h, &state, 3),
        ["stop 1013", "prepare 1013", "cast #42"]
    );
    h.replied(["You gain 12 mana!"], Some(1_001));
    assert_eq!(tick(&mut h, &state), "stop 1013");
}

#[test]
fn efury_and_wait_hold_until_what_they_wait_for_is_heard() {
    let state = kobold(1_000);
    let mut fury = hunt(&["efury fire", "kick"]).unwrap();
    assert_eq!(ticks(&mut fury, &state, 2), ["incant 917 fire", "wait 1"]);
    fury.heard("The ground beneath the kobold suddenly calms.", Some(1_001));
    assert_eq!(tick(&mut fury, &state), "kick");

    let mut wait = hunt(&["wait 10", "kick"]).unwrap();
    assert_eq!(ticks(&mut wait, &state, 2), ["wait 1", "wait 1"]);
    wait.heard("The kobold swings a club at you!", Some(1_001));
    assert_eq!(tick(&mut wait, &state), "kick", "it swung");

    let mut timed = hunt(&["wait 10", "kick"]).unwrap();
    assert_eq!(tick(&mut timed, &state), "wait 1");
    assert_eq!(tick(&mut timed, &kobold(1_009)), "wait 1");
    assert_eq!(tick(&mut timed, &kobold(1_010)), "kick", "ten seconds");
}

#[test]
fn berserk_holds_while_berserk_is_up_and_kills_when_too_tired() {
    let mut h = hunt(&["berserk", "kick"]).unwrap();
    assert_eq!(tick(&mut h, &stamina(kobold(1_000), 50)), "berserk");
    assert_eq!(
        tick(&mut h, &stamina(kobold(1_002), 50)),
        "wait 1",
        "not yet listed"
    );
    let raging = with(
        stamina(kobold(1_006), 50),
        "Buffs",
        "9607",
        "Berserk",
        1_030,
    );
    assert_eq!(tick(&mut h, &raging), "wait 1");
    assert_eq!(tick(&mut h, &stamina(kobold(1_031), 50)), "kick", "over");

    let mut tired = hunt(&["berserk"]).unwrap();
    assert_eq!(
        ticks(&mut tired, &stamina(kobold(1_000), 10), 2),
        ["target random", "kill"]
    );
}

#[test]
fn hide_tries_again_until_hidden() {
    let mut h = hunt(&["hide 2", "kick"]).unwrap();
    assert_eq!(
        ticks(&mut h, &kobold(1_000), 4),
        ["hide", "hide", "hide", "kick"],
        "the first try and two more"
    );
}

#[test]
fn dhurl_recovers_its_weapon() {
    let state = kobold(1_000);
    let mut h = hunt(&["dhurl head", "kick"]).unwrap();
    assert_eq!(tick(&mut h, &state), "hurl #42 head");
    h.replied(
        ["You take aim and throw a dagger at a kobold!"],
        Some(1_000),
    );
    assert_eq!(tick(&mut h, &state), "recover hurl");
    h.replied(
        ["You know your dagger is around here somewhere, but you don't see it."],
        Some(1_001),
    );
    assert_eq!(tick(&mut h, &state), "recover hurl");
    h.replied(["You spy a dagger and recover it."], Some(1_002));
    assert_eq!(tick(&mut h, &state), "kick");
}

#[test]
fn dislodge_frees_only_a_lodged_part() {
    let state = kobold(1_000);
    let mut h = hunt(&["dislodge head arm"]).unwrap();
    assert_eq!(tick(&mut h, &state), "wait 1", "nothing lodged");
    h.incidents(&[Incident::ArrowStuck {
        creature: None,
        at: "arm".to_owned(),
    }]);
    assert_eq!(tick(&mut h, &state), "cman dislodge #42 arm");
    h.replied(
        ["You manage to dislodge an arrow from the kobold!"],
        Some(1_000),
    );
    assert_eq!(tick(&mut h, &state), "wait 1", "freed");
}

#[test]
fn kick_is_punch_while_rooted_and_target_is_the_creature() {
    let state = kobold(1_000);
    assert_eq!(first("attack target", &state), "attack #42");
    let mut h = hunt(&["kick"]).unwrap();
    assert_eq!(tick(&mut h, &state), "kick");
    h.heard(
        "You don't seem to be able to move your legs to do that.",
        Some(1_000),
    );
    assert_eq!(tick(&mut h, &state), "punch");
}

#[test]
fn ambush_with_a_part_aims_there_and_at_the_creature() {
    let mut hidden = kobold(1_000);
    hidden.status.set("hidden", true);
    assert_eq!(first("ambush neck", &hidden), "ambush #42 neck");
    assert_eq!(first("ambush neck", &kobold(1_000)), "attack #42 neck");
    assert_eq!(
        first("ambush", &hidden),
        "ambush #42 head",
        "bigshot's list"
    );
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

#[test]
fn a_line_the_game_answers_with_wait_goes_again() {
    let state = kobold(1_000);
    let mut h = hunt(&["kick", "punch"]).unwrap();
    assert_eq!(tick(&mut h, &state), "kick");
    h.replied(["...wait 2 seconds."], Some(1_000));
    assert_eq!(tick(&mut h, &state), "kick", "the step again, not the next");
    h.replied(["You kick at a kobold!"], Some(1_002));
    assert_eq!(tick(&mut h, &state), "punch");

    let mut spell = hunt(&["incant 1106", "punch"]).unwrap();
    assert_eq!(ticks(&mut spell, &state, 2), ["prepare 1106", "cast #42"]);
    spell.replied(["...wait 1 second."], Some(1_000));
    assert_eq!(
        tick(&mut spell, &state),
        "cast #42",
        "the queued line again"
    );
}

#[test]
fn an_assault_and_a_bearhug_are_waited_out() {
    let state = kobold(1_000);
    let mut assault = hunt(&["barrage", "kick"]).unwrap();
    assert_eq!(
        ticks(&mut assault, &state, 2),
        ["weapon barrage #42", "wait 1"],
        "the rounds are not cut into"
    );
    assault.heard(
        "You complete your assault, pulling your weapon back to the ready.",
        Some(1_004),
    );
    assert_eq!(tick(&mut assault, &state), "kick");

    let mut hug = hunt(&["bearhug", "kick"]).unwrap();
    assert_eq!(ticks(&mut hug, &state, 2), ["cman bearhug #42", "wait 1"]);
    hug.heard("You release your grip on the kobold.", Some(1_006));
    assert_eq!(tick(&mut hug, &state), "kick");
    let mut timed = hunt(&["bearhug", "kick"]).unwrap();
    assert_eq!(tick(&mut timed, &state), "cman bearhug #42");
    assert_eq!(tick(&mut timed, &kobold(1_016)), "wait 1");
    assert_eq!(tick(&mut timed, &kobold(1_017)), "kick", "17 s at most");
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

#[test]
fn an_assault_the_attack_type_refuses_swaps_once_and_goes_again() {
    let state = kobold(1_000);
    let refusal = "Barrage can not be used with attack as the attack type.";
    let mut h = hunt(&["barrage", "kick"]).unwrap();
    assert_eq!(tick(&mut h, &state), "weapon barrage #42");
    h.heard(refusal, Some(1_000));
    h.replied([refusal], Some(1_000));
    assert_eq!(
        ticks(&mut h, &state, 2),
        ["swap", "weapon barrage #42"],
        "the bow to the other hand, and the assault again"
    );
    h.heard(refusal, Some(1_001));
    h.replied([refusal], Some(1_001));
    assert_eq!(
        tick(&mut h, &state),
        "kick",
        "refused again: not swapped back"
    );
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
