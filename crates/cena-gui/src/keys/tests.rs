use super::*;

#[test]
fn a_chord_is_its_modifiers_and_its_winit_key() {
    let chord = Chord::parse("ctrl+Shift+f1").expect("parses");
    assert_eq!(chord.key, "F1");
    assert_eq!(chord.held, CTRL | SHIFT);
    assert_eq!(
        Chord::parse("num_8").map(|c| c.key),
        Ok("Numpad8".to_owned())
    );
    assert_eq!(
        Chord::parse("numpad8").map(|c| c.key),
        Ok("Numpad8".to_owned())
    );
    assert_eq!(Chord::parse("cmd+KeyW").map(|c| c.held), Ok(CMD));
    assert_eq!(Chord::parse("F13").map(|c| c.key), Ok("F13".to_owned()));
}

/// A chord is written with each modifier by its own name, in one order,
/// and reads back as the same chord: what the Keys page saves.
#[test]
fn a_chord_is_written_as_it_reads() {
    for (typed, written) in [
        ("shift+F2", "Shift+F2"),
        ("alt+F3", "Alt+F3"),
        ("cmd+F4", "Cmd+F4"),
        ("alt+cmd+shift+ctrl+num_8", "Ctrl+Shift+Alt+Cmd+Numpad8"),
    ] {
        let chord = Chord::parse(typed).expect("parses");
        assert_eq!(chord.written(), written);
        assert_eq!(Chord::parse(written), Ok(chord));
    }
}

/// A key this build cannot see is told why; a name nobody knows, and a
/// modifier nobody has, are told so.
#[test]
fn a_key_hydra_cannot_see_yet_says_so() {
    let Err(pause) = Chord::parse("Pause") else {
        panic!("Pause is not reachable yet");
    };
    assert!(pause.contains("cannot see Pause yet"), "{pause}");
    assert!(Chord::parse("Blarg").is_err());
    assert!(Chord::parse("hyper+F1").is_err());
}

#[test]
fn egui_keys_are_named_as_winit_names_them() {
    assert_eq!(winit_name(Key::A).as_deref(), Some("KeyA"));
    assert_eq!(winit_name(Key::Num1).as_deref(), Some("Digit1"));
    assert_eq!(winit_name(Key::F13).as_deref(), Some("F13"));
    assert_eq!(winit_name(Key::ArrowUp).as_deref(), Some("ArrowUp"));
    assert_eq!(winit_name(Key::Equals).as_deref(), Some("Equal"));
    assert_eq!(winit_name(Key::Plus), None, "a character, not a key");
}

/// A file binds what it can and says what it cannot: a key that types,
/// a key that does not exist; `numpad = "always"` for a Mac.
#[test]
fn a_file_binds_what_it_can_and_says_the_rest() {
    let (keybinds, problems) = Keybinds::read(
        r#"
numpad = "always"
[keys]
Numpad8 = "north"
"ctrl+Numpad8" = "go2 bank"
F5 = "look"
KeyA = "attack"
"ctrl+KeyA" = "stance offensive"
Nowhere = "x"
"#,
    );
    assert!(keybinds.numpad_always);
    assert_eq!(keybinds.len(), 4);
    assert_eq!(
        keybinds.line(&Chord::parse("F5").expect("parses")),
        Some("look")
    );
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("types")));
    assert_eq!(
        keybinds.numpad_caught(),
        HashSet::from(["num_8".to_owned()]),
        "the fork catches the bound numpad key, whatever the modifiers"
    );
    let (none, broken) = Keybinds::read("keys = 3");
    assert_eq!(none.len(), 0);
    assert_eq!(broken.len(), 1);
}

/// A bound key's press is taken from the input, so the command input never
/// sees it; an unbound one is left.
#[test]
fn a_bound_press_is_taken_and_the_rest_left() {
    let (keybinds, _) = Keybinds::read("[keys]\nF5 = \"look\"\n");
    let press = |key| egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let mut input = egui::InputState::default();
    input.events = vec![press(Key::F5), press(Key::A)];
    assert_eq!(keybinds.take(&mut input), ["look"]);
    assert_eq!(input.events, [press(Key::A)]);
}

/// A numpad press the fork caught sends its line, named as winit names it;
/// one it let through to be typed sends nothing.
#[test]
fn a_caught_numpad_press_sends_its_line() {
    let (keybinds, _) = Keybinds::read("[keys]\nNumpad8 = \"north\"\n");
    let event = |consumed| eframe::NumpadKeyEvent {
        physical_key: winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Numpad8),
        consumed,
        numlock_on: Some(false),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
        character: None,
    };
    assert_eq!(
        numpad_line(&keybinds, &event(true)).as_deref(),
        Some("north")
    );
    assert_eq!(numpad_line(&keybinds, &event(false)), None, "typed instead");
}

/// A binding that is not one command -- a newline, a carriage return, a
/// NUL -- is said, not bound: it would send more than one (the crate review
/// of 2026-09-28, R10); the rest still bind.
#[test]
fn a_binding_of_more_than_one_command_is_said() {
    let (keybinds, problems) = Keybinds::read(
        r#"
[keys]
F5 = "look"
F6 = "look\nkill"
F7 = "stand\r"
F8 = "x\u0000"
"#,
    );
    assert_eq!(keybinds.len(), 1, "{problems:?}");
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert!(
        problems
            .iter()
            .all(|problem| problem.contains("CR, LF or NUL"))
    );
}
