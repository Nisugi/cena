//! Keys pressed in a play window (`plan/47` step 7, `plan/52` step 1):
//! bound in the menu and binding at once, sent on the window's character
//! even on a frame drawn twice, a macro's waits, the input filled and an
//! action asked. Moved out of `app/tests.rs` at its cap.

use super::*;

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
        .numpad
        .push(keys::Macro::Send("stale".to_owned()));
    harness.state_mut().numpad_pressed(&[press]);
    assert_eq!(harness.state().numpad_for_menu.as_deref(), Some("Numpad8"));
    assert!(harness.state().numpad.is_empty(), "no play window's");
    harness.run();
    assert!(!harness.state().menu.waiting_for_key());

    harness
        .state_mut()
        .menu_asked(MenuAsked::Key(crate::KeyChange::Bind {
            key: "Numpad8".to_owned(),
            does: keys::Macro::Send("hide".to_owned()),
            was: None,
        }));
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
