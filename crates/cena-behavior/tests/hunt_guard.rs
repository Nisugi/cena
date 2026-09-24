//! The guard vocabulary: how each word is written, what it reads, and that
//! unknown skips (`plan/30` §5, `plan/33`).

use cena_behavior::hunt::{Condition, Guard};
use cena_session::{Effect, Frame, GameState, Link, LinkKind, Run, Runs};

/// One guard, parsed. `None` is a broken fixture, which every test unwraps
/// into a failure.
fn one(text: &str) -> Option<Condition> {
    let mut group = Condition::parse_group(text).ok()?;
    (group.len() == 1).then(|| group.remove(0))
}

/// A state whose server clock reads `second`.
fn at(second: u32) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state
}

/// One creature in the room, with these `<crtrStatus>` attributes: the
/// `room objs` body names it in bold, then the status tag describes it.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors, and only its bold depth matters here"
)]
fn with_creature(mut state: GameState, id: i64, noun: &str, attrs: &[(&str, &str)]) -> GameState {
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
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: vec![run] },
    });
    let mut attrs: Vec<(String, String)> = attrs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    attrs.insert(0, ("exist".to_owned(), id.to_string()));
    state.apply(&Frame::CreatureStatus {
        id: id.to_string(),
        attrs,
    });
    state
}

/// A buff listed with this end time.
fn buff(state: &mut GameState, id: &str, text: &str, ends_at: u32) {
    state.effects.insert(
        id.to_owned(),
        Effect {
            category: "Buffs".to_owned(),
            text: text.to_owned(),
            ends_at: Some(ends_at),
            percent: 50,
        },
    );
}

#[test]
fn every_word_reads_back_as_written() {
    for text in [
        "hidden",
        "!hidden",
        "thp 20",
        "!thp 20",
        "empowered_below 30",
        "immobilized",
        "!immobilized",
        "expiring \"Tangleweed Vigor\" 5",
        "!expiring \"Tangleweed Vigor\" 5",
    ] {
        assert_eq!(one(text).unwrap().to_string(), text);
    }
    let group = Condition::parse_group("thp 20 empowered_below 30").unwrap();
    assert_eq!(group.len(), 2);
    assert_eq!(group[1].guard, Guard::EmpoweredBelow(30));
    assert!(!group[1].negated);
}

#[test]
fn what_is_refused_and_why() {
    let err = Condition::parse_group("hiden").unwrap_err();
    assert!(
        err.contains("`hiden` is not a guard") && err.contains("empowered_below N"),
        "names the word and lists the vocabulary: {err}"
    );
    assert!(
        Condition::parse_group("thp")
            .unwrap_err()
            .contains("needs a number")
    );
    assert!(
        Condition::parse_group("thp20")
            .unwrap_err()
            .contains("`thp20` is not a guard"),
        "bigshot's fused form is not Hydra's; the importer writes `thp 20`"
    );
    assert!(
        Condition::parse_group("expiring Vigor 5")
            .unwrap_err()
            .contains("double quotes")
    );
    assert!(
        Condition::parse_group("expiring \"Vigor")
            .unwrap_err()
            .contains("not closed")
    );
    assert!(
        Condition::parse_group("!")
            .unwrap_err()
            .contains("`!` needs")
    );
}

#[test]
fn hidden_is_the_status_the_game_reported() {
    let mut state = GameState::default();
    let hidden = one("hidden").unwrap();
    let not = one("!hidden").unwrap();
    assert_eq!(hidden.holds(&state, None), None, "never reported: unknown");
    assert_eq!(
        not.holds(&state, None),
        None,
        "and the negation is as unknown"
    );
    state.status.set("hidden", true);
    assert_eq!(hidden.holds(&state, None), Some(true));
    assert_eq!(not.holds(&state, None), Some(false));
    state.status.set("hidden", false);
    assert_eq!(hidden.holds(&state, None), Some(false));
    assert_eq!(not.holds(&state, None), Some(true));
}

#[test]
fn thp_needs_the_targets_health_to_be_known() {
    let stated = with_creature(
        at(1_000),
        42,
        "mastodon",
        &[("health", "10"), ("maxhealth", "100")],
    );
    assert_eq!(one("thp 20").unwrap().holds(&stated, Some(42)), Some(true));
    assert_eq!(one("thp 5").unwrap().holds(&stated, Some(42)), Some(false));
    assert_eq!(one("!thp 5").unwrap().holds(&stated, Some(42)), Some(true));

    let unknown = with_creature(at(1_000), 43, "zzyzx wobbler", &[]);
    assert_eq!(
        one("thp 20").unwrap().holds(&unknown, Some(43)),
        None,
        "no stated HP and no bestiary template: unknown, not full health"
    );
    assert_eq!(
        one("!thp 20").unwrap().holds(&unknown, Some(43)),
        None,
        "unknown skips both readings, as bigshot's does (`:4336`)"
    );
    assert_eq!(
        one("thp 20").unwrap().holds(&stated, None),
        None,
        "no target chosen"
    );
    assert_eq!(
        one("thp 20").unwrap().holds(&stated, Some(99)),
        None,
        "no such creature here"
    );
}

#[test]
fn immobilized_is_the_targets_flag() {
    let free = with_creature(at(1_000), 42, "mastodon", &[]);
    assert_eq!(
        one("immobilized").unwrap().holds(&free, Some(42)),
        Some(false)
    );
    assert_eq!(
        one("!immobilized").unwrap().holds(&free, Some(42)),
        Some(true)
    );
    let held = with_creature(at(1_000), 42, "mastodon", &[("immobile", "1")]);
    assert_eq!(
        one("immobilized").unwrap().holds(&held, Some(42)),
        Some(true)
    );
    assert_eq!(
        one("!immobilized").unwrap().holds(&held, Some(42)),
        Some(false)
    );
    assert_eq!(
        one("immobilized").unwrap().holds(&held, None),
        None,
        "no target chosen"
    );
}

#[test]
fn empowered_below_reads_the_strongest_empowered_up() {
    let below = one("empowered_below 30").unwrap();
    assert_eq!(below.holds(&GameState::default(), None), None, "no clock");
    let mut state = at(1_000);
    assert_eq!(
        below.holds(&state, None),
        None,
        "the Buffs dialog has not been stated"
    );
    state.effects.clear_category("Buffs");
    assert_eq!(
        below.holds(&state, None),
        Some(true),
        "stated, and nothing is up"
    );
    buff(&mut state, "9005", "Empowered (+20)", 1_060);
    assert_eq!(below.holds(&state, None), Some(true));
    buff(&mut state, "9006", "Empowered (+30)", 1_060);
    assert_eq!(below.holds(&state, None), Some(false));
    assert_eq!(
        one("!empowered_below 30").unwrap().holds(&state, None),
        Some(true)
    );
    buff(&mut state, "9006", "Empowered (+30)", 999);
    assert_eq!(
        below.holds(&state, None),
        Some(true),
        "an expired buff is not up"
    );
}

#[test]
fn expiring_is_the_named_effects_last_seconds() {
    let soon = one("expiring \"Tangleweed Vigor\" 5").unwrap();
    let not_soon = one("!expiring \"Tangleweed Vigor\" 5").unwrap();
    let mut state = at(1_000);
    assert_eq!(
        soon.holds(&state, None),
        None,
        "nothing listed at all: unknown"
    );
    buff(&mut state, "9015", "Tangleweed Vigor", 1_003);
    assert_eq!(soon.holds(&state, None), Some(true));
    assert_eq!(
        not_soon.holds(&state, None),
        Some(false),
        "kweed does not fire into the expiry window (`bigshot.lic:4240`)"
    );
    buff(&mut state, "9015", "Tangleweed Vigor", 1_030);
    assert_eq!(soon.holds(&state, None), Some(false));
    assert_eq!(not_soon.holds(&state, None), Some(true));
    buff(&mut state, "9015", "tangleweed vigor", 1_003);
    assert_eq!(
        soon.holds(&state, None),
        Some(true),
        "the name is matched ignoring case"
    );

    let mut other = at(1_000);
    buff(&mut other, "515", "Rapid Fire", 1_002);
    assert_eq!(
        not_soon.holds(&other, None),
        Some(true),
        "the buff being down is not expiring, so kweed runs (`plan/30` §5)"
    );
}
