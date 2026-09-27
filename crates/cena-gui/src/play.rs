//! A character's play window (`plan/47` step 4): its story, one command
//! input that sends on this character, and what a player glances at --
//! connection, roundtime and cast time, hands, vitals, the room -- with
//! Hydra's own messages in their own pane, never in the game's text.
//!
//! The author: *"in my head you have one command input period. sending a
//! command in it sends a command on that character."* One window per
//! character, so no window mixes two characters' story (`plan/29` §5a R2).
//! Closing it leaves the character running headless; the hub opens it again.
//!
//! It draws from the character's `GameState` directly, as the author chose
//! for the native GUI (`plan/28` §7b, *"Third option is the one"*), with the
//! room's player names painted by the character's triggers as Despana paints
//! them ([`cena_ui::room_player`]).
//!
//! **The focus rule**, `VellumFE`'s (`reference/VellumFE/src/frontend/gui/app.rs:3426`),
//! for the author's complaint: a click nothing else took returns the
//! keyboard to the command input, so the player can type without clicking it.

use std::time::Instant;

use cena_session::hands::Hand;
use cena_session::{Body, Notice, NoticeKind, RoomItem, Snapshot};
use cena_ui::LifecycleView;
use egui::{Color32, Id, RichText};

use crate::bar::{self, Amount, Bar, Says};
use crate::story::{Shown, Story};
use crate::text::{self, AMBER, CREATURE, OBJECT, PLAYER, WRONG};

/// What a play window asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    /// Send this line on the character, as typed: Hydra's command line
    /// first, then the game.
    Send(String),
    /// Stop everything Hydra is doing on the character (`;stop`).
    Stop,
}

/// What a play window shows this frame.
pub(crate) struct PlayView<'a> {
    /// The character.
    pub(crate) name: &'a str,
    /// How it is connected.
    pub(crate) lifecycle: &'a LifecycleView,
    /// What it knows, once its feed has seen anything.
    pub(crate) snapshot: Option<&'a Snapshot>,
    /// Its story, messages and banners.
    pub(crate) story: &'a Story,
    /// Now, for which banners are still up.
    pub(crate) now: Instant,
}

/// A play window's own state, which outlives a frame.
#[derive(Debug)]
pub(crate) struct Play {
    /// Which session: every id in the window is its own.
    session: u32,
    /// The line being typed.
    input: String,
    /// What was sent, oldest first, for up and down.
    history: Vec<String>,
    /// Where up and down have reached in the history.
    back: Option<usize>,
}

/// Lines of history kept for up and down.
const MAX_HISTORY: usize = 100;

impl Play {
    /// A play window for session `session`, with nothing typed.
    pub(crate) fn new(session: u32) -> Self {
        Self {
            session,
            input: String::new(),
            history: Vec::new(),
            back: None,
        }
    }

    /// The command input's id.
    pub(crate) fn input_id(&self) -> Id {
        Id::new(("play-input", self.session))
    }

    /// Draw the window into `ui` -- a viewport's whole area -- and return
    /// what the player asked for, if anything.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, view: &PlayView<'_>) -> Option<Asked> {
        let mut asked = None;
        let session = self.session;
        egui::Panel::top(Id::new(("play-top", session))).show(ui, |ui| {
            if top(ui, view) {
                asked = Some(Asked::Stop);
            }
        });
        egui::Panel::bottom(Id::new(("play-input-panel", session))).show(ui, |ui| {
            if let Some(line) = self.input(ui) {
                asked = Some(Asked::Send(line));
            }
        });
        egui::Panel::right(Id::new(("play-side", session)))
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| side(ui, view, session));
        story(ui, &view.story.lines, session);
        // The focus rule: nothing holds the keyboard, so the input takes it.
        if ui.ctx().memory(|memory| memory.focused().is_none()) {
            let id = self.input_id();
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
        }
        asked
    }

    /// The command input: Enter sends, up and down walk what was sent.
    fn input(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let id = self.input_id();
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.input)
                .id(id)
                .hint_text("Type a command")
                .desired_width(f32::INFINITY),
        );
        if response.has_focus() {
            let (up, down) = ui.input(|input| {
                (
                    input.key_pressed(egui::Key::ArrowUp),
                    input.key_pressed(egui::Key::ArrowDown),
                )
            });
            if up {
                self.walk(true);
            } else if down {
                self.walk(false);
            }
        }
        let entered =
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if !entered {
            return None;
        }
        response.request_focus();
        let line = std::mem::take(&mut self.input);
        self.back = None;
        if line.trim().is_empty() {
            return None;
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
        }
        Some(line)
    }

    /// One step back (`up`) or forward through what was sent.
    fn walk(&mut self, up: bool) {
        let last = self.history.len().checked_sub(1);
        self.back = match (self.back, up) {
            (None, true) => last,
            (Some(at), true) => Some(at.saturating_sub(1)),
            (Some(at), false) if Some(at) < last => Some(at + 1),
            (_, false) => None,
        };
        self.input = self
            .back
            .and_then(|at| self.history.get(at).cloned())
            .unwrap_or_default();
    }
}

/// The top bar: who, how connected, the clocks, the hands, Stop; and any
/// banner. `true` when Stop was pressed.
fn top(ui: &mut egui::Ui, view: &PlayView<'_>) -> bool {
    let mut stop = false;
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
            stop = ui
                .button("Stop")
                .on_hover_text("Stop everything Hydra is doing on this character")
                .clicked();
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
    stop
}

/// What a hand holds, in a word or its item's name.
fn hand(hand: &Hand) -> &str {
    match hand {
        Hand::Unknown => "?",
        Hand::Empty => "empty",
        Hand::Holding { name, .. } => name,
    }
}

/// The side pane: vitals, the room, Hydra's messages.
fn side(ui: &mut egui::Ui, view: &PlayView<'_>, session: u32) {
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
    ui.separator();
    if let Some(snapshot) = view.snapshot {
        room(ui, snapshot);
    }
    ui.separator();
    ui.strong("Hydra");
    egui::ScrollArea::vertical()
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
    let known = room.component("room objs").is_some();
    if known {
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

#[cfg(test)]
mod tests;
