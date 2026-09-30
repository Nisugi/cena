//! The settings menu's window, and what the window does with what it asks
//! (`plan/50` §7): the binary's pages through the binary, Hydra's own and
//! the keybinds here, and a widget's own page in its play window's layout.
//! Moved out of `app.rs` when widget pages took it to its cap.

use std::path::{Path, PathBuf};

use super::{App, TITLE};

/// The settings menu's key among the windows' places.
const SETTINGS: &str = "settings";
use crate::keys::page::{KeyChange, Place};
use crate::keys::{self, Whose};
use crate::{KeysView, MenuAsked, MenuView};

impl App {
    /// The settings menu, in a window of its own, while it is open.
    pub(super) fn settings(&mut self, context: &egui::Context, glance: &crate::sessions::Glance) {
        if !self.menu.open {
            return;
        }
        let own = [self.own.page()];
        let shown = self.shown_play();
        let overlays = match shown {
            Some(_) => self.overlays(),
            None => crate::play::Pictures::default(),
        };
        // The keys of the character the menu shows, over every character's.
        let file = self.menu_mine();
        self.load_mine(file.as_deref());
        let mine = file
            .as_ref()
            .and_then(|file| self.mine.get(file))
            .map(|(mine, _)| mine);
        let bound = self.keys.rows(self.menu.keys_set(), mine);
        let keys_said = match &file {
            Some(file) => {
                let mut said = vec![format!(
                    "This character's own keys are kept in {}; every character's in the keybinds file.",
                    file.display()
                )];
                said.extend(
                    self.mine
                        .get(file)
                        .into_iter()
                        .flat_map(|(_, problems)| problems.iter().cloned()),
                );
                said
            }
            None => self.keys_said.clone(),
        };
        let widgets = shown
            .map(|session| {
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
                numpad_always: self.keys.numpad_always(),
                said: &keys_said,
                caught: self.caught_for_page.as_deref(),
                character: file.is_some(),
                chosen: mine.map_or(0, |mine| mine.chosen),
            },
            widgets: &widgets,
        };
        let menu = &mut self.menu;
        let mut asked = Vec::new();
        let closed = self.placements.show(
            context,
            SETTINGS,
            egui::ViewportId::from_hash_of("settings"),
            ([760.0, 560.0], format!("Settings — {TITLE}")),
            |ui| asked.extend(menu.show(ui, &view)),
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
            MenuAsked::Key { character, change } => {
                let said = self.key_changed(character.as_deref(), &change);
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

    /// Write `change` to the keys of `character`, as the roster names it,
    /// or to every character's: a key shared, to one file and out of the
    /// other.
    fn key_changed(&self, character: Option<&str>, change: &KeyChange) -> Result<String, String> {
        let every = self.keys_file.as_deref();
        let data = every.and_then(Path::parent);
        let mine = character.and_then(|character| {
            let (game, name) = character.split_once(':')?;
            Some((self.mine_path(game, name)?, name))
        });
        let (Some(data), Some(every)) = (data, every) else {
            return Err("Keys: nothing is kept here.".to_owned());
        };
        let file = |place_every: bool| match (&mine, place_every) {
            (Some((file, name)), false) => {
                Ok(((file.as_path(), Whose::Character), format!("{name}'s keys")))
            }
            (None, false) => Err("Keys: that character's keys cannot be kept.".to_owned()),
            (_, true) => Ok(((every, Whose::Every), "Every character's keys".to_owned())),
        };
        let place_every = match change {
            KeyChange::Bind { place, .. }
            | KeyChange::Unbind { place, .. }
            | KeyChange::Restore { place, .. } => place.every,
            KeyChange::NumpadAlways(_) => true,
            KeyChange::Choose(_) => false,
            KeyChange::Share {
                key,
                does,
                set,
                every: to_every,
            } => {
                let ((to, to_whose), said) = file(*to_every)?;
                let ((from, from_whose), _) = file(!*to_every)?;
                let place = |every| Place { set: *set, every };
                let bound = KeyChange::Bind {
                    key: key.clone(),
                    does: does.clone(),
                    was: None,
                    place: place(*to_every),
                };
                let taken = KeyChange::Restore {
                    key: key.clone(),
                    place: place(!*to_every),
                };
                keys::write::apply_pair(
                    data,
                    (to, to_whose, &bound),
                    (from, from_whose, &taken),
                    (&self.keys, &said),
                )?;
                return Ok(format!(
                    "{said}: {key} is {} now.",
                    if *to_every {
                        "every character's"
                    } else {
                        "only this character's"
                    }
                ));
            }
        };
        let (at, said) = file(place_every)?;
        keys::write::apply(data, at, change, (&self.keys, &said))
    }

    /// The keys file of the character the menu shows; `None` while it
    /// shows every character's.
    fn menu_mine(&self) -> Option<PathBuf> {
        let (game, name) = self.menu.character()?.split_once(':')?;
        self.mine_path(game, name)
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

    /// The images a bar may lay over itself, each PNG in the data folder's
    /// `overlays`, and the doll pictures in its `dolls`, by name. **Listed
    /// again no oftener than every two
    /// seconds**: the menu asks each frame it is drawn, and the folder was
    /// read from the disk sixty times a second for as long as Settings
    /// stood open on a character (the review of 2026-09-29).
    fn overlays(&mut self) -> crate::play::Pictures {
        let now = std::time::Instant::now();
        if let Some((at, found)) = &self.overlays
            && now.duration_since(*at) < OVERLAYS_KEPT
        {
            return found.clone();
        }
        let found = crate::play::Pictures {
            overlays: self.list_pictures("overlays"),
            dolls: self
                .list_pictures("dolls")
                .into_iter()
                .filter(|path| !crate::doll_art::is_layer(path))
                .collect(),
        };
        self.overlays = Some((now, found.clone()));
        found
    }

    /// The PNGs in the data folder's `folder` as it is on the disk now.
    fn list_pictures(&self, folder: &str) -> Vec<PathBuf> {
        let Some(folder) = self
            .layouts
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(|data| data.join(folder))
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

/// How long a listing of the overlays folder is good for.
const OVERLAYS_KEPT: std::time::Duration = std::time::Duration::from_secs(2);
