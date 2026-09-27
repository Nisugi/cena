//! Arranging a custom window's inside (`plan/49` Stage A step 4). A widget
//! in a custom window is bare, with no title bar to take hold of, so while
//! the play window's Layout menu has **Arrange** on, each cell shows its
//! outline and its widget's name and takes the pointer: dragged from inside
//! it moves, from near an edge that edge moves, and it lands snapped to the
//! inside's edges, the other cells and the grid, as a window does one level
//! up (`crate::snap`). Let go outside its window, its widget leaves: into the
//! custom window beneath, or into a standalone window of its own
//! (`Layout::release`).
//!
//! With Arrange off, a cell takes nothing, so no press in play rearranges
//! anything: the author asked that the everyday surface stay simple
//! (`plan/49` §1 row 3).

use egui::{
    Align2, Color32, CursorIcon, FontId, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, Vec2,
};

use crate::layout::{Custom, SMALLEST_CELL};
use crate::snap;
use crate::text::AMBER;

/// How far inside a cell's edge a press takes the edge rather than the cell.
const EDGE: f32 = 5.0;

/// A cell being moved or resized.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CellGesture {
    /// The custom window it is in.
    holder: u32,
    /// The widget it shows, which names it.
    placed: u32,
    /// Where it was when the press began, from the inside's top left.
    start: Rect,
    /// Which edges the press took; none, and it moves.
    grab: Grab,
    /// Where the press began.
    origin: Pos2,
    /// Where the pointer was last seen.
    last: Pos2,
}

/// Which edge of a cell a press took along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Edge {
    /// Neither.
    #[default]
    Neither,
    /// The left, or the top.
    Low,
    /// The right, or the bottom.
    High,
}

impl Edge {
    /// The edge of `low..high` within [`EDGE`] of `at`.
    fn at(low: f32, high: f32, at: f32) -> Self {
        if at - low < EDGE {
            Self::Low
        } else if high - at < EDGE {
            Self::High
        } else {
            Self::Neither
        }
    }

    /// `low..high` with this edge moved by `by`, never nearer the other
    /// than `smallest`.
    fn apply(self, low: f32, high: f32, by: f32, smallest: f32) -> (f32, f32) {
        match self {
            Self::Neither => (low, high),
            Self::Low => ((low + by).min(high - smallest), high),
            Self::High => (low, (high + by).max(low + smallest)),
        }
    }
}

/// Which edges of a cell a press took: one along each axis, or none, when
/// the cell moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Grab {
    across: Edge,
    down: Edge,
}

impl Grab {
    /// The edges within [`EDGE`] of `at`, in `rect`.
    fn at(rect: Rect, at: Pos2) -> Self {
        Self {
            across: Edge::at(rect.min.x, rect.max.x, at.x),
            down: Edge::at(rect.min.y, rect.max.y, at.y),
        }
    }

    /// Whether it took no edge, so the cell moves.
    fn moves(self) -> bool {
        self == Self::default()
    }

    /// The pointer over a cell there.
    fn cursor(self) -> CursorIcon {
        match (self.across, self.down) {
            (Edge::Neither, Edge::Neither) => CursorIcon::Grab,
            (_, Edge::Neither) => CursorIcon::ResizeHorizontal,
            (Edge::Neither, _) => CursorIcon::ResizeVertical,
            (across, down) if across == down => CursorIcon::ResizeNwSe,
            _ => CursorIcon::ResizeNeSw,
        }
    }

    /// `start` moved by `by`: all of it, or only the edges taken, never below
    /// a cell's smallest.
    fn apply(self, start: Rect, by: Vec2) -> Rect {
        if self.moves() {
            return start.translate(by);
        }
        let (left, right) = self
            .across
            .apply(start.min.x, start.max.x, by.x, SMALLEST_CELL.x);
        let (top, bottom) = self
            .down
            .apply(start.min.y, start.max.y, by.y, SMALLEST_CELL.y);
        Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom))
    }
}

/// A widget dragged out of its custom window and let go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Released {
    /// The custom window it left.
    pub(super) holder: u32,
    /// The widget.
    pub(super) placed: u32,
    /// Where it was let go.
    pub(super) at: Pos2,
}

/// Custom window `holder`'s cells, drawn at `inside`, arranged: each shows
/// its outline and name and follows the pointer as the module says. The
/// widget let go outside the window, if one was.
pub(super) fn cells(
    ui: &mut egui::Ui,
    custom: &mut Custom,
    holder: u32,
    inside: Rect,
    grid: f32,
    gesture: &mut Option<CellGesture>,
) -> Option<Released> {
    let mut released = None;
    let rects: Vec<Rect> = custom.cells.iter().map(crate::layout::Cell::rect).collect();
    let bounds = Rect::from_min_size(Pos2::ZERO, inside.size());
    let shift = ui.input(|input| input.modifiers.shift);
    for (index, cell) in custom.cells.iter_mut().enumerate() {
        let Some(placed) = cell.shown().copied() else {
            continue;
        };
        let rect = rects[index].translate(inside.min.to_vec2());
        let response = ui.interact(
            rect,
            Id::new(("arrange-cell", holder, placed.id)),
            Sense::drag(),
        );
        // A screen reader, and a test, finds a cell by its widget's name.
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, placed.widget.name())
        });
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(170));
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, AMBER), egui::StrokeKind::Inside);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            placed.widget.name(),
            FontId::proportional(12.0),
            AMBER,
        );
        if let Some(at) = response.hover_pos() {
            ui.ctx().set_cursor_icon(Grab::at(rect, at).cursor());
        }
        if response.drag_started()
            && let Some(origin) = ui.input(|input| input.pointer.press_origin())
        {
            *gesture = Some(CellGesture {
                holder,
                placed: placed.id,
                start: rects[index],
                grab: Grab::at(rect, origin),
                origin,
                last: origin,
            });
        }
        let Some(ours) = gesture
            .as_mut()
            .filter(|ours| ours.holder == holder && ours.placed == placed.id)
        else {
            continue;
        };
        if let Some(at) = response.interact_pointer_pos() {
            ours.last = at;
        }
        let now = ours.grab.apply(ours.start, ours.last - ours.origin);
        let siblings: Vec<Rect> = rects
            .iter()
            .enumerate()
            .filter(|(at, _)| *at != index)
            .map(|(_, rect)| *rect)
            .collect();
        let (snapped, guides) = if shift {
            (now, Vec::new())
        } else {
            snap::snap(ours.start, now, bounds, &siblings, SMALLEST_CELL, grid)
        };
        let outside = ours.grab.moves() && !inside.contains(ours.last);
        if response.drag_stopped() {
            if outside {
                released = Some(Released {
                    holder,
                    placed: placed.id,
                    at: ours.last,
                });
            } else {
                cell.set(kept_in(snapped, bounds, ours.grab));
            }
            *gesture = None;
        } else if response.dragged() {
            cell.set(snapped);
            if outside {
                ghost(ui.ctx(), ours.last, placed.widget.name());
            } else {
                let accent = Stroke::new(1.5, AMBER);
                for guide in guides {
                    if guide.vertical {
                        painter.vline(inside.min.x + guide.at, inside.y_range(), accent);
                    } else {
                        painter.hline(inside.x_range(), inside.min.y + guide.at, accent);
                    }
                }
            }
        }
    }
    released
}

/// Where a cell lands, inside its window's inside `bounds`: a move slid back
/// in, a resize cut at the edge it was dragged past.
fn kept_in(rect: Rect, bounds: Rect, grab: Grab) -> Rect {
    if !grab.moves() {
        return rect.intersect(bounds);
    }
    let over = |low: f32, high: f32, min: f32, max: f32| {
        if low < min {
            min - low
        } else if high > max {
            (max - high).max(min - low)
        } else {
            0.0
        }
    };
    rect.translate(Vec2::new(
        over(rect.min.x, rect.max.x, bounds.min.x, bounds.max.x),
        over(rect.min.y, rect.max.y, bounds.min.y, bounds.max.y),
    ))
}

/// While a widget is dragged outside its window: its name at the pointer,
/// over everything, where it would land.
fn ghost(context: &egui::Context, at: Pos2, name: &str) {
    let painter = context.layer_painter(LayerId::new(Order::Tooltip, Id::new("arrange-ghost")));
    let rect = Rect::from_center_size(at, Vec2::new(140.0, 24.0));
    painter.rect_filled(rect, 3.0, AMBER.gamma_multiply(0.85));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(13.0),
        egui::Color32::BLACK,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(80.0, 40.0))
    }

    fn at(x: f32, y: f32) -> Grab {
        Grab::at(cell(), Pos2::new(x, y))
    }

    /// A press near an edge takes that edge, near a corner both, and inside
    /// the cell none, so the cell moves; each shows its pointer.
    #[test]
    fn a_press_takes_the_edges_it_is_near() {
        let grab = |across, down| Grab { across, down };
        assert!(at(140.0, 120.0).moves());
        assert_eq!(at(140.0, 120.0).cursor(), CursorIcon::Grab);
        assert_eq!(at(102.0, 120.0), grab(Edge::Low, Edge::Neither));
        assert_eq!(at(102.0, 120.0).cursor(), CursorIcon::ResizeHorizontal);
        assert_eq!(at(178.0, 120.0), grab(Edge::High, Edge::Neither));
        assert_eq!(at(140.0, 102.0), grab(Edge::Neither, Edge::Low));
        assert_eq!(at(140.0, 138.0), grab(Edge::Neither, Edge::High));
        assert_eq!(at(140.0, 138.0).cursor(), CursorIcon::ResizeVertical);
        assert_eq!(at(102.0, 102.0).cursor(), CursorIcon::ResizeNwSe);
        assert_eq!(at(178.0, 138.0).cursor(), CursorIcon::ResizeNwSe);
        assert_eq!(at(178.0, 102.0).cursor(), CursorIcon::ResizeNeSw);
        assert_eq!(at(102.0, 138.0).cursor(), CursorIcon::ResizeNeSw);
    }

    /// A move carries the whole cell; an edge moves alone, never past the
    /// smallest a cell can be.
    #[test]
    fn a_grab_moves_what_it_took() {
        let by = Vec2::new(10.0, 5.0);
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
            Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
        };
        let moved = |across, down, by| Grab { across, down }.apply(cell(), by);
        assert_eq!(Grab::default().apply(cell(), by), cell().translate(by));
        assert_eq!(
            moved(Edge::Low, Edge::Neither, by),
            rect(110.0, 100.0, 180.0, 140.0)
        );
        assert_eq!(
            moved(Edge::High, Edge::Neither, by),
            rect(100.0, 100.0, 190.0, 140.0)
        );
        assert_eq!(
            moved(Edge::Neither, Edge::Low, by),
            rect(100.0, 105.0, 180.0, 140.0)
        );
        assert_eq!(
            moved(Edge::Neither, Edge::High, by),
            rect(100.0, 100.0, 180.0, 145.0)
        );
        let far = Vec2::new(500.0, 500.0);
        assert_eq!(moved(Edge::Low, Edge::Low, far).size(), SMALLEST_CELL);
        assert_eq!(moved(Edge::High, Edge::High, -far).size(), SMALLEST_CELL);
    }
}
