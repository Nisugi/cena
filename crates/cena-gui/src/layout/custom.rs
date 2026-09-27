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

use egui::{Rect, Vec2, pos2};
use serde::{Deserialize, Serialize};

use super::{Placed, kept, rect};
use crate::widget::LINE;

/// How near the bottom edge a cell's bottom must be to keep to it.
const TOUCHING: f32 = 0.5;

/// The smallest a cell becomes when its window shrinks.
const SMALLEST: Vec2 = Vec2::new(20.0, LINE);

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

    /// The widget showing: the only one, or the tab chosen.
    pub(crate) fn shown(&self) -> Option<&Placed> {
        self.tabs.get(self.showing).or_else(|| self.tabs.first())
    }
}
