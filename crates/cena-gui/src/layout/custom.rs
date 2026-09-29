//! A custom window's inside (`plan/49` §2): its widgets held bare, each in a
//! cell -- a free rect, kept from the inside's top left -- the play window's
//! own model one level down (`plan/28` §7d).
//!
//! When the window is resized its cells follow: across, each scales with the
//! inside's width, so a row of bars stays as wide as the window and two
//! clocks side by side stay halves; down, a cell on the bottom edge keeps to
//! it, so the last list takes the new room and a line stays a line. That is
//! the one anchor a custom window needs, where Vellum solved a graph of them
//! per frame for its drawers (`plan/28` §7d, the cost table).

use egui::{Pos2, Rect, Vec2, pos2};
use serde::{Deserialize, Serialize};

use super::{Holds, Layout, Placed, kept, rect};
use crate::widget::LINE;

/// How near the bottom edge a cell's bottom must be to keep to it.
const TOUCHING: f32 = 0.5;

/// The smallest a cell becomes, when its window shrinks or it is resized.
pub(crate) const SMALLEST: Vec2 = Vec2::new(20.0, LINE);

/// A custom window: its title, and its widgets in cells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Custom {
    /// Its title bar's words.
    pub(crate) title: String,
    /// The inside's size the cells were laid in: width, height.
    inside: [f32; 2],
    /// Its cells, in the order they are drawn.
    pub(crate) cells: Vec<Cell>,
}

/// One place in a custom window: a widget, or a tab stack of several with
/// one showing (`plan/49` §2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Cell {
    /// Its rect from the inside's top left: x, y, width, height.
    rect: [f32; 4],
    /// Its widgets: one, or a tab stack's.
    pub(crate) tabs: Vec<Placed>,
    /// Which of them shows.
    pub(crate) showing: usize,
}

impl Custom {
    /// An empty custom window titled `title`, whose inside is `inside`
    /// across.
    pub(crate) fn empty(title: &str, inside: Vec2) -> Self {
        Self {
            title: title.to_owned(),
            inside: [inside.x, inside.y],
            cells: Vec::new(),
        }
    }

    /// Take widget `placed` out: its cell goes, or, when the cell is a tab
    /// stack of others too, its tab.
    pub(crate) fn take(&mut self, placed: u32) -> Option<Placed> {
        let at = self
            .cells
            .iter()
            .position(|cell| cell.tabs.iter().any(|tab| tab.id == placed))?;
        let cell = &mut self.cells[at];
        let tab = cell.tabs.iter().position(|tab| tab.id == placed)?;
        let taken = cell.tabs.remove(tab);
        if cell.tabs.is_empty() {
            self.cells.remove(at);
        } else {
            cell.showing = cell.showing.min(cell.tabs.len() - 1);
        }
        Some(taken)
    }

    /// A custom window titled `title` of one cell filling its inside, `inside`
    /// across: a tab stack of `tabs`, `showing` the one shown.
    pub(crate) fn stack(title: &str, tabs: Vec<Placed>, showing: usize, inside: Vec2) -> Self {
        Self {
            title: title.to_owned(),
            inside: [inside.x, inside.y],
            cells: vec![Cell {
                rect: kept(Rect::from_min_size(Pos2::ZERO, inside)),
                tabs,
                showing,
            }],
        }
    }

    /// Take out the whole cell showing or stacking widget `placed`.
    pub(crate) fn take_cell(&mut self, placed: u32) -> Option<Cell> {
        let at = self
            .cells
            .iter()
            .position(|cell| cell.tabs.iter().any(|tab| tab.id == placed))?;
        Some(self.cells.remove(at))
    }

    /// The cell whose stacking middle is at `at`, from the inside's top left
    /// (`stacks_at`): the one drawn last, on top, where cells overlap.
    pub(crate) fn stack_at(&self, at: Pos2) -> Option<usize> {
        self.cells
            .iter()
            .rposition(|cell| stacks_at(cell.rect(), at))
    }

    /// `tabs`, `showing` the one shown, let go at `at` inside: onto the
    /// middle of a cell, whose tab stack they join, or else in a cell of
    /// their own with its corner there, as big as the one shown asks and the
    /// inside allows, and kept inside.
    pub(crate) fn land(&mut self, tabs: Vec<Placed>, showing: usize, at: Pos2) {
        if let Some(onto) = self.stack_at(at) {
            self.cells[onto].tabs.extend(tabs);
            return;
        }
        let Some(shown) = tabs.get(showing).or_else(|| tabs.first()) else {
            return;
        };
        let inside = Vec2::new(self.inside[0], self.inside[1]);
        let strip = if tabs.len() > 1 { LINE } else { 0.0 };
        let size = (shown.widget.size() + Vec2::new(0.0, strip))
            .min(inside)
            .max(SMALLEST);
        let furthest = (inside - size).max(Vec2::ZERO).to_pos2();
        self.cells.push(Cell {
            rect: kept(Rect::from_min_size(at.clamp(Pos2::ZERO, furthest), size)),
            tabs,
            showing,
        });
    }

    /// `from` -- a tab, or a whole cell -- taken out and joined to the tab
    /// stack of the cell holding widget `into`. Nothing moves when that is
    /// where it already is.
    pub(crate) fn stack_onto(&mut self, from: super::Taking, into: u32) {
        let holding = |cell: &Cell, placed: u32| cell.tabs.iter().any(|tab| tab.id == placed);
        let already = match from {
            super::Taking::Tab(placed) | super::Taking::Cell(placed) => self
                .cells
                .iter()
                .any(|cell| holding(cell, placed) && holding(cell, into)),
        };
        if already {
            return;
        }
        let tabs = match from {
            super::Taking::Tab(placed) => self.take(placed).map(|tab| vec![tab]),
            super::Taking::Cell(placed) => self.take_cell(placed).map(|cell| cell.tabs),
        };
        if let (Some(tabs), Some(cell)) =
            (tabs, self.cells.iter_mut().find(|cell| holding(cell, into)))
        {
            cell.tabs.extend(tabs);
        }
    }

    /// A custom window titled `title` whose inside is `inside` across, its
    /// widgets in `rows`: a row's widgets side by side, sharing its width, as
    /// tall as its tallest asks, and the last row down to the bottom. When
    /// the rows ask more than the inside has, those taller than a line share
    /// what the one-line rows leave.
    pub(crate) fn rows(title: &str, rows: Vec<Vec<Placed>>, inside: Vec2) -> Self {
        let asked: Vec<f32> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|placed| placed.widget.size().y)
                    .fold(LINE, f32::max)
            })
            .collect();
        let total: f32 = asked.iter().sum();
        let heights: Vec<f32> = if total <= inside.y {
            asked
        } else {
            let lines: f32 = asked.iter().filter(|height| **height <= LINE).sum();
            let tall = asked.iter().filter(|height| **height > LINE).count();
            let share = (inside.y - lines) / f32::from(u16::try_from(tall.max(1)).unwrap_or(1));
            asked
                .iter()
                .map(|height| {
                    if *height <= LINE {
                        *height
                    } else {
                        share.max(LINE)
                    }
                })
                .collect()
        };
        let count = rows.len();
        let mut cells = Vec::new();
        let mut y = 0.0;
        for (at, (row, height)) in rows.into_iter().zip(heights).enumerate() {
            let height = if at + 1 == count {
                (inside.y - y).max(height)
            } else {
                height
            };
            let width = inside.x / f32::from(u16::try_from(row.len().max(1)).unwrap_or(1));
            let mut x = 0.0;
            for placed in row {
                cells.push(Cell {
                    rect: kept(Rect::from_min_size(pos2(x, y), Vec2::new(width, height))),
                    tabs: vec![placed],
                    showing: 0,
                });
                x += width;
            }
            y += height;
        }
        Self {
            title: title.to_owned(),
            inside: [inside.x, inside.y],
            cells,
        }
    }

    /// The inside's size its cells were last kept to.
    pub(crate) fn inside(&self) -> Vec2 {
        Vec2::new(self.inside[0], self.inside[1])
    }

    /// Keep the cells to an inside now `inside` across, as the module says:
    /// scaled across, the bottom row kept to the bottom. Whether anything
    /// moved.
    pub(crate) fn fit(&mut self, inside: Vec2) -> bool {
        let [width, height] = self.inside;
        if (inside.x - width).abs() < TOUCHING && (inside.y - height).abs() < TOUCHING {
            return false;
        }
        let across = if width > 0.0 { inside.x / width } else { 1.0 };
        for cell in &mut self.cells {
            let mut at = cell.rect();
            at.min.x *= across;
            at.max.x = (at.max.x * across).max(at.min.x + SMALLEST.x);
            if at.max.y >= height - TOUCHING {
                at.max.y = inside.y.max(at.min.y + SMALLEST.y);
            }
            cell.rect = kept(at);
        }
        self.inside = [inside.x, inside.y];
        true
    }
}

impl Cell {
    /// Where it sits, from the inside's top left.
    pub(crate) fn rect(&self) -> Rect {
        rect(self.rect)
    }

    /// Put it at `at`, from the inside's top left.
    pub(crate) fn set(&mut self, at: Rect) {
        self.rect = kept(at);
    }

    /// The widget showing: the only one, or the tab chosen.
    pub(crate) fn shown(&self) -> Option<&Placed> {
        self.tabs.get(self.showing).or_else(|| self.tabs.first())
    }
}

/// Whether something let go at `at` stacks onto a cell at `rect`: over the
/// middle half of its width. Anywhere else over it, a cell is moved or
/// placed there instead, so a full custom window can still be rearranged.
pub(crate) fn stacks_at(rect: Rect, at: Pos2) -> bool {
    rect.shrink2(Vec2::new(rect.width() / 4.0, 0.0))
        .contains(at)
}

/// A cell drawn at `at` holding `tabs` widgets: a rect for each tab, side by
/// side in a strip one line tall across its top, when it is a tab stack; and
/// the rest, which the widget shown fills.
pub(crate) fn tabs_and_body(at: Rect, tabs: usize) -> (Vec<Rect>, Rect) {
    if tabs < 2 {
        return (Vec::new(), at);
    }
    let (strip, body) = at.split_top_bottom_at_y(at.min.y + LINE);
    let width = strip.width() / f32::from(u16::try_from(tabs).unwrap_or(u16::MAX));
    let mut x = strip.min.x;
    let rects = (0..tabs)
        .map(|_| {
            let tab = Rect::from_min_size(pos2(x, strip.min.y), Vec2::new(width, LINE));
            x += width;
            tab
        })
        .collect();
    (rects, body)
}
/// Every widget placed in a layout, and its tabs turned (moved here from
/// `layout.rs` at its cap).
impl Layout {
    /// Every widget showing, in the order the windows are drawn: a
    /// standalone one, and the tab showing in each cell.
    pub(crate) fn showing(&self) -> Vec<u32> {
        self.holders
            .iter()
            .flat_map(|holder| match &holder.holds {
                Holds::One(placed) => vec![placed.id],
                Holds::Custom(custom) => custom
                    .cells
                    .iter()
                    .filter_map(|cell| cell.shown().map(|placed| placed.id))
                    .collect(),
            })
            .collect()
    }

    /// The cell whose tabs hold widget `placed`, and where in them it is.
    fn cell_of(&mut self, placed: u32) -> Option<(&mut Cell, usize)> {
        self.holders
            .iter_mut()
            .filter_map(|holder| match &mut holder.holds {
                Holds::Custom(custom) => Some(custom),
                Holds::One(_) => None,
            })
            .flat_map(|custom| custom.cells.iter_mut())
            .find_map(|cell| {
                let at = cell.tabs.iter().position(|tab| tab.id == placed)?;
                Some((cell, at))
            })
    }

    /// The tab after widget `placed` in its stack shown, or the one before
    /// (`forward` or not), round from the last to the first: which shows
    /// now. `None` for a widget in no custom window.
    pub(crate) fn turn_tab(&mut self, placed: u32, forward: bool) -> Option<u32> {
        let (cell, at) = self.cell_of(placed)?;
        let tabs = cell.tabs.len();
        cell.showing = if forward {
            (at + 1) % tabs
        } else {
            (at + tabs - 1) % tabs
        };
        cell.shown().map(|shown| shown.id)
    }

    /// Show the tab that is widget `placed`; whether it is one.
    pub(crate) fn show_tab(&mut self, placed: u32) -> bool {
        let Some((cell, at)) = self.cell_of(placed) else {
            return false;
        };
        cell.showing = at;
        true
    }

    /// Every widget placed here, standalone or in a custom window, showing
    /// or a tab behind another.
    pub(crate) fn placed(&self) -> Vec<&Placed> {
        self.holders
            .iter()
            .flat_map(|holder| match &holder.holds {
                Holds::One(placed) => vec![placed],
                Holds::Custom(custom) => custom
                    .cells
                    .iter()
                    .flat_map(|cell| cell.tabs.iter())
                    .collect(),
            })
            .collect()
    }
}
