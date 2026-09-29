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

use super::{CHROME, Custom, Holder, Holds, Layout, Placed, Zones};
use crate::widget::{LINE, Widget};

/// Where a new custom window first sits, from the main area's top left.
const NEW_AT: Pos2 = pos2(20.0, 20.0);

/// A new custom window's size.
const NEW_SIZE: Vec2 = vec2(300.0, 200.0);

/// How far down and right of the last one a widget added from the list is
/// placed, so several added in a row stay apart.
const ADDED_STEP: f32 = 24.0;

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

    /// `widget` in a standalone window of its own, following `follows` when
    /// that is another character, placed down and right of the last one so
    /// several added in a row stay apart; its id.
    pub(crate) fn add_widget(&mut self, widget: Widget, follows: Option<String>) -> u32 {
        let size = widget.size() + CHROME;
        let placed = self.place(widget);
        let id = placed.id;
        let corner = self.next_corner();
        self.add(Rect::from_min_size(corner, size), Holds::One(placed));
        if let Some(who) = follows {
            self.follows.insert(id, who);
        }
        id
    }

    /// `widget` as a new tab beside widget `beside` in window `holder`, and
    /// the one shown: in its cell's tab stack in a custom window, or, in a
    /// standalone window, that window made a custom one of the two, where it
    /// stood (the author, 2026-09-28: *"what about adding/removing
    /// tabs/streams to the stream window?"*). It follows whom `beside`
    /// follows. Its id; `None` when `beside` is not there.
    pub(crate) fn add_tab(&mut self, holder: u32, beside: u32, widget: Widget) -> Option<u32> {
        let placed = self.place(widget);
        let id = placed.id;
        let found = self.holders.iter_mut().find(|found| found.id == holder)?;
        let size = found.rect().size();
        match &mut found.holds {
            Holds::Custom(custom) => {
                let cell = custom
                    .cells
                    .iter_mut()
                    .find(|cell| cell.tabs.iter().any(|tab| tab.id == beside))?;
                cell.tabs.push(placed);
                cell.showing = cell.tabs.len() - 1;
            }
            Holds::One(one) if one.id == beside => {
                // Named as a window stacked by a drag is: for its first.
                let title = one.widget.name().into_owned();
                let inside = (size - CHROME).max(Vec2::ZERO);
                let tabs = vec![one.clone(), placed];
                found.holds = Holds::Custom(Custom::stack(&title, tabs, 1, inside));
            }
            Holds::One(_) => return None,
        }
        if let Some(who) = self.follows.get(&beside).cloned() {
            self.follows.insert(id, who);
        }
        Some(id)
    }

    /// Where the next window added from the list goes: down and right of the
    /// last, so several added in a row stay apart, starting over every ten.
    pub(super) fn next_corner(&self) -> Pos2 {
        let step = f32::from(u16::try_from(self.holders.len() % 10).unwrap_or(0)) * ADDED_STEP;
        NEW_AT + vec2(step, step)
    }

    /// Take widget `placed` out of window `holder`: its standalone window
    /// goes with it; a custom window stays, even empty, to be filled or
    /// removed. What it followed is forgotten.
    pub(crate) fn remove_widget(&mut self, holder: u32, placed: u32) {
        let Some(found) = self.holders.iter_mut().find(|found| found.id == holder) else {
            return;
        };
        match &mut found.holds {
            Holds::One(one) if one.id == placed => {
                self.holders.retain(|found| found.id != holder);
            }
            Holds::One(_) => return,
            Holds::Custom(custom) => {
                if custom.take(placed).is_none() {
                    return;
                }
            }
        }
        self.follows.remove(&placed);
        self.looks.remove(&placed);
        self.rooms.remove(&placed);
        self.lines.remove(&placed);
    }

    /// Window `holder` gone, with every widget in it.
    pub(crate) fn remove_window(&mut self, holder: u32) {
        let Some(at) = self.holders.iter().position(|found| found.id == holder) else {
            return;
        };
        let gone = self.holders.remove(at);
        let ids: Vec<u32> = match &gone.holds {
            Holds::One(placed) => vec![placed.id],
            Holds::Custom(custom) => custom
                .cells
                .iter()
                .flat_map(|cell| cell.tabs.iter().map(|tab| tab.id))
                .collect(),
        };
        for id in ids {
            self.follows.remove(&id);
            self.looks.remove(&id);
            self.rooms.remove(&id);
            self.lines.remove(&id);
        }
    }

    /// Custom window `holder` titled `title`.
    pub(crate) fn rename(&mut self, holder: u32, title: &str) {
        if let Some(Holder {
            holds: Holds::Custom(custom),
            ..
        }) = self.holders.iter_mut().find(|found| found.id == holder)
        {
            title.trim().clone_into(&mut custom.title);
        }
    }

    /// Widget `placed` following `who`, or its window's character when
    /// `None`.
    pub(crate) fn follow(&mut self, placed: u32, who: Option<String>) {
        match who {
            Some(who) => self.follows.insert(placed, who),
            None => self.follows.remove(&placed),
        };
    }

    /// `taking`, dragged out of custom window `from`, let go at `at` (from
    /// the play area's top left), landing as the module says: in the custom
    /// window whose inside is under `at`, or else in a window of its own
    /// there, kept inside the zone of `zones` it was let go in. `insides`
    /// are the custom windows' insides as drawn, from the play area's top
    /// left. A custom window its last widget leaves goes with it.
    pub(crate) fn release(
        &mut self,
        from: u32,
        taking: Taking,
        at: Pos2,
        insides: &[(u32, Rect)],
        zones: &Zones,
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
        self.land(tabs, showing, at, insides, zones);
        if emptied {
            self.holders.retain(|holder| holder.id != from);
        }
    }

    /// Standalone window `window`, dropped with the pointer at `at`: onto
    /// another standalone window's title bar, the two become one tab stack
    /// there; onto a custom window's inside (of `insides`), its widget lands
    /// there. Either way the standalone window goes. Whether it did. The
    /// windows are where `zones` draws them.
    pub(crate) fn join(
        &mut self,
        window: u32,
        at: Pos2,
        insides: &[(u32, Rect)],
        zones: &Zones,
    ) -> bool {
        let Some(Holds::One(placed)) = self.holder(window).map(|holder| holder.holds.clone())
        else {
            return false;
        };
        let onto = self.holders.iter().map(|holder| holder.id).find(|&id| {
            id != window
                && matches!(
                    self.holder(id).map(|holder| &holder.holds),
                    Some(Holds::One(_))
                )
                && self.shown(id, zones).is_some_and(|shown| {
                    Rect::from_min_size(shown.min, vec2(shown.width(), TITLE)).contains(at)
                })
        });
        if let Some(onto) =
            onto.and_then(|onto| self.holders.iter_mut().find(|holder| holder.id == onto))
        {
            let Holds::One(first) = &onto.holds else {
                return false;
            };
            let (first, title) = (first.clone(), first.widget.name().into_owned());
            let inside = (onto.rect().size() - CHROME).max(Vec2::ZERO);
            onto.holds = Holds::Custom(Custom::stack(&title, vec![first, placed], 0, inside));
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
    /// its own, kept inside the zone it was let go in -- a standalone window
    /// for one widget, a custom window of one tab stack for several.
    fn land(
        &mut self,
        tabs: Vec<Placed>,
        showing: usize,
        at: Pos2,
        insides: &[(u32, Rect)],
        zones: &Zones,
    ) {
        if let Some((into, inside)) = insides.iter().find(|(_, inside)| inside.contains(at)) {
            self.land_in(*into, tabs, showing, at - inside.min.to_vec2());
            return;
        }
        let Some(shown) = tabs.get(showing).or_else(|| tabs.first()).cloned() else {
            return;
        };
        let strip = if tabs.len() > 1 { LINE } else { 0.0 };
        let size = shown.widget.size() + CHROME + vec2(0.0, strip);
        let corner = at - vec2(size.x / 2.0, CHROME.y / 2.0);
        let zone = zones.at(at);
        let holds = if tabs.len() == 1 {
            Holds::One(shown)
        } else {
            let inside = (size - CHROME).max(Vec2::ZERO);
            Holds::Custom(Custom::stack(&shown.widget.name(), tabs, showing, inside))
        };
        let id = self.add(Rect::from_min_size(Pos2::ZERO, size), holds);
        self.put(id, zone, Rect::from_min_size(corner, size), zones);
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
