//! What arranging does to a layout (`plan/49` Stage A steps 4 and 5): a
//! custom window made empty; a widget, or a whole tab stack, dragged out of
//! one into a window of its own or into another custom window; a standalone
//! window's widget dropped into one; and two standalone windows made one tab
//! stack. Chrome follows each move: a widget that leaves a custom window
//! gets a frame of its own, and one that joins loses it (`plan/28` §7d.3).
//!
//! Where anything lands, the rule is one: onto a widget, it joins that
//! widget's tab stack; onto empty space, it takes a place of its own.

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use super::{CHROME, Custom, Holder, Holds, Layout, Placed};
use crate::widget::LINE;

/// Where a new custom window first sits, from the play area's top left.
const NEW_AT: Pos2 = pos2(20.0, 20.0);

/// A new custom window's size.
const NEW_SIZE: Vec2 = vec2(300.0, 200.0);

/// How much of a standalone window's top is its title bar, which another
/// dropped there stacks onto (`plan/49` §2).
const TITLE: f32 = 32.0;

/// What is taken out of a custom window: one tab of a stack, or a whole
/// cell, named by a widget it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Taking {
    /// This widget alone.
    Tab(u32),
    /// The cell holding this widget, every tab of it.
    Cell(u32),
}

impl Layout {
    /// A new, empty custom window, on top of the rest; its id.
    pub(crate) fn new_custom(&mut self) -> u32 {
        let inside = (NEW_SIZE - CHROME).max(Vec2::ZERO);
        self.add(
            Rect::from_min_size(NEW_AT, NEW_SIZE),
            Holds::Custom(Custom::empty("Custom window", inside)),
        )
    }

    /// `taking`, dragged out of custom window `from`, let go at `at` (from
    /// the play area's top left), landing as the module says: in the custom
    /// window whose inside is under `at`, or else in a window of its own
    /// there, kept inside a play area `area` across. `insides` are the
    /// custom windows' insides as drawn, from the play area's top left. A
    /// custom window its last widget leaves goes with it.
    pub(crate) fn release(
        &mut self,
        from: u32,
        taking: Taking,
        at: Pos2,
        insides: &[(u32, Rect)],
        area: Vec2,
    ) {
        let Some(Holder {
            holds: Holds::Custom(custom),
            ..
        }) = self.holders.iter_mut().find(|holder| holder.id == from)
        else {
            return;
        };
        let taken = match taking {
            Taking::Tab(placed) => custom.take(placed).map(|tab| (vec![tab], 0)),
            Taking::Cell(placed) => custom
                .take_cell(placed)
                .map(|cell| (cell.tabs, cell.showing)),
        };
        let Some((tabs, showing)) = taken else {
            return;
        };
        let emptied = custom.cells.is_empty();
        self.land(tabs, showing, at, insides, area);
        if emptied {
            self.holders.retain(|holder| holder.id != from);
        }
    }

    /// Standalone window `window`, dropped with the pointer at `at`: onto
    /// another standalone window's title bar, the two become one tab stack
    /// there; onto a custom window's inside (of `insides`), its widget lands
    /// there. Either way the standalone window goes. Whether it did.
    pub(crate) fn join(&mut self, window: u32, at: Pos2, insides: &[(u32, Rect)]) -> bool {
        let Some(Holds::One(placed)) = self.holder(window).map(|holder| holder.holds.clone())
        else {
            return false;
        };
        if let Some(onto) = self.holders.iter_mut().find(|holder| {
            holder.id != window
                && matches!(holder.holds, Holds::One(_))
                && Rect::from_min_size(holder.rect().min, vec2(holder.rect().width(), TITLE))
                    .contains(at)
        }) {
            let Holds::One(first) = onto.holds else {
                return false;
            };
            let inside = (onto.rect().size() - CHROME).max(Vec2::ZERO);
            onto.holds = Holds::Custom(Custom::stack(
                first.widget.name(),
                vec![first, placed],
                0,
                inside,
            ));
        } else if let Some((into, inside)) = insides
            .iter()
            .find(|(_, inside)| inside.contains(at))
            .copied()
        {
            self.land_in(into, vec![placed], 0, at - inside.min.to_vec2());
        } else {
            return false;
        }
        self.holders.retain(|holder| holder.id != window);
        true
    }

    /// `tabs`, `showing` the one shown, let go at `at`: into the custom
    /// window whose inside (of `insides`) is there, or else into a window of
    /// its own, kept inside a play area `area` across -- a standalone window
    /// for one widget, a custom window of one tab stack for several.
    fn land(
        &mut self,
        tabs: Vec<Placed>,
        showing: usize,
        at: Pos2,
        insides: &[(u32, Rect)],
        area: Vec2,
    ) {
        if let Some((into, inside)) = insides.iter().find(|(_, inside)| inside.contains(at)) {
            self.land_in(*into, tabs, showing, at - inside.min.to_vec2());
            return;
        }
        let Some(shown) = tabs.get(showing).or_else(|| tabs.first()).copied() else {
            return;
        };
        let strip = if tabs.len() > 1 { LINE } else { 0.0 };
        let size = shown.widget.size() + CHROME + vec2(0.0, strip);
        let corner = at - vec2(size.x / 2.0, CHROME.y / 2.0);
        let furthest = (area - size).max(Vec2::ZERO).to_pos2();
        let placed_at = Rect::from_min_size(corner.clamp(Pos2::ZERO, furthest), size);
        let holds = if tabs.len() == 1 {
            Holds::One(shown)
        } else {
            let inside = (size - CHROME).max(Vec2::ZERO);
            Holds::Custom(Custom::stack(shown.widget.name(), tabs, showing, inside))
        };
        self.add(placed_at, holds);
    }

    /// `tabs` let go at `at` inside custom window `into`.
    fn land_in(&mut self, into: u32, tabs: Vec<Placed>, showing: usize, at: Pos2) {
        if let Some(Holder {
            holds: Holds::Custom(custom),
            ..
        }) = self.holders.iter_mut().find(|holder| holder.id == into)
        {
            custom.land(tabs, showing, at);
        }
    }
}
