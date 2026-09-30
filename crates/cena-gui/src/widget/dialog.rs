//! One of the game's own dialogs, drawn from what the model keeps of it
//! (`cena_model::state::dialogs`): the Betrayer panel's blood points and
//! items, the combat panel's buttons, a panel an event adds.
//!
//! Drawn in the order its parts came, one to a line: a label's text, a
//! bar, a button, a link or a drop-down that sends its command. Not where
//! the game places each part: its anchors, percent coordinates and
//! `justify` bitfield (`plan/28-gui-inventory.md` §6a) are for a later
//! step. A part that sends nothing and says nothing, an edit box or a
//! skin, is left out.
//!
//! A command starting `_` is the client's own and is sent without an echo,
//! as a dragged item is (`crate::carry`); any other is sent as typed. A
//! dialog of another character's, followed, sends nothing.

use cena_session::GameState;
use cena_session::dialogs::Part;

use super::Clicked;

/// Draw the dialog `id` from `state`; the command a part sent, if one did
/// and the dialog is the window's own character's (`own`).
pub(super) fn dialog(
    ui: &mut egui::Ui,
    state: Option<&GameState>,
    id: &str,
    own: bool,
) -> Option<Clicked> {
    let Some(dialog) = state.and_then(|state| state.dialogs.get(id)) else {
        ui.weak("Nothing yet.");
        return None;
    };
    if let Some(title) = &dialog.title {
        ui.strong(title);
    }
    if !dialog.open {
        ui.weak("Closed by the game.");
    }
    let mut sent = None;
    ui.add_enabled_ui(own, |ui| {
        for (part_id, part) in &dialog.parts {
            if let Some(command) = draw_part(ui, part_id, part) {
                sent = Some(command);
            }
        }
    });
    sent.map(|command| {
        if command.starts_with('_') {
            Clicked::Quietly(command)
        } else {
            Clicked::Send(command)
        }
    })
}

/// Draw one part; the command it sends, when clicked or chosen.
fn draw_part(ui: &mut egui::Ui, id: &str, part: &Part) -> Option<String> {
    let attrs = match part {
        Part::Label(text) => {
            if !text.trim().is_empty() {
                ui.label(text);
            }
            return None;
        }
        Part::Bar { percent, text } => {
            let fill = f32::from(u16::try_from(*percent).unwrap_or(100).min(100)) / 100.0;
            ui.add(egui::ProgressBar::new(fill).text(text));
            return None;
        }
        Part::Widget { kind, attrs } => (kind.as_str(), attrs),
    };
    let (kind, attrs) = attrs;
    let get = |name: &str| {
        attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let command = get("cmd").filter(|cmd| !cmd.trim().is_empty())?;
    let tip = get("tooltip").unwrap_or_default();
    let words = get("value")
        .filter(|words| !words.is_empty())
        .or_else(|| get("echo"))
        .or(get("tooltip"))
        .unwrap_or(id);
    match kind {
        "cmdButton" => ui
            .button(words)
            .on_hover_text(tip)
            .clicked()
            .then(|| command.to_owned()),
        "link" => ui
            .link(words)
            .on_hover_text(tip)
            .clicked()
            .then(|| command.to_owned()),
        // An image with a command is a button: the combat panel's
        // sheathe and shield. Its picture is a skin's, which Hydra draws
        // as words (`plan/28` §6b).
        "image" => {
            let words = get("tooltip").or(get("echo")).unwrap_or(id);
            ui.button(words).clicked().then(|| command.to_owned())
        }
        "dropDownBox" => drop_down(ui, id, command, &get),
        _ => None,
    }
}

/// A drop-down's choices, `content_text` shown and `content_value` sent in
/// place of its `%id%` in the command; the choice sent, once made.
fn drop_down<'a>(
    ui: &mut egui::Ui,
    id: &str,
    command: &str,
    get: &dyn Fn(&str) -> Option<&'a str>,
) -> Option<String> {
    let texts = get("content_text").unwrap_or_default().split(',');
    let values = get("content_value").unwrap_or_default().split(',');
    let current = get("value").unwrap_or_default();
    let mut chosen = None;
    egui::ComboBox::from_id_salt(("dialog", id))
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (text, value) in texts.zip(values) {
                if ui.selectable_label(text == current, text).clicked() {
                    chosen = Some(command.replace(&format!("%{id}%"), value));
                }
            }
        });
    chosen
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use egui::Id;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use crate::fixture::{snapshot, story};
    use crate::widget::{Seen, Widget};

    /// A game dialog shows its title and parts in the order they came; its
    /// button sends its command, and one starting `_` goes without an echo.
    #[test]
    fn a_game_dialog_shows_its_parts_and_sends_its_commands() {
        use cena_session::Frame;
        let mut ashryn = snapshot();
        let dialog = Some("BetrayerPanel".to_owned());
        let button = |id: &str, tooltip: &str, cmd: &str| Frame::InjuryImage {
            id: id.to_owned(),
            name: String::new(),
            dialog: dialog.clone(),
            attrs: [("id", id), ("tooltip", tooltip), ("cmd", cmd)]
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .to_vec(),
        };
        for frame in [
            Frame::DialogOpen {
                id: "BetrayerPanel".to_owned(),
                title: Some("Betrayer".to_owned()),
                attrs: Vec::new(),
            },
            Frame::Label {
                id: "lblBPs".to_owned(),
                value: "Blood Points: 1".to_owned(),
                dialog: dialog.clone(),
                attrs: Vec::new(),
            },
            button("trade", "trade", "trade points"),
            button("configure", "configure", "_cmbtpl configure dialog"),
        ] {
            ashryn.state.apply(&frame);
        }
        let sent = Arc::new(Mutex::new(Vec::new()));
        let heard = Arc::clone(&sent);
        let story = story();
        let widget = Widget::Dialog("BetrayerPanel".to_owned());
        assert_eq!(widget.name(), "Betrayer panel");
        let mut harness = Harness::builder()
            .with_size((300.0, 200.0))
            .build_ui(move |ui| {
                let seen = Seen {
                    snapshot: Some(&ashryn),
                    story: &story,
                    hunt: None,
                    who: None,
                    open: &[],
                    minimap: None,
                    tags: None,
                };
                let clicked = widget.draw(ui, &seen, Id::new("dialog"));
                let line = match clicked {
                    Some(crate::widget::Clicked::Send(line)) => line,
                    Some(crate::widget::Clicked::Quietly(line)) => format!("quietly {line}"),
                    _ => return,
                };
                heard
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(line);
            });
        harness.run();
        harness.get_by_label("Betrayer");
        harness.get_by_label("Blood Points: 1");
        harness.get_by_label("trade").click();
        harness.run();
        harness.get_by_label("configure").click();
        harness.run();
        assert_eq!(
            *sent
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ["trade points", "quietly _cmbtpl configure dialog"]
        );
    }
}
