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
mod moves;
mod preset;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::{Rect, Vec2, pos2};
use serde::{Deserialize, Serialize};

use crate::widget::Widget;
pub(crate) use custom::{Cell, Custom, SMALLEST as SMALLEST_CELL, stacks_at, tabs_and_body};
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
}

/// One window in a play window: a standalone window or a custom window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Holder {
    /// Its own id.
    pub(crate) id: u32,
    /// Its rect from the play area's top left: x, y, width, height.
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

    /// Where it sits, from the play area's top left.
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
        let mut streams = Vec::new();
        for holder in &self.holders {
            let placed: Vec<&Placed> = match &holder.holds {
                Holds::One(placed) => vec![placed],
                Holds::Custom(custom) => custom
                    .cells
                    .iter()
                    .flat_map(|cell| cell.tabs.iter())
                    .collect(),
            };
            for placed in placed {
                if let Widget::Stream(id) = &placed.widget {
                    streams.push(id.clone());
                }
            }
        }
        streams
    }

    /// Where window `id` sits, if it is here.
    pub(crate) fn rect(&self, id: u32) -> Option<Rect> {
        self.holder(id).map(Holder::rect)
    }

    /// Put window `id` at `at`.
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

    /// `character`'s saved layout on `instance` in `dir`: by game and name,
    /// so a character on Prime and one of the same name on Shattered each
    /// keep their own (`plan/50` §6 item 6). One saved under the name alone,
    /// as layouts were kept before, is taken as the first. `None` when there
    /// is none, or it cannot be read, when a fitted one does.
    pub(crate) fn load(dir: &Path, instance: Option<&str>, character: &str) -> Option<Self> {
        let read = |path: PathBuf| {
            let text = std::fs::read_to_string(path).ok()?;
            serde_json::from_str::<Self>(&text)
                .ok()
                .filter(|layout| layout.version == VERSION)
        };
        read(file(dir, instance, character))
            .or_else(|| instance.and_then(|_| read(file(dir, None, character))))
    }

    /// Save this as `character`'s layout on `instance` in `dir`.
    ///
    /// # Errors
    ///
    /// The folder could not be made or the file written.
    pub(crate) fn save(
        &self,
        dir: &Path,
        instance: Option<&str>,
        character: &str,
    ) -> std::io::Result<()> {
        cena_session::store::save_json(dir, &file(dir, instance, character), self)
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

/// `character`'s layout file on `instance`, `prime_nisugi.json`, or under
/// its name alone with no instance: each in lower case, letters and digits
/// only, so the same character is one file on every filesystem (the
/// character store's lesson: one file on NTFS was two on ext4).
fn file(dir: &Path, instance: Option<&str>, character: &str) -> PathBuf {
    let clean = |words: &str| -> String {
        words
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect()
    };
    match instance {
        Some(instance) => dir.join(format!("{}_{}.json", clean(instance), clean(character))),
        None => dir.join(format!("{}.json", clean(character))),
    }
}

#[cfg(test)]
mod tests;
