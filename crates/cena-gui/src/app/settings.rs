//! The settings menu's window, and what the window does with what it asks
//! (`plan/50` §7): the binary's pages through the binary, Hydra's own and
//! the keybinds here, and a widget's own page in its play window's layout.
//! Moved out of `app.rs` when widget pages took it to its cap.

use std::path::PathBuf;

use super::{App, TITLE};
use crate::keys;
use crate::{KeysView, MenuAsked, MenuView};

impl App {
    /// The settings menu, in a window of its own, while it is open.
    pub(super) fn settings(&mut self, context: &egui::Context, glance: &crate::sessions::Glance) {
        if !self.menu.open {
            return;
        }
        let own = [self.own.page()];
        let bound = self.keys.listed();
        let widgets = self
            .shown_play()
            .map(|session| {
                let overlays = self.overlays();
                self.plays
                    .get(&session)
                    .map(|window| window.play.widget_pages(&overlays))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let view = MenuView {
            roster: &glance.roster,
            pages: glance
                .settings
                .as_ref()
                .map(|(whose, pages)| (whose.as_str(), pages.as_slice())),
            said: glance.said.as_ref().map(|(said, _)| said.as_str()),
            own: &own,
            keys: KeysView {
                bound: &bound,
                numpad_always: self.keys.numpad_always,
                said: &self.keys_said,
                numpad: self.numpad_for_menu.as_deref(),
            },
            widgets: &widgets,
        };
        let menu = &mut self.menu;
        // What every pass asks, kept: see `App::play`.
        let (mut asked, mut closed) = (Vec::new(), false);
        context.show_viewport_immediate(
            egui::ViewportId::from_hash_of("settings"),
            egui::ViewportBuilder::default()
                .with_title(format!("Settings — {TITLE}"))
                .with_inner_size([760.0, 560.0]),
            |ui, _class| {
                closed |= ui.input(|input| input.viewport().close_requested());
                asked.extend(menu.show(ui, &view));
            },
        );
        if closed {
            self.menu.open = false;
        }
        for asked in asked {
            self.menu_asked(asked);
        }
    }

    /// Do what the settings menu asked: the binary's through it, Hydra's
    /// own, the keybinds and a widget's page here, saying what was done.
    pub(super) fn menu_asked(&mut self, asked: MenuAsked) {
        let said = match asked {
            MenuAsked::Binary(request) => return self.sessions.ask(request),
            MenuAsked::Own { key, to } => {
                let said = self.own.change(&key, to.as_deref());
                self.hub.card_width = self.own.card_width();
                said
            }
            MenuAsked::Key(change) => {
                let data = self.keys_file.as_deref().and_then(std::path::Path::parent);
                let said = data.map_or_else(
                    || Err("Keys: nothing is kept here.".to_owned()),
                    |data| keys::write::apply(data, &change),
                );
                if said.is_ok() {
                    self.read_keys();
                }
                said
            }
            MenuAsked::Widget { page, key, to } => self
                .shown_play()
                .and_then(|session| self.plays.get_mut(&session))
                .map_or_else(
                    || Err("That character has no window open to change.".to_owned()),
                    |window| window.play.widget_change(&page, &key, to.as_deref()),
                ),
        };
        self.menu.tell(said.unwrap_or_else(|why| why));
    }

    /// The session of the character the menu shows, when it has a play
    /// window: its roster name, `GAME:Name`, matched to a seat.
    fn shown_play(&self) -> Option<u32> {
        let (game, name) = self.menu.character()?.split_once(':')?;
        self.sessions
            .seated()
            .iter()
            .find(|seat| seat.game == game && seat.name.eq_ignore_ascii_case(name))
            .map(|seat| seat.id.0)
    }

    /// The images a bar may lay over itself: each PNG in the data folder's
    /// `overlays`, by name.
    fn overlays(&self) -> Vec<PathBuf> {
        let Some(folder) = self
            .layouts
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(|data| data.join("overlays"))
        else {
            return Vec::new();
        };
        let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|x| x.eq_ignore_ascii_case("png"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        found.sort();
        found
    }
}
