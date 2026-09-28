//! `plan/47` steps 1 and 3: the hub, drawn from the web hub's own cards, in
//! two tabs, and what it asks of the binary; and `plan/49` Stage C's cards,
//! tiled and without red flashes. The third tab's are in `not_launched.rs`. Driven through `egui_kittest`,
//! which finds widgets as a screen reader would; the last test renders the
//! hub and compares it with the images under `tests/snapshots/`
//! (`UPDATE_SNAPSHOTS=1` rewrites them).

mod common;

use cena_gui::{CardWidth, HubAction, SHUT_DOWN_QUESTION, Tab};
use cena_ui::{HubRequest, LifecycleView};
use common::{Board, ask, board, hub};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

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
/// removed.
#[test]
fn each_button_asks_for_its_own_character() {
    let mut harness = hub(board());
    assert!(
        harness.query_by_label("Start Orsen").is_none(),
        "started from Launch"
    );
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

/// The hub's Settings button opens the one settings menu.
#[test]
fn the_hub_opens_the_settings_menu() {
    let mut harness = hub(board());
    harness.get_by_label("Settings").click();
    harness.run();
    assert_eq!(harness.state().asked, [HubAction::Settings]);
}

/// Each live card's Lich switch shows whether the character's own Lich
/// runs, and asks for it on or off (`plan/51`).
#[test]
fn each_card_switches_its_own_lich() {
    let mut harness = hub(Board {
        lich: vec![1],
        ..board()
    });
    // Ashryn's, off; then Baelor's, on.
    for switch in 0..2 {
        if let Some(lich) = harness.get_all_by_label("Lich").nth(switch) {
            lich.click();
        }
        harness.run();
    }
    assert_eq!(
        harness.state().asked,
        [HubAction::Lich(0, true), HubAction::Lich(1, false)]
    );
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

/// With no character playing -- only closed ones -- there is nothing to
/// lose, so one click shuts down.
#[test]
fn shut_down_with_nothing_playing_does_not_ask() {
    let mut closed = board();
    closed
        .cards
        .retain(|card| matches!(card.lifecycle, LifecycleView::Closed { .. }));
    let mut harness = hub(closed);
    harness.get_by_label("Shut down").click();
    harness.run();
    assert!(harness.query_by_label(SHUT_DOWN_QUESTION).is_none());
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
        .with_size((900.0, 520.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board());
    harness.run();
    harness.snapshot("hub_live");
    harness.get_by_label("Closed (1)").click();
    harness.run();
    harness.snapshot("hub_closed");
    harness.get_by_label("Not launched (2)").click();
    harness.run();
    harness.snapshot("hub_not_launched");
    harness.get_by_label("New login").click();
    harness.run();
    harness.snapshot("hub_new_login");
}

/// egui's warnings that a widget changed its id in place -- each one a red
/// box in a debug build (`warn_if_rect_changes_id`,
/// `egui/src/context.rs:4177`) -- with the thread that drew each, since tests
/// running side by side log to the one logger.
type Heard = std::sync::Arc<std::sync::Mutex<Vec<(std::thread::ThreadId, String)>>>;

/// The logger that hears them.
struct Hearing(Heard);

impl log::Log for Hearing {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        let said = record.args().to_string();
        if said.contains("changed id between passes") {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((std::thread::current().id(), said));
        }
    }

    fn flush(&self) {}
}

/// Hear egui's warnings from here on. `log` allows one logger a process,
/// boxed and never dropped, so no `static` of ours holds it (Rule 5.2); one
/// test in this file sets it.
fn hear() -> Result<Heard, log::SetLoggerError> {
    let heard = Heard::default();
    log::set_boxed_logger(Box::new(Hearing(std::sync::Arc::clone(&heard))))?;
    log::set_max_level(log::LevelFilter::Warn);
    Ok(heard)
}

/// A change to the board, and what it is called.
type Change = (&'static str, fn(&mut Board));

/// The warnings this thread's frames drew since the last call.
fn renumbered(heard: &Heard) -> Vec<String> {
    let me = std::thread::current().id();
    let mut heard = heard
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mine, others): (Vec<_>, Vec<_>) = heard.drain(..).partition(|(thread, _)| *thread == me);
    *heard = others;
    mine.into_iter().map(|(_, said)| said).collect()
}

/// What changes above a widget -- the answer shown, the shut-down question,
/// a card come or gone -- leaves the widgets below with their ids, so a
/// debug build draws no red boxes (the author, 2026-09-27: *"red flashes
/// ... when the login window changes, clicking on shutdown, launching the
/// first character"*).
#[test]
fn nothing_below_is_renumbered_when_something_above_changes() {
    let heard = hear().expect("this file's one logger");
    for tab in ["Live (2)", "Not launched (2)", "New login"] {
        let mut quiet = board();
        quiet.merged.clear();
        let mut harness = hub(quiet);
        harness.get_by_label(tab).click();
        harness.run();
        let _ = renumbered(&heard);
        let changes: [Change; 3] = [
            ("the answer", |board| {
                board.said = Some("Starting Orsen.".to_owned());
            }),
            ("the shut-down question", |board| {
                board.hub.confirm_shutdown();
            }),
            ("a card gone", |board| {
                board.cards.remove(0);
            }),
        ];
        for (change, apply) in changes {
            apply(harness.state_mut());
            harness.run();
            let boxed = renumbered(&heard);
            assert!(boxed.is_empty(), "{tab}, {change}: {boxed:#?}");
        }
    }
}

/// Cards are as wide as their four bars and tile: side by side where the
/// hub is wide enough, one to a row where it is not.
#[test]
fn cards_tile_as_many_to_a_row_as_fit() {
    let top = |harness: &Harness<'_, Board>, name: &str| {
        harness
            .get_by_role_and_label(Role::Label, name)
            .rect()
            .min
            .y
    };
    let narrow = hub(board());
    assert!(
        top(&narrow, "Ashryn") < top(&narrow, "Baelor"),
        "one to a row"
    );
    let wide = Harness::builder()
        .with_size((900.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board());
    assert!(
        (top(&wide, "Ashryn") - top(&wide, "Baelor")).abs() < 0.5,
        "side by side"
    );
    let grip = wide
        .get_all_by_label("Card width")
        .next()
        .map(|grip| grip.rect());
    assert!(
        grip.is_some_and(|grip| grip.center().x < 400.0),
        "not stretched across the hub: {grip:?}"
    );
}

/// Dragging a card's side sets every card's width, and no narrower than
/// its least.
#[test]
fn a_cards_side_dragged_sets_every_cards_width() {
    let mut harness = hub(board());
    harness.run();
    let Some(side) = harness
        .get_all_by_label("Card width")
        .next()
        .map(|grip| grip.rect().center())
    else {
        panic!("a card's side");
    };
    let to = side + egui::vec2(60.0, 0.0);
    harness.hover_at(side);
    harness.step();
    harness.drag_at(side);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(side + (to - side) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.drop_at(to);
    harness.run();
    assert_eq!(
        harness.state().hub.card_width,
        CardWidth(CardWidth::FOUR_BARS.0 + 60.0)
    );
    let widths: Vec<f32> = harness
        .get_all_by_label("Card width")
        .map(|grip| grip.rect().center().x)
        .collect();
    assert!(
        widths.iter().all(|x| (x - (side.x + 60.0)).abs() < 0.5),
        "every card is wider: {widths:?}"
    );
    assert!(
        widths
            .windows(2)
            .all(|pair| (pair[0] - pair[1]).abs() < 0.5),
        "{widths:?}"
    );

    harness.state_mut().hub.card_width = CardWidth(CardWidth::NARROWEST);
    harness.run();
    let Some(side) = harness
        .get_all_by_label("Card width")
        .next()
        .map(|grip| grip.rect().center())
    else {
        panic!("a card's side");
    };
    harness.drag_at(side);
    harness.step();
    harness.hover_at(side - egui::vec2(100.0, 0.0));
    harness.step();
    harness.drop_at(side - egui::vec2(100.0, 0.0));
    harness.run();
    assert_eq!(
        harness.state().hub.card_width,
        CardWidth(CardWidth::NARROWEST),
        "no narrower"
    );
}
