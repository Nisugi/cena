//! What a play window draws: its top bar, and each pane's content.

use cena_session::hands::Hand;
use cena_session::{Body, Notice, NoticeKind, RoomItem, Snapshot};
use egui::{Color32, RichText};

use super::PlayView;
use crate::bar::{self, Amount, Bar, Says};
use crate::layout::Pane;
use crate::story::Shown;
use crate::text::{self, AMBER, CREATURE, OBJECT, PLAYER, WRONG};

/// What the top bar was asked this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Top {
    /// Stop was pressed.
    Stop,
    /// The layout was to be fitted afresh.
    Fit,
    /// The grid's pitch was changed.
    Grid,
    /// The keybinds were to be read again.
    ReloadKeys,
}

/// The top bar: who, how connected, the clocks, the hands, the keybinds,
/// the layout's grid, Stop; and any banner.
pub(super) fn top(
    ui: &mut egui::Ui,
    view: &PlayView<'_>,
    grid: &mut f32,
    unsaved: Option<&str>,
) -> Option<Top> {
    let mut asked = None;
    let state = view.snapshot.map(|snapshot| &snapshot.state);
    ui.horizontal(|ui| {
        ui.strong(view.name);
        ui.label(crate::hub::lifecycle(view.lifecycle));
        if let Some(seconds) = state
            .and_then(cena_session::GameState::roundtime_remaining)
            .filter(|s| *s > 0)
        {
            ui.colored_label(AMBER, format!("RT {seconds}s"));
        }
        if let Some(seconds) = state
            .and_then(cena_session::GameState::casttime_remaining)
            .filter(|s| *s > 0)
        {
            ui.colored_label(bar::MANA, format!("CT {seconds}s"));
        }
        if let Some(state) = state {
            ui.label(format!(
                "Left: {} · Right: {}",
                hand(&state.left_hand),
                hand(&state.right_hand)
            ));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Stop")
                .on_hover_text("Stop everything Hydra is doing on this character")
                .clicked()
            {
                asked = Some(Top::Stop);
            }
            ui.menu_button("Keys", |ui| {
                for said in view.keys {
                    ui.label(said);
                }
                if ui.button("Read the keybinds again").clicked() {
                    asked = Some(Top::ReloadKeys);
                    ui.close();
                }
            });
            ui.menu_button("Layout", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Grid");
                    let changed = ui
                        .add(egui::DragValue::new(grid).range(0.0..=64.0).suffix(" pt"))
                        .on_hover_text("What the panes' edges snap to; 0 for none. Shift while dragging snaps to nothing.")
                        .changed();
                    if changed {
                        asked = Some(Top::Grid);
                    }
                });
                if ui.button("Fit the panes afresh").clicked() {
                    asked = Some(Top::Fit);
                    ui.close();
                }
            });
            if let Some(why) = unsaved {
                ui.colored_label(WRONG, format!("Layout not saved: {why}"));
            }
            if let Some(on) = view.numlock {
                ui.weak(if on { "NumLock on" } else { "NumLock off" });
            }
        });
    });
    for alert in view.story.alerts_at(view.now) {
        egui::Frame::new()
            .fill(AMBER)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.colored_label(Color32::BLACK, alert);
            });
    }
    asked
}

/// What a hand holds, in a word or its item's name.
fn hand(hand: &Hand) -> &str {
    match hand {
        Hand::Unknown => "?",
        Hand::Empty => "empty",
        Hand::Holding { name, .. } => name,
    }
}

/// One pane's content, filling its window: a window sizes itself to what
/// it holds, and a pane is the size its layout says, not its content's.
/// The story and Hydra's messages scroll themselves; the others scroll
/// when what they hold does not fit, never growing the pane: none asks for a
/// height of its own, which is the layout's to say.
pub(super) fn pane(ui: &mut egui::Ui, pane: Pane, view: &PlayView<'_>, session: u32) {
    ui.set_min_size(ui.available_size());
    let scrolled = |ui: &mut egui::Ui, add: &mut dyn FnMut(&mut egui::Ui)| {
        egui::ScrollArea::vertical()
            .min_scrolled_height(0.0)
            .id_salt(("play-pane-scroll", session, pane))
            .auto_shrink(false)
            .show(ui, |ui| add(ui));
    };
    match pane {
        Pane::Story => story(ui, &view.story.lines, session),
        Pane::Hydra => hydra(ui, view, session),
        Pane::Vitals => scrolled(ui, &mut |ui| vitals(ui, view)),
        Pane::Room => scrolled(ui, &mut |ui| match view.snapshot {
            Some(snapshot) => room(ui, snapshot),
            None => {
                ui.weak("Room unknown");
            }
        }),
        Pane::Hunt => scrolled(ui, &mut |ui| hunt(ui, view.hunt)),
    }
}

/// What the hunt is doing: what runs, where it is in its cycle, what it did
/// last, the creature it fights, and -- the reason for the pane, since a
/// stuck hunt is a waiting one -- why it waits.
fn hunt(ui: &mut egui::Ui, hunt: Option<&cena_ui::HuntView>) {
    let Some(hunt) = hunt else {
        ui.weak("No hunt running.");
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.strong(&hunt.running);
        ui.label(&hunt.phase);
    });
    ui.label(&hunt.doing);
    if let Some(target) = &hunt.target {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Fighting:");
            ui.colored_label(CREATURE, target);
        });
    }
    if let Some(waiting) = &hunt.waiting {
        ui.colored_label(AMBER, format!("Waiting: {waiting}"));
    }
}

/// The four vitals, each a bar as wide as the pane.
fn vitals(ui: &mut egui::Ui, view: &PlayView<'_>) {
    let state = view.snapshot.map(|snapshot| &snapshot.state);
    let width = ui.available_width();
    for (label, vital, color) in [
        (
            "HP",
            state.and_then(cena_session::GameState::health),
            bar::HEALTH,
        ),
        (
            "MP",
            state.and_then(cena_session::GameState::mana),
            bar::MANA,
        ),
        (
            "SP",
            state.and_then(cena_session::GameState::stamina),
            bar::STAMINA,
        ),
        (
            "Sp",
            state.and_then(cena_session::GameState::spirit),
            bar::SPIRIT,
        ),
    ] {
        let amount = vital.map(|vital| Amount {
            percent: vital.percent,
            current: vital.current,
            max: vital.max,
        });
        ui.add(
            Bar::new(label, amount)
                .fill(color)
                .size([width, 18.0])
                .says(Says {
                    label: true,
                    numbers: true,
                    percent: true,
                }),
        );
    }
}

/// The room as the character stands in it: its name, what is here, the
/// ways out. From the room window's feed, which is where the character is
/// (`plan/15` §2.6), never from the story's text.
fn room(ui: &mut egui::Ui, snapshot: &Snapshot) {
    let (state, room) = (&snapshot.state, &snapshot.state.room);
    ui.label(
        RichText::new(room.title.as_deref().unwrap_or("Room unknown"))
            .color(AMBER)
            .strong(),
    );
    if room.component("room objs").is_some() {
        items(ui, "Creatures", &room.creatures, CREATURE);
        items(ui, "Also here", &room.objects, OBJECT);
    }
    if room.saw_players() && !room.players.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Players:");
            for player in &room.players {
                match cena_ui::room_player(&player.text, &snapshot.triggers, state) {
                    Some(runs) => ui.label(text::job(&runs, ui.style())),
                    None => ui.colored_label(PLAYER, &player.text),
                };
            }
        });
    }
    ui.label(match &room.exits {
        Some(exits) if exits.is_empty() => "Obvious exits: none".to_owned(),
        Some(exits) => format!("Obvious exits: {}", exits.join(", ")),
        None => "Exits unknown".to_owned(),
    });
}

/// A labelled list of room items, each with its status when it has one.
fn items(ui: &mut egui::Ui, label: &str, items: &[RoomItem], color: Color32) {
    if items.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.weak(format!("{label}:"));
        for item in items {
            let text = match &item.status {
                Some(status) => format!("{} ({status})", item.text),
                None => item.text.clone(),
            };
            ui.colored_label(color, text);
        }
    });
}

/// Hydra's own messages, the newest at the bottom.
fn hydra(ui: &mut egui::Ui, view: &PlayView<'_>, session: u32) {
    egui::ScrollArea::vertical()
        .min_scrolled_height(0.0)
        .id_salt(("play-said", session))
        .stick_to_bottom(true)
        .auto_shrink(false)
        .show(ui, |ui| {
            if view.story.said.is_empty() {
                ui.weak("Nothing yet.");
            }
            for notice in &view.story.said {
                said(ui, notice);
            }
        });
}

/// One of Hydra's messages, coloured by what it is about.
fn said(ui: &mut egui::Ui, notice: &Notice) {
    let color = match notice.kind {
        NoticeKind::Error => WRONG,
        NoticeKind::Warn => AMBER,
        NoticeKind::Info | NoticeKind::Debug => ui.visuals().text_color(),
    };
    match &notice.body {
        Body::Lines(lines) => {
            for line in lines {
                ui.colored_label(color, line);
            }
        }
        Body::Mono(lines) => {
            for line in lines {
                ui.label(RichText::new(line).monospace().color(color));
            }
        }
    }
}

/// The story, newest at the bottom, where it stays unless the player
/// scrolls back.
fn story(ui: &mut egui::Ui, lines: &std::collections::VecDeque<Shown>, session: u32) {
    egui::ScrollArea::vertical()
        .min_scrolled_height(0.0)
        .id_salt(("play-story", session))
        .stick_to_bottom(true)
        .auto_shrink(false)
        .show(ui, |ui| {
            for shown in lines {
                match shown {
                    Shown::Game(runs) => {
                        ui.label(text::job(runs, ui.style()));
                    }
                    Shown::Typed(line) => {
                        ui.weak(format!("> {line}"));
                    }
                    Shown::Gap => {
                        ui.colored_label(WRONG, "Some lines were missed here.");
                    }
                }
            }
        });
}
