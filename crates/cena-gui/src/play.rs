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
mod calibrator;
mod draw;
mod drawers;
mod holders;
mod links;
mod menu;
mod options;

use std::path::PathBuf;
use std::time::Instant;

use cena_session::Snapshot;
use cena_ui::{HuntView, LifecycleView};
use egui::Id;

use crate::layout::{Drawers, GRID, Layout, Zones};
use crate::story::Story;
use crate::widget::Widget;
use holders::Engaged;
pub(crate) use options::Pictures;

/// The player's own Lich for the character (`plan/51`): ticked while it
/// runs; what it was switched to, when it was.
pub(crate) fn lich_switch(ui: &mut egui::Ui, running: bool) -> Option<bool> {
    let mut on = running;
    ui.checkbox(&mut on, "Lich")
        .on_hover_text("Your own Lich for this character, kept on or off: lich on, lich off")
        .changed()
        .then_some(on)
}

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
    /// Open the character's log window (`plan/25` step 8).
    Log,
    /// Open the trigger editor (`plan/54`).
    Triggers,
    /// Open the trigger editor on a new trigger for this line's words
    /// (`plan/54` step 4).
    TriggerFrom(String),
    /// Switch the player's own Lich on, or off (`;lich on`, `;lich off`).
    Lich(bool),
    /// Use this macro set over set 0, kept for the character; 0 for set 0
    /// alone (`plan/52` step 2).
    UseSet(u8),
    /// The play window of the hub's first to ninth character, 1 to 9, open
    /// and with the keyboard (`plan/52` step 8).
    Character(u8),
    /// Route the minimap to this room, or to none (`plan/53` §6 item 6).
    Aim(Option<u32>),
    /// Send Hydra's command `word` with the character's own symbol, echoed
    /// or not.
    Hydra {
        /// The command, without its symbol.
        word: String,
        /// Whether the story shows it.
        echo: bool,
    },
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
    /// Where it is on the map, when there is one.
    pub(crate) minimap: Option<&'a cena_ui::MinimapView>,
    /// Each creature's tag after its name (`.targetid`).
    pub(crate) tags: bool,
    /// `NumLock`, once a numpad press has shown it.
    pub(crate) numlock: Option<bool>,
    /// The macro set the character uses over set 0; 0 for none.
    pub(crate) set: u8,
    /// What the keybinds file bound, and what is wrong in it.
    pub(crate) keys: &'a [String],
    /// The other characters running, which a widget may follow.
    pub(crate) others: &'a [crate::widget::Character],
    /// The presets a player saved, which every character adds from.
    pub(crate) presets: &'a crate::layout::Library,
    /// Whether the player's own Lich runs for it.
    pub(crate) lich: bool,
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
    /// What the gesture under way has done to the windows it let go: the
    /// grid and the guides show once one has moved or changed its size,
    /// not merely been clicked (`holders.rs`).
    moving: holders::Moving,
    /// Fitted with the room's parts in a custom window, for the tests of
    /// arranging cells (`Layout::with_room_parts`).
    #[cfg(test)]
    pub(super) room_parts: bool,
    /// The cell being moved or resized, with Arrange on.
    cell: Option<arrange::CellGesture>,
    /// Each custom window's inside as last drawn, from the play area's top
    /// left: where its cells were when a press comes.
    insides: Vec<(u32, egui::Rect)>,
    /// Where the main area and each open drawer lie this frame
    /// (`plan/49` Stage E).
    zones: Zones,
    /// How far each widget that counts what it says was read, by its id: a
    /// tab not showing shows what came since (`draw.rs`, `unread`).
    read: std::collections::HashMap<u32, u64>,
    /// The Add-a-widget list, while open (`menu.rs`).
    adding: Option<menu::Adding>,
    /// A window's or widget's right-click menu, while open (`menu.rs`).
    menu: Option<menu::Menu>,
    /// An Injuries widget's calibrator, while open (`calibrator.rs`).
    calibrating: Option<calibrator::Calibrator>,
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
    /// What was sent, for up and down, kept per character.
    history: history::History,
    /// A key filled the input (`plan/52` §2): where its cursor goes, the
    /// end, as the input takes the keyboard.
    filled: Option<usize>,
    /// The widget last clicked in, by its placed id: the window in use,
    /// which the scrolling keys act on (`plan/52` step 4). The story until
    /// one is, or once the one clicked is gone.
    in_use: Option<u32>,
    /// The tabs not showing that have lines unread, by placed id, in the
    /// order they were drawn: what `next_unread_tab` shows (step 5).
    unread_tabs: Vec<u32>,
    /// The Find bar, while it is open (step 6).
    find: Option<find::FindBar>,
    /// Where each widget showing was drawn last, by placed id.
    shown_rects: Vec<(u32, egui::Rect)>,
    /// What the game last listed as ones to attack, in its order, and the
    /// one targeted (step 7).
    targets: (Vec<i64>, Option<i64>),
}

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
            moving: holders::Moving::Not,
            #[cfg(test)]
            room_parts: false,
            cell: None,
            insides: Vec::new(),
            zones: Zones::default(),
            read: std::collections::HashMap::new(),
            adding: None,
            menu: None,
            calibrating: None,
            asking: None,
            menus_asked: 0,
            out: None,
            unsaved: None,
            input: String::new(),
            history: history::History::default(),
            filled: None,
            in_use: None,
            unread_tabs: Vec::new(),
            find: None,
            shown_rects: Vec::new(),
            targets: (Vec::new(), None),
        }
    }

    /// Put `text` in the command input, not sent: a fill macro's key
    /// (`plan/52` §2).
    pub(crate) fn fill(&mut self, text: &str) {
        text.clone_into(&mut self.input);
        self.history.reset();
        self.filled = Some(self.input.chars().count());
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
        let unsaved = self
            .unsaved
            .as_ref()
            .map(|why| format!("Layout not saved: {why}"))
            .or_else(|| (self.history.unsaved()).map(|why| format!("History not saved: {why}")));
        let mut arranging = self.arranging;
        let mut locked = self.layout.as_ref().is_some_and(|layout| layout.locked);
        let mut drawers = self
            .layout
            .as_ref()
            .map_or_else(Drawers::default, |layout| layout.drawers);
        let top = egui::Panel::top(Id::new(("play-top", session)))
            .show(ui, |ui| {
                draw::top(
                    ui,
                    view,
                    (&mut grid, &mut drawers),
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
            Some(draw::Top::Log) => asked = Some(Asked::Log),
            Some(draw::Top::Triggers) => asked = Some(Asked::Triggers),
            Some(draw::Top::ReloadKeys) => asked = Some(Asked::ReloadKeys),
            Some(draw::Top::Lich(on)) => asked = Some(Asked::Lich(on)),
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
            Some(draw::Top::Drawers) => {
                if let Some(layout) = &mut self.layout {
                    layout.drawers = drawers;
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
        self.ask_find(ui.ctx());
        if let Some(snapshot) = view.snapshot {
            let targeting = &snapshot.state.targeting;
            self.targets = (targeting.ids().to_vec(), targeting.current());
        }
        changed |= self.arrange(ui, view);
        let received: Vec<Widget> = (view.story.streams.ids())
            .map(|id| Widget::Stream(id.to_owned()))
            .chain(view.snapshot.into_iter().flat_map(|snapshot| {
                (snapshot.state.dialogs.iter()).map(|(id, _)| Widget::Dialog(id.to_owned()))
            }))
            .collect();
        changed |= self.add_list(ui.ctx(), area, view.others, view.presets, &received);
        changed |= self.right_click(ui.ctx(), area, view.others, &received);
        self.object_menu(ui.ctx(), view);
        self.calibrator(ui.ctx());
        self.find_bar(ui.ctx(), area);
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

    /// The command input: Enter sends. Up and down walk what was sent as
    /// the keys' actions do (`plan/52` step 3), which may be bound elsewhere.
    fn input(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let id = self.input_id();
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.input)
                .id(id)
                // Tab is the keys', never egui's to move the keyboard on.
                .lock_focus(true)
                .hint_text("Type a command")
                .desired_width(f32::INFINITY),
        );
        if let Some(end) = self.filled.take() {
            let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            let end = egui::text::CCursor::new(end);
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(ui.ctx(), id);
            response.request_focus();
        }
        let entered =
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if !entered {
            return None;
        }
        response.request_focus();
        self.enter()
    }

    /// What is typed, sent: taken from the input, and kept in the history
    /// unless it is the last line again. Nothing for an empty line.
    fn enter(&mut self) -> Option<String> {
        let line = std::mem::take(&mut self.input);
        if line.trim().is_empty() {
            self.history.reset();
            return None;
        }
        self.history.keep(&line);
        Some(line)
    }

    /// Keep what is sent in `dir`, by the character's game and name, and
    /// take up what was kept there: the app's windows do, and a test's
    /// need not.
    #[must_use]
    pub(crate) fn keeping_history(mut self, dir: &std::path::Path) -> Self {
        self.history = history::History::kept(dir, self.instance, &self.name);
        self
    }
}

mod find;
mod history;
mod keyed;
#[cfg(test)]
mod tests;
