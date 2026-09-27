//! `plan/47` steps 1 and 3: the hub, drawn from the web hub's own cards, in
//! two tabs, and what it asks of the binary. Driven through `egui_kittest`,
//! which finds widgets as a screen reader would; the last test renders the
//! hub and compares it with the images under `tests/snapshots/`
//! (`UPDATE_SNAPSHOTS=1` rewrites them).

use cena_gui::{Hub, HubAction, HubView, SHUT_DOWN_QUESTION, Tab};
use cena_ui::{
    GroupView, HubRequest, LifecycleView, MergedLine, RoundtimeView, SessionCard, StyledRun,
    VitalView, VitalsView,
};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// What the window would gather, and what the hub asked for.
#[derive(Default)]
struct Board {
    hub: Hub,
    cards: Vec<SessionCard>,
    offered: Vec<String>,
    merged: Vec<MergedLine>,
    said: Option<String>,
    windowed: Vec<u32>,
    asked: Vec<HubAction>,
}

impl Board {
    fn draw(&mut self, ui: &mut egui::Ui) {
        let view = HubView {
            cards: &self.cards,
            offered: &self.offered,
            merged: &self.merged,
            said: self.said.as_deref(),
            windowed: &self.windowed,
        };
        if let Some(action) = self.hub.show(ui, &view) {
            self.asked.push(action);
        }
    }
}

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
            detail: Some("not logged in: [auth] bad password".to_owned()),
        },
    );
    vec![ashryn, baelor, lorwyn]
}

/// A thought both live characters heard.
fn merged() -> Vec<MergedLine> {
    vec![MergedLine {
        id: "0".to_owned(),
        stream: "thoughts".to_owned(),
        runs: vec![StyledRun {
            text: "[General] Maravel: anyone hunting?".to_owned(),
            ..StyledRun::default()
        }],
        from: vec!["Ashryn".to_owned(), "Baelor".to_owned()],
    }]
}

/// Ashryn's play window open; Baelor's closed, so Baelor runs headless.
fn board() -> Board {
    Board {
        cards: cards(),
        offered: vec!["Orsen".to_owned()],
        merged: merged(),
        windowed: vec![0],
        ..Board::default()
    }
}

fn ask(request: HubRequest) -> HubAction {
    HubAction::Ask(request)
}

fn hub<'a>(board: Board) -> Harness<'a, Board> {
    Harness::builder()
        .with_size((560.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board)
}

#[test]
fn live_and_closed_are_apart_and_counted() {
    let mut harness = hub(board());
    assert!(harness.query_by_label("Ashryn").is_some());
    assert!(harness.query_by_label("Baelor").is_some());
    assert!(
        harness.query_by_label("Lorwyn").is_none(),
        "closed, so not live"
    );

    harness.get_by_label("Closed (1)").click();
    harness.run();
    assert_eq!(harness.state().hub.tab, Tab::Closed);
    assert!(harness.query_by_label("Lorwyn").is_some());
    assert!(harness.query_by_label("Ashryn").is_none());
    assert!(
        harness
            .query_by_label("Game closed — not logged in: [auth] bad password")
            .is_some(),
        "a closed card says why"
    );

    harness.get_by_label("Live (2)").click();
    harness.run();
    assert_eq!(harness.state().hub.tab, Tab::Live);
}

#[test]
fn a_card_says_what_a_player_glances_at() {
    let harness = hub(board());
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
    let mut harness = hub(Board::default());
    assert!(harness.query_by_label("No character is running.").is_some());
    harness.get_by_label("Closed (0)").click();
    harness.run();
    assert!(
        harness
            .query_by_label("No character has closed this run.")
            .is_some()
    );
}

/// Each live card quits its own character; a closed one reconnects or is
/// removed; an offered character is started by name.
#[test]
fn each_button_asks_for_its_own_character() {
    let mut harness = hub(board());
    harness.get_by_label("Start Orsen").click();
    harness.run();
    // Ashryn's Quit, then Baelor's: the second.
    if let Some(quit) = harness.get_all_by_label("Quit").nth(1) {
        quit.click();
    }
    harness.run();
    harness.get_by_label("Closed (1)").click();
    harness.run();
    harness.get_by_label("Reconnect").click();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [
            ask(HubRequest::Add("Orsen".to_owned())),
            ask(HubRequest::Remove(1)),
            ask(HubRequest::Reconnect(2)),
            ask(HubRequest::Remove(2)),
        ]
    );
}

/// A character running headless -- its play window closed -- offers to
/// open it again; one whose window is open does not.
#[test]
fn a_headless_character_offers_its_window() {
    let mut harness = hub(board());
    assert_eq!(
        harness.query_all_by_label("Open window").count(),
        1,
        "Baelor's only"
    );
    harness.get_by_label("Open window").click();
    harness.run();
    assert_eq!(harness.state().asked, [HubAction::Open(1)]);
}

/// Shutting down ends every character, so it asks first; keeping on asks
/// nothing.
#[test]
fn shut_down_asks_first() {
    let mut harness = hub(board());
    harness.get_by_label("Shut down").click();
    harness.run();
    assert!(harness.state().asked.is_empty(), "one click shuts nothing");
    assert!(harness.query_by_label(SHUT_DOWN_QUESTION).is_some());

    harness.get_by_label("Keep playing").click();
    harness.run();
    assert!(harness.query_by_label(SHUT_DOWN_QUESTION).is_none());
    assert!(harness.state().asked.is_empty());

    harness.state_mut().hub.confirm_shutdown();
    harness.run();
    harness.get_by_label("Shut down every character").click();
    harness.run();
    assert_eq!(harness.state().asked, [ask(HubRequest::Shutdown)]);
}

/// The merged streams, tagged with who heard each line, and the binary's
/// answer to the last request.
#[test]
fn the_hub_shows_the_merged_streams_and_the_last_answer() {
    let mut with_answer = board();
    with_answer.said = Some("Starting Orsen.".to_owned());
    let harness = hub(with_answer);
    assert!(
        harness
            .query_by_label("[Ashryn, Baelor] [General] Maravel: anyone hunting?")
            .is_some()
    );
    assert!(harness.query_by_label("Starting Orsen.").is_some());
    let quiet = hub(Board::default());
    assert!(quiet.query_by_label("Nothing yet.").is_some());
}

/// The hub as a player sees it, rendered and compared with the committed
/// images. Rendered on every OS CI runs (`plan/47` step 9): by WARP on
/// Windows, Metal on macOS, and lavapipe on Linux, which CI installs; the
/// images are Windows', and `kittest.toml` says how near the others must be.
#[test]
fn the_hub_as_drawn() {
    let mut harness = Harness::builder()
        .with_size((560.0, 520.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board());
    harness.run();
    harness.snapshot("hub_live");
    harness.get_by_label("Closed (1)").click();
    harness.run();
    harness.snapshot("hub_closed");
}
