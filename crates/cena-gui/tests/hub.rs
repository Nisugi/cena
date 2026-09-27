//! `plan/47` step 1: the hub, drawn from the web hub's own cards, in two
//! tabs. Driven through `egui_kittest`, which finds widgets as a screen
//! reader would; the last test renders the hub and compares it with the
//! images under `tests/snapshots/` (`UPDATE_SNAPSHOTS=1` rewrites them).

use cena_gui::{Hub, Tab};
use cena_ui::{GroupView, LifecycleView, RoundtimeView, SessionCard, VitalView, VitalsView};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

type State = (Hub, Vec<SessionCard>);

fn vital(percent: u32) -> VitalView {
    VitalView {
        percent,
        current: None,
        max: None,
    }
}

fn card(session: &str, name: &str, lifecycle: LifecycleView) -> SessionCard {
    SessionCard {
        session: session.to_owned(),
        name: name.to_owned(),
        lifecycle,
        vitals: VitalsView {
            health: None,
            mana: None,
            stamina: None,
            spirit: None,
        },
        roundtime: RoundtimeView {
            ends_at: None,
            remaining_seconds: None,
        },
        room: None,
        group: None,
    }
}

/// Two characters live -- one hunting, leading the other, which is
/// reconnecting -- and one closed.
fn cards() -> Vec<SessionCard> {
    let mut ashryn = card("0", "Ashryn", LifecycleView::Ready);
    ashryn.vitals = VitalsView {
        health: Some(vital(100)),
        mana: Some(vital(80)),
        stamina: Some(vital(60)),
        spirit: None,
    };
    ashryn.roundtime.remaining_seconds = Some(3);
    ashryn.room = Some("Rawknuckle's, Watering Hole".to_owned());
    ashryn.group = Some(GroupView {
        leader: None,
        members: vec!["Baelor".to_owned()],
    });
    let baelor = card(
        "1",
        "Baelor",
        LifecycleView::Reconnecting {
            attempt: Some(2),
            retry_delay_ms: Some(2_000),
            detail: None,
        },
    );
    let lorwyn = card(
        "2",
        "Lorwyn",
        LifecycleView::Closed {
            detail: Some("login refused".to_owned()),
        },
    );
    vec![ashryn, baelor, lorwyn]
}

fn hub<'a>(cards: Vec<SessionCard>) -> Harness<'a, State> {
    Harness::builder().with_size((520.0, 300.0)).build_ui_state(
        |ui, (hub, cards): &mut State| hub.show(ui, cards),
        (Hub::default(), cards),
    )
}

#[test]
fn live_and_closed_are_apart_and_counted() {
    let mut harness = hub(cards());
    assert!(harness.query_by_label("Ashryn").is_some());
    assert!(harness.query_by_label("Baelor").is_some());
    assert!(
        harness.query_by_label("Lorwyn").is_none(),
        "closed, so not live"
    );

    harness.get_by_label("Closed (1)").click();
    harness.run();
    assert_eq!(harness.state().0.tab, Tab::Closed);
    assert!(harness.query_by_label("Lorwyn").is_some());
    assert!(harness.query_by_label("Ashryn").is_none());
    assert!(
        harness
            .query_by_label("Game closed — login refused")
            .is_some(),
        "a closed card says why"
    );

    harness.get_by_label("Live (2)").click();
    harness.run();
    assert_eq!(harness.state().0.tab, Tab::Live);
}

#[test]
fn a_card_says_what_a_player_glances_at() {
    let harness = hub(cards());
    assert!(harness.query_by_label("Ready").is_some());
    assert!(
        harness
            .query_by_label("Game reconnecting · attempt 2 · retry delay 2.0s")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("RT 3s · Rawknuckle's, Watering Hole · leading Baelor")
            .is_some()
    );
    // Ashryn's three reported gauges, and a `?` for each one never reported:
    // Ashryn's spirit and all four of Baelor's.
    for (gauge, cards) in [
        ("HP 100%", 1),
        ("MP 80%", 1),
        ("SP 60%", 1),
        ("HP ?", 1),
        ("Sp ?", 2),
    ] {
        assert_eq!(harness.query_all_by_label(gauge).count(), cards, "{gauge}");
    }
    assert!(
        harness.query_by_label("Room unknown").is_some(),
        "Baelor's room is not known"
    );
}

#[test]
fn an_empty_tab_says_so() {
    let mut harness = hub(Vec::new());
    assert!(harness.query_by_label("No character is running.").is_some());
    harness.get_by_label("Closed (0)").click();
    harness.run();
    assert!(
        harness
            .query_by_label("No character has closed this run.")
            .is_some()
    );
}

/// The hub as a player sees it, rendered and compared with the committed
/// images. Rendered where a software GPU adapter is certain: Windows ships
/// one, and CI's Linux job gains one at `plan/47` step 9.
#[test]
#[cfg_attr(
    not(windows),
    ignore = "rendered on Windows until CI has a software adapter elsewhere (plan/47 step 9)"
)]
fn the_hub_as_drawn() {
    let mut harness = Harness::builder()
        .with_size((520.0, 300.0))
        .wgpu()
        .build_ui_state(
            |ui, (hub, cards): &mut State| hub.show(ui, cards),
            (Hub::default(), cards()),
        );
    harness.run();
    harness.snapshot("hub_live");
    harness.get_by_label("Closed (1)").click();
    harness.run();
    harness.snapshot("hub_closed");
}
