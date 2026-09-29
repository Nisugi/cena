//! The sending keys in a play window (`plan/52` step 3): the repeats, the
//! history, Escape clearing the input and left to a menu that is open,
//! `NumpadEnter` through the fork, and the fork told to catch nothing while
//! no play window has the keyboard.

use super::*;

/// What `seat`'s window echoed as typed, in order.
fn typed(seat: &Seat) -> Vec<String> {
    lock(&seat.story)
        .lines
        .iter()
        .filter_map(|(_, shown)| match shown {
            crate::story::Shown::Typed { line, .. } => Some(line.clone()),
            _ => None,
        })
        .collect()
}

/// The command input's text.
fn input(harness: &Harness<'_, App>) -> Option<String> {
    harness.get_by_role(Role::TextInput).value()
}

/// Ctrl+Enter sends the last command typed again and Alt+Enter the one
/// before it; `NumpadEnter`, caught by the fork, sends what is typed or, on
/// an empty line, repeats; Up walks back; Escape clears the input, but
/// closes a menu that is open instead.
#[test]
fn the_sending_keys() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    for line in ["look", "north"] {
        harness.get_by_role(Role::TextInput).type_text(line);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
    }
    harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Enter);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::ALT, egui::Key::Enter);
    harness.run();
    assert_eq!(typed(&seat), ["look", "north", "north", "look"]);
    assert_eq!(
        input(&harness).as_deref(),
        Some(""),
        "the input never saw them"
    );

    let numpad_enter = || keys::Chord::parse("NumpadEnter").expect("a key");
    harness.state_mut().caught.push(numpad_enter());
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("search");
    harness.run();
    harness.state_mut().caught.push(numpad_enter());
    harness.run();
    assert_eq!(
        typed(&seat),
        ["look", "north", "north", "look", "north", "search"],
        "an empty line repeats; a typed one is sent"
    );

    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    assert_eq!(input(&harness).as_deref(), Some("search"));
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert_eq!(input(&harness).as_deref(), Some(""), "cleared");

    harness.get_by_role(Role::TextInput).type_text("hide");
    harness.run();
    harness.get_by_label("Keys").click();
    harness.run();
    assert!(
        harness.query_by_label("Change the keys...").is_some(),
        "open"
    );
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(
        harness.query_by_label("Change the keys...").is_none(),
        "Escape closed the menu"
    );
    assert_eq!(
        input(&harness).as_deref(),
        Some("hide"),
        "and kept the line"
    );
}

/// Enter alone sends what is typed and is bound to nothing: the file says
/// so, and Enter with a modifier binds.
#[test]
fn enter_alone_is_not_bound() {
    let (keybinds, problems) =
        Keybinds::read("[keys]\nEnter = \"look\"\n\"Shift+Enter\" = \"hide\"\n");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with("Enter sends what is typed"),
        "{problems:?}"
    );
    let key = |written: &str| keys::Chord::parse(written).expect("a key");
    assert_eq!(keybinds.does(&key("Enter")), None);
    assert_eq!(
        keybinds.does(&key("Shift+Enter")),
        Some(&Macro::Send("hide".to_owned()))
    );
}

/// With no play window having the keyboard -- the hub has it, or another
/// program -- the fork is told again, to catch none of their keys.
#[test]
fn no_window_with_the_keyboard_catches_nothing() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let mut app = App::new(Sessions::new(runtime.handle().clone()));
    app.focused = Some((7, None));
    app.catch_again = false;
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.run();
    assert_eq!(harness.state().focused, None);
    assert!(harness.state().catch_again, "the fork told again");
    let (numpad, captured) = harness.state_mut().keys_to_catch(false, false);
    assert_eq!(
        numpad,
        Some(std::collections::HashSet::new()),
        "no numpad key"
    );
    assert!(captured.is_empty());
    let (_, captured) = harness.state_mut().keys_to_catch(false, true);
    assert_eq!(
        captured,
        std::collections::HashSet::from([keys::NUM_LOCK]),
        "but Clear on a Mac"
    );

    harness.state_mut().focused = Some((7, None));
    let (numpad, _) = harness.state_mut().keys_to_catch(false, false);
    assert!(
        numpad.is_some_and(|numpad| numpad.contains("num_enter")),
        "a play window's NumpadEnter"
    );
    let (numpad, captured) = harness.state_mut().keys_to_catch(true, false);
    assert_eq!(numpad, None, "the Keys page waits: every one");
    assert_eq!(captured, keys::capturable());
}

/// A key the fork caught that is bound to an action on the input waits, as
/// Escape does, while a menu is open: the line is kept.
#[test]
fn a_caught_key_on_the_input_waits_for_a_menu() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let mut app = App::new(sessions);
    app.keys = Keybinds::read("[keys]\nNumpad5 = { action = \"clear_input\" }\n").0;
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), app);
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("hide");
    harness.run();
    harness.get_by_label("Keys").click();
    harness.run();
    let numpad5 = keys::Chord::parse("Numpad5").expect("a key");
    harness.state_mut().caught.push(numpad5.clone());
    harness.run();
    assert_eq!(input(&harness).as_deref(), Some("hide"), "kept");
    harness.key_press(egui::Key::Escape);
    harness.run();
    harness.state_mut().caught.push(numpad5);
    harness.run();
    assert_eq!(
        input(&harness).as_deref(),
        Some(""),
        "the menu shut, cleared"
    );
}

/// Tab and Shift+Tab are the keys' -- they target -- and never move the
/// keyboard off the command input, as egui would.
#[test]
fn tab_keeps_the_keyboard_on_the_input() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let sessions = Sessions::new(runtime.handle().clone());
    sessions.seat_for_test(handle(), "Ashryn");
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.draw(ui), App::new(sessions));
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("hi");
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
    harness.run();
    assert!(
        harness.get_by_role(Role::TextInput).is_focused(),
        "still typing"
    );
    assert_eq!(input(&harness).as_deref(), Some("hi"), "no tab typed");
}
