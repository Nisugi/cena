//! bigshot's handlers that gate what they send, hold after it, or read the
//! answer (`hunt/verbs/gated.rs`, `hunt/follow.rs`). Every reply line is
//! synthetic, from the handler's own pattern in `bigshot.lic`. The errands
//! -- `wield`, `briar`, `wandolier`, `nudgeweapons`, `throw` -- are in
//! `hunt_errands.rs`.

mod gated_support;

use cena_session::incident::Incident;
use cena_session::{Frame, gameobj};
use gated_support::{fighting, first, hunt, kobold, stamina, tick, ticks, with};

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
