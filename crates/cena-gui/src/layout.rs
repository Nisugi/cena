//! A play window's layout (`plan/49` Stage A): the windows it holds -- a
//! standalone window around one widget, or a custom window of several held
//! bare -- where each sits, and the grid their edges snap to. It is kept by
//! the character's name, since a `SessionId` starts again at 0 every run
//! (`plan/23` §D1a) and a name does not.
//!
//! The author's model (`plan/28` §7d): *"free rects, with the grid as a snap
//! target"*, the pitch adjustable, since a fixed one was the complaint about
//! Saga (§7d.1). A window's rect is kept from the play area's top left, so a
//! play window moved across the screen keeps its layout; a custom window's
//! cells are kept from its inside's top left (`custom.rs`).
//!
//! Every window and every placed widget has an id of its own, given when it
//! is placed and never derived from what it shows (`plan/28` §7c), so two of
//! one kind can live side by side.

mod custom;
mod drawers;
mod kept;
mod moves;
mod preset;

use std::collections::BTreeMap;

use egui::{Rect, Vec2, pos2};
use serde::{Deserialize, Serialize};

use crate::widget::Widget;
pub(crate) use custom::{Cell, Custom, SMALLEST as SMALLEST_CELL, stacks_at, tabs_and_body};
pub(crate) use drawers::{CLEAR, Drawers, Mode, THINNEST, Zone, Zones};
#[cfg(test)]
use kept::file;
pub(crate) use kept::file as character_file;
pub(crate) use moves::Taking;
pub(crate) use preset::{Library, Preset};

/// The smallest a window can be made.
pub(crate) const SMALLEST: Vec2 = Vec2::new(120.0, 60.0);

/// The grid a new layout starts with, in points.
pub(crate) const GRID: f32 = 10.0;

/// What a window's frame and title bar take from its rect, as measured in a
/// rendered play window: near enough for a first layout, since a custom
/// window's cells are kept to its real inside when it is first drawn
/// (`Custom::fit`).
const CHROME: Vec2 = Vec2::new(12.0, 44.0);

/// The version of the layout file this build writes and reads. Version 1
/// was M10's five panes; none outlived the M10 branch, whose data folder
/// went with it (`plan/49` Stage A), so one is not read, and a fitted layout
/// takes its place.
const VERSION: u32 = 2;

/// What a play window holds, and where.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Layout {
    /// This file's shape; a file of another is not read.
    version: u32,
    /// The grid's pitch in points; 0 for none.
    pub(crate) grid: f32,
    /// The id the next window or widget placed is given.
    next: u32,
    /// Its windows, in the order they are drawn.
    pub(crate) holders: Vec<Holder>,
    /// The character each widget follows, by the widget's id, when it is
    /// not its window's: chosen only in the Advanced place (`plan/49` §1 row
    /// 3). A widget not here follows its window's character.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) follows: BTreeMap<u32, String>,
    /// How each bar widget draws its bar, by the widget's id, when the
    /// player picked it from its right-click menu (`plan/49` §2, a widget's
    /// options). A bar not here draws as its kind says.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) looks: BTreeMap<u32, crate::bar::Look>,
    /// Its windows stay where they are: none is dragged or resized (the
    /// author, 2026-09-28: *"a window lock you can toggle to prevent
    /// dragging them windows"*).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) locked: bool,
    /// Which parts each Room widget shows, by the widget's id, when the
    /// player chose on its page; one not here shows them all, joined.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) rooms: BTreeMap<u32, crate::widget::RoomParts>,
    /// How each story or stream widget draws its lines, by the widget's id,
    /// when the player chose on its page.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) lines: BTreeMap<u32, crate::widget::Lines>,
    /// Each Injuries widget's picture, by the widget's id (`plan/55`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) dolls: BTreeMap<u32, crate::widget::doll::DollLook>,
    /// Its four drawers (`plan/49` Stage E, `drawers.rs`).
    #[serde(default, skip_serializing_if = "Drawers::is_default")]
    pub(crate) drawers: Drawers,
}

/// One window in a play window: a standalone window or a custom window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Holder {
    /// Its own id.
    pub(crate) id: u32,
    /// The main area or the drawer it lives in.
    #[serde(default, skip_serializing_if = "Zone::is_main")]
    pub(crate) zone: Zone,
    /// Its rect from its zone's top left: x, y, width, height.
    rect: [f32; 4],
    /// What it holds.
    pub(crate) holds: Holds,
}

/// What a window holds: chrome follows from it (`plan/28` §7d.3), never from
/// a setting of the widget's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Holds {
    /// One widget, framed and titled: a standalone window.
    One(Placed),
    /// Widgets held bare, with one frame for all: a custom window.
    Custom(Custom),
}

/// A widget placed somewhere, with the id it was given there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Placed {
    /// Its own id.
    pub(crate) id: u32,
    /// What it shows.
    pub(crate) widget: Widget,
}

impl Holder {
    /// Its title bar's words: its widget's name, or the custom window's own:
    /// for a test, which finds a window as a player does. The play window
    /// adds whose it is to a widget following another character
    /// (`play/draw.rs`, `title`).
    #[cfg(test)]
    pub(crate) fn title(&self) -> std::borrow::Cow<'_, str> {
        match &self.holds {
            Holds::One(placed) => placed.widget.name(),
            Holds::Custom(custom) => std::borrow::Cow::Borrowed(&custom.title),
        }
    }

    /// Where it sits, from its zone's top left: the play area's, for a
    /// window in the main area while no drawer pushes it.
    pub(crate) fn rect(&self) -> Rect {
        rect(self.rect)
    }

    /// Put it at `at`.
    pub(crate) fn set(&mut self, at: Rect) {
        self.rect = kept(at);
    }
}

impl Layout {
    /// The first layout, for a play area `area` across: the story on the
    /// left, and down the right the vitals, what the hands hold and the
    /// clocks, the hunt, the room and Hydra's messages, their edges on the
    /// grid. The vitals, the hands and the room are custom windows of single
    /// widgets, as the author asked (`plan/49` §1 row 1). The first three
    /// are as tall as what they hold; in a short area they give way, so the
    /// room and Hydra's messages keep the smallest a window can be.
    pub(crate) fn fitted(area: Vec2) -> Self {
        Self::fitted_as(area, false)
    }

    /// The same, but for its Room window: a custom window of the room's
    /// parts, each its own widget, as the default was before the Room
    /// widget; the tests of arranging cells arrange those.
    #[cfg(test)]
    pub(crate) fn with_room_parts(area: Vec2) -> Self {
        Self::fitted_as(area, true)
    }

    /// [`Self::fitted`], its Room window the one Room widget, or with
    /// `parts` the room's parts in a custom window.
    fn fitted_as(area: Vec2, parts: bool) -> Self {
        let on_grid = |value: f32| (value / GRID).round() * GRID;
        let split = on_grid(area.x * 0.66).max(SMALLEST.x);
        let side = (area.x - split).max(SMALLEST.x);
        let fixed = [130.0, 110.0, 110.0];
        let left = area.y - 2.0 * SMALLEST.y;
        let scale = (left / fixed.iter().sum::<f32>()).clamp(0.0, 1.0);
        let [vitals, loadout, hunt] = fixed.map(|height| (height * scale).floor());
        let rest = area.y - vitals - loadout - hunt;
        let room = on_grid(rest * 0.6).clamp(SMALLEST.y, (rest - SMALLEST.y).max(SMALLEST.y));
        let hydra = (rest - room).max(SMALLEST.y);
        let mut layout = Self {
            version: VERSION,
            grid: GRID,
            next: 1,
            holders: Vec::new(),
            follows: BTreeMap::new(),
            looks: BTreeMap::new(),
            locked: false,
            rooms: BTreeMap::new(),
            lines: BTreeMap::new(),
            dolls: BTreeMap::new(),
            drawers: Drawers::default(),
        };
        let story = layout.place(Widget::Story);
        layout.add(
            Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(split, area.y.max(SMALLEST.y))),
            Holds::One(story),
        );
        let mut y = 0.0;
        let mut next = |height: f32| {
            let at = Rect::from_min_size(pos2(split, y), Vec2::new(side, height));
            y += height;
            at
        };
        let vitals_at = next(vitals);
        layout.custom(
            vitals_at,
            "Vitals",
            &[
                &[Widget::Health],
                &[Widget::Mana],
                &[Widget::Stamina],
                &[Widget::Spirit],
            ],
        );
        let loadout_at = next(loadout);
        layout.custom(
            loadout_at,
            "Loadout",
            &[
                &[Widget::RightHand],
                &[Widget::LeftHand],
                &[Widget::Roundtime, Widget::CastTime],
            ],
        );
        let hunt_at = next(hunt);
        let hunt = layout.place(Widget::Hunt);
        layout.add(hunt_at, Holds::One(hunt));
        let room_at = next(room);
        if parts {
            layout.custom(
                room_at,
                "Room",
                &[
                    &[Widget::RoomTitle],
                    &[Widget::Creatures],
                    &[Widget::Objects],
                    &[Widget::Players],
                    &[Widget::Exits],
                ],
            );
        } else {
            let room = layout.place(Widget::Room);
            layout.add(room_at, Holds::One(room));
        }
        let hydra_at = next(hydra);
        let hydra = layout.place(Widget::Hydra);
        layout.add(hydra_at, Holds::One(hydra));
        layout
    }

    /// `widget`, given an id of its own.
    fn place(&mut self, widget: Widget) -> Placed {
        let id = self.next;
        self.next += 1;
        Placed { id, widget }
    }

    /// A window holding `holds` at `at`, on top of the rest; its id.
    fn add(&mut self, at: Rect, holds: Holds) -> u32 {
        let id = self.next;
        self.next += 1;
        self.holders.push(Holder {
            id,
            zone: Zone::Main,
            rect: kept(at),
            holds,
        });
        id
    }

    /// A custom window titled `title` at `at`, its widgets in `rows`.
    fn custom(&mut self, at: Rect, title: &str, rows: &[&[Widget]]) -> u32 {
        let rows: Vec<Vec<Placed>> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|widget| self.place(widget.clone()))
                    .collect()
            })
            .collect();
        let inside = (at.size() - CHROME).max(Vec2::ZERO);
        self.add(at, Holds::Custom(Custom::rows(title, rows, inside)))
    }

    /// The window `id`, if it is here.
    pub(crate) fn holder(&self, id: u32) -> Option<&Holder> {
        self.holders.iter().find(|holder| holder.id == id)
    }

    /// The streams a widget here shows, showing or a tab behind another:
    /// the story leaves their lines out (`plan/49` Stage B step 4).
    pub(crate) fn streams(&self) -> Vec<String> {
        self.placed()
            .into_iter()
            .filter_map(|placed| match &placed.widget {
                Widget::Stream(id) => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    /// Where window `id` sits, if it is here, from its zone's top left.
    #[cfg(test)]
    pub(crate) fn rect(&self, id: u32) -> Option<Rect> {
        self.holder(id).map(Holder::rect)
    }

    /// Put window `id` at `at`, from its zone's top left.
    #[cfg(test)]
    pub(crate) fn set(&mut self, id: u32, at: Rect) {
        if let Some(holder) = self.holders.iter_mut().find(|holder| holder.id == id) {
            holder.set(at);
        }
    }

    /// The window titled `title`: for a test, which finds a window as a
    /// player does.
    #[cfg(test)]
    pub(crate) fn titled(&self, title: &str) -> Option<&Holder> {
        self.holders.iter().find(|holder| holder.title() == title)
    }
}

/// A kept rect as a rect.
fn rect([x, y, width, height]: [f32; 4]) -> Rect {
    Rect::from_min_size(pos2(x, y), Vec2::new(width, height))
}

/// A rect as it is kept: x, y, width, height.
fn kept(at: Rect) -> [f32; 4] {
    [at.min.x, at.min.y, at.width(), at.height()]
}

#[cfg(test)]
mod tests;
