//! `;keys import` in a play window (`plan/52` step 9): the author's own
//! Wrayth key set into the character's keys, said in its Hydra pane, and
//! never sent to the game.

use super::*;

/// What `seat`'s Hydra pane says, line by line.
fn said(seat: &Seat) -> Vec<String> {
    lock(&seat.story)
        .said
        .iter()
        .flat_map(cena_session::Notice::lines)
        .cloned()
        .collect()
}

/// A Wrayth file imported into the character's keys: what it binds is
/// bound, what was already so is left, a key of Hydra's it changes is
/// named, what Hydra has no action for is said; `;keys` alone says how.
#[test]
fn a_wrayth_set_is_imported() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let data = std::env::temp_dir().join(format!("cena-app-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).expect("a folder");
    let sessions = Sessions::new(runtime.handle().clone());
    let seat = sessions.seat_for_test(handle(), "Ashryn");
    let mut harness = Harness::builder()
        .with_size((1200.0, 900.0))
        .build_ui_state(
            |ui, app: &mut App| app.draw(ui),
            App::keeping(sessions, &data),
        );
    harness.run();
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/wrayth_macros.xml"
    );
    let typed = |harness: &mut Harness<'_, App>, line: &str| {
        harness.get_by_role(Role::TextInput).type_text(line);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
    };
    typed(&mut harness, ";keys");
    assert!(
        said(&seat)
            .iter()
            .any(|line| line.starts_with("keys import")),
        "{:?}",
        said(&seat)
    );
    typed(&mut harness, &format!(";keys import {fixture}"));
    let mine =
        keys::character_path(&data, cena_session::DEFAULT_GAME_CODE, "Ashryn").expect("a path");
    let (file, problems) = keys::KeyFile::load(&mine, keys::Whose::Character);
    assert!(problems.is_empty(), "{problems:?}");
    let key = |written: &str| keys::Chord::parse(written).expect("a key");
    assert_eq!(
        file.sets[1].get(&key("F1")),
        Some(&Some(Macro::Fill("prep 111".to_owned())))
    );
    assert_eq!(
        file.sets[0].get(&key("Tab")),
        Some(&Some(Macro::Act(keys::Action::NextWindow)))
    );
    assert_eq!(file.sets[0].get(&key("PageUp")), None, "already Hydra's");
    let lines = said(&seat);
    assert!(
        lines.iter().any(|line| line.starts_with("Keys: ")
            && line.contains("wrayth_macros.xml into Ashryn's keys")),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("not Hydra's: ") && line.contains("Tab")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("{Rest}")),
        "{lines:?}"
    );
    assert!(
        lock(&seat.story).lines.is_empty(),
        "nothing sent, nothing echoed"
    );

    typed(&mut harness, &format!(";keys import global {fixture}"));
    let (every, _) = keys::KeyFile::load(&keys::path(&data), keys::Whose::Every);
    assert_eq!(
        every.sets[1].get(&key("F1")),
        Some(&Some(Macro::Fill("prep 111".to_owned()))),
        "into every character's"
    );
    let _ = std::fs::remove_dir_all(&data);
}
