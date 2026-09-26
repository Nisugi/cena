//! The hunt machine, driven tick by tick with no game (`plan/30` §3): each
//! policy in eohunter's order, and the rest cycle end to end.

use cena_behavior::hunt::engine::Phase;
use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{Effect, Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs};

const PROFILE: &str = r#"
prepare = ["ready weapon", "incant 515"]
signs = ["515"]
targets = [
  { name = "mastodon", routine = "b" },
  { any = true, routine = "a" },
]

[rooms]
hunting = 10
boundaries = [30]
resting = 20

[stance]
hunting = "offensive"
wander = "defensive"

[rest]
fried = 100
encumbered = 20
until = { experience = 90, mana = 50 }
when = { bleeding = true, health_at_most = 60 }
commands = ["store all"]

[flee]
count = 2

[loot]
delay = true

[wander]
wait = 3

[routines]
a = ["attack"]
b = ["fire (hidden)", "kweed (!expiring \"Tangleweed Vigor\" 5)", "volley", "coupdegrace (thp 20)", "fire"]

[sequences]
volley = ["store weapon", "weapon volley"]
"#;

/// The profile above. An error is a broken fixture, which every test unwraps
/// into a failure.
fn profile() -> Result<Profile, String> {
    Profile::parse(PROFILE)
}

/// A state at game second `second`, standing, in the game's room `room`.
fn state(second: u32, room: &str) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state.room.id = Some(room.to_owned());
    state.status.set("standing", true);
    // The game states who is here with every room; a room whose players
    // were never stated cannot be claimed (`claim_room`).
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state
}

/// A creature in the room, with these `<crtrStatus>` attributes, hostile
/// unless the attributes say otherwise (a status without `hostile="1"` is
/// a companion's, `hunt_replay.rs`).
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn creature(state: &mut GameState, id: i64, noun: &str, attrs: &[(&str, &str)]) {
    let mut run = Run {
        text: noun.to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: id.to_string(),
                noun: noun.to_owned(),
            },
            text: noun.to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    // Every creature already here stays in the list the room re-states.
    let mut runs: Vec<Run> = state
        .creatures()
        .in_room()
        .map(|kept| {
            let mut kept_run = run.clone();
            kept_run.text.clone_from(&kept.name);
            kept_run.link = Some(Link {
                kind: LinkKind::Exist {
                    id: kept.id.to_string(),
                    noun: kept.noun.clone().unwrap_or_default(),
                },
                text: kept.name.clone(),
                coord: None,
            });
            kept_run
        })
        .collect();
    runs.push(run);
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs },
    });
    let mut attrs: Vec<(String, String)> = attrs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    attrs.insert(0, ("exist".to_owned(), id.to_string()));
    if !attrs.iter().any(|(k, _)| k == "hostile") {
        attrs.push(("hostile".to_owned(), "1".to_owned()));
    }
    state.apply(&Frame::CreatureStatus {
        id: id.to_string(),
        attrs,
    });
}

fn here(room: u32, exits: &[RoomId]) -> Here<'_> {
    Here {
        room: Some(RoomId(room)),
        exits,
        tags: &[],
    }
}

fn send(line: &str, target: Option<i64>) -> Said {
    Said::Send {
        line: line.to_owned(),
        target,
    }
}

const NO_EXITS: &[RoomId] = &[];

#[test]
fn survival_comes_first() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut dead = state(1_000, "10");
    dead.status.set("dead", true);
    assert_eq!(
        hunt.tick(&dead, here(10, NO_EXITS), Some(1_000)),
        Said::Done(Ending::Dead)
    );

    let mut down = state(1_000, "10");
    down.status.set("standing", false);
    down.status.set("prone", true);
    assert_eq!(
        hunt.tick(&down, here(10, NO_EXITS), Some(1_000)),
        send("stand", None)
    );
    down.status.set("stunned", true);
    assert_ne!(
        hunt.tick(&down, here(10, NO_EXITS), Some(1_000)),
        send("stand", None),
        "stunned: standing waits"
    );
}

#[test]
fn engage_targets_takes_the_stance_and_runs_the_routine() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    creature(
        &mut state,
        42,
        "mastodon",
        &[("health", "50"), ("maxhealth", "100")],
    );
    let at = here(10, NO_EXITS);

    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("target #42", Some(42))
    );
    assert_eq!(hunt.target(), Some(42));
    state.targeting.read("#42", None);

    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("stance offensive", Some(42)),
        "the hunting stance before the first step"
    );
    state.character.stance = Some("offensive".to_owned());
    state.character.stance_percent = Some(0);

    // Step 1 `fire (hidden)`: hidden is unknown, so it is skipped. Step 2
    // `kweed (!expiring ...)`: nothing listed, unknown, skipped. Step 3 is
    // the sequence `volley`, expanded in place.
    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("store weapon", Some(42))
    );
    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("weapon volley", Some(42))
    );
    // Step 4 `coupdegrace (thp 20)`: health is 50%, skipped. Step 5 `fire`.
    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("fire #42", Some(42))
    );
    // Round again: hidden now known, so step 1 runs.
    state.status.set("hidden", true);
    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("fire #42", Some(42))
    );
}

#[test]
fn the_routine_follows_the_target_list_and_the_catch_all() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    creature(&mut state, 7, "berserker", &[]);
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        send("target #7", Some(7))
    );
    state.targeting.read("#7", None);
    state.character.stance = Some("offensive".to_owned());
    state.character.stance_percent = Some(0);
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        send("attack", Some(7)),
        "the catch-all's routine"
    );
    // A listed creature arrives: the current target is kept while it is here.
    creature(&mut state, 42, "mastodon", &[]);
    hunt.tick(&state, here(10, NO_EXITS), Some(1_000));
    assert_eq!(
        hunt.target(),
        Some(7),
        "no switch while the current target stands"
    );
}

#[test]
fn loot_once_per_corpse_with_a_target_standing_or_the_room_clear() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    creature(
        &mut state,
        42,
        "mastodon",
        &[("health", "0"), ("maxhealth", "100")],
    );
    creature(
        &mut state,
        43,
        "mastodon",
        &[("health", "100"), ("maxhealth", "100")],
    );
    // A corpse by hit points, a live target beside it: `loot.delay` takes
    // the first corpse at once (bigshot's timer passes on its first call,
    // and Nisugi's log has the search two seconds after the kill), then
    // the fight goes on.
    let at = here(10, NO_EXITS);
    assert_eq!(hunt.tick(&state, at, Some(1_000)), send("loot #42", None));
    assert_eq!(
        hunt.tick(&state, at, Some(1_001)),
        send("target #43", Some(43)),
        "then the live one"
    );

    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut alone = self::state(1_000, "10");
    creature(
        &mut alone,
        42,
        "mastodon",
        &[("health", "0"), ("maxhealth", "100")],
    );
    assert_eq!(
        hunt.tick(&alone, here(10, NO_EXITS), Some(1_000)),
        send("loot #42", None)
    );
    assert_ne!(
        hunt.tick(&alone, here(10, NO_EXITS), Some(1_000)),
        send("loot #42", None),
        "looted once"
    );
}

#[test]
fn maintain_casts_a_sign_the_game_says_is_down() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    let at = here(10, NO_EXITS);
    assert_ne!(
        hunt.tick(&state, at, Some(1_000)),
        send("incant 515", None),
        "the effects have never been stated: unknown, no cast"
    );
    state.effects.clear_category("Active Spells");
    assert_eq!(hunt.tick(&state, at, Some(1_000)), send("incant 515", None));
    assert_ne!(
        hunt.tick(&state, at, Some(1_010)),
        send("incant 515", None),
        "not asked again within the retry window"
    );
    state.effects.insert(
        "515".to_owned(),
        Effect {
            category: "Active Spells".to_owned(),
            text: "Rapid Fire".to_owned(),
            ends_at: Some(2_000),
            percent: 100,
        },
    );
    assert_ne!(
        hunt.tick(&state, at, Some(1_100)),
        send("incant 515", None),
        "up"
    );
}

#[test]
fn assume_aspect_is_cast_one_step_a_tick_and_confirmed_by_the_buffs() {
    let mut profile = profile().unwrap();
    profile.signs = vec!["650 panther evoke".to_owned()];
    let mut hunt = Hunt::new(profile, 1);
    let mut state = state(1_000, "10");
    let at = here(10, NO_EXITS);
    assert_ne!(
        hunt.tick(&state, at, Some(999)),
        send("incant 650 evoke", None),
        "no list seen yet, so nothing is known to be down (found by `hunt_replay`)"
    );
    state.effects.clear_category("Buffs");
    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("incant 650 evoke", None),
        "the spell first, evoked (`cmd_assume`)"
    );
    assert_ne!(
        hunt.tick(&state, at, Some(1_000)),
        send("incant 650 evoke", None),
        "a cast that has not landed is not asked for again within the retry window"
    );
    state.effects.insert(
        "650".to_owned(),
        Effect {
            category: "Buffs".to_owned(),
            text: "Assume Aspect".to_owned(),
            ends_at: Some(1_600),
            percent: 100,
        },
    );
    assert_eq!(
        hunt.tick(&state, at, Some(1_001)),
        send("assume panther", None)
    );
    assert_ne!(
        hunt.tick(&state, at, Some(1_002)),
        send("assume panther", None),
        "not asked again within the retry window"
    );
    state.effects.insert(
        "9650".to_owned(),
        Effect {
            category: "Buffs".to_owned(),
            text: "Aspect of the Panther".to_owned(),
            ends_at: Some(2_600),
            percent: 100,
        },
    );
    assert_ne!(
        hunt.tick(&state, at, Some(1_100)),
        send("incant 650 evoke", None),
        "an aspect up: nothing to cast"
    );
}

#[test]
fn delayed_looting_takes_the_first_corpse_at_once_and_spaces_the_rest() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    state.effects.clear_category("Buffs");
    creature(&mut state, 41, "warg", &[("hostile", "1")]);
    creature(&mut state, 42, "warg", &[("hostile", "1"), ("dead", "1")]);
    creature(&mut state, 43, "warg", &[("hostile", "1"), ("dead", "1")]);
    let at = here(10, NO_EXITS);
    // bigshot's `time_between(:need_to_loot?, 15)` passes on its first call.
    assert_eq!(hunt.tick(&state, at, Some(1_000)), send("loot #42", None));
    assert_ne!(
        hunt.tick(&state, at, Some(1_010)),
        send("loot #43", None),
        "the second corpse waits while the live warg stands"
    );
    assert_eq!(hunt.tick(&state, at, Some(1_015)), send("loot #43", None));
}

#[test]
fn with_a_loot_profile_corpses_go_to_the_planner_and_full_bags_send_the_hunt_to_rest() {
    use cena_behavior::loot::{Left, LootProfile};
    let mut hunt = Hunt::new(profile().unwrap(), 1).with_loot(LootProfile::default());
    let mut state = state(1_000, "10");
    state.effects.clear_category("Buffs");
    creature(&mut state, 42, "warg", &[("hostile", "1"), ("dead", "1")]);
    creature(&mut state, 43, "warg", &[("hostile", "1"), ("dead", "1")]);
    let at = here(10, &[RoomId(20)]);
    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        Said::Loot(vec![42, 43]),
        "every corpse here, once, for the planner"
    );
    assert!(
        !matches!(hunt.tick(&state, at, Some(1_001)), Said::Loot(_)),
        "not asked for again"
    );
    hunt.loot_ended(Left::BagsFull);
    assert_eq!(
        hunt.tick(&state, at, Some(1_002)),
        Said::Walk(RoomId(20)),
        "too much loot: to the resting room"
    );
    assert_eq!(
        hunt.phase(),
        Phase::ToRest(cena_behavior::hunt::engine::Why::Loaded)
    );
    assert!(
        hunt.take_notes()
            .iter()
            .any(|n| n.contains("too much loot"))
    );
}

#[test]
fn wander_waits_then_walks_to_a_fresh_room_inside_the_boundaries() {
    let mut hunt = Hunt::new(profile().unwrap(), 7);
    let mut state = state(1_000, "10");
    state.character.stance = Some("defensive".to_owned());
    state.character.stance_percent = Some(100);
    let exits = [RoomId(11), RoomId(30)];
    assert_eq!(
        hunt.tick(&state, here(10, &exits), Some(1_000)),
        Said::Wait(1),
        "wander.wait has not passed"
    );
    assert_eq!(
        hunt.tick(&state, here(10, &exits), Some(1_004)),
        Said::Walk(RoomId(11)),
        "30 is a boundary, so 11 is the only way"
    );
    // In room 11 with the way back and a new room: the fresh one wins.
    state.room.id = Some("11".to_owned());
    let exits = [RoomId(10), RoomId(12)];
    assert_eq!(
        hunt.tick(&state, here(11, &exits), Some(1_010)),
        Said::Wait(1),
        "just arrived"
    );
    assert_eq!(
        hunt.tick(&state, here(11, &exits), Some(1_014)),
        Said::Walk(RoomId(12))
    );
    // Nowhere fresh: the least recently visited.
    state.room.id = Some("12".to_owned());
    let exits = [RoomId(11), RoomId(10)];
    hunt.tick(&state, here(12, &exits), Some(1_020));
    assert_eq!(
        hunt.tick(&state, here(12, &exits), Some(1_024)),
        Said::Walk(RoomId(10))
    );
}

#[test]
fn flee_when_the_room_is_too_crowded() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    creature(&mut state, 1, "mastodon", &[]);
    creature(&mut state, 2, "mastodon", &[]);
    creature(&mut state, 3, "mastodon", &[]);
    let exits = [RoomId(11)];
    assert_eq!(
        hunt.tick(&state, here(10, &exits), Some(1_000)),
        Said::Walk(RoomId(11))
    );
}

/// The profile above with `count = 2` in `[flee]` replaced by `flee`, and
/// `top` added before the first table.
fn profile_with(top: &str, flee: &str) -> Result<Profile, String> {
    Profile::parse(&format!("{top}\n{}", PROFILE.replace("count = 2", flee)))
}

#[test]
fn an_uncounted_creature_does_not_crowd_the_room_and_is_still_fought() {
    // `invalid_targets`, "but don't count these" (`bigshot.lic:3484`).
    let mut state = state(1_000, "10");
    creature(&mut state, 1, "rat", &[]);
    creature(&mut state, 2, "rat", &[]);
    let exits = [RoomId(11)];

    let mut counted = Hunt::new(profile_with("", "count = 1").unwrap(), 1);
    assert_eq!(
        counted.tick(&state, here(10, &exits), Some(1_000)),
        Said::Walk(RoomId(11)),
        "two rats counted: more than one, so the room is crowded"
    );

    let mut hunt = Hunt::new(
        profile_with("", "count = 1\nuncounted = [\"rat\"]").unwrap(),
        1,
    );
    let said = hunt.tick(&state, here(10, &exits), Some(1_000));
    assert!(
        matches!(hunt.target(), Some(1 | 2)),
        "neither rat counts, so no flight, and the catch-all fights one: {said:?}"
    );
}

#[test]
fn a_never_attack_creature_is_neither_fought_nor_counted() {
    let mut hunt = Hunt::new(
        profile_with("never_attack = [\"rat\"]", "count = 1").unwrap(),
        1,
    );
    let mut state = state(1_000, "10");
    creature(&mut state, 1, "rat", &[]);
    creature(&mut state, 2, "rat", &[]);
    let exits = [RoomId(11)];
    assert_eq!(
        hunt.tick(&state, here(10, &exits), Some(1_000)),
        Said::Wait(1),
        "nothing to fight and nothing crowding: waiting out `wander.wait`, not flight"
    );
    assert_eq!(hunt.target(), None);
}

#[test]
fn hostile_creatures_off_the_target_list_count_toward_fleeing() {
    // bigshot counts its whole hostile roster against `flee_count`, not only
    // what its target list names (`bigshot.lic:8579-8591`).
    let only_mastodons = PROFILE
        .replace("  { any = true, routine = \"a\" },\n", "")
        .replace("count = 2", "count = 1");
    let mut hunt = Hunt::new(Profile::parse(&only_mastodons).unwrap(), 1);
    let mut state = state(1_000, "10");
    creature(&mut state, 1, "mastodon", &[]);
    creature(&mut state, 2, "kobold", &[]);
    let exits = [RoomId(11)];
    assert_eq!(
        hunt.tick(&state, here(10, &exits), Some(1_000)),
        Said::Walk(RoomId(11)),
        "one mastodon to fight, but two hostile creatures here"
    );
}

#[test]
fn the_rest_cycle_end_to_end() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    state.character.experience.mind_percent = Some(100);
    let at_hunting = here(10, NO_EXITS);
    assert_eq!(
        hunt.tick(&state, at_hunting, Some(1_000)),
        Said::Walk(RoomId(20))
    );
    assert_eq!(
        hunt.phase(),
        Phase::ToRest(cena_behavior::hunt::engine::Why::Fried)
    );
    assert!(hunt.take_notes().iter().any(|n| n.contains("fried")));

    state.room.id = Some("20".to_owned());
    let at_rest = here(20, NO_EXITS);
    assert_eq!(
        hunt.tick(&state, at_rest, Some(1_100)),
        send("store all", None)
    );
    assert_eq!(
        hunt.tick(&state, at_rest, Some(1_100)),
        Said::Wait(5),
        "mind still above 90"
    );
    state.character.experience.mind_percent = Some(80);
    assert_eq!(
        hunt.tick(&state, at_rest, Some(1_200)),
        Said::Wait(5),
        "mana unknown keeps resting"
    );
    state.apply(&Frame::Prompt {
        time: "1300".into(),
        text: ">".into(),
    });
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "mana".to_owned(),
        dialog: None,
        percent: 60,
        text: "mana 60/100".to_owned(),
        amount: None,
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
    assert_eq!(
        hunt.tick(&state, at_rest, Some(1_300)),
        Said::Walk(RoomId(10))
    );
    assert_eq!(hunt.phase(), Phase::Returning);

    state.room.id = Some("10".to_owned());
    assert_eq!(
        hunt.tick(&state, at_hunting, Some(1_400)),
        send("ready weapon", None)
    );
    assert_eq!(
        hunt.tick(&state, at_hunting, Some(1_400)),
        send("incant 515", None)
    );
    hunt.tick(&state, at_hunting, Some(1_400));
    assert_eq!(hunt.phase(), Phase::Hunting);
}

#[test]
fn a_rest_with_no_resting_room_ends_the_hunt() {
    let mut profile = profile().unwrap();
    profile.rooms.resting = None;
    let mut hunt = Hunt::new(profile, 1);
    let mut state = state(1_000, "10");
    state.status.set("bleeding", true);
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        Said::Done(Ending::NoRestingRoom)
    );
}

/// Arriving to rest with a gem in the gem sack: the selling round first
/// (`plan/31` Stage 4), then the rest commands once home again.
#[test]
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
fn arriving_to_rest_with_something_to_sell_runs_the_round_first() {
    let mut town = toml::Table::new();
    town.insert(
        "sell_loot_types".to_owned(),
        toml::Value::Array(vec![toml::Value::String("gem".to_owned())]),
    );
    town.insert(
        "sell_container".to_owned(),
        toml::Value::Array(vec![toml::Value::String("gem".to_owned())]),
    );
    let loot = cena_behavior::loot::LootProfile {
        town,
        ..cena_behavior::loot::LootProfile::default()
    };
    let mut hunt = Hunt::new(profile().unwrap(), 1).with_loot(loot);
    let mut state = state(1_000, "10");
    state.character.experience.mind_percent = Some(100);
    // A gem sack on the stow list with a pearl in it.
    state.containers.apply(&ContainerEvent::StowListBegins);
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Gem,
        item: ItemRef {
            id: "901".to_owned(),
            noun: "sack".to_owned(),
            text: "sack".to_owned(),
        },
    });
    state.apply(&Frame::Container {
        id: "901".to_owned(),
        title: Some("My Sack".to_owned()),
        target: None,
    });
    state.apply(&Frame::ContainerItem {
        container_id: "901".to_owned(),
        content: Runs {
            runs: vec![Run {
                text: "black pearl".to_owned(),
                style: Default::default(),
                link: Some(Link {
                    kind: LinkKind::Exist {
                        id: "1".to_owned(),
                        noun: "pearl".to_owned(),
                    },
                    text: "black pearl".to_owned(),
                    coord: None,
                }),
                inner_link: None,
            }],
        },
    });
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        Said::Walk(RoomId(20))
    );
    state.room.id = Some("20".to_owned());
    assert_eq!(
        hunt.tick(&state, here(20, NO_EXITS), Some(1_100)),
        Said::Sell,
        "arrived with a gem to sell: the round before the rest"
    );
    assert_eq!(
        hunt.phase(),
        Phase::Selling(cena_behavior::hunt::engine::Why::Fried)
    );
    // Home again after the round: the rest commands.
    assert_eq!(
        hunt.tick(&state, here(20, NO_EXITS), Some(1_200)),
        send("store all", None)
    );
    assert_eq!(
        hunt.phase(),
        Phase::Resting(cena_behavior::hunt::engine::Why::Fried)
    );
}

#[test]
#[expect(
    clippy::default_trait_access,
    reason = "the image's attributes type is not re-exported for behaviors"
)]
fn arriving_to_rest_hurt_heals_with_herbs_first() {
    let heal = cena_behavior::heal::HealProfile {
        container: "herb pouch".to_owned(),
        ..cena_behavior::heal::HealProfile::default()
    };
    let mut hunt = Hunt::new(profile().unwrap(), 1).with_heal(heal);
    let mut state = state(1_000, "10");
    state.character.experience.mind_percent = Some(100);
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        Said::Walk(RoomId(20))
    );
    state.room.id = Some("20".to_owned());
    state.apply(&Frame::InjuryImage {
        id: "leftArm".to_owned(),
        name: "Injury1".to_owned(),
        dialog: Some("injuries".to_owned()),
        attrs: Default::default(),
    });
    assert_eq!(
        hunt.tick(&state, here(20, NO_EXITS), Some(1_100)),
        Said::Heal,
        "arrived hurt: the herbs before the rest"
    );
    assert_eq!(
        hunt.tick(&state, here(20, NO_EXITS), Some(1_200)),
        send("store all", None),
        "then the rest commands"
    );
}

#[test]
fn heal_alone_heals_once_and_ends() {
    let mut hunt = Hunt::heal_only(cena_behavior::heal::HealProfile::default(), false, false);
    let state = state(1_000, "10");
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        Said::Heal
    );
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_001)),
        Said::Done(Ending::Healed)
    );
}
