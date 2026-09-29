use super::*;
use egui::Key;

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
    assert_eq!(keybinds.changed(), 4);
    assert_eq!(
        keybinds.does(&Chord::parse("F5").expect("parses")),
        Some(&Macro::Send("look".to_owned()))
    );
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("types")));
    let caught = keybinds.numpad_caught();
    assert!(
        caught.contains("num_8") && caught.contains("num_decimal"),
        "the fork catches each bound numpad key, whatever the modifiers: {caught:?}"
    );
    assert!(!caught.contains("num_enter"), "bound by nobody yet");
    let (hydras, broken) = Keybinds::read("keys = 3");
    assert_eq!(hydras, Keybinds::default(), "Hydra's alone");
    assert_eq!(broken.len(), 1);
}

/// With no file, Hydra's defaults bind: the numpad walks, and Shift with
/// it peers. The file's `""` unbinds one, and its own binding replaces
/// one; the rest stay Hydra's.
#[test]
fn hydras_defaults_bind_under_the_file() {
    let key = |written: &str| Chord::parse(written).expect("parses");
    let send = |line: &str| Macro::Send(line.to_owned());
    let hydras = Keybinds::default();
    assert_eq!(hydras.does(&key("Numpad8")), Some(&send("north")));
    assert_eq!(hydras.does(&key("Shift+Numpad0")), Some(&send("peer down")));
    assert_eq!(hydras.does(&key("NumpadAdd")), Some(&send("look")));
    assert_eq!(hydras.len(), 26);
    assert_eq!(hydras.changed(), 0);

    let (keybinds, problems) =
        Keybinds::read("[keys]\nNumpad8 = \"\"\nNumpad2 = \"go2 bank\"\nF5 = \"look\"\n");
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(keybinds.does(&key("Numpad8")), None, "unbound");
    assert_eq!(keybinds.does(&key("Numpad2")), Some(&send("go2 bank")));
    assert_eq!(
        keybinds.does(&key("Numpad6")),
        Some(&send("east")),
        "still Hydra's"
    );
    assert_eq!(keybinds.len(), 26, "one gone, one added");
    let rows = keybinds.rows();
    let row = |written: &str| rows.iter().find(|row| row.key == written).cloned();
    assert_eq!(
        row("Numpad8"),
        Some(KeyRow {
            key: "Numpad8".to_owned(),
            does: None,
            default: Some(send("north")),
        }),
        "an unbound default is listed, to be restored"
    );
    assert_eq!(row("F5").and_then(|row| row.default), None);
}

/// A key fills the input or performs an action, as the file writes it;
/// an action Hydra does not have is said.
#[test]
fn a_key_fills_or_acts() {
    let (keybinds, problems) = Keybinds::read(
        "[keys]\nF3 = { fill = \"prep 111 \" }\nF4 = { action = \"stop\" }\nF6 = { action = \"fly\" }\n",
    );
    let key = |written: &str| Chord::parse(written).expect("parses");
    assert_eq!(
        keybinds.does(&key("F3")),
        Some(&Macro::Fill("prep 111 ".to_owned()))
    );
    assert_eq!(keybinds.does(&key("F4")), Some(&Macro::Act(Action::Stop)));
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("`fly`"), "{problems:?}");
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
    assert_eq!(keybinds.take(&mut input), [Macro::Send("look".to_owned())]);
    assert_eq!(input.events, [press(Key::A)]);
}

/// A numpad press the fork caught does its macro, named as winit names it;
/// one it let through to be typed does nothing.
#[test]
fn a_caught_numpad_press_does_its_macro() {
    let keybinds = Keybinds::default();
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
        numpad_macro(&keybinds, &event(true)),
        Some(Macro::Send("north".to_owned()))
    );
    assert_eq!(
        numpad_macro(&keybinds, &event(false)),
        None,
        "typed instead"
    );
}

/// A send macro's commands are cut apart at each break, so each is one
/// command (the crate review of 2026-09-28, R10); one that is still not
/// one line -- a NUL in it -- is said, not bound, and the rest still bind.
#[test]
fn a_command_that_is_not_one_line_is_said() {
    let (keybinds, problems) = Keybinds::read(
        r#"
[keys]
F5 = "look"
F6 = "look\nkill"
F7 = "stand\r"
F8 = "x\u0000"
"#,
    );
    assert_eq!(keybinds.changed(), 3, "{problems:?}");
    let steps = |key: &str| {
        keybinds
            .does(&Chord::parse(key).expect("parses"))
            .map(|made| made.steps().map(|steps| steps.len()))
    };
    assert_eq!(steps("F6"), Some(Ok(2)));
    assert_eq!(steps("F7"), Some(Ok(1)));
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("CR, LF or NUL"), "{problems:?}");
}
