//! The hub's Launch tab (`plan/49` Stage C): the roster's characters, each
//! started with a click when its account's password is kept, or with the
//! password typed here; a new login typed whole; and the accounts whose
//! passwords are kept, each forgotten with a click. A star puts a character
//! first. Lich's launcher is the reference (`saved_login_tab.rb`,
//! `manual_login_tab.rb`, `account_manager_ui.rb` and `favorites_manager.rb`
//! under `reference/lich-5/lib/common/gui/`).
//!
//! A password typed here goes to the binary in a [`Password`], which never
//! prints, and leaves the tab once asked for. The binary keeps it in the
//! keyring only when its box was ticked, and only once the login proved it.

use std::collections::HashMap;
use std::fmt;

use cena_session::{DEFAULT_GAME_CODE, GAMES};
use cena_ui::{HubRequest, LifecycleView, Login, Password, RosterCard};

use crate::hub::{HubAction, HubView};

/// What the Launch tab is typing, which outlives a frame.
#[derive(Default)]
pub(crate) struct Launch {
    /// A password being typed for a roster character whose account keeps
    /// none, by the character's roster name ([`roster_name`]).
    typed: HashMap<String, Typed>,
    /// The new login being typed.
    new: New,
}

/// A password being typed, and whether to keep it.
#[derive(Default)]
struct Typed {
    password: String,
    keep: bool,
}

/// A login being typed whole.
struct New {
    account: String,
    typed: Typed,
    character: String,
    /// The game's code, one of [`GAMES`].
    game: String,
}

impl Default for New {
    fn default() -> Self {
        Self {
            account: String::new(),
            typed: Typed::default(),
            character: String::new(),
            game: DEFAULT_GAME_CODE.to_owned(),
        }
    }
}

/// Never a password: the hub is printed by its `Debug`.
impl fmt::Debug for Launch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Launch")
            .field("account", &self.new.account)
            .field("character", &self.new.character)
            .field("game", &self.new.game)
            .finish_non_exhaustive()
    }
}

/// A roster character as the binary's roster reads it: `GAME:Name`, that one
/// character whatever other game has one of the same name.
fn roster_name(card: &RosterCard) -> String {
    format!("{}:{}", card.game, card.character)
}

/// What a player calls the game `code` names: its name, or the code itself
/// when Hydra knows no name for it.
fn game_name(code: &str) -> &str {
    GAMES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(code))
        .map_or(code, |(_, name)| name)
}

impl Launch {
    /// Draw the tab over `view`; what the player asked for goes in `asked`.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        view: &HubView<'_>,
        asked: &mut Option<HubAction>,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("hub-launch")
            .show(ui, |ui| {
                ui.strong("Characters");
                self.characters(ui, view, asked);
                ui.separator();
                ui.strong("New login");
                self.new_login(ui, asked);
                ui.separator();
                ui.strong("Kept passwords");
                kept(ui, view.roster, asked);
            });
    }

    /// The roster, starred first, then by name.
    fn characters(&mut self, ui: &mut egui::Ui, view: &HubView<'_>, asked: &mut Option<HubAction>) {
        if view.roster.is_empty() {
            ui.weak("No character has logged in through Hydra yet.");
            return;
        }
        let mut roster: Vec<&RosterCard> = view.roster.iter().collect();
        roster.sort_by_key(|card| (!card.favourite, card.character.to_lowercase()));
        for card in roster {
            ui.horizontal(|ui| self.character(ui, card, view, asked));
        }
    }

    /// One roster character: its star, who and where, then Start when its
    /// password is kept, or a password to type when it is not, or that it
    /// is playing; and Forget.
    fn character(
        &mut self,
        ui: &mut egui::Ui,
        card: &RosterCard,
        view: &HubView<'_>,
        asked: &mut Option<HubAction>,
    ) {
        let name = roster_name(card);
        let (star, says) = if card.favourite {
            ("★", "A favourite, listed first; click to unstar")
        } else {
            ("☆", "Star it, to list it first")
        };
        if ui
            .selectable_label(card.favourite, star)
            .on_hover_text(says)
            .clicked()
        {
            *asked = Some(HubAction::Ask(HubRequest::Favourite(
                name.clone(),
                !card.favourite,
            )));
        }
        let who = ui.strong(&card.character);
        ui.weak(format!("{} · {}", game_name(&card.game), card.account));
        let playing = view.cards.iter().any(|playing| {
            playing.name.eq_ignore_ascii_case(&card.character)
                && !matches!(playing.lifecycle, LifecycleView::Closed { .. })
        });
        // The binary offers a character by the name it can find it by.
        let offered = view.offered.iter().find(|offered| {
            offered.eq_ignore_ascii_case(&card.character) || offered.eq_ignore_ascii_case(&name)
        });
        if playing {
            ui.weak("Playing");
        } else if let Some(offered) = offered {
            if ui.button(format!("Start {}", card.character)).clicked() {
                *asked = Some(HubAction::Ask(HubRequest::Add(offered.clone())));
            }
        } else {
            let typed = self.typed.entry(name.clone()).or_default();
            ui.add(
                egui::TextEdit::singleline(&mut typed.password)
                    .password(true)
                    .hint_text("Password")
                    .desired_width(120.0),
            )
            .labelled_by(who.id);
            ui.checkbox(&mut typed.keep, "Keep");
            let ready = !typed.password.is_empty();
            if ui
                .add_enabled(
                    ready,
                    egui::Button::new(format!("Log {} in", card.character)),
                )
                .clicked()
            {
                let typed = self.typed.remove(&name).unwrap_or_default();
                *asked = Some(HubAction::Ask(HubRequest::Login(Login {
                    account: card.account.clone(),
                    password: Password::new(typed.password),
                    game: card.game.clone(),
                    character: card.character.clone(),
                    remember: typed.keep,
                })));
            }
        }
        if ui
            .button("Forget")
            .on_hover_text("Take it off the roster")
            .clicked()
        {
            self.typed.remove(&name);
            *asked = Some(HubAction::Ask(HubRequest::Forget(name)));
        }
    }

    /// A login typed whole: account, password, character and game.
    fn new_login(&mut self, ui: &mut egui::Ui, asked: &mut Option<HubAction>) {
        let new = &mut self.new;
        egui::Grid::new("hub-new-login")
            .num_columns(2)
            .show(ui, |ui| {
                let label = ui.label("Account");
                ui.text_edit_singleline(&mut new.account)
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label("Password");
                ui.add(egui::TextEdit::singleline(&mut new.typed.password).password(true))
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label("Character");
                ui.text_edit_singleline(&mut new.character)
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label("Game");
                egui::ComboBox::from_id_salt("hub-new-login-game")
                    .selected_text(game_name(&new.game))
                    .show_ui(ui, |ui| {
                        for (code, name) in GAMES {
                            ui.selectable_value(&mut new.game, code.to_owned(), name);
                        }
                    })
                    .response
                    .labelled_by(label.id);
                ui.end_row();
            });
        ui.checkbox(&mut new.typed.keep, "Keep the password once it logs in");
        let ready = !new.account.trim().is_empty()
            && !new.character.trim().is_empty()
            && !new.typed.password.is_empty();
        if ui.add_enabled(ready, egui::Button::new("Log in")).clicked() {
            // The password leaves the tab; what else was typed stays, for a
            // second try after a refusal.
            let typed = std::mem::take(&mut new.typed);
            *asked = Some(HubAction::Ask(HubRequest::Login(Login {
                account: new.account.trim().to_owned(),
                password: Password::new(typed.password),
                game: new.game.clone(),
                character: new.character.trim().to_owned(),
                remember: typed.keep,
            })));
        }
    }
}

/// The accounts whose passwords Hydra can read without asking, each
/// forgotten with a click.
fn kept(ui: &mut egui::Ui, roster: &[RosterCard], asked: &mut Option<HubAction>) {
    let mut accounts: Vec<&str> = roster
        .iter()
        .filter(|card| card.kept)
        .map(|card| card.account.as_str())
        .collect();
    accounts.sort_unstable_by_key(|account| account.to_lowercase());
    accounts.dedup_by_key(|account| account.to_lowercase());
    if accounts.is_empty() {
        ui.weak("No password is kept.");
        return;
    }
    for account in accounts {
        ui.horizontal(|ui| {
            ui.label(account);
            if ui.button(format!("Forget {account}'s password")).clicked() {
                *asked = Some(HubAction::Ask(HubRequest::ForgetPassword(
                    account.to_owned(),
                )));
            }
        });
    }
}
