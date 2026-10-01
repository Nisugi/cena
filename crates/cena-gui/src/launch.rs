//! The hub's Not launched and New login tabs (`plan/49` Stage C, as the
//! author revised it on 2026-09-27, the two tabs apart as the author
//! corrected it the same day):
//!
//! - *Not launched*: the roster's characters that are not on the table, as
//!   cards in the hub's grid, starred ones first;
//! - *New login*: a login by account, which lists that account's characters
//!   to add, star or play; and the kept passwords.
//!
//! Lich's launcher is the reference (`saved_login_tab.rb`,
//! `manual_login_tab.rb`, `account_manager_ui.rb` and `favorites_manager.rb`
//! under `reference/lich-5/lib/common/gui/`).
//!
//! A password typed here goes to the binary in a [`Password`], which never
//! prints, and leaves the form once asked for; its field keeps no undo
//! history, so nothing brings it back once it has left. An account logged in is held
//! here with its password, so a character it lists can be played without
//! typing the password again; *Log out* lets go of both. The binary keeps a
//! password in the keyring only when its box was ticked, and only once the
//! service has proved it.

use std::collections::HashMap;
use std::fmt;

use cena_session::GAMES;
use cena_ui::{Account, HubRequest, Listing, Login, Password, RosterCard, Saved};

use crate::hub::{CardWidth, HubAction, HubView, card_scope, side, tiled};

/// What the Not launched and New login tabs are typing, and the account
/// logged in, which outlive a frame.
#[derive(Default)]
pub(crate) struct Launch {
    /// A password being typed for a roster character whose account keeps
    /// none, by the character's roster name ([`roster_name`]).
    typed: HashMap<String, Typed>,
    /// The login by account being typed.
    new: New,
    /// The account logged in, whose characters are listed; `None` before
    /// *Log in* and after *Log out*.
    held: Option<Held>,
}

/// A password being typed, and whether to keep it.
#[derive(Default)]
struct Typed {
    password: String,
    keep: bool,
}

/// A login by account being typed: no character, which the listing names.
struct New {
    account: String,
    typed: Typed,
    /// The game's code, one of [`GAMES`]: Prime until the player picks
    /// another (the author, 2026-09-27: *"Game needs to default to Prime and
    /// not test"*).
    game: String,
}

impl Default for New {
    fn default() -> Self {
        Self {
            account: String::new(),
            typed: Typed::default(),
            game: GAMES[0].0.to_owned(),
        }
    }
}

/// An account logged in: which listing is its, and the password to play its
/// characters with.
struct Held {
    account: String,
    game: String,
    password: Password,
}

/// Never a password: the hub is printed by its `Debug`.
impl fmt::Debug for Launch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Launch")
            .field("account", &self.new.account)
            .field("game", &self.new.game)
            .field("held", &self.held.as_ref().map(|held| &held.account))
            .finish_non_exhaustive()
    }
}

/// A roster character as the binary's roster reads it: `GAME:Name`, that one
/// character whatever other game has one of the same name.
fn roster_name(game: &str, character: &str) -> String {
    format!("{game}:{character}")
}

/// What a player calls the game `code` names: its name, or the code itself
/// when Hydra knows no name for it.
pub(crate) fn game_name(code: &str) -> &str {
    GAMES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(code))
        .map_or(code, |(_, name)| name)
}

/// Whether `character` on `game` is on the session table, live or closed:
/// it is not waiting to be launched. By game and name: one name on another
/// game is another character (the crate review of 2026-09-28, R6: it hid this one).
fn on_table(view: &HubView<'_>, game: &str, character: &str) -> bool {
    view.cards.iter().any(|card| {
        card.game.eq_ignore_ascii_case(game) && card.name.eq_ignore_ascii_case(character)
    })
}

/// The roster's characters not on the table, starred first, then by name:
/// the tab's cards (the author: *"You can favorite a card and that pushes
/// them to the top of the grid"*).
pub(crate) fn waiting<'a>(view: &HubView<'a>) -> Vec<&'a RosterCard> {
    let mut waiting: Vec<&RosterCard> = view
        .roster
        .iter()
        .filter(|card| !on_table(view, &card.game, &card.character))
        .collect();
    waiting.sort_by_key(|card| (!card.favourite, card.character.to_lowercase()));
    waiting
}

/// A star: filled when `starred`.
fn star(ui: &mut egui::Ui, starred: bool) -> egui::Response {
    let (star, says) = if starred {
        ("★", "A favourite, listed first; click to unstar")
    } else {
        ("☆", "Star it, to list it first")
    };
    ui.selectable_label(starred, star).on_hover_text(says)
}

impl Launch {
    /// Draw the Not launched tab over `view`, its cards `width` wide; what
    /// the player asked for goes in `asked`.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        view: &HubView<'_>,
        width: &mut CardWidth,
        asked: &mut Option<HubAction>,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("hub-not-launched")
            .auto_shrink(false)
            .show(ui, |ui| {
                let waiting = waiting(view);
                if waiting.is_empty() {
                    ui.weak("No saved character is waiting to be launched.");
                }
                tiled(ui, &waiting, *width, |ui, card| {
                    let id = egui::Id::new(("hub-roster", &card.game, &card.character));
                    let drawn = card_scope(ui, id, |ui| self.card(ui, card, view, *width));
                    if let Some(action) = drawn.inner {
                        *asked = Some(action);
                    }
                    side(ui, drawn.response.rect, id, width);
                });
            });
    }

    /// Draw the New login tab over `view`: the login by account, what it
    /// lists, and the kept passwords; what the player asked for goes in
    /// `asked`.
    pub(crate) fn show_login(
        &mut self,
        ui: &mut egui::Ui,
        view: &HubView<'_>,
        asked: &mut Option<HubAction>,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("hub-new-login")
            .auto_shrink(false)
            .show(ui, |ui| {
                self.login(ui, asked);
                if let Some(listing) = self.listing(view) {
                    self.listed(ui, listing, view, asked);
                }
                ui.separator();
                ui.strong("Kept passwords");
                kept(ui, view.roster, asked);
            });
    }

    /// One roster character's card: its star, who and where, then Start
    /// when its password is kept, or a password to type when it is not; and
    /// Forget.
    fn card(
        &mut self,
        ui: &mut egui::Ui,
        card: &RosterCard,
        view: &HubView<'_>,
        width: CardWidth,
    ) -> Option<HubAction> {
        let name = roster_name(&card.game, &card.character);
        // The binary offers a character by the name it can find it by.
        let offered = view.offered.iter().find(|offered| {
            offered.eq_ignore_ascii_case(&card.character) || offered.eq_ignore_ascii_case(&name)
        });
        let (mut starred, mut start, mut log_in, mut forget) = (false, false, false, false);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(width.0);
            let who = ui
                .horizontal(|ui| {
                    starred = star(ui, card.favourite).clicked();
                    ui.strong(&card.character)
                })
                .inner;
            ui.weak(format!("{} · {}", game_name(&card.game), card.account));
            if offered.is_none() {
                let typed = self.typed.entry(name.clone()).or_default();
                ui.horizontal(|ui| {
                    password_field(
                        ui,
                        egui::TextEdit::singleline(&mut typed.password)
                            .hint_text("Password")
                            .desired_width((width.0 - 70.0).max(60.0)),
                    )
                    .labelled_by(who.id);
                    ui.checkbox(&mut typed.keep, "Keep");
                });
                let ready = !typed.password.is_empty();
                ui.horizontal(|ui| {
                    log_in = ui
                        .add_enabled(
                            ready,
                            egui::Button::new(format!("Log {} in", card.character)),
                        )
                        .clicked();
                    forget = ui.button("Forget").clicked();
                });
            } else {
                ui.horizontal(|ui| {
                    start = ui.button(format!("Start {}", card.character)).clicked();
                    forget = ui.button("Forget").clicked();
                });
            }
        });
        if starred {
            return Some(HubAction::Ask(HubRequest::Favourite(name, !card.favourite)));
        }
        if let (true, Some(offered)) = (start, offered) {
            return Some(HubAction::Ask(HubRequest::Add(offered.clone())));
        }
        if log_in {
            let typed = self.typed.remove(&name).unwrap_or_default();
            return Some(HubAction::Ask(HubRequest::Login(Login {
                account: card.account.clone(),
                password: Password::new(typed.password),
                game: card.game.clone(),
                character: card.character.clone(),
                remember: typed.keep,
            })));
        }
        if forget {
            self.typed.remove(&name);
            return Some(HubAction::Ask(HubRequest::Forget(name)));
        }
        None
    }

    /// The login by account: account, password and game, then *Log in*,
    /// which asks the service for the account's characters, and *Log out*.
    fn login(&mut self, ui: &mut egui::Ui, asked: &mut Option<HubAction>) {
        let Self { new, held, .. } = self;
        egui::Grid::new("hub-new-login")
            .num_columns(2)
            .show(ui, |ui| {
                let label = ui.label("Account");
                ui.text_edit_singleline(&mut new.account)
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label("Password");
                password_field(ui, egui::TextEdit::singleline(&mut new.typed.password))
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
        ui.checkbox(&mut new.typed.keep, "Keep the password");
        ui.horizontal(|ui| {
            let ready = !new.account.trim().is_empty() && !new.typed.password.is_empty();
            if ui.add_enabled(ready, egui::Button::new("Log in")).clicked() {
                // The password leaves the form and is held for the listing's
                // Play; what else was typed stays.
                let typed = std::mem::take(&mut new.typed);
                let account = new.account.trim().to_owned();
                *held = Some(Held {
                    account: account.clone(),
                    game: new.game.clone(),
                    password: Password::new(typed.password.clone()),
                });
                *asked = Some(HubAction::Ask(HubRequest::Characters(Account {
                    account,
                    password: Password::new(typed.password),
                    game: new.game.clone(),
                    remember: typed.keep,
                })));
            }
            if ui
                .add_enabled(held.is_some(), egui::Button::new("Log out"))
                .clicked()
            {
                *held = None;
            }
        });
        if let Some(held) = held {
            ui.weak(format!(
                "Logged in as {} on {}.",
                held.account,
                game_name(&held.game)
            ));
        }
    }

    /// The listing of the account logged in, once the service has answered.
    fn listing<'v>(&self, view: &HubView<'v>) -> Option<&'v Listing> {
        let held = self.held.as_ref()?;
        view.listing.filter(|listing| {
            listing.account.eq_ignore_ascii_case(&held.account) && listing.game == held.game
        })
    }

    /// The account's characters, each to add or forget, star, and play.
    fn listed(
        &self,
        ui: &mut egui::Ui,
        listing: &Listing,
        view: &HubView<'_>,
        asked: &mut Option<HubAction>,
    ) {
        let Some(held) = &self.held else { return };
        if listing.characters.is_empty() {
            ui.weak("This account has no character on this game.");
            return;
        }
        egui::Grid::new("hub-listing")
            .num_columns(4)
            .show(ui, |ui| {
                for character in &listing.characters {
                    let name = roster_name(&listing.game, character);
                    let kept = view.roster.iter().find(|card| {
                        card.game.eq_ignore_ascii_case(&listing.game)
                            && card.character.eq_ignore_ascii_case(character)
                    });
                    let saved = |favourite| {
                        HubAction::Ask(HubRequest::Remember(Saved {
                            character: character.clone(),
                            account: listing.account.clone(),
                            game: listing.game.clone(),
                            favourite,
                        }))
                    };
                    ui.strong(character);
                    match kept {
                        Some(_) if ui.button("Forget").clicked() => {
                            *asked = Some(HubAction::Ask(HubRequest::Forget(name.clone())));
                        }
                        None if ui.button("Add").clicked() => *asked = Some(saved(false)),
                        _ => {}
                    }
                    let starred = kept.is_some_and(|card| card.favourite);
                    if star(ui, starred).clicked() {
                        *asked = Some(match kept {
                            Some(_) => HubAction::Ask(HubRequest::Favourite(name, !starred)),
                            None => saved(true),
                        });
                    }
                    if on_table(view, &listing.game, character) {
                        ui.weak("Playing");
                    } else if ui.button("Play").clicked() {
                        *asked = Some(HubAction::Ask(HubRequest::Login(Login {
                            account: held.account.clone(),
                            password: held.password.clone(),
                            game: listing.game.clone(),
                            character: character.clone(),
                            remember: false,
                        })));
                    }
                    ui.end_row();
                }
            });
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

/// A password field, masked, that keeps **no undo history**: egui's field
/// keeps every stable text it held, so once *Log in* or *Log out* had emptied
/// it, Ctrl+Z put the password back (the crate review of 2026-10-01, GU-A-2).
fn password_field(ui: &mut egui::Ui, edit: egui::TextEdit<'_>) -> egui::Response {
    let response = ui.add(edit.password(true));
    if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), response.id) {
        state.clear_undoer();
        state.store(ui.ctx(), response.id);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate review of 2026-10-01, GU-A-2: a password field kept its
    /// undo history, so after *Log out* emptied it, Ctrl+Z put the password
    /// back. A password field keeps none.
    #[test]
    fn a_password_cleared_is_not_brought_back_by_undo() {
        use egui_kittest::kittest::Queryable as _;
        let mut harness = egui_kittest::Harness::new_ui_state(
            |ui, text: &mut String| {
                password_field(ui, egui::TextEdit::singleline(text));
            },
            String::new(),
        );
        harness
            .get_by_role(egui::accesskit::Role::PasswordInput)
            .click();
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::PasswordInput)
            .type_text("hunter2");
        harness.run();
        // egui keeps an undo point once the text has been still a second.
        for _ in 0..180 {
            harness.step();
        }
        assert_eq!(harness.state(), "hunter2");
        // Log out: the form lets go of it.
        harness.state_mut().clear();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        harness.run();
        assert_eq!(harness.state(), "", "undo brought the password back");
    }

    /// On the table is by game and name: the same name on another game is
    /// another character, whether on the Not launched tab or in an
    /// account's list (the crate review of 2026-09-28, R6).
    #[test]
    fn on_the_table_is_by_game_and_name() {
        let cards = [cena_ui::SessionCard {
            game: "GS3".to_owned(),
            ..cena_ui::SessionCard::of("0".to_owned(), "Ashryn".to_owned(), None)
        }];
        let view = HubView {
            cards: &cards,
            offered: &[],
            roster: &[],
            listing: None,
            merged: &[],
            said: None,
            windowed: &[],
            lich: &[],
        };
        assert!(on_table(&view, "gs3", "ashryn"));
        assert!(!on_table(&view, "GSF", "Ashryn"));
        assert!(!on_table(&view, "GS3", "Baelor"));
    }
}
