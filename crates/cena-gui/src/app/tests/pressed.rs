//! Keys pressed in a play window (`plan/47` step 7, `plan/52` step 1):
//! bound in the menu and binding at once, sent on the window's character
//! even on a frame drawn twice, a macro's waits, the input filled and an
//! action asked. Moved out of `app/tests.rs` at its cap.

use super::*;
use crate::keys::page::Place;

/// Set 0 of every character's keys.
const EVERY: Place = Place {
    set: 0,
    every: true,
};

/// A bound key pressed on a frame egui draws twice still sends its line:
/// the key is in the first pass alone, and what that pass asked was lost
/// with it.
#[test]
fn a_key_on_a_frame_drawn_twice_still_sends() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    let mut app = App::new(sessions);
    app.keys = Keybinds::read("[keys]\nF5 = \"look\"\n").0;
    let press = egui::Event::Key {
        key: egui::Key::F5,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let play = egui::ViewportId::from_hash_of(("play", seat.id.0));
    let context = windows_drawn_twice(&Arc::new(std::sync::Mutex::new(Some((play, vec![press])))));
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

/// A key bound on the *Keys* page is written to the keybinds file and
/// binds at once; while the page waits for a key, a numpad press goes
/// to it and to no play window.
#[test]
fn a_key_bound_in_the_menu_binds_at_once() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let data = std::env::temp_dir().join(format!("cena-app-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let app = App::keeping(Sessions::new(runtime.handle().clone()), &data);
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.state_mut().menu.open_for(None);
    harness.run();
    harness.get_by_label("Keys").click();
    harness.run();
    harness.get_by_label("Add a key").click();
    harness.run();
    assert!(harness.state().menu.waiting_for_key());
    let press = eframe::NumpadKeyEvent {
        physical_key: winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Numpad8),
        consumed: true,
        numlock_on: Some(true),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
        character: None,
    };
    harness
        .state_mut()
        .caught
        .push(keys::Chord::parse("F9").expect("a key"));
    harness.state_mut().caught_pressed(&[press], &[], false);
    assert_eq!(harness.state().caught_for_page.as_deref(), Some("Numpad8"));
    assert!(harness.state().caught.is_empty(), "no play window's");
    harness.run();
    assert!(!harness.state().menu.waiting_for_key());

    harness.state_mut().menu_asked(MenuAsked::Key {
        character: None,
        change: crate::KeyChange::Bind {
            key: "Numpad8".to_owned(),
            does: keys::Macro::Send("hide".to_owned()),
            was: None,
            place: EVERY,
        },
    });
    let numpad8 = keys::Chord::parse("Numpad8").expect("a key");
    assert_eq!(
        harness.state().keys.does(&numpad8),
        Some(&keys::Macro::Send("hide".to_owned()))
    );
    assert!(
        std::fs::read_to_string(keys::path(&data))
            .is_ok_and(|text| text.contains("\"Numpad8\" = \"hide\"")),
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// A bound key sends its line on the character whose window has it, as
/// if typed there; the command input never sees the key.
#[test]
fn a_bound_key_sends_on_its_window() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let mut app = App::new(sessions);
    app.keys = Keybinds::read("[keys]\nF5 = \"look\"\n").0;
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.run();
    harness.key_press(egui::Key::F5);
    harness.run();
    harness.run();
    assert!(harness.query_by_label(">look").is_some());
    assert_eq!(
        harness.get_by_role(Role::TextInput).value().as_deref(),
        Some(""),
        "the input never saw it"
    );
}

/// The commands a key's macro sends, in order: those before a wait at
/// once, the rest once it is over, and a wait's frame asked for.
#[test]
fn a_macro_waits_between_its_commands() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    let seats = sessions.seated();
    let mut app = App::new(sessions);
    let typed = |seat: &Arc<Seat>| -> Vec<String> {
        lock(&seat.story)
            .lines
            .iter()
            .filter_map(|(_, shown)| match shown {
                crate::story::Shown::Typed { line, .. } => Some(line.clone()),
                _ => None,
            })
            .collect()
    };
    let now = Instant::now();
    let made = keys::Macro::Send("stance off\rprep 118\rs1.5\rcast\rs1\rstance def".to_owned());
    app.send_macro(&seat, &made, now);
    assert_eq!(typed(&seat), ["stance off", "prep 118"]);
    assert_eq!(
        app.send_due(&seats, now + Duration::from_millis(1400)),
        Some(Duration::from_millis(100)),
        "nothing yet, and a frame asked for when the first is due"
    );
    assert_eq!(
        app.send_due(&seats, now + Duration::from_millis(1500)),
        Some(Duration::from_secs(1))
    );
    assert_eq!(typed(&seat), ["stance off", "prep 118", "cast"]);
    assert_eq!(app.send_due(&seats, now + Duration::from_secs(3)), None);
    assert_eq!(
        typed(&seat),
        ["stance off", "prep 118", "cast", "stance def"]
    );
}

/// A key that fills the input puts its text there, not sent, the cursor
/// at its end; one that acts asks what the window's own button asks.
#[test]
fn a_key_fills_the_input_or_acts() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let mut app = App::new(sessions);
    app.keys =
        Keybinds::read("[keys]\nF5 = { fill = \"prep 111 \" }\nF6 = { action = \"stop\" }\n").0;
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.run();
    harness.key_press(egui::Key::F5);
    harness.run();
    assert_eq!(
        harness.get_by_role(Role::TextInput).value().as_deref(),
        Some("prep 111 ")
    );
    assert!(harness.query_by_label(">prep 111").is_none(), "not sent");
    harness.get_by_role(Role::TextInput).type_text("kobold");
    harness.run();
    assert_eq!(
        harness.get_by_role(Role::TextInput).value().as_deref(),
        Some("prep 111 kobold"),
        "typed on at its end"
    );
    harness.key_press(egui::Key::F6);
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label(">;stop").is_some(),
        "Stop, as its button"
    );
}

/// A press of `code` the fork's key capture caught, with `modifiers`.
fn captured(
    code: winit::keyboard::KeyCode,
    modifiers: egui::Modifiers,
) -> eframe::CapturedKeyEvent {
    eframe::CapturedKeyEvent {
        physical_key: winit::keyboard::PhysicalKey::Code(code),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// A key egui has no name for, bound, is caught by the fork and does its
/// macro on the play window with the keyboard; while the Keys page waits
/// for a key, it goes to the page, so Pause can be bound by pressing it.
#[test]
fn a_key_egui_cannot_name_does_its_macro() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let mut app = App::new(Sessions::new(runtime.handle().clone()));
    app.keys =
        Keybinds::read("[keys]\n\"Ctrl+Pause\" = \"stance defensive\"\nScrollLock = \"hide\"\n").0;
    let code = winit::keyboard::KeyCode::Pause;
    assert_eq!(
        app.keys.of(None).key_capture(),
        std::collections::HashSet::from([code, winit::keyboard::KeyCode::ScrollLock]),
        "the fork is told the bound ones, whatever the modifiers"
    );
    app.caught_pressed(&[], &[captured(code, egui::Modifiers::CTRL)], false);
    assert_eq!(
        app.caught,
        keys::Chord::parse("Ctrl+Pause")
            .ok()
            .into_iter()
            .collect::<Vec<_>>()
    );
    app.caught_pressed(&[], &[captured(code, egui::Modifiers::NONE)], false);
    assert!(app.caught.is_empty(), "Pause alone is not bound");

    app.menu.open_for(None);
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.run();
    harness.get_by_label("Keys").click();
    harness.run();
    harness.get_by_label("Add a key").click();
    harness.run();
    harness
        .state_mut()
        .caught_pressed(&[], &[captured(code, egui::Modifiers::SHIFT)], false);
    assert_eq!(
        harness.state().caught_for_page.as_deref(),
        Some("Shift+Pause")
    );
    assert!(harness.state().caught.is_empty(), "no play window's");
}

/// On a Mac, Clear, which winit calls `NumLock`, switches the numpad
/// between typing and sending its keys, from what the file's `numpad`
/// says; elsewhere `NumLock` is the system's, and switches nothing here.
#[test]
fn clear_switches_the_numpad_on_a_mac() {
    use eframe::NumpadCaptureMode::{Always, NumLockAware, Off};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let mut app = App::new(Sessions::new(runtime.handle().clone()));
    let clear = || [captured(keys::NUM_LOCK, egui::Modifiers::NONE)];
    assert!(!app.clear_sends);
    app.caught_pressed(&[], &clear(), false);
    assert!(!app.clear_sends, "not on Windows or Linux");
    app.catch_again = false;
    app.caught_pressed(&[], &clear(), true);
    assert!(app.clear_sends, "switched");
    assert!(app.catch_again, "and the fork told again");
    assert!(app.caught.is_empty(), "Clear does no macro");
    app.caught_pressed(&[], &clear(), true);
    assert!(!app.clear_sends, "and back");

    let mode = super::super::keyed::numpad_mode;
    assert_eq!(
        mode(false, Some(false), false),
        Off,
        "a Mac types until Clear"
    );
    assert_eq!(mode(false, Some(true), false), Always);
    assert_eq!(mode(false, None, false), NumLockAware, "NumLock decides");
    assert_eq!(mode(false, None, true), Always, "numpad = \"always\"");
    assert_eq!(
        mode(true, Some(false), false),
        Always,
        "the Keys page waits"
    );
}

/// A character's own keys go over every character's in its play window,
/// and Alt with a digit chooses its macro set, kept in its own file and
/// shown on the window's bar.
#[test]
fn a_characters_keys_and_its_set() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let data = std::env::temp_dir().join(format!("cena-app-sets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).expect("a folder");
    std::fs::write(keys::path(&data), "[keys]\nF5 = \"look\"\nF6 = \"hide\"\n").expect("written");
    let mine =
        keys::character_path(&data, cena_session::DEFAULT_GAME_CODE, "Ashryn").expect("a path");
    std::fs::write(&mine, "[keys]\nF5 = \"search\"\n[set1]\nF6 = \"stand\"\n").expect("written");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let app = App::keeping(sessions, &data);
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.run();
    harness.key_press(egui::Key::F5);
    harness.key_press(egui::Key::F6);
    harness.run();
    harness.run();
    assert!(
        harness.query_by_label(">search").is_some(),
        "the character's own"
    );
    assert!(
        harness.query_by_label(">hide").is_some(),
        "every character's"
    );
    assert!(harness.query_by_label("Set 1").is_none());

    harness.key_press_modifiers(egui::Modifiers::ALT, egui::Key::Num1);
    harness.run();
    harness.run();
    assert!(
        std::fs::read_to_string(&mine).is_ok_and(|text| text.starts_with("set = 1\n")),
        "kept in its own file"
    );
    assert!(
        harness.query_by_label("Set 1").is_some(),
        "shown on the bar"
    );
    harness.key_press(egui::Key::F6);
    harness.run();
    harness.run();
    assert!(harness.query_by_label(">stand").is_some(), "set 1's F6");

    harness.key_press_modifiers(egui::Modifiers::ALT, egui::Key::Num0);
    harness.run();
    harness.run();
    assert!(harness.query_by_label("Set 1").is_none(), "set 0 alone");
    assert!(
        std::fs::read_to_string(&mine).is_ok_and(|text| !text.contains("set =")),
        "taken out of its file"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// A key made every character's leaves the character's file for the
/// keybinds file, and one made the character's alone goes back; a key
/// added on a character's page is its own.
#[test]
fn a_key_is_shared_and_taken_back() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let data = std::env::temp_dir().join(format!("cena-app-share-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let mut app = App::keeping(Sessions::new(runtime.handle().clone()), &data);
    let who = Some(format!("{}:Ashryn", cena_session::DEFAULT_GAME_CODE));
    let mine =
        keys::character_path(&data, cena_session::DEFAULT_GAME_CODE, "Ashryn").expect("a path");
    let read = |path: &std::path::Path| std::fs::read_to_string(path).unwrap_or_default();
    let look = keys::Macro::Send("look".to_owned());
    app.menu_asked(MenuAsked::Key {
        character: who.clone(),
        change: crate::KeyChange::Bind {
            key: "F5".to_owned(),
            does: look.clone(),
            was: None,
            place: Place {
                set: 2,
                every: false,
            },
        },
    });
    assert_eq!(read(&mine), "[set2]\n\"F5\" = \"look\"\n");
    let shared = |every| MenuAsked::Key {
        character: who.clone(),
        change: crate::KeyChange::Share {
            key: "F5".to_owned(),
            does: look.clone(),
            set: 2,
            every,
        },
    };
    app.menu_asked(shared(true));
    assert_eq!(read(&keys::path(&data)), "[set2]\n\"F5\" = \"look\"\n");
    assert!(!read(&mine).contains("F5"), "{}", read(&mine));
    app.menu_asked(shared(false));
    assert!(read(&mine).contains("\"F5\" = \"look\""));
    assert!(!read(&keys::path(&data)).contains("F5"));
    let _ = std::fs::remove_dir_all(&data);
}

/// The play window with the keyboard is the one whose keys the fork is told
/// to catch, its character's own among them: a window taking the keyboard
/// has the fork told again, and a key it caught is that character's.
#[test]
fn the_fork_catches_the_keys_of_the_window_with_the_keyboard() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let data = std::env::temp_dir().join(format!("cena-app-focus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).expect("a folder");
    let mine =
        keys::character_path(&data, cena_session::DEFAULT_GAME_CODE, "Ashryn").expect("a path");
    std::fs::write(&mine, "[keys]\nPause = \"hide\"\n").expect("written");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    let pause = || {
        [captured(
            winit::keyboard::KeyCode::Pause,
            egui::Modifiers::NONE,
        )]
    };
    let mut app = App::keeping(sessions, &data);
    app.caught_pressed(&[], &pause(), false);
    assert!(app.caught.is_empty(), "no window has had the keyboard");
    app.catch_again = false;
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.run();
    assert_eq!(
        harness.state().focused,
        Some((seat.id.0, Some(mine.clone()))),
        "Ashryn's window"
    );
    assert!(harness.state().catch_again, "the fork told again");
    harness.state_mut().caught_pressed(&[], &pause(), false);
    assert_eq!(
        harness.state().caught,
        keys::Chord::parse("Pause")
            .ok()
            .into_iter()
            .collect::<Vec<_>>(),
        "Ashryn's own key"
    );
    let _ = std::fs::remove_dir_all(&data);
}
