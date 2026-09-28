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
//! them ([`cena_ui::room_player`]). What it shows are widgets, each one thing
//! (`crate::widget`), held by windows inside it -- a standalone window around
//! one, or a custom window of several, bare -- that drag, resize and snap,
//! their layout kept by the character's name (`plan/49` Stage A,
//! `holders.rs`, `crate::layout`).
//!
//! **The focus rule**, `VellumFE`'s (`reference/VellumFE/src/frontend/gui/app.rs:3426`),
//! for the author's complaint: a click nothing else took returns the
//! keyboard to the command input, so the player can type without clicking it.

mod arrange;
mod draw;
mod holders;
mod links;
mod menu;
mod options;

use std::path::PathBuf;
use std::time::Instant;

use cena_session::Snapshot;
use cena_ui::{HuntView, LifecycleView};
use egui::Id;

use crate::layout::{GRID, Layout};
use crate::story::Story;
use holders::Engaged;

/// What a play window asks for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Asked {
    /// Send this line on the character, as typed: Hydra's command line
    /// first, then the game.
    Send(String),
    /// Send this line as `Send` does, but not echoed in the story: a menu
    /// asked for on a click, which the player did not type.
    Quietly(String),
    /// Stop everything Hydra is doing on the character (`;stop`).
    Stop,
    /// Read the keybinds file again.
    ReloadKeys,
    /// Keep this custom window in the presets every character adds from.
    SavePreset(crate::layout::Preset),
    /// Forget the preset of this name.
    ForgetPreset(String),
    /// Open the settings menu on this character (`plan/50` §7 step 1): at
    /// a widget's own page, when its right-click asked (step 8, as the
    /// author corrected it).
    Settings(Option<String>),
    /// Open the settings menu at Hydra's *Keys* page (step 8).
    Keys,
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
    /// What its hunt is doing, when one runs.
    pub(crate) hunt: Option<&'a HuntView>,
    /// `NumLock`, once a numpad press has shown it.
    pub(crate) numlock: Option<bool>,
    /// What the keybinds file bound, and what is wrong in it.
    pub(crate) keys: &'a [String],
    /// The other characters running, which a widget may follow.
    pub(crate) others: &'a [crate::widget::Character],
    /// The presets a player saved, which every character adds from.
    pub(crate) presets: &'a crate::layout::Library,
}

/// A play window's own state, which outlives a frame.
#[derive(Debug)]
pub(crate) struct Play {
    /// Which session: every id in the window is its own.
    session: u32,
    /// The character, whose name its layout is kept by.
    name: String,
    /// The game it is on, by the instance its files are named for: its
    /// layout is kept by game and name.
    instance: Option<&'static str>,
    /// Where layouts are kept; `None`, and nothing is saved.
    layouts: Option<PathBuf>,
    /// Where its windows sit: saved, or fitted to the window when first drawn.
    layout: Option<Layout>,
    /// The windows let go for a drag or resize under way.
    engaged: Vec<Engaged>,
    /// Arrange is on: a custom window's cells take the pointer
    /// (`arrange.rs`). Never saved: a play window opens with it off.
    arranging: bool,
    /// The grid and the snapping guides are showing: a window pressed has
    /// moved or changed its size, not merely been clicked.
    guiding: bool,
    /// Fitted with the room's parts in a custom window, for the tests of
    /// arranging cells (`Layout::with_room_parts`).
    #[cfg(test)]
    pub(super) room_parts: bool,
    /// The cell being moved or resized, with Arrange on.
    cell: Option<arrange::CellGesture>,
    /// Each custom window's inside as last drawn, from the play area's top
    /// left: where its cells were when a press comes.
    insides: Vec<(u32, egui::Rect)>,
    /// How far each widget that counts what it says was read, by its id: a
    /// tab not showing shows what came since (`draw.rs`, `unread`).
    read: std::collections::HashMap<u32, u64>,
    /// The Add-a-widget list, while open (`menu.rs`).
    adding: Option<menu::Adding>,
    /// A window's or widget's right-click menu, while open (`menu.rs`).
    menu: Option<menu::Menu>,
    /// An object's menu, asked of the game on a click, until it is chosen
    /// from or closed (`links.rs`).
    asking: Option<links::Asking>,
    /// Object menus asked for, ever: the next request's number.
    menus_asked: u32,
    /// What a menu asked of the app this frame, which the window hands on.
    out: Option<Asked>,
    /// Why the layout could not be saved, until it can.
    unsaved: Option<String>,
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
    /// A play window for session `session`, the character `name` on the
    /// game `instance` names, with its layout from `layouts` when one was
    /// saved there.
    pub(crate) fn new(
        session: u32,
        name: &str,
        instance: Option<&'static str>,
        layouts: Option<PathBuf>,
    ) -> Self {
        let layout = layouts
            .as_deref()
            .and_then(|dir| Layout::load(dir, instance, name));
        Self {
            session,
            name: name.to_owned(),
            instance,
            layouts,
            layout,
            engaged: Vec::new(),
            arranging: false,
            guiding: false,
            #[cfg(test)]
            room_parts: false,
            cell: None,
            insides: Vec::new(),
            read: std::collections::HashMap::new(),
            adding: None,
            menu: None,
            asking: None,
            menus_asked: 0,
            out: None,
            unsaved: None,
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
        let mut grid = self.layout.as_ref().map_or(GRID, |layout| layout.grid);
        let unsaved = self.unsaved.clone();
        let mut arranging = self.arranging;
        let mut locked = self.layout.as_ref().is_some_and(|layout| layout.locked);
        let top = egui::Panel::top(Id::new(("play-top", session)))
            .show(ui, |ui| {
                draw::top(
                    ui,
                    view,
                    &mut grid,
                    (&mut arranging, &mut locked),
                    unsaved.as_deref(),
                )
            })
            .inner;
        self.arranging = arranging && !locked;
        if let Some(layout) = &mut self.layout
            && layout.locked != locked
        {
            layout.locked = locked;
            self.save();
        }
        let mut changed = false;
        match top {
            Some(draw::Top::Stop) => asked = Some(Asked::Stop),
            Some(draw::Top::Settings) => asked = Some(Asked::Settings(None)),
            Some(draw::Top::Keys) => asked = Some(Asked::Keys),
            Some(draw::Top::ReloadKeys) => asked = Some(Asked::ReloadKeys),
            Some(draw::Top::Fit) => {
                self.layout = None;
                changed = true;
            }
            Some(draw::Top::Grid) => {
                if let Some(layout) = &mut self.layout {
                    layout.grid = grid;
                    changed = true;
                }
            }
            Some(draw::Top::AddWidget) => self.adding = Some(menu::Adding::default()),
            Some(draw::Top::NewCustom) => {
                if let Some(layout) = &mut self.layout {
                    layout.new_custom();
                    self.arranging = !layout.locked;
                    changed = true;
                }
            }
            None => {}
        }
        egui::Panel::bottom(Id::new(("play-input-panel", session))).show(ui, |ui| {
            if let Some(line) = self.input(ui) {
                asked = Some(Asked::Send(line));
            }
        });
        let area = ui.available_rect_before_wrap();
        changed |= self.arrange(ui, view);
        let received: Vec<&str> = view.story.streams.ids().collect();
        changed |= self.add_list(ui.ctx(), area, view.others, view.presets, &received);
        changed |= self.right_click(ui.ctx(), area, view.others, &received);
        self.object_menu(ui.ctx(), view);
        crate::carry::show(ui.ctx(), session);
        asked = asked.or(self.out.take());
        if changed {
            self.save();
        }
        // The focus rule: nothing holds the keyboard, so the input takes it.
        if ui.ctx().memory(|memory| memory.focused().is_none()) {
            let id = self.input_id();
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
        }
        asked
    }

    /// Keep the layout under the character's name, or say why it could not be.
    fn save(&mut self) {
        let (Some(dir), Some(layout)) = (&self.layouts, &self.layout) else {
            return;
        };
        self.unsaved = layout
            .save(dir, self.instance, &self.name)
            .err()
            .map(|why| why.to_string());
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

#[cfg(test)]
mod tests;
