//! What arranging does to a layout (`plan/49` Stage A step 4): a custom
//! window made empty, a widget dragged out of one into a standalone window
//! or another custom window, and a standalone window's widget dropped into
//! one. Chrome follows each move: a widget that leaves a custom window gets
//! a frame of its own, and one that joins loses it (`plan/28` §7d.3).

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use super::{CHROME, Custom, Holder, Holds, Layout, Placed};

/// Where a new custom window first sits, from the play area's top left.
const NEW_AT: Pos2 = pos2(20.0, 20.0);

/// A new custom window's size.
const NEW_SIZE: Vec2 = vec2(300.0, 200.0);

impl Layout {
    /// A new, empty custom window, on top of the rest; its id.
    pub(crate) fn new_custom(&mut self) -> u32 {
        let inside = (NEW_SIZE - CHROME).max(Vec2::ZERO);
        self.add(
            Rect::from_min_size(NEW_AT, NEW_SIZE),
            Holds::Custom(Custom::empty("Custom window", inside)),
        )
    }

    /// Widget `placed`, dragged out of custom window `from`, let go at `at`
    /// (from the play area's top left): into the custom window whose inside
    /// is under `at` -- its own, if that is where it came back to -- or else
    /// into a standalone window of its own there, kept
    /// inside a play area `area` across. `insides` are the custom windows'
    /// insides as drawn, from the play area's top left. A custom window its
    /// last widget leaves goes with it.
    pub(crate) fn release(
        &mut self,
        from: u32,
        placed: u32,
        at: Pos2,
        insides: &[(u32, Rect)],
        area: Vec2,
    ) {
        let Some(placed) = self.take(from, placed) else {
            return;
        };
        match insides.iter().find(|(_, inside)| inside.contains(at)) {
            Some((into, inside)) => self.put(*into, placed, at - inside.min.to_vec2()),
            None => self.standalone(placed, at, area),
        }
        let emptied = matches!(
            self.holder(from).map(|holder| &holder.holds),
            Some(Holds::Custom(custom)) if custom.cells.is_empty()
        );
        if emptied {
            self.holders.retain(|holder| holder.id != from);
        }
    }

    /// Standalone window `window`, dropped with the pointer at `at`: when a
    /// custom window's inside (of `insides`) is there, its widget goes into
    /// it, bare, and the standalone window goes. Whether it did.
    pub(crate) fn join(&mut self, window: u32, at: Pos2, insides: &[(u32, Rect)]) -> bool {
        let Some((into, inside)) = insides
            .iter()
            .find(|(_, inside)| inside.contains(at))
            .copied()
        else {
            return false;
        };
        let Some(Holds::One(placed)) = self.holder(window).map(|holder| holder.holds.clone())
        else {
            return false;
        };
        self.holders.retain(|holder| holder.id != window);
        self.put(into, placed, at - inside.min.to_vec2());
        true
    }

    /// Take widget `placed` out of custom window `from`.
    fn take(&mut self, from: u32, placed: u32) -> Option<Placed> {
        match self.holders.iter_mut().find(|holder| holder.id == from) {
            Some(Holder {
                holds: Holds::Custom(custom),
                ..
            }) => custom.take(placed),
            _ => None,
        }
    }

    /// Put `placed` into custom window `into`, its cell's corner at `at`
    /// inside it.
    fn put(&mut self, into: u32, placed: Placed, at: Pos2) {
        if let Some(Holder {
            holds: Holds::Custom(custom),
            ..
        }) = self.holders.iter_mut().find(|holder| holder.id == into)
        {
            custom.put(placed, at);
        }
    }

    /// `placed` in a standalone window of its own, as big as the widget
    /// asks, its title bar under `at` and the whole of it kept inside a play
    /// area `area` across.
    fn standalone(&mut self, placed: Placed, at: Pos2, area: Vec2) {
        let size = placed.widget.size() + CHROME;
        let corner = at - vec2(size.x / 2.0, CHROME.y / 2.0);
        let furthest = (area - size).max(Vec2::ZERO).to_pos2();
        self.add(
            Rect::from_min_size(corner.clamp(Pos2::ZERO, furthest), size),
            Holds::One(placed),
        );
    }
}
