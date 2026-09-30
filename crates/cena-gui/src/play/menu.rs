//! A play window's two menus (`plan/49` Stage A step 6).
//!
//! **Add a widget**, from the Layout menu: every kind in one list, found by
//! typing, grouped as `plan/49` §3 sorts Saga's panels (Saga's *"Find a
//! panel..."*), each saying whether one is shown already. A click adds it in
//! a standalone window of its own. Above them the presets, Hydra's and those
//! saved (step 7): a click places a copy. At the bottom, closed, the **Advanced**
//! place (§1 row 8): the character what is added follows, when another than
//! the window's own -- a party's vitals in one window, say (§1 row 3).
//!
//! **A right-click** on a window or a widget in one opens its menu: remove
//! it, or add a tab beside it of a kind of its own group (a stream beside a
//! stream); rename, save as a preset or remove a custom window; and one
//! **Advanced** entry, closed,
//! for the character the widget follows. Neither menu offers a story to
//! another character: no window mixes two characters' story (§1 row 7).

use std::collections::HashSet;

use egui::{Id, Order, Pos2, Rect, RichText, vec2};

use super::{Asked, Play};
use crate::layout::{Holds, Layout, Library, Preset};
use crate::widget::{Character, Group, Widget};

/// The Add-a-widget list, open.
#[derive(Debug, Default)]
pub(super) struct Adding {
    /// What was typed to find a widget.
    search: String,
    /// The character what is added follows, chosen in the Advanced place;
    /// `None`, the window's own.
    whose: Option<String>,
}

/// A window's or widget's right-click menu, open.
#[derive(Debug)]
pub(super) struct Menu {
    /// The window it is for.
    holder: u32,
    /// The widget it is for, when it was opened on one.
    placed: Option<u32>,
    /// Where it opened.
    at: Pos2,
    /// A custom window's new title, while it is being typed.
    renaming: Option<String>,
    /// The name a custom window is saved as a preset under, while typed.
    saving: Option<String>,
    /// The kinds a tab could be added of, listed.
    tabbing: bool,
    /// The story line it was opened on, as its words (`plan/54` step 4).
    line: Option<String>,
}

/// What a menu was asked to do.
enum Act {
    Remove(u32),
    RemoveWindow,
    Rename(String),
    Follow(u32, Option<String>),
    Save(String),
    /// The settings menu, at this page: the widget's own.
    Settings(String),
    /// A tab of this kind beside the widget.
    AddTab(u32, Widget),
    /// The trigger editor, a new trigger on this line's words.
    Trigger(String),
    /// The calibrator, on this Injuries widget's picture (`calibrator.rs`).
    Calibrate(u32, String),
}

impl Play {
    /// The Add-a-widget list, while it is open, over the play `area`, with
    /// `others` the characters a widget could follow. Whether it changed the
    /// layout.
    pub(super) fn add_list(
        &mut self,
        context: &egui::Context,
        area: Rect,
        others: &[Character],
        library: &Library,
        received: &[Widget],
    ) -> bool {
        let Some(shown) = self.layout.as_ref().map(kinds_shown) else {
            return false;
        };
        let Some(adding) = self.adding.as_mut() else {
            return false;
        };
        let every = kinds(received);
        let mut open = true;
        let mut chosen = None;
        let mut preset = None;
        let mut forget = None;
        egui::Window::new("Add a widget")
            .id(Id::new(("add-widget", self.session)))
            .open(&mut open)
            .collapsible(false)
            .default_size([260.0, 420.0])
            .default_pos(area.right_top() + vec2(-290.0, 10.0))
            .show(context, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut adding.search).hint_text("Find a widget..."),
                );
                let search = adding.search.to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    presets(ui, &search, library, &mut preset, &mut forget);
                    for group in Group::ALL {
                        let kinds: Vec<&Widget> = every
                            .iter()
                            .filter(|kind| kind.group() == group)
                            .filter(|kind| kind.name().to_lowercase().contains(&search))
                            .collect();
                        if kinds.is_empty() {
                            continue;
                        }
                        ui.label(RichText::new(group.name().to_uppercase()).small().weak());
                        for kind in kinds {
                            ui.horizontal(|ui| {
                                let own_only = adding.whose.is_some() && kind.is_story();
                                if ui
                                    .add_enabled(!own_only, egui::Button::new(kind.name()))
                                    .clicked()
                                {
                                    chosen = Some(kind.clone());
                                }
                                if shown.contains(kind) {
                                    ui.weak("shown");
                                }
                            });
                        }
                    }
                    advanced(ui, &mut adding.whose, others);
                });
            });
        let whose = adding_whose(self.adding.as_ref());
        if !open {
            self.adding = None;
        }
        if let Some(name) = forget {
            self.out = Some(Asked::ForgetPreset(name));
        }
        let Some(layout) = self.layout.as_mut() else {
            return false;
        };
        if let Some(preset) = preset {
            layout.add_preset(&preset, whose.as_deref());
            return true;
        }
        let Some(kind) = chosen else {
            return false;
        };
        layout.add_widget(kind, whose);
        true
    }

    /// A right-click on a window or a widget in one, at the play `area`,
    /// opens its menu; the menu, while open, over everything. Whether it
    /// changed the layout.
    pub(super) fn right_click(
        &mut self,
        context: &egui::Context,
        area: Rect,
        others: &[Character],
        received: &[Widget],
    ) -> bool {
        let (secondary, pressed, pointer, escape) = context.input(|input| {
            (
                input.pointer.secondary_clicked(),
                input.pointer.primary_pressed(),
                input.pointer.interact_pos(),
                input.key_pressed(egui::Key::Escape),
            )
        });
        if escape {
            self.menu = None;
        }
        if secondary
            && let Some(at) = pointer
            && let Some((holder, placed)) = self.under(context, area, at)
        {
            self.menu = Some(Menu {
                holder,
                placed,
                at,
                renaming: None,
                saving: None,
                tabbing: false,
                line: crate::widget::line_under(context),
            });
        }
        let (Some(menu), Some(layout)) = (self.menu.as_mut(), self.layout.as_ref()) else {
            return false;
        };
        let mut act = None;
        let shown = egui::Area::new(Id::new(("play-menu", self.session)))
            .order(Order::Foreground)
            .fixed_pos(menu.at)
            .show(context, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(170.0);
                    // A story line's words, offered first (`plan/54` step 4).
                    let mut trigger = None;
                    if let Some(line) = &menu.line {
                        if ui
                            .button("Make a trigger from this line")
                            .on_hover_text(line.as_str())
                            .clicked()
                        {
                            trigger = Some(Act::Trigger(line.clone()));
                        }
                        ui.separator();
                    }
                    act = trigger.or(items(ui, menu, layout, (others, received)));
                });
            });
        let outside = pressed && pointer.is_some_and(|at| !shown.response.rect.contains(at));
        let Some(act) = act else {
            if outside {
                self.menu = None;
            }
            return false;
        };
        let holder = menu.holder;
        self.menu = None;
        let Some(layout) = self.layout.as_mut() else {
            return false;
        };
        if let Act::Save(name) = &act {
            if let Some(Holds::Custom(custom)) = layout.holder(holder).map(|found| &found.holds) {
                self.out = Some(Asked::SavePreset(Preset::of(name, custom)));
            }
            return false;
        }
        if let Act::Settings(page) = act {
            self.out = Some(Asked::Settings(Some(page)));
            return false;
        }
        if let Act::Trigger(line) = act {
            self.out = Some(Asked::TriggerFrom(line));
            return false;
        }
        if let Act::Calibrate(placed, picture) = act {
            self.calibrating = Some(super::calibrator::Calibrator::open(placed, picture));
            return false;
        }
        match act {
            Act::Remove(placed) => layout.remove_widget(holder, placed),
            Act::RemoveWindow => layout.remove_window(holder),
            Act::Rename(title) => layout.rename(holder, &title),
            Act::Follow(placed, who) => layout.follow(placed, who),
            Act::AddTab(beside, kind) => {
                let _ = layout.add_tab(holder, beside, kind);
            }
            Act::Save(_) | Act::Settings(_) | Act::Trigger(_) | Act::Calibrate(..) => {}
        }
        true
    }

    /// The window at `at` on top, and the widget there in it, if any: a
    /// standalone window's own, or the cell's in a custom window.
    fn under(&self, context: &egui::Context, area: Rect, at: Pos2) -> Option<(u32, Option<u32>)> {
        let layout = self.layout.as_ref()?;
        let on_top = context.layer_id_at(at)?.id;
        let holder = layout
            .holders
            .iter()
            .find(|holder| on_top == super::holders::id(self.session, holder.id))?;
        let placed = match &holder.holds {
            Holds::One(placed) => Some(placed.id),
            Holds::Custom(custom) => {
                let inside = self
                    .insides
                    .iter()
                    .find(|(id, _)| *id == holder.id)
                    .map(|(_, inside)| inside.translate(area.min.to_vec2()))?;
                custom
                    .cells
                    .iter()
                    .rev()
                    .find(|cell| cell.rect().translate(inside.min.to_vec2()).contains(at))
                    .and_then(|cell| cell.shown().map(|shown| shown.id))
            }
        };
        Some((holder.id, placed))
    }
}

/// The presets, Hydra's and those saved, whose names hold `search`: a click
/// on one chooses it, a player's own can be forgotten.
fn presets(
    ui: &mut egui::Ui,
    search: &str,
    library: &Library,
    chosen: &mut Option<Preset>,
    forget: &mut Option<String>,
) {
    let found = |preset: &&Preset| preset.name.to_lowercase().contains(search);
    let hydras = Preset::hydras();
    let hydras: Vec<&Preset> = hydras.iter().filter(found).collect();
    let saved: Vec<&Preset> = library.presets().iter().filter(found).collect();
    if hydras.is_empty() && saved.is_empty() {
        return;
    }
    ui.label(RichText::new("PRESETS").small().weak());
    for preset in hydras {
        if ui.button(&preset.name).clicked() {
            *chosen = Some(preset.clone());
        }
    }
    for preset in saved {
        ui.horizontal(|ui| {
            if ui.button(&preset.name).clicked() {
                *chosen = Some(preset.clone());
            }
            if ui
                .small_button("Forget")
                .on_hover_text("Forget this preset; windows placed from it stay")
                .clicked()
            {
                *forget = Some(preset.name.clone());
            }
        });
    }
    if let Some(why) = &library.unsaved {
        ui.colored_label(
            crate::theme::color(ui.ctx(), crate::theme::T::Wrong),
            format!("Presets not saved: {why}"),
        );
    }
}

/// The character the list's Advanced place chose, if another.
fn adding_whose(adding: Option<&Adding>) -> Option<String> {
    adding.and_then(|adding| adding.whose.clone())
}

/// Every kind shown somewhere in `layout`.
fn kinds_shown(layout: &Layout) -> HashSet<Widget> {
    layout
        .placed()
        .into_iter()
        .map(|placed| placed.widget.clone())
        .collect()
}

/// The Advanced place, closed until opened: which character `whose` is, of
/// this one and `others`.
fn advanced(ui: &mut egui::Ui, whose: &mut Option<String>, others: &[Character]) {
    egui::CollapsingHeader::new("Advanced")
        .default_open(false)
        .show(ui, |ui| {
            ui.label("Show for");
            ui.radio_value(whose, None, "This character");
            for other in others {
                ui.radio_value(whose, Some(other.name.clone()), &other.name);
            }
            if others.is_empty() {
                ui.weak("No other character is running.");
            }
            if whose.is_some() {
                ui.weak("A story stays with its own character.");
            }
        });
}

/// A menu's items, for what it was opened on; what was asked, if anything.
fn items(
    ui: &mut egui::Ui,
    menu: &mut Menu,
    layout: &Layout,
    (others, received): (&[Character], &[Widget]),
) -> Option<Act> {
    let mut act = None;
    let holder = layout.holder(menu.holder)?;
    let widget = menu
        .placed
        .and_then(|placed| widget_in(layout, menu.holder, placed));
    if let (Some(placed), Some(widget)) = (menu.placed, widget) {
        act = widget_items(ui, menu, layout, (placed, widget), received);
    }
    if let Holds::Custom(custom) = &holder.holds {
        ui.separator();
        ui.weak(&custom.title);
        match &mut menu.renaming {
            Some(title) if menu.saving.is_none() => {
                // Enter is the menu's, taken before the field sees it: a
                // field that keeps the keyboard never loses it to Enter.
                let enter = ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                ui.text_edit_singleline(title).request_focus();
                if enter {
                    act = Some(Act::Rename(title.clone()));
                }
            }
            _ => {
                if ui.button("Rename...").clicked() {
                    menu.renaming = Some(custom.title.clone());
                    menu.saving = None;
                }
            }
        }
        match &mut menu.saving {
            Some(name) => {
                ui.label("Save as preset:");
                let enter = ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                ui.text_edit_singleline(name).request_focus();
                if enter && !name.trim().is_empty() {
                    act = Some(Act::Save(name.clone()));
                }
            }
            None => {
                if ui.button("Save as preset...").clicked() {
                    menu.saving = Some(custom.title.clone());
                    menu.renaming = None;
                }
            }
        }
        if ui.button("Remove window").clicked() {
            act = Some(Act::RemoveWindow);
        }
    }
    if let (Some(placed), Some(widget)) = (menu.placed, widget)
        && !widget.is_story()
    {
        ui.separator();
        egui::CollapsingHeader::new("Advanced")
            .default_open(false)
            .show(ui, |ui| {
                let now = layout.follows.get(&placed).cloned();
                let mut whose = now.clone();
                ui.label("Show for");
                ui.radio_value(&mut whose, None, "This character");
                for other in others {
                    ui.radio_value(&mut whose, Some(other.name.clone()), &other.name);
                }
                if whose != now {
                    act = Some(Act::Follow(placed, whose));
                }
            });
    }
    act
}

/// A widget's own items: its settings, a tab beside it, and removing it.
fn widget_items(
    ui: &mut egui::Ui,
    menu: &mut Menu,
    layout: &Layout,
    (placed, widget): (u32, &Widget),
    received: &[Widget],
) -> Option<Act> {
    let mut act = None;
    ui.weak(widget.name());
    if super::options::has_page(widget)
        && ui
            .button("Settings...")
            .on_hover_text("This widget's own settings: how it draws")
            .clicked()
    {
        act = Some(Act::Settings(super::options::page_id(placed)));
    }
    if let Some(picture) = layout
        .dolls
        .get(&placed)
        .and_then(|look| look.picture.clone())
        && ui
            .button("Calibrate doll...")
            .on_hover_text("Where each part is on the doll's picture")
            .clicked()
    {
        act = Some(Act::Calibrate(placed, picture));
    }
    let beside = tab_kinds(layout, menu.holder, (placed, widget), received);
    if !beside.is_empty() {
        if menu.tabbing {
            ui.label("Add a tab:");
            for kind in beside {
                if ui.button(kind.name()).clicked() {
                    act = Some(Act::AddTab(placed, kind));
                }
            }
        } else if ui
            .button("Add a tab...")
            .on_hover_text("Another widget beside this one, a tab each")
            .clicked()
        {
            menu.tabbing = true;
        }
    }
    if ui.button("Remove").clicked() {
        act = Some(Act::Remove(placed));
    }
    act
}

/// Every kind: the catalog's, the streams named among them, and any other
/// stream or game dialog this character has `received`: a stream that
/// comes with a position the game gives appears for the characters that
/// hold it (`plan/49` §3), and a dialog once the game has sent it.
fn kinds(received: &[Widget]) -> Vec<Widget> {
    let mut every = Widget::all();
    for kind in received {
        if !every.contains(kind) {
            every.push(kind.clone());
        }
    }
    every
}

/// The kinds a tab beside widget `placed` could be: of its own tab group
/// (`Widget::tab_group`), the
/// story only for the window's own character, and none already beside it.
pub(super) fn tab_kinds(
    layout: &Layout,
    holder: u32,
    (placed, widget): (u32, &Widget),
    received: &[Widget],
) -> Vec<Widget> {
    let beside: Vec<&Widget> = match layout.holder(holder).map(|found| &found.holds) {
        Some(Holds::Custom(custom)) => custom
            .cells
            .iter()
            .find(|cell| cell.tabs.iter().any(|tab| tab.id == placed))
            .map(|cell| cell.tabs.iter().map(|tab| &tab.widget).collect())
            .unwrap_or_default(),
        Some(Holds::One(one)) => vec![&one.widget],
        None => Vec::new(),
    };
    let following = layout.follows.contains_key(&placed);
    kinds(received)
        .into_iter()
        .filter(|kind| kind.tab_group() == widget.tab_group() && !beside.contains(&kind))
        .filter(|kind| !(following && kind.is_story()))
        .collect()
}

/// The kind of widget `placed` in window `holder`.
fn widget_in(layout: &Layout, holder: u32, placed: u32) -> Option<&Widget> {
    layout
        .holder(holder)?
        .holds
        .placed()
        .find(|one| one.id == placed)
        .map(|one| &one.widget)
}
