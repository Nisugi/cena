//! The game's dialogs that nothing else reads, kept for a viewer
//! (`cena_model::state::dialogs`), from real traffic: the Betrayer panel
//! and the combat panel. The claimed ones are not kept twice.

use cena_model::GameState;
use cena_model::state::dialogs::{MAX_DIALOGS, MAX_PARTS, Part};
use cena_protocol::Parser;

fn read(markup: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(markup.as_bytes()) {
        state.apply(&frame);
    }
    state
}

/// The Betrayer panel's blood points and items, as the event sent them.
#[test]
fn the_betrayer_panel_is_kept() {
    let state = read(include_str!(
        "../../cena-behavior/tests/fixtures/arch_kill.xml"
    ));
    let panel = state.dialogs.get("BetrayerPanel").expect("the panel");
    assert!(panel.open);
    let labels: Vec<(&str, &Part)> = panel
        .parts
        .iter()
        .map(|(id, part)| (id.as_str(), part))
        .collect();
    assert_eq!(
        labels.first(),
        Some(&("lblBPs", &Part::Label("Blood Points: 1".to_owned())))
    );
    assert!(
        labels.iter().any(|(id, part)| *id == "lblitem1"
            && matches!(part, Part::Label(text) if text.contains("dwarf skin backpack"))),
        "{labels:?}"
    );
}

/// The combat panel keeps its title and its buttons with their commands;
/// the vitals, experience and effects are read elsewhere and not kept.
#[test]
fn the_combat_panel_is_kept_and_the_claimed_ones_are_not() {
    let state = read(concat!(
        include_str!("../../cena-protocol/tests/fixtures/m2_model.xml"),
        include_str!("../../cena-protocol/tests/fixtures/vitals.xml"),
    ));
    let combat = state.dialogs.get("combat").expect("the combat panel");
    assert_eq!(combat.title.as_deref(), Some("Combat"));
    let unsheathe = combat
        .parts
        .iter()
        .find(|(id, _)| id == "unsheathe")
        .map(|(_, part)| part);
    let Some(Part::Widget { kind, attrs }) = unsheathe else {
        panic!("the unsheathe button: {:?}", combat.parts);
    };
    assert_eq!(kind, "image");
    assert!(
        attrs
            .iter()
            .any(|(k, v)| k == "cmd" && v == "_ready weapon")
    );
    for claimed in ["minivitals", "expr", "encum", "stance", "Buffs"] {
        assert!(state.dialogs.get(claimed).is_none(), "{claimed} kept twice");
    }
}

/// A label again replaces its part; a clear empties the dialog; a close
/// leaves what it held; the aim timer is an end time until it is `0`.
#[test]
fn a_dialog_changes_as_the_game_says() {
    let mut state = read(concat!(
        "<openDialog type='dynamic' id='Event' title='Event'></openDialog>",
        "<dialogData id='Event'><label id='a' value='one'/><label id='b' value='two'/></dialogData>",
        "<dialogData id='Event'><label id='a' value='three'/></dialogData>\n",
    ));
    let parts = |state: &GameState| {
        state.dialogs.get("Event").map(|dialog| {
            dialog
                .parts
                .iter()
                .map(|(id, part)| match part {
                    Part::Label(text) => format!("{id}={text}"),
                    other => format!("{id}:{other:?}"),
                })
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(parts(&state), Some(vec!["a=three".into(), "b=two".into()]));
    for frame in Parser::new().push_bytes(
        b"<dialogData id='Event' clear='t'></dialogData><closeDialog id='Event'/>\n\
          <dialogData id='AimTimerDialog'><timer id='aimTimer' value='1790045900'/></dialogData>\n",
    ) {
        state.apply(&frame);
    }
    let event = state.dialogs.get("Event").expect("kept when closed");
    assert!(!event.open);
    assert!(event.parts.is_empty(), "cleared");
    assert_eq!(state.dialogs.aim_ends, Some(1_790_045_900));
    assert!(
        state.dialogs.get("AimTimerDialog").is_none(),
        "read, not kept"
    );
    for frame in Parser::new().push_bytes(b"<timer id='aimTimer' value='0'/>\n") {
        state.apply(&frame);
    }
    assert_eq!(state.dialogs.aim_ends, None);
}

/// Past the bound, the dialog changed longest ago goes first.
#[test]
fn the_dialogs_kept_are_bounded() {
    use std::fmt::Write;
    let mut markup = String::new();
    for n in 0..=MAX_DIALOGS {
        let _ = writeln!(
            markup,
            "<dialogData id='d{n}'><label id='x' value='{n}'/></dialogData>"
        );
    }
    let state = read(&markup);
    assert_eq!(state.dialogs.iter().count(), MAX_DIALOGS);
    assert!(state.dialogs.get("d0").is_none());
    assert!(state.dialogs.get(&format!("d{MAX_DIALOGS}")).is_some());
}

/// Past its bound a dialog takes no new part, and a part it has is still
/// changed in place.
#[test]
fn a_dialogs_parts_are_bounded_and_the_ones_kept_still_change() {
    use std::fmt::Write;
    let mut markup = String::from("<dialogData id='wide'>");
    for n in 0..MAX_PARTS + 5 {
        let _ = write!(markup, "<label id='p{n}' value='{n}'/>");
    }
    markup.push_str(
        "</dialogData>\n<dialogData id='wide'><label id='p0' value='changed'/></dialogData>\n",
    );
    let state = read(&markup);
    let wide = state.dialogs.get("wide").expect("kept");
    assert_eq!(wide.parts.len(), MAX_PARTS);
    assert!(
        !wide
            .parts
            .iter()
            .any(|(id, _)| *id == format!("p{MAX_PARTS}")),
        "the one past the bound is not taken"
    );
    assert_eq!(
        wide.parts.first(),
        Some(&("p0".to_owned(), Part::Label("changed".to_owned()))),
        "full, and its first part still follows the game"
    );
}
