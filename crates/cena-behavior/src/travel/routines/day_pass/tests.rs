//! Table tests for the day pass, and for the pre-flight readers.

use cena_session::hands::Hand;
use cena_session::{ChunkLine, Frame, Link, LinkKind};

use super::super::testing::Scene;
use super::*;

/// 2026-09-21 14:03:22 at UTC-5.
const EXPIRES: i64 = 1_790_017_402;
const CALLIGRAPHY: &str = "Bold calligraphy states simply, \"This pass entitles the original \
    purchaser to one (1) day of unlimited travel between the towns of Wehnimer's Landing and \
    Icemule Trace, commencing at the time of purchase.\"";
const EXPIRY: &str = "[Your pass will expire on Mon Sep 21 14:03:22 ET 2026.]";
const STAMPED: &str = "Bold red block letters spelling out \"EXPIRED\" appear to have been \
    stamped across the face and reverse of the pass.";

fn pass_link(id: &str) -> Link {
    Link {
        kind: LinkKind::Exist {
            id: id.to_owned(),
            noun: "pass".to_owned(),
        },
        text: "Chronomage day pass".to_owned(),
        coord: None,
    }
}

/// `In the cloak you see …`, showing these passes.
fn sack_showing(ids: &[&str]) -> ChunkLine {
    let mut line = ChunkLine::plain("In the cloak you see a rock");
    for id in ids {
        let mut run = ChunkLine::plain("Chronomage day pass").runs.runs.remove(0);
        run.link = Some(pass_link(id));
        line.runs.runs.push(run);
    }
    line
}

/// A walker with a sack, at a time `before` seconds ahead of the expiry.
fn scene(buy: Option<&str>, before: i64) -> Scene {
    let mut scene = Scene::at(1, 2);
    let settings = &mut scene.walker.settings;
    settings.insert("day_pass_sack".into(), "cloak".into());
    if let Some(buy) = buy {
        settings.insert("buy_day_pass".into(), buy.into());
    }
    scene.state.apply(&Frame::Prompt {
        time: (EXPIRES - before).to_string(),
        text: ">".into(),
    });
    scene
}

fn put(command: &str) -> Next {
    Next::Put(command.to_owned())
}

fn steps(action: Action) -> Next {
    Next::Steps(vec![step(action)])
}

/// Up to the first pass being looked at.
fn opened(pass: &mut DayPass, buy: Option<&str>, before: i64, ids: &[&str]) {
    assert_eq!(scene(buy, before).ask(pass), put("look in my cloak"));
    let mut looked = scene(buy, before);
    looked.answer = vec![sack_showing(ids)];
    assert_eq!(looked.ask(pass), steps(Action::EmptyHands));
}

#[test]
fn a_valid_pass_in_the_sack_is_raised_and_put_back() {
    let mut pass = DayPass::new("wl,imt");
    opened(&mut pass, None, 3_600, &["77"]);
    assert_eq!(scene(None, 3_600).ask(&mut pass), put("look #77"));
    assert_eq!(
        scene(None, 3_600)
            .answered(&[CALLIGRAPHY, EXPIRY])
            .ask(&mut pass),
        put("get #77")
    );
    assert_eq!(scene(None, 3_600).ask(&mut pass), put("raise #77"));
    assert_eq!(scene(None, 3_600).ask(&mut pass), put("_drag #77 my cloak"));
    assert_eq!(scene(None, 3_600).ask(&mut pass), steps(Action::FillHands));
    assert_eq!(scene(None, 3_600).ask(&mut pass), Next::Done);
}

#[test]
fn an_expired_pass_is_dropped_and_the_next_one_used() {
    let mut pass = DayPass::new("imt,wl");
    opened(&mut pass, None, 3_600, &["5", "6", "7"]);
    assert_eq!(scene(None, 3_600).ask(&mut pass), put("look #5"));
    assert_eq!(
        scene(None, 3_600).answered(&[STAMPED]).ask(&mut pass),
        put("_drag #5 drop")
    );
    assert_eq!(scene(None, 3_600).ask(&mut pass), put("look #6"));
    // Not stamped, but eleven seconds past what it says.
    assert_eq!(
        scene(None, -11)
            .answered(&[CALLIGRAPHY, EXPIRY])
            .ask(&mut pass),
        put("_drag #6 drop")
    );
    assert_eq!(scene(None, 3_600).ask(&mut pass), put("look #7"));
    assert_eq!(
        scene(None, 3_600)
            .answered(&[CALLIGRAPHY, EXPIRY])
            .ask(&mut pass),
        put("get #7")
    );
}

#[test]
fn a_pass_inside_the_margin_is_neither_dropped_nor_used() {
    let mut pass = DayPass::new("wl,imt");
    opened(&mut pass, None, 5, &["7"]);
    assert_eq!(scene(None, 5).ask(&mut pass), put("look #7"));
    assert_eq!(
        scene(None, 5)
            .answered(&[CALLIGRAPHY, EXPIRY])
            .ask(&mut pass),
        steps(Action::FillHands)
    );
    assert_eq!(scene(None, 5).ask(&mut pass), Next::Failed);
}

#[test]
fn a_pass_for_other_towns_is_left_alone() {
    let mut pass = DayPass::new("wl,sol");
    opened(&mut pass, Some("wl,imt"), 3_600, &["7"]);
    assert_eq!(scene(None, 3_600).ask(&mut pass), put("look #7"));
    // The profile buys another route, not this one.
    assert_eq!(
        scene(Some("wl,imt"), 3_600)
            .answered(&[CALLIGRAPHY, EXPIRY])
            .ask(&mut pass),
        steps(Action::FillHands)
    );
    assert_eq!(scene(Some("wl,imt"), 3_600).ask(&mut pass), Next::Failed);
}

#[test]
fn none_and_not_buying_fails_with_the_hands_given_back() {
    let mut pass = DayPass::new("wl,imt");
    opened(&mut pass, Some("no"), 0, &[]);
    assert_eq!(
        scene(Some("no"), 0).ask(&mut pass),
        steps(Action::FillHands)
    );
    assert_eq!(scene(Some("no"), 0).ask(&mut pass), Next::Failed);
}

#[test]
fn no_sack_named_fails_at_once() {
    assert_eq!(
        Scene::at(1, 2).ask(&mut DayPass::new("wl,imt")),
        Next::Failed
    );
}

/// A scene where the clerk has handed the pass over.
fn handed(buy: &str) -> Scene {
    let mut scene = scene(Some(buy), 0).answered(&["The clerk quickly hands you a pass."]);
    scene.state.left_hand = Hand::read("Chronomage day pass", Some(&pass_link("88")));
    scene
}

#[test]
fn none_and_buying_asks_twice_and_raises_the_new_pass() {
    let mut pass = DayPass::new("wl,imt");
    opened(&mut pass, Some("Yes"), 0, &[]);
    assert_eq!(
        scene(Some("Yes"), 0).ask(&mut pass),
        Next::Go("south".into())
    );
    let mut hidden = scene(Some("Yes"), 0);
    hidden.walker.flags.insert("hidden".into(), true);
    assert_eq!(hidden.ask(&mut pass), put("unhide"));
    assert_eq!(
        scene(Some("Yes"), 0).ask(&mut pass),
        put("ask clerk for icemule")
    );
    assert_eq!(
        scene(Some("Yes"), 0)
            .answered(&["The clerk says to you, \"That will be 5000.\""])
            .ask(&mut pass),
        put("ask clerk for icemule")
    );
    assert_eq!(handed("Yes").ask(&mut pass), put("look #88"));
    assert_eq!(
        scene(Some("Yes"), 0).ask(&mut pass),
        Next::Go("north".into())
    );
    assert_eq!(scene(Some("Yes"), 0).ask(&mut pass), put("raise #88"));
    assert_eq!(
        scene(Some("Yes"), 0).ask(&mut pass),
        put("_drag #88 my cloak")
    );
    assert_eq!(
        scene(Some("Yes"), 0).ask(&mut pass),
        steps(Action::FillHands)
    );
    assert_eq!(scene(Some("Yes"), 0).ask(&mut pass), Next::Done);
}

/// Ask twice and be told there is not enough.
fn short_of_silver(pass: &mut DayPass, silvers: Option<&str>) -> Next {
    let poor = || {
        let mut scene = scene(Some("true"), 0).answered(&["You don't have enough silver."]);
        if let Some(silvers) = silvers {
            let settings = &mut scene.walker.settings;
            settings.insert("get_silvers".into(), silvers.into());
        }
        scene
    };
    opened(pass, Some("true"), 0, &[]);
    assert_eq!(poor().ask(pass), Next::Go("south".into()));
    assert_eq!(poor().ask(pass), put("ask clerk for solhaven"));
    assert_eq!(poor().ask(pass), put("ask clerk for solhaven"));
    poor().ask(pass)
}

#[test]
fn short_of_silver_walks_to_the_bank_once_and_only_once() {
    let mut pass = DayPass::new("wl,sol");
    let mut next = short_of_silver(&mut pass, Some("true"));
    let ask = |pass: &mut DayPass| {
        let mut scene = scene(Some("true"), 0).answered(&["You don't have enough silver."]);
        let settings = &mut scene.walker.settings;
        settings.insert("get_silvers".into(), "true".into());
        scene.ask(pass)
    };
    for dir in ["up", "north", "out", "north", "go bank", "go arch"] {
        assert_eq!(next, Next::Go(dir.into()));
        next = ask(&mut pass);
    }
    assert_eq!(next, put("withdraw 5000"));
    for dir in [
        "go arch",
        "out",
        "south",
        "go wood-sided shop",
        "south",
        "down",
    ] {
        assert_eq!(ask(&mut pass), Next::Go(dir.into()));
    }
    // Still short: no second trip.
    assert_eq!(ask(&mut pass), put("ask clerk for solhaven"));
    assert_eq!(ask(&mut pass), steps(Action::FillHands));
    assert_eq!(ask(&mut pass), Next::Failed);
}

#[test]
fn after_the_bank_a_quoted_price_is_asked_again_and_the_pass_taken() {
    let mut pass = DayPass::new("wl,sol");
    let mut next = short_of_silver(&mut pass, Some("yes"));
    let mut moves = 0;
    while let Next::Go(_) | Next::Put(_) = next {
        if next == put("ask clerk for solhaven") {
            break;
        }
        moves += 1;
        assert!(moves < 20, "the walk is bounded");
        next = scene(Some("true"), 0).ask(&mut pass);
    }
    assert_eq!(moves, 13, "six there, the withdrawal, six back");
    assert_eq!(
        scene(Some("true"), 0)
            .answered(&["The clerk says to you, \"5000.\""])
            .ask(&mut pass),
        put("ask clerk for solhaven")
    );
    assert_eq!(handed("true").ask(&mut pass), put("look #88"));
}

#[test]
fn short_of_silver_and_not_fetching_fails() {
    let mut pass = DayPass::new("wl,sol");
    assert_eq!(short_of_silver(&mut pass, None), steps(Action::FillHands));
    assert_eq!(scene(Some("true"), 0).ask(&mut pass), Next::Failed);
}

#[test]
fn a_failed_step_on_the_way_to_the_bank_ends_it() {
    let mut pass = DayPass::new("wl,sol");
    assert_eq!(
        short_of_silver(&mut pass, Some("true")),
        Next::Go("up".into())
    );
    let mut stuck = scene(Some("true"), 0);
    stuck.failed = true;
    assert_eq!(stuck.ask(&mut pass), steps(Action::FillHands));
    assert_eq!(scene(Some("true"), 0).ask(&mut pass), Next::Done);
}

#[test]
fn a_shut_sack_is_opened_and_shut_again() {
    let mut pass = DayPass::new("wl,imt");
    assert_eq!(scene(None, 0).ask(&mut pass), put("look in my cloak"));
    assert_eq!(
        scene(None, 0).answered(&["That is closed."]).ask(&mut pass),
        put("open my cloak")
    );
    assert_eq!(
        scene(None, 0)
            .answered(&["You open the cloak."])
            .ask(&mut pass),
        put("look in my cloak")
    );
    // Still said to be shut: it is not opened twice.
    assert_eq!(
        scene(None, 0).answered(&["That is closed."]).ask(&mut pass),
        steps(Action::EmptyHands)
    );
    assert_eq!(scene(None, 0).ask(&mut pass), steps(Action::FillHands));
    assert_eq!(scene(None, 0).ask(&mut pass), put("close my cloak"));
    assert_eq!(scene(None, 0).ask(&mut pass), Next::Failed);
}

#[test]
fn the_sack_is_sent_by_id_when_the_inventory_names_it() {
    let worn = |id: &str, name: &str, noun: &str| cena_session::InventoryItem {
        id: id.into(),
        parent: "player".into(),
        name: name.into(),
        noun: noun.into(),
        ..Default::default()
    };
    let mut scene = scene(None, 0);
    let items = [
        worn("3", "a dark cloak", "cloak"),
        worn("4", "a Grey Cloak", "x"),
    ];
    scene
        .state
        .inventory_snapshot
        .apply_snapshot("r", &items, &[], None);
    assert_eq!(scene.ask(&mut DayPass::new("wl,imt")), put("look in #3"));
    // By the end of the name, in any case, when no noun matches.
    let settings = &mut scene.walker.settings;
    settings.insert("day_pass_sack".into(), "grey cloak".into());
    assert_eq!(scene.ask(&mut DayPass::new("wl,imt")), put("look in #4"));
}

#[test]
fn each_route_has_its_clerk_its_step_and_what_to_ask_for() {
    let table = [
        ("wl,imt", "south", "ask clerk for icemule", "north", 6),
        ("wl,sol", "south", "ask clerk for solhaven", "north", 6),
        (
            "imt,wl",
            "go corridor",
            "ask halfling for wehnimer",
            "go corridor",
            17,
        ),
        (
            "imt,sol",
            "go corridor",
            "ask halfling for solhaven",
            "go corridor",
            17,
        ),
        ("sol,wl", "out", "ask agent for wehnimer", "go arch", 20),
        ("sol,imt", "out", "ask agent for icemule", "go arch", 20),
    ];
    for (route, step_in, ask, step_back, bank) in table {
        let mut pass = DayPass::new(route);
        opened(&mut pass, Some(route), 0, &[]);
        assert_eq!(
            scene(Some(route), 0).ask(&mut pass),
            Next::Go(step_in.into())
        );
        assert_eq!(scene(Some(route), 0).ask(&mut pass), put(ask), "{route}");
        assert_eq!(scene(Some(route), 0).ask(&mut pass), put(ask), "{route}");
        assert_eq!(handed(route).ask(&mut pass), put("look #88"));
        assert_eq!(
            scene(Some(route), 0).ask(&mut pass),
            Next::Go(step_back.into())
        );
        let from = pass
            .from
            .map(|town| (town.to_bank.len(), town.from_bank.len()));
        assert_eq!(from, Some((bank, bank)), "{route}");
    }
    assert_eq!(
        Scene::at(1, 2).ask(&mut DayPass::new("wl,rr")),
        Next::Failed
    );
}

#[test]
fn reading_a_pass_says_its_towns_and_when_it_ends() {
    let lines = |text: &[&str]| -> Vec<ChunkLine> {
        text.iter().map(|line| ChunkLine::plain(line)).collect()
    };
    let read = read_pass(&lines(&["You see nothing unusual.", CALLIGRAPHY, EXPIRY]));
    assert_eq!(
        read,
        Some(Pass {
            towns: Some(("Wehnimer's Landing".into(), "Icemule Trace".into())),
            expiry: Expiry::At(EXPIRES),
        })
    );
    let pass = read.unwrap_or_else(|| unreachable!());
    assert!(pass.serves("Icemule Trace", "Wehnimer's Landing", Some(EXPIRES - 11)));
    assert!(!pass.serves("Icemule Trace", "Wehnimer's Landing", Some(EXPIRES - 10)));
    assert!(!pass.serves("Solhaven", "Wehnimer's Landing", Some(0)));
    assert!(pass.serves("Icemule Trace", "Wehnimer's Landing", None));
    assert!(pass.lapsed(Some(EXPIRES + 11)) && !pass.lapsed(Some(EXPIRES + 10)));
    assert!(!pass.lapsed(None));

    let stamped = read_pass(&lines(&[STAMPED])).unwrap_or_else(|| unreachable!());
    assert!(stamped.lapsed(None) && !stamped.serves("Solhaven", "Icemule Trace", None));
    assert_eq!(read_pass(&lines(&["I could not find that."])), None);
    // A single-digit day is padded with a second space, and the epoch is 1970.
    assert_eq!(expiry_of("Thu Jan  1 00:00:00 ET 1970."), Some(18_000));
    assert_eq!(
        expiry_of("Thu Feb 29 12:00:00 ET 2024."),
        Some(1_709_226_000)
    );
    assert_eq!(expiry_of("Thu Jan  1 00:00:00 UTC 1970."), None);
}

#[test]
fn the_flag_names_both_towns_in_alphabetical_order() {
    let flag = |a, b| flag_for(a, b);
    assert_eq!(
        flag("Wehnimer's Landing", "Icemule Trace").as_deref(),
        Some("day_pass:imt,wl")
    );
    assert_eq!(
        flag("Icemule Trace", "Wehnimer's Landing").as_deref(),
        Some("day_pass:imt,wl")
    );
    assert_eq!(
        flag("Wehnimer's Landing", "Solhaven").as_deref(),
        Some("day_pass:sol,wl")
    );
    assert_eq!(
        flag("Solhaven", "Icemule Trace").as_deref(),
        Some("day_pass:imt,sol")
    );
    assert_eq!(flag("Solhaven", "Solhaven"), None);
    assert_eq!(flag("Solhaven", "Ta'Illistim"), None);
}

#[test]
fn only_day_passes_are_found_in_the_sack_and_each_once() {
    let mut line = sack_showing(&["5", "5", "6"]);
    let mut other = ChunkLine::plain("hall pass").runs.runs.remove(0);
    other.link = Some(Link {
        text: "hall pass".into(),
        ..pass_link("9")
    });
    line.runs.runs.push(other);
    assert_eq!(passes_in(&[line]), vec!["5".to_owned(), "6".to_owned()]);
}
