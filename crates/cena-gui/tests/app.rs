//! `plan/47` step 3: the window asks the binary through the control it was
//! given, on the runtime, and shows the answer when it comes.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use cena_gui::{App, Sessions};
use cena_ui::{HubRequest, RosterCard};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

fn runtime() -> Option<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .ok()
}

/// Run frames until `label` is shown, for up to a second of wall time: the
/// answer is made on another thread.
fn until_shown(harness: &mut Harness<'_, App>, label: &str) -> bool {
    for _ in 0..100 {
        harness.run();
        if harness.query_by_label(label).is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

/// Orsen on the roster, its password kept, and so offered.
fn orsen(sessions: &Sessions) {
    sessions.offer(vec!["Orsen".to_owned()]);
    sessions.roster(vec![RosterCard {
        character: "Orsen".to_owned(),
        account: "orsen01".to_owned(),
        game: "GS3".to_owned(),
        kept: true,
        favourite: false,
    }]);
}

#[test]
fn a_start_reaches_the_binary_and_its_answer_is_shown() {
    let runtime = runtime().expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let heard = Arc::clone(&asked);
    sessions.control(Arc::new(move |request: HubRequest| {
        heard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Box::pin(async move { format!("handled {request:?}") })
    }));
    orsen(&sessions);
    let mut harness = Harness::builder()
        .with_size((560.0, 400.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));

    harness.get_by_label("Launch").click();
    harness.run();
    harness.get_by_label("Start Orsen").click();
    assert!(until_shown(&mut harness, "handled Add(\"Orsen\")"));
    assert_eq!(
        *asked.lock().unwrap_or_else(PoisonError::into_inner),
        [HubRequest::Add("Orsen".to_owned())]
    );
}

#[test]
fn with_nobody_to_answer_the_hub_says_so() {
    let runtime = runtime().expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    orsen(&sessions);
    let mut harness = Harness::builder()
        .with_size((560.0, 400.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.get_by_label("Launch").click();
    harness.run();
    harness.get_by_label("Start Orsen").click();
    assert!(until_shown(
        &mut harness,
        "Nothing here can start or stop characters."
    ));
}

/// With no character playing, the window closes when asked: there is
/// nothing to lose.
#[test]
fn an_empty_window_closes_without_asking() {
    let runtime = runtime().expect("a runtime");
    let mut app = App::new(Sessions::new(runtime.handle().clone()));
    assert!(app.close_asked());
}
