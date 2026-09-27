//! A play window's panes, free inside it (`plan/47` step 6): each its own
//! window that drags by its title bar and resizes by its edges, and lands
//! snapped (`crate::snap`). Each pane's content scrolls, and a scroll area
//! keeps its own presses, so the title bar is where a pane is moved from.
//!
//! `VellumFE`'s way with egui windows (`reference/VellumFE/src/frontend/gui/app/zones.rs:1927-2084`),
//! at the scale of four panes: while nobody presses on a pane, it is pinned
//! to the rect the layout keeps -- its position fed and its size held --
//! because egui's own memory of a window must never win. From a press that
//! reaches it until the release, egui owns it and every handle behaves
//! natively; each frame the rect egui drew is snapped and kept, and the
//! release frame's is where it lands. Shift snaps to nothing.
//!
//! **Every pane a press reaches is let go, not only one.** Panes tile, so
//! two windows' resize handles meet on each shared edge, and it is egui
//! that picks which of them a press resizes -- the one on top. `VellumFE`
//! latches one window and needs a page of rules to latch the same one egui
//! chose (`zones.rs`, `should_claim_latch`); here each pane the press
//! reaches is let go, and whichever egui moved is the one kept. The others,
//! let go but untouched, stay where they were.

use egui::{Id, LayerId, Order, Pos2, Rect, Stroke};

use super::{Play, PlayView, draw};
use crate::layout::{Layout, Pane, SMALLEST};
use crate::snap::{self, Guide};
use crate::text::AMBER;

/// How far outside a pane its resize handles reach, so a press there
/// reaches it.
const EDGE: f32 = 8.0;

/// A pane let go for a gesture: which, and where it was when the press
/// began. Its size is let go too, for a resize; a move cannot change it,
/// since every pane's content scrolls rather than growing the window
/// (`draw.rs`, `pane`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Engaged {
    pub(super) pane: Pane,
    pub(super) start: Rect,
}

/// A pane's window id: its own in every play window.
fn id(session: u32, pane: Pane) -> Id {
    Id::new(("play-pane", session, pane))
}

impl Play {
    /// Draw each pane as a window inside `ui`'s remaining area, following a
    /// drag or resize and snapping it. `true` when a gesture ended and moved
    /// a pane, so the layout wants saving.
    pub(super) fn arrange(&mut self, ui: &mut egui::Ui, view: &PlayView<'_>) -> bool {
        let area = ui.available_rect_before_wrap();
        let session = self.session;
        let context = ui.ctx().clone();
        let layout = self
            .layout
            .get_or_insert_with(|| Layout::fitted(area.size()));
        if !layout.complete() {
            layout.fill_from(&Layout::fitted(area.size()));
        }
        let (pressed, down, origin, shift) = context.input(|input| {
            (
                input.pointer.any_pressed(),
                input.pointer.any_down(),
                input.pointer.press_origin(),
                input.modifiers.shift,
            )
        });
        if pressed
            && self.engaged.is_empty()
            && let Some(origin) = origin
        {
            self.engaged = reached(&context, layout, area, origin, session)
                .into_iter()
                .map(|pane| Engaged {
                    pane,
                    start: layout.rect(pane),
                })
                .collect();
        }
        let offset = area.min.to_vec2();
        let mut drawn = Vec::new();
        for pane in Pane::ALL {
            let at = layout.rect(pane).translate(offset);
            let held = self.engaged.iter().any(|engaged| engaged.pane == pane);
            let window = pane_window(pane, at, held, area, session);
            let shown = window.show(&context, |ui| draw::pane(ui, pane, view, session));
            if held && let Some(shown) = shown {
                drawn.push((pane, shown.response.rect.translate(-offset)));
            }
        }
        if self.engaged.is_empty() {
            return false;
        }
        let bounds = Rect::from_min_size(Pos2::ZERO, area.size());
        let mut guides = Vec::new();
        for engaged in &self.engaged {
            let Some((_, now)) = drawn.iter().find(|(pane, _)| *pane == engaged.pane) else {
                continue;
            };
            let siblings: Vec<Rect> = Pane::ALL
                .iter()
                .filter(|pane| **pane != engaged.pane)
                .map(|pane| layout.rect(*pane))
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
            layout.set(engaged.pane, snapped);
            guides.extend(engaged_guides);
        }
        if down {
            guide(&context, area, layout.grid, &guides, session);
            return false;
        }
        let moved = self
            .engaged
            .iter()
            .any(|engaged| layout.rect(engaged.pane) != engaged.start);
        self.engaged.clear();
        moved
    }
}

/// Pane `pane`'s window at `at` inside `area`: pinned to it, or, `held` by a
/// gesture, let go.
fn pane_window(
    pane: Pane,
    at: Rect,
    held: bool,
    area: Rect,
    session: u32,
) -> egui::Window<'static> {
    // Dragged from anywhere, as `VellumFE`'s are: dragged by its title in
    // `TitleBar` mode the fork hands the move over apart from the area, and
    // the rect the window reports stays where the press began, so nothing
    // could be snapped. Content that takes its own presses keeps them.
    let window = egui::Window::new(pane.title())
        .id(id(session, pane))
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

/// The panes a press at `origin` reaches: the one whose window is on top
/// there, and every pane whose edges reach it, since a resize handle lies
/// just outside a pane and two meet on a shared edge. A press on anything
/// else on top -- a menu, a popup -- reaches none: without that, choosing
/// from the Layout menu over a pane moved the pane beneath. `VellumFE`'s
/// latch asks the same of `layer_id_at` (`zones.rs`, `should_claim_latch`).
fn reached(
    context: &egui::Context,
    layout: &Layout,
    area: Rect,
    origin: Pos2,
    session: u32,
) -> Vec<Pane> {
    let on_top = context.layer_id_at(origin);
    let over_a_pane = on_top.and_then(|layer| {
        Pane::ALL
            .into_iter()
            .find(|pane| layer.id == id(session, *pane))
    });
    if on_top.is_some_and(|layer| layer.order != Order::Background) && over_a_pane.is_none() {
        return Vec::new();
    }
    Pane::ALL
        .into_iter()
        .filter(|pane| {
            Some(*pane) == over_a_pane
                || layout
                    .rect(*pane)
                    .translate(area.min.to_vec2())
                    .expand(EDGE)
                    .contains(origin)
        })
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
