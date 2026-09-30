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
//! A tab stack's tabs stay clickable, and each drags alone (step 5); its
//! body drags the whole stack. Wherever something is let go the rule is the
//! layout's (`moves.rs`): onto a widget, it joins that widget's tab stack;
//! onto empty space, it takes a place of its own.
//!
//! With Arrange off, a cell takes nothing, so no press in play rearranges
//! anything: the author asked that the everyday surface stay simple
//! (`plan/49` §1 row 3).

use egui::{Align2, CursorIcon, FontId, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, Vec2};

use crate::layout::{Cell, Custom, SMALLEST_CELL, Taking, stacks_at, tabs_and_body};
use crate::snap;
use crate::theme::{self, T, readable_on};

/// How far inside a cell's edge a press takes the edge rather than the cell.
const EDGE: f32 = 5.0;

/// A cell being moved or resized, or one tab of a stack being moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CellGesture {
    /// The custom window it is in.
    holder: u32,
    /// What is being dragged: a tab, or the cell showing this widget.
    taking: Taking,
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

/// A widget, or a whole stack, dragged out of its custom window and let go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Released {
    /// The custom window it left.
    pub(super) holder: u32,
    /// What left.
    pub(super) taking: Taking,
    /// Where it was let go.
    pub(super) at: Pos2,
}

/// Where a gesture let go inside its own window comes to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Landing {
    /// Outside the window: it leaves.
    Out(Released),
    /// On another cell of the window, whose stack it joins: that cell named
    /// by a widget it holds.
    Onto { from: Taking, into: u32 },
    /// A tab let go on empty space in the window: a cell of its own there,
    /// from the inside's top left.
    Apart { tab: u32, at: Pos2 },
}

/// What every cell of one custom window shares while it is arranged.
struct Arranging<'a> {
    holder: u32,
    inside: Rect,
    grid: f32,
    shift: bool,
    /// Each cell's rect, from the inside's top left, as the frame began.
    rects: &'a [Rect],
    /// A widget in each cell, which names the cell.
    owners: &'a [u32],
}

/// Custom window `holder`'s cells, drawn at `inside`, arranged as the module
/// says. A widget or stack let go outside the window, if one was.
pub(super) fn cells(
    ui: &mut egui::Ui,
    custom: &mut Custom,
    holder: u32,
    inside: Rect,
    grid: f32,
    gesture: &mut Option<CellGesture>,
) -> Option<Released> {
    let rects: Vec<Rect> = custom.cells.iter().map(Cell::rect).collect();
    let owners: Vec<u32> = custom
        .cells
        .iter()
        .map(|cell| cell.shown().map_or(0, |placed| placed.id))
        .collect();
    let arranging = Arranging {
        holder,
        inside,
        grid,
        shift: ui.input(|input| input.modifiers.shift),
        rects: &rects,
        owners: &owners,
    };
    let mut landing = None;
    for (index, cell) in custom.cells.iter_mut().enumerate() {
        let rect = rects[index].translate(inside.min.to_vec2());
        let (tabs, body) = tabs_and_body(rect, cell.tabs.len());
        let mut clicked = None;
        let stack = cell.tabs.clone();
        for (at, (tab, tab_rect)) in stack.iter().zip(tabs).enumerate() {
            let response = ui.interact(
                tab_rect,
                Id::new(("arrange-tab", holder, tab.id)),
                Sense::click_and_drag(),
            );
            if response.clicked() {
                clicked = Some(at);
            }
            let taking = Taking::Tab(tab.id);
            let name = tab.widget.name();
            landing =
                landing.or(arranging.follow(ui, &response, gesture, taking, index, &name, cell));
        }
        if let Some(at) = clicked {
            cell.showing = at;
        }
        let Some(placed) = cell.shown().cloned() else {
            continue;
        };
        let names: Vec<_> = cell.tabs.iter().map(|tab| tab.widget.name()).collect();
        let names = names.join(", ");
        let response = overlay(ui, body, holder, placed.id, &names);
        let taking = Taking::Cell(placed.id);
        landing = landing.or(arranging.follow(ui, &response, gesture, taking, index, &names, cell));
    }
    match landing? {
        Landing::Out(released) => return Some(released),
        Landing::Onto { from, into } => custom.stack_onto(from, into),
        Landing::Apart { tab, at } => {
            if let Some(tab) = custom.take(tab) {
                custom.land(vec![tab], 0, at);
            }
        }
    }
    None
}

/// A cell's body while arranging: washed, outlined and named, taking the
/// pointer; its name said to a screen reader, and a test.
fn overlay(ui: &egui::Ui, body: Rect, holder: u32, placed: u32, names: &str) -> egui::Response {
    let response = ui.interact(
        body,
        Id::new(("arrange-cell", holder, placed)),
        Sense::drag(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, names));
    let painter = ui.painter();
    let accent = theme::color(ui.ctx(), T::Accent);
    let veil = theme::color(ui.ctx(), T::Veil).gamma_multiply(170.0 / 255.0);
    painter.rect_filled(body, 0.0, veil);
    painter.rect_stroke(
        body,
        0.0,
        Stroke::new(1.0, accent),
        egui::StrokeKind::Inside,
    );
    painter.text(
        body.center(),
        Align2::CENTER_CENTER,
        names,
        FontId::proportional(12.0),
        accent,
    );
    if let Some(at) = response.hover_pos() {
        ui.ctx().set_cursor_icon(Grab::at(body, at).cursor());
    }
    response
}

impl Arranging<'_> {
    /// Follow `response` for `taking`, cell `index`'s, named `name`: a
    /// gesture begun on it, the cell following a cell's gesture, and where
    /// the gesture lands when it is let go, when that is not where the cell
    /// already is.
    #[allow(
        clippy::too_many_arguments,
        reason = "one call per handle; the rest is the window's, in self"
    )]
    fn follow(
        &self,
        ui: &egui::Ui,
        response: &egui::Response,
        gesture: &mut Option<CellGesture>,
        taking: Taking,
        index: usize,
        name: &str,
        cell: &mut Cell,
    ) -> Option<Landing> {
        if response.drag_started()
            && let Some(origin) = ui.input(|input| input.pointer.press_origin())
        {
            let grab = match taking {
                Taking::Tab(_) => Grab::default(),
                Taking::Cell(_) => Grab::at(response.rect, origin),
            };
            *gesture = Some(CellGesture {
                holder: self.holder,
                taking,
                start: self.rects[index],
                grab,
                origin,
                last: origin,
            });
        }
        let ours = gesture
            .as_mut()
            .filter(|ours| ours.holder == self.holder && ours.taking == taking)?;
        if let Some(at) = response.interact_pointer_pos() {
            ours.last = at;
        }
        let pointer = ours.last - self.inside.min.to_vec2();
        let outside = ours.grab.moves() && !self.inside.contains(ours.last);
        // The cell it would stack onto: never its own, which a moved cell
        // lies over by now.
        let onto = self
            .rects
            .iter()
            .enumerate()
            .rev()
            .find(|(at, rect)| *at != index && stacks_at(**rect, pointer))
            .map(|(at, _)| at)
            .filter(|_| ours.grab.moves());
        let (snapped, guides) = self.snapped(ours, index);
        let (grab, last) = (ours.grab, ours.last);
        if response.dragged() {
            if outside || matches!(taking, Taking::Tab(_)) {
                ghost(ui.ctx(), last, name);
            }
            if let Some(onto) = onto {
                // Let go here, it stacks: the cell it would join lights up.
                let target = self.rects[onto].translate(self.inside.min.to_vec2());
                let accent = theme::color(ui.ctx(), T::Accent);
                ui.painter()
                    .rect_filled(target, 0.0, accent.gamma_multiply(0.35));
            }
            if matches!(taking, Taking::Cell(_)) {
                cell.set(snapped);
                if !outside {
                    self.guide(ui, &guides);
                }
            }
            return None;
        }
        if !response.drag_stopped() {
            return None;
        }
        *gesture = None;
        if outside {
            return Some(Landing::Out(Released {
                holder: self.holder,
                taking,
                at: last,
            }));
        }
        if let Some(onto) = onto {
            return Some(Landing::Onto {
                from: taking,
                into: self.owners[onto],
            });
        }
        match taking {
            Taking::Tab(tab) if !self.rects[index].contains(pointer) => {
                Some(Landing::Apart { tab, at: pointer })
            }
            Taking::Tab(_) => None,
            Taking::Cell(_) => {
                let bounds = Rect::from_min_size(Pos2::ZERO, self.inside.size());
                cell.set(kept_in(snapped, bounds, grab));
                None
            }
        }
    }

    /// Where `ours`, on cell `index`, puts the cell now: snapped to the
    /// inside's edges, the other cells and the grid, unless Shift is held.
    fn snapped(&self, ours: &CellGesture, index: usize) -> (Rect, Vec<snap::Guide>) {
        let now = ours.grab.apply(ours.start, ours.last - ours.origin);
        if self.shift {
            return (now, Vec::new());
        }
        let siblings: Vec<Rect> = self
            .rects
            .iter()
            .enumerate()
            .filter(|(at, _)| *at != index)
            .map(|(_, rect)| *rect)
            .collect();
        let bounds = Rect::from_min_size(Pos2::ZERO, self.inside.size());
        snap::snap(ours.start, now, bounds, &siblings, SMALLEST_CELL, self.grid)
    }

    /// A line for each snap a cell's gesture has engaged, in its window.
    fn guide(&self, ui: &egui::Ui, guides: &[snap::Guide]) {
        let accent = Stroke::new(1.5, theme::color(ui.ctx(), T::Accent));
        for guide in guides {
            if guide.vertical {
                ui.painter()
                    .vline(self.inside.min.x + guide.at, self.inside.y_range(), accent);
            } else {
                ui.painter()
                    .hline(self.inside.x_range(), self.inside.min.y + guide.at, accent);
            }
        }
    }
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

/// While a widget is dragged where it would leave its cell -- out of its
/// window, or a tab anywhere -- its name at the pointer, over everything,
/// where it would land.
fn ghost(context: &egui::Context, at: Pos2, name: &str) {
    let painter = context.layer_painter(LayerId::new(Order::Tooltip, Id::new("arrange-ghost")));
    let rect = Rect::from_center_size(at, Vec2::new(140.0, 24.0));
    let accent = theme::color(context, T::Accent);
    painter.rect_filled(rect, theme::corner(context), accent.gamma_multiply(0.85));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(13.0),
        readable_on(accent),
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
