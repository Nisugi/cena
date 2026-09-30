//! What a play window draws: its top bar, and what each of its windows
//! holds -- one widget, framed by its window, or a custom window's widgets,
//! bare in their cells (`plan/49` §2).

use std::collections::{BTreeMap, HashMap};

use egui::{Id, UiBuilder};

use super::PlayView;
use crate::layout::{Holds, Placed, tabs_and_body};
use crate::theme::{self, T, readable_on};
use crate::widget::{Character, Seen};

/// What a play window's widgets are drawn with, this frame.
pub(super) struct Drawing<'a> {
    /// The window's own character.
    pub(super) seen: Seen<'a>,
    /// The other characters running, which a widget may follow.
    pub(super) others: &'a [Character],
    /// Which widget follows which of them, by id (`Layout::follows`).
    pub(super) follows: &'a BTreeMap<u32, String>,
    /// How each bar widget draws its bar, by id (`Layout::looks`).
    pub(super) looks: &'a BTreeMap<u32, crate::bar::Look>,
    /// Which parts each Room widget shows, by id (`Layout::rooms`).
    pub(super) rooms: &'a BTreeMap<u32, crate::widget::RoomParts>,
    /// How each story or stream widget draws its lines, by id
    /// (`Layout::lines`).
    pub(super) lines: &'a BTreeMap<u32, crate::widget::Lines>,
    /// Each Injuries widget's picture, by id (`Layout::dolls`).
    pub(super) dolls: &'a BTreeMap<u32, crate::widget::doll::DollLook>,
    /// Each minimap's zooms and maps next door, by id (`Layout::minimaps`).
    pub(super) minimaps: &'a BTreeMap<u32, crate::widget::minimap::MinimapLook>,
    /// How each room list lays out its names, by id (`Layout::lists`).
    pub(super) lists: &'a BTreeMap<u32, crate::widget::Listing>,
    /// Which session: every id in the window is its own.
    pub(super) session: u32,
    /// How far each widget that counts what it says was read, by id.
    pub(super) read: &'a mut HashMap<u32, u64>,
    /// A line a widget asked to send this frame, as if typed.
    pub(super) sent: Option<crate::widget::Clicked>,
    /// The window in use, by placed id, which is marked (`plan/52` step 4).
    pub(super) in_use: Option<u32>,
    /// Arrange is on: a window resized leaves its widgets their size (the
    /// author, 2026-09-30).
    pub(super) arranging: bool,
    /// The widget pressed in this frame, by placed id: the window in use
    /// from now.
    pub(super) pressed: Option<u32>,
    /// The tabs not showing that have lines unread, by placed id.
    pub(super) unread: Vec<u32>,
    /// Where each widget showing was drawn, by placed id.
    pub(super) rects: Vec<(u32, egui::Rect)>,
}

impl Drawing<'_> {
    /// What the player chose for the widget placed as `placed` on its own
    /// page.
    fn chosen(&self, placed: &Placed) -> crate::widget::Chosen {
        let (placed, widget) = (placed.id, &placed.widget);
        crate::widget::Chosen {
            look: self
                .looks
                .get(&placed)
                .cloned()
                .zip(widget.bar_token())
                .map(|(look, token)| look.themed(token)),
            room: self.rooms.get(&placed).copied(),
            lines: self.lines.get(&placed).copied(),
            doll: self.dolls.get(&placed).cloned(),
            minimap: self.minimaps.get(&placed).copied(),
            list: self.lists.get(&placed).copied(),
        }
    }
}

/// What the top bar was asked this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Top {
    /// Stop was pressed.
    Stop,
    /// The layout was to be fitted afresh.
    Fit,
    /// The grid's pitch was changed.
    Grid,
    /// A drawer was opened, shut, or changed (`plan/49` Stage E).
    Drawers,
    /// The keybinds were to be read again.
    ReloadKeys,
    /// A new custom window was asked for.
    NewCustom,
    /// The Add-a-widget list was asked for.
    AddWidget,
    /// The settings menu was asked for.
    Settings,
    /// The character's log window was asked for.
    Log,
    /// The trigger editor was asked for.
    Triggers,
    /// The theme editor was asked for.
    Theme,
    /// The settings menu's *Keys* page was asked for.
    Keys,
    /// The player's own Lich was switched on, or off.
    Lich(bool),
}

impl Top {
    /// What it asks of the app, for the buttons that change nothing in the
    /// window itself: stop, and the windows they open.
    pub(super) fn asked(self) -> Option<super::Asked> {
        use super::Asked;
        Some(match self {
            Top::Stop => Asked::Stop,
            Top::Settings => Asked::Settings(None),
            Top::Keys => Asked::Keys,
            Top::Log => Asked::Log,
            Top::Triggers => Asked::Triggers,
            Top::Theme => Asked::Theme,
            Top::ReloadKeys => Asked::ReloadKeys,
            Top::Lich(on) => Asked::Lich(on),
            _ => return None,
        })
    }
}

/// The buttons that open another window: *Settings*, *Triggers* and *Log*,
/// right to left as the top bar draws them.
fn windows(ui: &mut egui::Ui) -> Option<Top> {
    let mut asked = None;
    for (label, hover, top) in [
        (
            "Settings",
            "This character's settings, and Hydra's",
            Top::Settings,
        ),
        (
            "Triggers",
            "Every trigger: what it watches, what it does, for whom",
            Top::Triggers,
        ),
        (
            "Log",
            "What this character saw, read back: by day, searched, or exported",
            Top::Log,
        ),
        (
            "Theme",
            "Make a theme: its colours, shape and type",
            Top::Theme,
        ),
    ] {
        if ui.button(label).on_hover_text(hover).clicked() {
            asked = Some(top);
        }
    }
    asked
}

/// The top bar: who, how connected, the keybinds, the layout's grid, Stop;
/// and any banner. What the hands hold and the clocks are widgets now, in
/// the layout with the rest.
pub(super) fn top(
    ui: &mut egui::Ui,
    view: &PlayView<'_>,
    (grid, drawers): (&mut f32, &mut crate::layout::Drawers),
    (arranging, locked): (&mut bool, &mut bool),
    unsaved: Option<&str>,
) -> Option<Top> {
    let mut asked = None;
    ui.horizontal(|ui| {
        ui.strong(view.name);
        ui.label(crate::hub::lifecycle(view.lifecycle));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Stop")
                .on_hover_text("Stop everything Hydra is doing on this character")
                .clicked()
            {
                asked = Some(Top::Stop);
            }
            asked = windows(ui).or(asked);
            if let Some(on) = super::lich_switch(ui, view.lich) {
                asked = Some(Top::Lich(on));
            }
            ui.menu_button("Keys", |ui| {
                for said in view.keys {
                    ui.label(said);
                }
                if ui.button("Read the keybinds again").clicked() {
                    asked = Some(Top::ReloadKeys);
                    ui.close();
                }
                if ui.button("Change the keys...").clicked() {
                    asked = Some(Top::Keys);
                    ui.close();
                }
            });
            ui.menu_button("Layout", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Grid");
                    let changed = ui
                        .add(egui::DragValue::new(grid).range(0.0..=64.0).suffix(" pt"))
                        .on_hover_text("What the windows' edges snap to; 0 for none. Shift while dragging snaps to nothing.")
                        .changed();
                    if changed {
                        asked = Some(Top::Grid);
                    }
                });
                if ui.button("Add a widget...").clicked() {
                    asked = Some(Top::AddWidget);
                    ui.close();
                }
                if ui.button("New custom window").clicked() {
                    asked = Some(Top::NewCustom);
                    ui.close();
                }
                if ui.button("Lay out afresh").clicked() {
                    asked = Some(Top::Fit);
                    ui.close();
                }
            });
            ui.menu_button("Drawers", |ui| {
                if super::drawers::menu(ui, drawers) {
                    asked = Some(Top::Drawers);
                }
            });
            // On the bar, lit while it is on, so the mode is seen and left in
            // one click: a new custom window turns it on (the author,
            // 2026-09-28, stuck in it with the switch in the Layout menu).
            ui.add_enabled_ui(!*locked, |ui| {
                ui.toggle_value(arranging, "Arrange")
                    .on_hover_text("Move, resize and drag widgets in and out of custom windows");
            });
            ui.toggle_value(locked, "Lock")
                .on_hover_text("Keep every window where it is: none is dragged or resized");
            if let Some(why) = unsaved {
                ui.colored_label(theme::color(ui.ctx(), T::Wrong), why);
            }
            if let Some(on) = view.numlock {
                ui.weak(if on { "NumLock on" } else { "NumLock off" });
            }
            if view.set != 0 {
                ui.weak(format!("Set {}", view.set))
                    .on_hover_text("The macro set in use over set 0; Alt+0 goes back to set 0 alone");
            }
        });
    });
    let ground = theme::color(ui.ctx(), T::Warning);
    for alert in view.story.alerts_at(view.now) {
        egui::Frame::new()
            .fill(ground)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.colored_label(readable_on(ground), alert);
            });
    }
    asked
}

/// A widget's egui id: its own in every play window, whatever holds it.
pub(super) fn widget_id(session: u32, placed: u32) -> Id {
    Id::new(("play-widget", session, placed))
}

/// What a window holds, filling it: its window is the size its layout says,
/// never its content's. One widget is given everything; a custom window's
/// widgets are drawn bare in their cells, once the cells are kept to the
/// inside it has now (`Custom::fit`). Nothing spills out of its cell: a
/// one-line widget stays one line, and the rest scroll. A tab stack shows a
/// tab for each of its widgets, a click on one showing it, each not showing
/// with what it has said since it last showed. Its inside, where it drew.
pub(super) fn holder(
    ui: &mut egui::Ui,
    holds: &mut Holds,
    drawing: &mut Drawing<'_>,
) -> egui::Rect {
    let inside = ui.available_rect_before_wrap();
    ui.set_min_size(inside.size());
    match holds {
        Holds::One(placed) => shown(ui, placed, drawing),
        Holds::Custom(custom) => {
            // Not to the pass egui lays a window out in at its narrowest as
            // a resize begins, then discards: kept to it, a narrow cell came
            // back from the scaling at its smallest, not its width.
            // Resized with Arrange on, a window leaves its widgets their
            // size, room made or taken for arranging them; with it off they
            // scale with the window (the author, 2026-09-30).
            if ui.is_sizing_pass() {
            } else if drawing.arranging {
                custom.keep(inside.size());
            } else {
                custom.fit(inside.size());
            }
            if custom.cells.is_empty() {
                ui.new_child(UiBuilder::new().max_rect(inside.shrink(4.0)))
                    .weak("Empty. With Arrange on, in the Layout menu, drop widgets here.");
            }
            for cell in &mut custom.cells {
                let at = cell
                    .rect()
                    .translate(inside.min.to_vec2())
                    .intersect(inside);
                if !at.is_positive() {
                    continue;
                }
                let (tabs, body) = tabs_and_body(at, cell.tabs.len());
                for (index, (tab, rect)) in cell.tabs.iter().zip(tabs).enumerate() {
                    let showing = index == cell.showing;
                    let name = match unread(drawing.read, tab, &drawing.seen).filter(|_| !showing) {
                        Some(unread) => {
                            drawing.unread.push(tab.id);
                            format!("{} {unread}", tab.widget.name())
                        }
                        None => tab.widget.name().into_owned(),
                    };
                    let mut child =
                        ui.new_child(UiBuilder::new().max_rect(rect).id_salt(("tab", tab.id)));
                    let button = egui::Button::selectable(showing, name)
                        .wrap_mode(egui::TextWrapMode::Truncate);
                    if child.add_sized(rect.size(), button).clicked() {
                        cell.showing = index;
                    }
                }
                let Some(placed) = cell.shown() else {
                    continue;
                };
                let mut child =
                    ui.new_child(UiBuilder::new().max_rect(body).id_salt(("cell", placed.id)));
                shown(&mut child, placed, drawing);
            }
            ui.advance_cursor_after_rect(inside);
        }
    }
    inside
}

/// `placed` drawn into `ui`, showing: so what it has said is read, up to
/// now. The one way a widget is drawn, standalone, alone in a cell or a tab.
/// One that follows another character draws from that character, named on
/// it, or says that character is not running; a story never follows.
fn shown(ui: &mut egui::Ui, placed: &Placed, drawing: &mut Drawing<'_>) {
    let rect = ui.max_rect();
    drawing.rects.push((placed.id, rect));
    if ui.input(|input| input.pointer.primary_pressed()) && ui.rect_contains_pointer(rect) {
        drawing.pressed = Some(placed.id);
    }
    if drawing.in_use == Some(placed.id) {
        // The window in use, which a scrolling key acts on.
        let stroke = egui::Stroke::new(1.0, theme::color(ui.ctx(), T::Accent).gamma_multiply(0.5));
        ui.painter().rect_stroke(
            rect,
            theme::corner(ui.ctx()),
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    drawn(ui, placed, drawing);
}

/// `placed` drawn, as [`shown`] says.
fn drawn(ui: &mut egui::Ui, placed: &Placed, drawing: &mut Drawing<'_>) {
    let id = widget_id(drawing.session, placed.id);
    let follows = drawing
        .follows
        .get(&placed.id)
        .filter(|_| !placed.widget.is_story());
    let Some(who) = follows else {
        if let Some(count) = placed.widget.count(&drawing.seen) {
            drawing.read.insert(placed.id, count);
        }
        let chosen = drawing.chosen(placed);
        if let Some(line) = placed.widget.draw_with(ui, &drawing.seen, id, &chosen) {
            drawing.sent = Some(line);
        }
        return;
    };
    match drawing
        .others
        .iter()
        .find(|other| other.name.eq_ignore_ascii_case(who))
    {
        Some(other) => {
            let seen = Seen {
                snapshot: other.snapshot.as_deref(),
                hunt: other.hunt.as_ref(),
                who: Some(&other.name),
                // Another character's place is not followed yet.
                minimap: None,
                ..drawing.seen
            };
            // Another character's widget sends nothing on this one's.
            let chosen = drawing.chosen(placed);
            let _ = placed.widget.draw_with(ui, &seen, id, &chosen);
        }
        None => {
            ui.weak(format!("{who} is not running."));
        }
    }
}

/// A window's title: its widget's name, and whose when it follows another
/// character; or the custom window's own.
pub(super) fn title(holds: &Holds, follows: &BTreeMap<u32, String>) -> String {
    match holds {
        Holds::One(placed) => match follows.get(&placed.id) {
            Some(who) if !placed.widget.is_story() => format!("{} ({who})", placed.widget.name()),
            _ => placed.widget.name().into_owned(),
        },
        Holds::Custom(custom) => custom.title.clone(),
    }
}

/// What `placed` has said since it last showed, when it counts and that is
/// something. A widget first seen here is read up to now, so a tab counts
/// from when it was stacked.
pub(super) fn unread(
    read: &mut HashMap<u32, u64>,
    placed: &Placed,
    seen: &Seen<'_>,
) -> Option<u64> {
    let count = placed.widget.count(seen)?;
    let last = *read.entry(placed.id).or_insert(count);
    Some(count.saturating_sub(last)).filter(|unread| *unread > 0)
}
