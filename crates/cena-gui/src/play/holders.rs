//! A play window's windows, free inside it (`plan/47` step 6, `plan/49`
//! Stage A): each a standalone window around one widget or a custom window
//! of several, that drags by its title bar and resizes by its edges, and
//! lands snapped (`crate::snap`). What each holds scrolls rather than grows,
//! and a scroll area keeps its own presses, so the title bar is where a
//! window is moved from.
//!
//! `VellumFE`'s way with egui windows (`reference/VellumFE/src/frontend/gui/app/zones.rs:1927-2084`):
//! while nobody presses on a window, it is pinned to the rect the layout
//! keeps -- its position fed and its size held -- because egui's own memory
//! of a window must never win. From a press that reaches it until the
//! release, egui owns it and every handle behaves natively; each frame the
//! rect egui drew is snapped and kept, and the release frame's is where it
//! lands. Shift snaps to nothing.
//!
//! **Every window a press reaches is let go, not only one.** Windows tile,
//! so two windows' resize handles meet on each shared edge, and it is egui
//! that picks which of them a press resizes -- the one on top. `VellumFE`
//! latches one window and needs a page of rules to latch the same one egui
//! chose (`zones.rs`, `should_claim_latch`); here each window the press
//! reaches is let go, and whichever egui moved is the one kept. The others,
//! let go but untouched, stay where they were.

use egui::{Id, LayerId, Order, Pos2, Rect, Stroke};

use super::{Play, PlayView, arrange, draw};
use crate::layout::{Holder, Holds, Layout, SMALLEST};
use crate::snap::{self, Guide};
use crate::text::AMBER;
use crate::widget::{Clicked, Seen};

/// How far outside a window its resize handles reach, so a press there
/// reaches it.
const EDGE: f32 = 8.0;

/// A window let go for a gesture: which, and where it was when the press
/// began. Its size is let go too, for a resize; a move cannot change it,
/// since what every window holds scrolls rather than growing it
/// (`draw.rs`, `holder`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Engaged {
    pub(super) holder: u32,
    pub(super) start: Rect,
}

/// A window's egui id: its own in every play window.
pub(super) fn id(session: u32, holder: u32) -> Id {
    Id::new(("play-window", session, holder))
}

/// Windows drawn this frame, from the play area's top left: each held
/// window's rect as egui drew it, and each custom window's inside.
type Drawn = (Vec<(u32, Rect)>, Vec<(u32, Rect)>);

impl Play {
    /// Draw each window inside `ui`'s remaining area, following a drag or
    /// resize and snapping it, and, with Arrange on, a custom window's cells
    /// too (`arrange.rs`): a widget dragged out of one leaves it, and a
    /// standalone window dropped on one joins it. `true` when the layout
    /// changed, so it wants saving.
    pub(super) fn arrange(&mut self, ui: &mut egui::Ui, view: &PlayView<'_>) -> bool {
        let area = ui.available_rect_before_wrap();
        let context = ui.ctx().clone();
        if self.layout.is_none() {
            #[cfg(test)]
            let fitted = if self.room_parts {
                Layout::with_room_parts(area.size())
            } else {
                Layout::fitted(area.size())
            };
            #[cfg(not(test))]
            let fitted = Layout::fitted(area.size());
            self.layout = Some(fitted);
        }
        self.press(&context, area);
        let open = self
            .layout
            .as_ref()
            .map(Layout::streams)
            .unwrap_or_default();
        let seen = Seen {
            snapshot: view.snapshot,
            story: view.story,
            hunt: view.hunt,
            who: None,
            open: &open,
        };
        let ((drawn, insides), released) = self.draw_windows(&context, area, seen, view.others);
        let mut changed = false;
        if let (Some(out), Some(layout)) = (released, self.layout.as_mut()) {
            let at = out.at - area.min.to_vec2();
            layout.release(out.holder, out.taking, at, &insides, area.size());
            changed = true;
        }
        self.insides.clone_from(&insides);
        self.settle(&context, area, &drawn, &insides) || changed
    }

    /// A press this frame lets go of the windows it reaches (`reached`),
    /// unless, with Arrange on, it is on a cell, which is the cell's
    /// (`arrange.rs`): its window is not let go, nor the grid shown.
    fn press(&mut self, context: &egui::Context, area: Rect) {
        let Some(layout) = self.layout.as_ref().filter(|layout| !layout.locked) else {
            return;
        };
        let (pressed, origin) =
            context.input(|input| (input.pointer.any_pressed(), input.pointer.press_origin()));
        let Some(origin) = origin.filter(|_| pressed && self.engaged.is_empty()) else {
            return;
        };
        if self.arranging && on_cell(context, layout, &self.insides, area, origin, self.session) {
            return;
        }
        self.engaged = reached(context, layout, area, origin, self.session)
            .into_iter()
            .filter_map(|holder| layout.rect(holder).map(|start| Engaged { holder, start }))
            .collect();
    }

    /// Each window, pinned or let go, with what it holds; with Arrange on, a
    /// custom window's cells arranged. What was drawn, and a widget let go
    /// outside its custom window, if one was.
    fn draw_windows(
        &mut self,
        context: &egui::Context,
        area: Rect,
        seen: Seen<'_>,
        others: &[crate::widget::Character],
    ) -> (Drawn, Option<arrange::Released>) {
        let (session, arranging, offset) = (self.session, self.arranging, area.min.to_vec2());
        let (snapshot, story) = (seen.snapshot, seen.story);
        let mut drawn = Vec::new();
        let mut insides = Vec::new();
        let mut released = None;
        let Some(layout) = self.layout.as_mut() else {
            return ((drawn, insides), released);
        };
        let (grid, locked) = (layout.grid, layout.locked);
        let mut drawing = draw::Drawing {
            seen,
            others,
            follows: &layout.follows,
            looks: &layout.looks,
            rooms: &layout.rooms,
            lines: &layout.lines,
            session,
            read: &mut self.read,
            sent: None,
        };
        for holder in &mut layout.holders {
            let at = holder.rect().translate(offset);
            let held = self
                .engaged
                .iter()
                .any(|engaged| engaged.holder == holder.id);
            let (id, title) = (holder.id, draw::title(&holder.holds, drawing.follows));
            let window = holder_window(title, id, at, held, area, session)
                .movable(!locked)
                .resizable(!locked);
            let shown = window.show(context, |ui| {
                let inside = draw::holder(ui, &mut holder.holds, &mut drawing);
                if let Holds::Custom(custom) = &mut holder.holds {
                    insides.push((id, inside.translate(-offset)));
                    if arranging {
                        released = released.or_else(|| {
                            arrange::cells(ui, custom, id, inside, grid, &mut self.cell)
                        });
                    }
                }
            });
            if held && let Some(shown) = shown {
                drawn.push((id, shown.response.rect.translate(-offset)));
            }
        }
        match drawing.sent {
            Some(Clicked::Send(line)) => self.out = Some(super::Asked::Send(line)),
            Some(Clicked::Quietly(line)) => self.out = Some(super::Asked::Quietly(line)),
            Some(Clicked::Link(link, at)) => self.clicked(context, (link, at), (snapshot, story)),
            None => {}
        }
        ((drawn, insides), released)
    }

    /// The windows let go this frame, snapped where egui drew them, with the
    /// guides while the gesture lasts; at its end, with Arrange on, a window
    /// carried onto a custom window joins it. `true` when the gesture ended
    /// and changed the layout.
    fn settle(
        &mut self,
        context: &egui::Context,
        area: Rect,
        drawn: &[(u32, Rect)],
        insides: &[(u32, Rect)],
    ) -> bool {
        let Some(layout) = self.layout.as_mut() else {
            return false;
        };
        if self.engaged.is_empty() {
            return false;
        }
        let (down, shift, latest, dragging) = context.input(|input| {
            (
                input.pointer.any_down(),
                input.modifiers.shift,
                input.pointer.latest_pos(),
                input.pointer.is_decidedly_dragging(),
            )
        });
        let bounds = Rect::from_min_size(Pos2::ZERO, area.size());
        let mut guides = Vec::new();
        for engaged in &self.engaged {
            let Some((_, now)) = drawn.iter().find(|(holder, _)| *holder == engaged.holder) else {
                continue;
            };
            // The grid shows once a window moves or changes its size under a
            // drag, never for a click (the author, 2026-09-28: "Clicking is
            // not an active drag"). The drag is egui's word -- the pointer
            // past a click's reach -- because a window egui draws off where
            // its layout keeps it has moved, by its rect, at a mere press.
            self.guiding |= dragging
                && ((now.min - engaged.start.min).length() > 0.5
                    || (now.size() - engaged.start.size()).length() > 0.5);
            let siblings: Vec<Rect> = layout
                .holders
                .iter()
                .filter(|holder| holder.id != engaged.holder)
                .map(Holder::rect)
                .collect();
            let (snapped, engaged_guides) = if shift {
                (*now, Vec::new())
            } else {
                snap::snap(
                    engaged.start,
                    *now,
                    bounds,
                    &siblings,
                    SMALLEST,
                    layout.grid,
                )
            };
            layout.set(engaged.holder, snapped);
            guides.extend(engaged_guides);
        }
        if down {
            if self.guiding {
                guide(context, area, layout.grid, &guides, self.session);
            }
            return false;
        }
        self.guiding = false;
        let mut changed = self
            .engaged
            .iter()
            .any(|engaged| layout.rect(engaged.holder) != Some(engaged.start));
        if self.arranging
            && let Some(at) = latest
        {
            // A window moved, not resized, and let go over a custom window's
            // inside, joins it.
            for engaged in &self.engaged {
                let carried = layout.rect(engaged.holder).is_some_and(|now| {
                    now != engaged.start && (now.size() - engaged.start.size()).length() < 0.5
                });
                if carried {
                    changed |= layout.join(engaged.holder, at - area.min.to_vec2(), insides);
                }
            }
        }
        self.engaged.clear();
        changed
    }
}

/// Whether a press at `origin` is on a cell of the custom window on top
/// there, its inside where `insides` last saw it.
fn on_cell(
    context: &egui::Context,
    layout: &Layout,
    insides: &[(u32, Rect)],
    area: Rect,
    origin: Pos2,
    session: u32,
) -> bool {
    let on_top = context.layer_id_at(origin).map(|layer| layer.id);
    insides.iter().any(|(holder, inside)| {
        let inside = inside.translate(area.min.to_vec2());
        on_top == Some(id(session, *holder))
            && matches!(
                layout.holder(*holder).map(|found| &found.holds),
                Some(Holds::Custom(custom)) if custom.cells.iter().any(|cell| {
                    cell.rect().translate(inside.min.to_vec2()).contains(origin)
                })
            )
    })
}

/// Window `holder`'s egui window at `at` inside `area`, titled `title`:
/// pinned to it, or, `held` by a gesture, let go.
fn holder_window(
    title: String,
    holder: u32,
    at: Rect,
    held: bool,
    area: Rect,
    session: u32,
) -> egui::Window<'static> {
    // Dragged from anywhere, as `VellumFE`'s are: dragged by its title in
    // `TitleBar` mode the fork hands the move over apart from the area, and
    // the rect the window reports stays where the press began, so nothing
    // could be snapped. Content that takes its own presses keeps them.
    let window = egui::Window::new(title)
        .id(id(session, holder))
        .drag_area(egui::WindowDrag::Anywhere)
        .collapsible(false)
        .resizable(true)
        .constrain_to(area)
        .default_pos(at.min)
        .default_size(at.size());
    if held {
        window.min_size(SMALLEST)
    } else {
        window
            .current_pos(at.min)
            .min_size(at.size())
            .max_size(at.size())
    }
}

/// The windows a press at `origin` reaches: the one on top there, and every
/// window whose edges reach it, since a resize handle lies just outside a
/// window and two meet on a shared edge. A press on anything else on top --
/// a menu, a popup -- reaches none: without that, choosing from the Layout
/// menu over a window moved the window beneath. `VellumFE`'s latch asks the
/// same of `layer_id_at` (`zones.rs`, `should_claim_latch`).
fn reached(
    context: &egui::Context,
    layout: &Layout,
    area: Rect,
    origin: Pos2,
    session: u32,
) -> Vec<u32> {
    let on_top = context.layer_id_at(origin);
    let over_a_window = on_top.and_then(|layer| {
        layout
            .holders
            .iter()
            .map(|holder| holder.id)
            .find(|holder| layer.id == id(session, *holder))
    });
    if on_top.is_some_and(|layer| layer.order != Order::Background) && over_a_window.is_none() {
        return Vec::new();
    }
    layout
        .holders
        .iter()
        .filter(|holder| {
            Some(holder.id) == over_a_window
                || holder
                    .rect()
                    .translate(area.min.to_vec2())
                    .expand(EDGE)
                    .contains(origin)
        })
        .map(|holder| holder.id)
        .collect()
}

/// While a gesture lasts: the grid, faint, over the whole area, and a line
/// for each snap it has engaged.
fn guide(context: &egui::Context, area: Rect, grid: f32, guides: &[Guide], session: u32) {
    let painter = context
        .layer_painter(LayerId::new(
            Order::Foreground,
            Id::new(("play-guides", session)),
        ))
        .with_clip_rect(area);
    if grid >= 4.0 {
        let faint = Stroke::new(1.0, egui::Color32::from_white_alpha(18));
        let mut x = area.min.x;
        while x <= area.max.x {
            painter.vline(x, area.y_range(), faint);
            x += grid;
        }
        let mut y = area.min.y;
        while y <= area.max.y {
            painter.hline(area.x_range(), y, faint);
            y += grid;
        }
    }
    let accent = Stroke::new(1.5, AMBER);
    for guide in guides {
        if guide.vertical {
            painter.vline(area.min.x + guide.at, area.y_range(), accent);
        } else {
            painter.hline(area.x_range(), area.min.y + guide.at, accent);
        }
    }
}
