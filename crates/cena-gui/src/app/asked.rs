//! What a play window asks of the app, done: moved out of `app.rs` at its
//! cap when `plan/52`'s keys took `App::play` past clippy's length.

use std::sync::Arc;

use super::App;
use crate::play::Asked;
use crate::sessions::Seat;

impl App {
    /// Do what `seat`'s play window asked; what the app itself opens --
    /// the settings, a set, another character's window -- handed back.
    pub(super) fn asked(&mut self, seat: &Arc<Seat>, asked: Asked) -> Option<Asked> {
        match asked {
            Asked::ReloadKeys => self.read_keys(),
            Asked::SavePreset(preset) => self.presets.keep(preset),
            Asked::ForgetPreset(name) => self.presets.forget(&name),
            Asked::Send(line) => {
                if !self.keys_command(seat, &line) {
                    self.sessions.send(seat, line);
                }
            }
            Asked::Quietly(line) => self.sessions.send_quietly(seat, line),
            asked @ (Asked::Settings(_) | Asked::Keys | Asked::UseSet(_) | Asked::Character(_)) => {
                return Some(asked);
            }
            Asked::Stop => self.hydras(seat, "stop"),
            Asked::Log => self.open_log(seat),
            Asked::Triggers => self.open_triggers(),
            Asked::Theme => self.open_theme_editor(),
            Asked::TriggerFrom(line) => {
                self.open_triggers();
                self.triggers.start_from(&line);
            }
            Asked::Lich(on) => self.hydras(seat, lich_word(on)),
            Asked::Aim(target) => seat.aim(target),
            Asked::Widget { page, key, to } => {
                if let Some(window) = self.plays.get_mut(&seat.id.0) {
                    let _ = window.play.widget_change(&page, &key, Some(&to));
                }
            }
            Asked::Hydra { word, echo: true } => self.hydras(seat, &word),
            Asked::Hydra { word, echo: false } => {
                self.sessions
                    .send_quietly(seat, format!("{}{word}", symbol(seat)));
            }
        }
        None
    }

    /// Send Hydra's command `word` on `seat`'s character, with its symbol.
    pub(super) fn hydras(&self, seat: &Arc<Seat>, word: &str) {
        self.sessions.send(seat, format!("{}{word}", symbol(seat)));
    }
}

/// What `seat`'s character marks Hydra's commands with.
fn symbol(seat: &Seat) -> char {
    seat.handle
        .command_symbol()
        .unwrap_or(cena_session::command::claimant::DEFAULT_SYMBOL)
}

/// `;lich`'s word for switching the player's own Lich `on`, or off.
pub(super) fn lich_word(on: bool) -> &'static str {
    if on { "lich on" } else { "lich off" }
}
