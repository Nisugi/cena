//! The window's tests: the hub, play windows, keybinds and the settings
//! menu, driven through [`App::draw`].

use super::*;
use crate::MenuAsked;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// A handle with no session behind it: session 0, as every test
/// handle is.
fn handle() -> cena_session::SessionHandle {
    cena_session::SessionHandle::new(
        tokio::sync::mpsc::channel(1).0,
        cena_session::GenerationCell::default(),
        tokio::sync::broadcast::channel(1).0,
    )
}

/// A character that starts gets its play window; closed, it runs
/// headless and its card offers the window again, which reopens it.
#[test]
fn a_character_gets_a_window_and_keeps_playing_without_it() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    assert!(
        harness.query_by_role(Role::TextInput).is_some(),
        "its window"
    );
    assert!(harness.query_by_label("Open window").is_none());

    if let Some(window) = harness.state_mut().plays.get_mut(&0) {
        window.open = false;
    }
    harness.run();
    assert!(harness.query_by_role(Role::TextInput).is_none(), "headless");
    harness.get_by_label("Open window").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_role(Role::TextInput).is_some(), "reopened");
}

/// The one settings menu opens from the hub on Hydra's own pages, and
/// from a play window on its character's (`plan/50` §7 steps 1 and 2),
/// in a window of its own.
#[test]
fn the_settings_menu_opens_from_the_hub_and_from_a_play_window() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let card = |character: &str| cena_ui::RosterCard {
        character: character.to_owned(),
        account: "acct".to_owned(),
        game: cena_session::DEFAULT_GAME_CODE.to_owned(),
        kept: true,
        favourite: false,
    };
    sessions.roster(vec![card("Baelor"), card("Ashryn")]);
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    assert!(!harness.state().menu.open);
    // The hub's is drawn first, then the play window's.
    if let Some(hubs) = harness.get_all_by_label("Settings").next() {
        hubs.click();
    }
    harness.run();
    assert!(harness.state().menu.open);
    assert_eq!(harness.state().menu.character(), None);
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Card width")
            .is_some(),
        "its window, on Hydra's own Window page"
    );

    harness.state_mut().menu.open = false;
    harness.run();
    if let Some(play) = harness.get_all_by_label("Settings").nth(1) {
        play.click();
    }
    harness.run();
    assert!(harness.state().menu.open);
    let ashryn = format!("{}:Ashryn", cena_session::DEFAULT_GAME_CODE);
    assert_eq!(harness.state().menu.character(), Some(ashryn.as_str()));
    assert!(harness.query_by_label("Reading the settings...").is_some());

    // The hub's opens Hydra's own again, whatever was showing.
    harness.state_mut().menu.open = false;
    harness.run();
    if let Some(hubs) = harness.get_all_by_label("Settings").next() {
        hubs.click();
    }
    harness.run();
    assert_eq!(harness.state().menu.character(), None);
}

/// The menu asks for a character's pages on its first opening even when
/// egui draws that frame twice and keeps only the second, as it does to
/// settle a new window's layout. The ask was lost with the first: the menu
/// had noted it as sent, and waited for pages that were never asked for (the
/// author, 2026-09-28: *"when clicking on settings for the first time it only
/// shows widget settings"*).
///
/// The settings menu is its own window here, as eframe draws it; in the
/// other tests it is drawn inside the hub, where every pass's asks are kept.
#[test]
fn the_menu_asks_even_when_its_first_frame_is_drawn_twice() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    sessions.roster(vec![cena_ui::RosterCard {
        character: "Ashryn".to_owned(),
        account: "acct".to_owned(),
        game: cena_session::DEFAULT_GAME_CODE.to_owned(),
        kept: true,
        favourite: false,
    }]);
    let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let heard = Arc::clone(&asked);
    sessions.control(Arc::new(move |request| {
        heard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request);
        Box::pin(async { String::new() })
    }));
    let mut app = App::new(sessions);
    let ashryn = format!("{}:Ashryn", cena_session::DEFAULT_GAME_CODE);
    app.menu.open_at(Some(ashryn.clone()), None);

    let context = windows_drawn_twice(&Arc::default());
    let _ = context.run_ui(egui::RawInput::default(), |ui| app.draw(ui));
    runtime.block_on(async {
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    });
    let asked = asked
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert!(
        asked.contains(&cena_ui::HubRequest::Settings(ashryn)),
        "{asked:?}"
    );
}

/// Events for one window's next frame, as a test hands them in.
type Given = Arc<std::sync::Mutex<Option<(egui::ViewportId, Vec<egui::Event>)>>>;

/// A context that draws each window as eframe does, in a pass of its own,
/// and draws every frame twice, keeping the second, as egui does when a
/// layout settles; what is put in `events` goes to the named window's next
/// first pass, as a click or a key is in the first pass alone.
fn windows_drawn_twice(events: &Given) -> egui::Context {
    let context = egui::Context::default();
    context.set_embed_viewports(false);
    let events = Arc::clone(events);
    egui::Context::set_immediate_viewport_renderer(move |context, viewport| {
        let mut input = egui::RawInput {
            viewport_id: viewport.ids.this,
            ..egui::RawInput::default()
        };
        input.viewports.insert(
            viewport.ids.this,
            egui::ViewportInfo {
                focused: Some(true),
                ..egui::ViewportInfo::default()
            },
        );
        let mut waiting = events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((_, given)) = waiting.take_if(|(id, _)| *id == viewport.ids.this) {
            input.events = given;
        }
        drop(waiting);
        let mut draw = viewport.viewport_ui_cb;
        let mut first = true;
        let _ = context.run_ui(input, |ui| {
            if std::mem::take(&mut first) {
                ui.ctx().request_discard("a layout settling");
            }
            draw(ui);
        });
    });
    context
}

/// A line typed and entered on a frame egui draws twice still goes: the
/// Enter is in the first pass alone, and the window's ask to send was lost
/// with it.
#[test]
fn a_line_entered_on_a_frame_drawn_twice_still_goes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    let mut app = App::new(sessions);
    let play = egui::ViewportId::from_hash_of(("play", seat.id.0));
    // Frames first, so the command input has the keyboard.
    let given: Given = Arc::default();
    let context = windows_drawn_twice(&given);
    for _ in 0..3 {
        let _ = context.run_ui(egui::RawInput::default(), |ui| app.draw(ui));
    }
    let enter = egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    *given
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some((play, vec![egui::Event::Text("look".to_owned()), enter]));
    let _ = context.run_ui(egui::RawInput::default(), |ui| app.draw(ui));
    let typed: Vec<String> = lock(&seat.story)
        .lines
        .iter()
        .filter_map(|(_, shown)| match shown {
            crate::story::Shown::Typed { line, .. } => Some(line.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(typed, ["look"]);
}

/// Hydra's own settings are read at start and kept: the card width a
/// drag leaves, and one typed on the *Window* page, which the hub takes
/// at once.
#[test]
fn hydras_own_settings_are_read_and_kept() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let data = std::env::temp_dir().join(format!("cena-app-own-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).expect("made");
    let file = data.join(crate::own::FILE);
    std::fs::write(&file, "card_width = 500.0\n").expect("written");
    let app = App::keeping(Sessions::new(runtime.handle().clone()), &data);
    assert_eq!(app.hub.card_width, crate::CardWidth(500.0));

    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.state_mut().hub.card_width = crate::CardWidth(420.0);
    harness.run();
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some("card_width = 420.0\n"),
        "kept once the drag let go"
    );
    harness.state_mut().menu_asked(MenuAsked::Own {
        key: "card_width".to_owned(),
        to: Some("380".to_owned()),
    });
    assert_eq!(harness.state().hub.card_width, crate::CardWidth(380.0));
    harness.run();
    assert_eq!(
        crate::own::Own::load(&data).card_width(),
        crate::CardWidth(380.0),
        "the hub did not put its old width back"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// With the setting on, a play window closes when its session does,
/// once: opened again from its card, it stays open. Off, the default, it
/// stays open with the character's last state.
#[test]
fn a_play_window_closes_with_its_session_when_asked() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    let closed = || LifecycleView::Closed { detail: None };
    lock(&seat.card).lifecycle = closed();
    harness.run();
    assert!(
        harness.query_by_role(Role::TextInput).is_some(),
        "off: open"
    );

    lock(&seat.card).lifecycle = LifecycleView::Ready;
    harness.run();
    harness
        .state_mut()
        .own
        .change("close_with_session", Some("on"))
        .expect("on");
    lock(&seat.card).lifecycle = closed();
    harness.run();
    assert!(harness.query_by_role(Role::TextInput).is_none(), "closed");
    if let Some(window) = harness.state_mut().plays.get_mut(&0) {
        window.open = true;
    }
    harness.run();
    assert!(
        harness.query_by_role(Role::TextInput).is_some(),
        "opened again, it stays"
    );
}

/// What the binary says of a character's hunt shows in its window's Hunt
/// pane, and goes when the hunt does.
#[test]
fn a_hunt_shows_in_its_window() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let told = sessions.clone();
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    assert!(harness.query_by_label("No hunt running.").is_some());
    told.hunt(
        cena_session::SessionId::FIRST,
        Some(cena_ui::HuntView {
            running: "ojandhaart".to_owned(),
            phase: "resting (out of mana)".to_owned(),
            doing: "waiting 5s".to_owned(),
            target: None,
            waiting: Some("mana 30%, wants 50%".to_owned()),
        }),
    );
    harness.run();
    assert!(
        harness
            .query_by_label("Waiting: mana 30%, wants 50%")
            .is_some()
    );
    told.hunt(cena_session::SessionId::FIRST, None);
    harness.run();
    assert!(harness.query_by_label("No hunt running.").is_some());
}

/// Typed before the window has seen the session, a line is echoed and
/// the player is told it did not go.
#[test]
fn a_line_before_any_snapshot_is_not_sent_and_says_so() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("look");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    assert!(harness.query_by_label(">look").is_some());
    assert!(
        harness
            .query_by_label("Not connected yet; nothing was sent.")
            .is_some()
    );
    // Stop is the character's own `;stop`, as if typed.
    harness.get_by_label("Stop").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label(">;stop").is_some());
    // The Lich switch is the character's own `;lich on`, as if typed:
    // the window's (the card's lies under the window here), then, with
    // the window closed, the card's.
    if let Some(lich) = harness.get_all_by_label("Lich").nth(1) {
        lich.click();
    }
    harness.run();
    harness.run();
    assert!(harness.query_by_label(">;lich on").is_some());
    if let Some(window) = harness.state_mut().plays.get_mut(&0) {
        window.open = false;
    }
    harness.run();
    harness.get_by_label("Lich").click();
    harness.run();
    harness.get_by_label("Open window").click();
    harness.run();
    harness.run();
    assert_eq!(harness.query_all_by_label(">;lich on").count(), 2);
}

/// A custom window saved as a preset from a play window is kept in the
/// library every character adds from, in its file beside the layouts.
#[test]
fn a_preset_saved_is_kept_for_every_character() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let data = std::env::temp_dir().join(format!("cena-app-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(
            |ui, app: &mut App| app.draw(ui),
            App::keeping(sessions, &data),
        );
    harness.run();
    harness.get_by_label("Left: ?").click_secondary();
    harness.run();
    harness.get_by_label("Save as preset...").click();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let kept = Library::load(Some(data.join("layouts")));
    let names: Vec<&str> = kept
        .presets()
        .iter()
        .map(|preset| preset.name.as_str())
        .collect();
    assert_eq!(names, ["Loadout"]);
    harness.get_by_label("Layout").click();
    harness.run();
    harness.get_by_label("Add a widget...").click();
    harness.run();
    harness.get_by_label("Forget").click();
    harness.run();
    harness.run();
    assert!(
        Library::load(Some(data.join("layouts")))
            .presets()
            .is_empty(),
        "forgotten"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// A window is drawn again soon while something counts down by itself:
/// a quarter of a second for the clocks, a second for an effect's time
/// left or the next pulse, and not at all when nothing does.
#[test]
fn a_window_is_drawn_again_while_something_counts_down() {
    let story = std::sync::Mutex::new(crate::story::Story::default());
    let mut quiet = crate::fixture::snapshot();
    quiet.state.roundtime_ends = None;
    assert_eq!(clocks_run(Some(&quiet), &story), None);
    let mut pulsing = quiet.clone();
    pulsing.state.apply(&cena_session::Frame::Pulse {
        mana: false,
        min: 46,
        max: 75,
    });
    assert_eq!(
        clocks_run(Some(&pulsing), &story),
        Some(Duration::from_secs(1))
    );
    let mut buffed = quiet.clone();
    let now = buffed.state.game_time_now().expect("a clock");
    buffed.state.effects.insert(
        "1".to_owned(),
        cena_session::Effect {
            category: "Buffs".to_owned(),
            text: "Rapid Fire".to_owned(),
            ends_at: Some(now + 60),
            percent: 100,
        },
    );
    assert_eq!(
        clocks_run(Some(&buffed), &story),
        Some(Duration::from_secs(1))
    );
    let mut struck = buffed.clone();
    struck.state.roundtime_ends = Some(now + 3);
    assert_eq!(
        clocks_run(Some(&struck), &story),
        Some(Duration::from_millis(250))
    );
}

/// A play window takes its character's layout for the seat's own game
/// (`plan/50` §6 item 6): here one without the Loadout window, over the one
/// kept under the name alone, which has it and its hands.
#[test]
fn a_play_window_takes_its_games_layout() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let data = std::env::temp_dir().join(format!("cena-app-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let layouts = data.join("layouts");
    let game = cena_session::instance(cena_session::DEFAULT_GAME_CODE);
    let mut own = crate::layout::Layout::fitted(egui::Vec2::new(980.0, 680.0));
    own.holders.retain(|holder| holder.title() != "Loadout");
    own.save(&layouts, game, "Ashryn").expect("saved");
    crate::layout::Layout::fitted(egui::Vec2::new(980.0, 680.0))
        .save(&layouts, None, "Ashryn")
        .expect("saved");

    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(
            |ui, app: &mut App| app.draw(ui),
            App::keeping(sessions, &data),
        );
    harness.run();
    assert!(
        harness.query_by_role(Role::TextInput).is_some(),
        "its window"
    );
    assert!(
        harness.query_by_label("Left: ?").is_none(),
        "the game's own"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// A bar's right-click opens the one menu on its character at the bar's own
/// page, where a change is made in the character's layout; the Keys menu
/// opens it at Hydra's Keys page (`plan/50` §7 step 8, as the author
/// corrected it).
#[test]
fn a_play_window_opens_the_menu_where_it_is_set() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    sessions.roster(vec![cena_ui::RosterCard {
        character: "Ashryn".to_owned(),
        account: "acct".to_owned(),
        game: cena_session::DEFAULT_GAME_CODE.to_owned(),
        kept: true,
        favourite: false,
    }]);
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    // The play window's, drawn after the hub card's.
    if let Some(bar) = harness.get_all_by_label_contains("HP ").last() {
        bar.click_secondary();
    }
    harness.run();
    harness.get_by_label("Settings...").click();
    harness.run();
    let ashryn = format!("{}:Ashryn", cena_session::DEFAULT_GAME_CODE);
    assert!(harness.state().menu.open);
    assert_eq!(harness.state().menu.character(), Some(ashryn.as_str()));
    let page = harness
        .state()
        .menu
        .page()
        .map(str::to_owned)
        .unwrap_or_default();
    assert!(page.starts_with("widget:"), "{page}");

    // A change on it is made in the character's layout.
    harness.state_mut().menu_asked(MenuAsked::Widget {
        page: page.clone(),
        key: "fills".to_owned(),
        to: Some("up".to_owned()),
    });
    let pages = harness.state().plays[&0].play.widget_pages(&[]);
    let fills = pages
        .iter()
        .find(|found| found.id == page)
        .and_then(|found| found.rows.iter().find(|row| row.key == "fills"))
        .map(|row| row.value.clone());
    assert_eq!(fills, Some(cena_ui::settings::Value::Text("up".to_owned())));

    harness.state_mut().menu.open = false;
    harness.run();
    harness.get_by_label("Keys").click();
    harness.run();
    harness.get_by_label("Change the keys...").click();
    harness.run();
    assert!(harness.state().menu.open);
    assert_eq!(harness.state().menu.character(), None);
    assert_eq!(harness.state().menu.page(), Some("keys"));
}

/// A menu asked for on a click goes quietly: not echoed in the story, as a
/// line the player typed is.
#[test]
fn a_menu_asked_for_is_not_echoed() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    sessions.send_quietly(&seat, "_menu #456 1".to_owned());
    sessions.send(&seat, "look".to_owned());
    let typed: Vec<String> = lock(&seat.story)
        .lines
        .iter()
        .filter_map(|(_, shown)| match shown {
            crate::story::Shown::Typed { line, .. } => Some(line.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(typed, ["look"]);
}

/// The drag key chosen on the *Window* page is the one every window reads
/// this frame (`carry.rs`).
#[test]
fn the_drag_key_chosen_is_the_windows() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    assert_eq!(crate::carry::key(&harness.ctx), egui::Modifiers::CTRL);
    harness
        .state_mut()
        .own
        .change("drag_with", Some("shift"))
        .expect("changed");
    harness.run();
    assert_eq!(crate::carry::key(&harness.ctx), egui::Modifiers::SHIFT);
}

/// A line that is not one command -- a key bound to two lines, say -- is
/// neither echoed nor sent, echoed or not, and is said in Hydra's pane (the
/// crate review of 2026-09-28, R10).
#[test]
fn a_line_of_two_commands_is_refused() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    sessions.send(&seat, "look\nkill".to_owned());
    sessions.send_quietly(&seat, "_drag #1 drop\r".to_owned());
    let story = lock(&seat.story);
    assert!(story.lines.is_empty(), "nothing echoed");
    let said: Vec<&str> = story
        .said
        .iter()
        .flat_map(cena_session::Notice::lines)
        .map(String::as_str)
        .collect();
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said.iter().all(|said| said.starts_with("Not sent:")),
        "{said:?}"
    );
}

mod pressed;
