//! The settings menu's *Keys* page (`plan/50` §7 step 2, `plan/52`): every
//! character's keys on Hydra's own page, and a character's own with its
//! macro sets (`plan/52` step 2). Moved out of `settings_menu.rs` when a
//! character's page took it past its line cap (`plan/05` Rule 4.1).

use cena_gui::{Action, KeyChange, KeyRow, Macro, MenuAsked, Place, Whose};
use cena_ui::settings::{RowKind, Value};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

mod settings;

use settings::{Board, card, page, row};

/// Type `text` into the field labelled `label`, and press Enter.
fn enter(harness: &mut Harness<'_, Board>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .focus();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
}

/// The menu as the hub opens it, on Hydra's own pages, with `bound` bound.
fn hydra<'a>(bound: &[(&str, &str)]) -> Harness<'a, Board> {
    let window = page(
        "window",
        "Window",
        vec![
            row(
                "card_width",
                "Card width",
                RowKind::Number {
                    min: 160.0,
                    max: 2000.0,
                },
                Value::Text("312".to_owned()),
                false,
            ),
            row(
                "close_with_session",
                "Close a play window when its session closes",
                RowKind::Toggle,
                Value::On(false),
                false,
            ),
        ],
    );
    let mut board = Board {
        roster: vec![card("Nisugi")],
        own: vec![window],
        bound: bound.iter().map(|(key, line)| sends(key, line)).collect(),
        ..Board::default()
    };
    board.menu.open_for(None);
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    harness
}

/// Replace what the field labelled `label` holds with `text`, and press
/// Enter.
fn replace(harness: &mut Harness<'_, Board>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    enter(harness, label, text);
}

/// Set 0 of every character's keys, which Hydra's own *Keys* page changes.
const EVERY: Place = Place {
    set: 0,
    every: true,
};

/// A change to every character's keys.
fn every(change: KeyChange) -> MenuAsked {
    MenuAsked::Key {
        character: None,
        change,
    }
}

fn bind(key: &str, line: &str, was: Option<&str>) -> MenuAsked {
    every(KeyChange::Bind {
        key: key.to_owned(),
        does: Macro::Send(line.to_owned()),
        was: was.map(str::to_owned),
        place: EVERY,
    })
}

fn unbind(key: &str) -> MenuAsked {
    every(KeyChange::Unbind {
        key: key.to_owned(),
        place: EVERY,
    })
}

fn restore(key: &str) -> MenuAsked {
    every(KeyChange::Restore {
        key: key.to_owned(),
        place: EVERY,
    })
}

/// A key of Hydra's, `default`, doing `does`: from every character's file
/// where that is not Hydra's own.
fn hydras(key: &str, does: Option<&str>, default: &str) -> KeyRow {
    let default = Macro::Send(default.to_owned());
    let does = does.map(|line| Macro::Send(line.to_owned()));
    let changed = does.as_ref() != Some(&default);
    KeyRow {
        key: key.to_owned(),
        does,
        default: Some(default.clone()),
        from: changed.then_some(Whose::Every),
        beneath: changed.then_some(default),
    }
}

/// A key the player bound to send `line`, which Hydra does not bind.
fn sends(key: &str, line: &str) -> KeyRow {
    KeyRow::players(key, Macro::Send(line.to_owned()))
}

/// A change on Hydra's *Window* page is asked of the window, not the
/// binary, and what the window answers is said at the top.
#[test]
fn hydras_own_page_is_changed_by_the_window() {
    let mut harness = hydra(&[]);
    harness
        .get_by_role_and_label(
            Role::CheckBox,
            "Close a play window when its session closes",
        )
        .click();
    harness.run();
    replace(&mut harness, "Card width", "400");
    assert_eq!(
        harness.state().asked,
        [
            MenuAsked::Own {
                key: "close_with_session".to_owned(),
                to: Some("on".to_owned()),
            },
            MenuAsked::Own {
                key: "card_width".to_owned(),
                to: Some("400".to_owned()),
            },
        ]
    );
    harness
        .state_mut()
        .menu
        .tell("Window: the card width is 400.".to_owned());
    harness.run();
    assert!(
        harness
            .query_by_label("Window: the card width is 400.")
            .is_some()
    );
}

/// A key is bound by pressing it: *Add a key*, the key, then the line it
/// sends. One that types, or one bound already, is refused and said;
/// Escape stops the wait.
#[test]
fn a_key_is_bound_by_pressing_it() {
    let mut harness = hydra(&[("Ctrl+F1", "look")]);
    harness.get_by_label("Keys").click();
    harness.run();
    harness.get_by_label("Add a key").click();
    harness.run();
    assert!(harness.state().menu.waiting_for_key());
    harness.key_press(egui::Key::A);
    harness.run();
    assert!(harness.query_by_label_contains("KeyA types").is_some());
    assert!(harness.state().menu.waiting_for_key(), "still waiting");
    harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::F1);
    harness.run();
    assert!(
        harness
            .query_by_label("Ctrl+F1 already sends `look`.")
            .is_some()
    );

    harness.get_by_label("Add a key").click();
    harness.run();
    harness.key_press(egui::Key::F5);
    harness.run();
    assert!(!harness.state().menu.waiting_for_key());
    enter(&mut harness, "F5", "hide");
    assert_eq!(harness.state().asked, [bind("F5", "hide", None)]);

    harness.get_by_label("Add a key").click();
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!harness.state().menu.waiting_for_key(), "stopped");
    assert_eq!(harness.state().asked.len(), 1);
}

/// A bound key's line is changed where it is typed; the key itself moves
/// by pressing another, a numpad key coming through the window; *Remove*
/// unbinds it; the numpad's switch is asked for as it is ticked.
#[test]
fn a_bound_key_is_changed_moved_and_removed() {
    let mut harness = hydra(&[("Ctrl+F1", "look"), ("Numpad8", "north")]);
    harness.get_by_label("Keys").click();
    harness.run();
    enter(&mut harness, "Ctrl+F1", " around");
    harness
        .get_by_role_and_label(Role::Button, "Ctrl+F1")
        .click();
    harness.run();
    assert!(harness.state().menu.waiting_for_key());
    harness.state_mut().caught = Some("Numpad2".to_owned());
    harness.run();
    harness.state_mut().caught = None;
    if let Some(remove) = harness.get_all_by_label("Remove").nth(1) {
        remove.click();
    }
    harness.run();
    harness
        .get_by_role_and_label(
            Role::CheckBox,
            "The numpad sends its keys with NumLock on too",
        )
        .click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [
            bind("Ctrl+F1", "look around", None),
            bind("Numpad2", "look", Some("Ctrl+F1")),
            unbind("Numpad8"),
            every(KeyChange::NumpadAlways(true)),
        ]
    );
}

/// Hydra's keys are listed with the player's: one the player changed is
/// restored, one Hydra's alone is removed (which unbinds it), and one
/// unbound is restored; each says where it came from.
#[test]
fn hydras_keys_are_restored_and_removed() {
    let mut board = Board {
        roster: vec![card("Nisugi")],
        bound: vec![
            hydras("Numpad2", Some("go2 bank"), "south"),
            hydras("Numpad8", Some("north"), "north"),
            hydras("Shift+Numpad8", None, "peer north"),
        ],
        ..Board::default()
    };
    board.menu.open_for(None);
    let mut harness = Harness::builder()
        .with_size((900.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    for word in ["changed", "Hydra's", "unbound"] {
        assert!(harness.query_by_label(word).is_some(), "{word}");
    }
    let restores: Vec<_> = harness.get_all_by_label("Restore").collect();
    assert_eq!(restores.len(), 2, "not for a key as Hydra binds it");
    restores[0].click();
    harness.run();
    if let Some(remove) = harness.get_all_by_label("Remove").nth(1) {
        remove.click();
    }
    harness.run();
    if let Some(restore) = harness.get_all_by_label("Restore").nth(1) {
        restore.click();
    }
    harness.run();
    assert_eq!(
        harness.state().asked,
        [
            restore("Numpad2"),
            unbind("Numpad8"),
            restore("Shift+Numpad8"),
        ]
    );
}

/// The *Keys* page as a player sees it, rendered and compared with the
/// committed image.
#[test]
fn the_keys_page_as_drawn() {
    let mut board = Board {
        roster: vec![card("Nisugi")],
        bound: vec![
            sends("Ctrl+F1", "stance off\rincant 610"),
            KeyRow::players("F3", Macro::Fill("prep 111 ".to_owned())),
            KeyRow::players("F4", Macro::Act(Action::Stop)),
            hydras("Numpad8", Some("north"), "north"),
            hydras("Numpad2", Some("go2 bank"), "south"),
            hydras("Shift+Numpad8", None, "peer north"),
        ],
        ..Board::default()
    };
    board.menu.open_for(None);
    let mut harness = Harness::builder()
        .with_size((760.0, 360.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    // With no other page of Hydra's given, Keys is the one showing.
    harness.run();
    harness.snapshot("settings_keys");
}

/// Choose `choice` in the drop-down list labelled `label`.
fn pick(harness: &mut Harness<'_, Board>, label: &str, choice: &str) {
    harness.get_by_role_and_label(Role::ComboBox, label).click();
    harness.run();
    harness.get_by_label(choice).click();
    harness.run();
}

/// A character's own *Keys* page: *global* on each key from a file, ticked
/// for every character's, moves it, in the set the page shows; a key is
/// removed from its own file; the set it uses is chosen; a key added there
/// is its own unless *global* is ticked.
#[test]
fn a_characters_keys_page() {
    let theirs = |key: &str, line: &str, whose| KeyRow {
        from: Some(whose),
        ..KeyRow::players(key, Macro::Send(line.to_owned()))
    };
    let mut board = Board {
        roster: vec![card("Nisugi")],
        bound: vec![
            theirs("F5", "search", Whose::Character),
            theirs("F6", "hide", Whose::Every),
            hydras("Numpad8", Some("north"), "north"),
        ],
        chosen: 1,
        ..Board::default()
    };
    let nisugi = cena_gui::roster_name(&card("Nisugi"));
    board.menu.open_at(Some(nisugi.clone()), Some("keys"));
    let mut harness = Harness::builder()
        .with_size((760.0, 360.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    harness.snapshot("settings_keys_character");
    assert!(
        harness
            .query_by_label("The numpad sends its keys with NumLock on too")
            .is_none(),
        "every character's, on Hydra's page"
    );
    let ticks: Vec<_> = harness.get_all_by_label("global").collect();
    assert_eq!(ticks.len(), 2, "not for Hydra's own");
    ticks[0].click();
    harness.run();
    for remove in [0, 1] {
        if let Some(button) = harness.get_all_by_label("Remove").nth(remove) {
            button.click();
        }
        harness.run();
    }
    pick(&mut harness, "In use over set 0", "set 3");
    pick(&mut harness, "Keys of", "Set 2");
    if let Some(tick) = harness.get_all_by_label("global").nth(1) {
        tick.click();
    }
    harness.run();
    harness.get_by_label("Add a key").click();
    harness.run();
    harness.key_press(egui::Key::F9);
    harness.run();
    enter(&mut harness, "F9", "loot");

    let mine = |change| MenuAsked::Key {
        character: Some(nisugi.clone()),
        change,
    };
    let shared = |key: &str, line: &str, set, every| {
        mine(KeyChange::Share {
            key: key.to_owned(),
            does: Macro::Send(line.to_owned()),
            set,
            every,
        })
    };
    let removed = |key: &str, every| {
        mine(KeyChange::Unbind {
            key: key.to_owned(),
            place: Place { set: 0, every },
        })
    };
    let keyed: Vec<MenuAsked> = harness
        .state()
        .asked
        .iter()
        .filter(|asked| matches!(asked, MenuAsked::Key { .. }))
        .cloned()
        .collect();
    assert_eq!(
        keyed,
        [
            shared("F5", "search", 0, true),
            removed("F5", false),
            removed("F6", true),
            mine(KeyChange::Choose(3)),
            shared("F6", "hide", 2, false),
            mine(KeyChange::Bind {
                key: "F9".to_owned(),
                does: Macro::Send("loot".to_owned()),
                was: None,
                place: Place {
                    set: 2,
                    every: false,
                },
            }),
        ]
    );
}
