//! A play window's drawers (`plan/49` Stage E): a main area and four
//! drawers, top, bottom, left and right, each holding windows as the main
//! area does, their edges on the same grid (`plan/28` §7d, the author:
//! *"The main area has a grid and snap system, so do the drawers"*).
//!
//! The author's three asks (`plan/28` §7d.5): *"drawers should have the
//! option to clip or push the center area over, same as vellum, drawers
//! should be able to go fully translucent, clicking in the drawer where
//! there is no window and it is translucent should click through."*
//!
//! - **Push** takes the drawer's strip from the main area, whose windows
//!   are drawn inside what is left (`VellumFE`'s `Reserve`). **Clip** lays
//!   the drawer over the main area, which does not move (`Overlay`).
//! - **Opacity** is the backdrop's, and a clip drawer's only: a push drawer
//!   has nothing behind it to show (`VellumFE`'s `zones.rs:326-340`).
//! - **Click-through is not a setting.** It follows from opacity, by the
//!   author's rule: *"the goal is to be able to see what you're clicking."*
//!   Below [`CLEAR`], the backdrop hides nothing, so a press on it reaches
//!   what is under it; a press on a window in the drawer is the window's.
//!   A drop is another question with another answer (§7d.5): it lands in
//!   the drawer under the pointer however clear it is ([`Zones::at`]).
//!
//! A window's rect is kept from its zone's top left, so the windows in a
//! drawer travel with it. **A push drawer squeezes the main area**: its
//! windows are drawn in what is left, in proportion, so windows that tiled
//! it still tile it (`VellumFE`: *"center windows displace/squeeze while
//! the zone is open"*); moving each over instead pushed the right-hand
//! column back across the story. What is kept is not changed, so shutting
//! the drawer puts every window back: a window made narrow for a moment
//! must not lose its layout (`VellumFE`'s `squeezed_sidebar_widths`, whose
//! lesson this keeps). A window is always drawn inside its zone
//! ([`Zones::fit`]).

use egui::{Pos2, Rect, Vec2, pos2};
use serde::{Deserialize, Serialize};

use super::SMALLEST;

/// A backdrop less opaque than this hides nothing: a press on it passes
/// through. A tuning value, not a design question (`plan/28` §7d.5).
pub(crate) const CLEAR: f32 = 0.2;

/// The narrowest a drawer is drawn, whatever it was dragged to.
pub(crate) const THINNEST: f32 = 40.0;

/// Where a window lives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Zone {
    /// The main area.
    #[default]
    Main,
    /// The drawer along the top.
    Top,
    /// Along the bottom.
    Bottom,
    /// Down the left.
    Left,
    /// Down the right.
    Right,
}

impl Zone {
    /// The four drawers, in the order the menu lists them.
    pub(crate) const DRAWERS: [Self; 4] = [Self::Top, Self::Bottom, Self::Left, Self::Right];

    /// Its name, for a player.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Main => "Main area",
            Self::Top => "Top drawer",
            Self::Bottom => "Bottom drawer",
            Self::Left => "Left drawer",
            Self::Right => "Right drawer",
        }
    }

    /// Whether it is the main area, which a kept window does not name.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's skip_serializing_if hands a reference"
    )]
    pub(crate) fn is_main(&self) -> bool {
        *self == Self::Main
    }

    /// Whether it lies across the play area, its size a height.
    fn across(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// How an open drawer shares the play area with the main area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    /// It takes its strip from the main area, which is drawn beside it.
    #[default]
    Push,
    /// It lies over the main area, which does not move.
    Clip,
}

/// One drawer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Drawer {
    /// It is open: drawn, with its windows.
    pub(crate) open: bool,
    /// Its height across the top or bottom, its width down a side.
    pub(crate) size: f32,
    /// Push or clip.
    pub(crate) mode: Mode,
    /// Its backdrop's opacity, 1 solid, 0 unseen: a clip drawer's.
    pub(crate) opacity: f32,
}

impl Default for Drawer {
    fn default() -> Self {
        Self {
            open: false,
            size: 240.0,
            mode: Mode::Push,
            opacity: 1.0,
        }
    }
}

impl Drawer {
    /// Its backdrop's opacity as drawn: solid when it pushes, since nothing
    /// is behind it, and solid for a value no slider gives -- a hand-edited
    /// layout must not make a drawer unseen and unrecoverable (`VellumFE`,
    /// `zone_opacity`).
    pub(crate) fn shown_opacity(&self) -> f32 {
        match self.mode {
            Mode::Clip if self.opacity.is_finite() => self.opacity.clamp(0.0, 1.0),
            Mode::Clip | Mode::Push => 1.0,
        }
    }

    /// Whether a press on its bare backdrop reaches what is under it: a
    /// clip drawer that hides nothing.
    pub(crate) fn clear(&self) -> bool {
        self.shown_opacity() < CLEAR
    }
}

/// A play window's four drawers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Drawers {
    /// Along the top.
    pub(crate) top: Drawer,
    /// Along the bottom.
    pub(crate) bottom: Drawer,
    /// Down the left.
    pub(crate) left: Drawer,
    /// Down the right.
    pub(crate) right: Drawer,
}

impl Default for Drawers {
    fn default() -> Self {
        let across = Drawer {
            size: 160.0,
            ..Drawer::default()
        };
        Self {
            top: across,
            bottom: across,
            left: Drawer::default(),
            right: Drawer::default(),
        }
    }
}

impl Drawers {
    /// Whether they are as a new layout has them, which is not written.
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Drawer `zone`, to change.
    pub(crate) fn get_mut(&mut self, zone: Zone) -> Option<&mut Drawer> {
        match zone {
            Zone::Main => None,
            Zone::Top => Some(&mut self.top),
            Zone::Bottom => Some(&mut self.bottom),
            Zone::Left => Some(&mut self.left),
            Zone::Right => Some(&mut self.right),
        }
    }
}

/// Where the main area and each open drawer lie in a play area this frame,
/// from its top left.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Zones {
    /// The main area.
    main: Rect,
    /// How much of the play area the main area is, across and down: its
    /// windows are drawn at this share of their kept size.
    squeeze: Vec2,
    /// Each open drawer.
    open: Vec<(Zone, Rect)>,
}

impl Default for Zones {
    /// No drawer open, and a main area without end: a layout's rects as
    /// they are kept.
    fn default() -> Self {
        Self {
            main: Rect::from_min_max(Pos2::ZERO, pos2(f32::INFINITY, f32::INFINITY)),
            squeeze: Vec2::splat(1.0),
            open: Vec::new(),
        }
    }
}

impl Zones {
    /// The zones of a play area `area` across with `drawers`. The drawers
    /// across it run its width; those down its sides, between them. Open
    /// drawers are drawn narrower when they would leave the main area less
    /// than the smallest window, keeping their proportions, and never
    /// narrower than [`THINNEST`]; what is kept is not changed.
    pub(crate) fn of(drawers: &Drawers, area: Vec2) -> Self {
        let shown = |drawer: &Drawer| {
            if drawer.open && drawer.size.is_finite() {
                drawer.size.max(THINNEST)
            } else {
                0.0
            }
        };
        let (top, bottom) = squeezed(
            area.y,
            SMALLEST.y,
            shown(&drawers.top),
            shown(&drawers.bottom),
        );
        let (left, right) = squeezed(
            area.x,
            SMALLEST.x,
            shown(&drawers.left),
            shown(&drawers.right),
        );
        let (width, height) = (area.x, area.y);
        let sides = (top, height - bottom);
        let mut open = Vec::new();
        let mut main = Rect::from_min_size(Pos2::ZERO, area);
        for (zone, drawer, rect) in [
            (
                Zone::Top,
                &drawers.top,
                Rect::from_min_max(Pos2::ZERO, pos2(width, top)),
            ),
            (
                Zone::Bottom,
                &drawers.bottom,
                Rect::from_min_max(pos2(0.0, height - bottom), pos2(width, height)),
            ),
            (
                Zone::Left,
                &drawers.left,
                Rect::from_min_max(pos2(0.0, sides.0), pos2(left, sides.1)),
            ),
            (
                Zone::Right,
                &drawers.right,
                Rect::from_min_max(pos2(width - right, sides.0), pos2(width, sides.1)),
            ),
        ] {
            if !drawer.open || !rect.is_positive() {
                continue;
            }
            if drawer.mode == Mode::Push {
                match zone {
                    Zone::Top => main.min.y = rect.max.y,
                    Zone::Bottom => main.max.y = rect.min.y,
                    Zone::Left => main.min.x = rect.max.x,
                    Zone::Right => main.max.x = rect.min.x,
                    Zone::Main => {}
                }
            }
            open.push((zone, rect));
        }
        let share = |part: f32, whole: f32| if whole > 0.0 { part / whole } else { 1.0 };
        let squeeze = Vec2::new(share(main.width(), area.x), share(main.height(), area.y));
        Self {
            main,
            squeeze,
            open,
        }
    }

    /// Where zone `zone` lies; `None` for a drawer that is shut.
    pub(crate) fn rect(&self, zone: Zone) -> Option<Rect> {
        if zone == Zone::Main {
            return Some(self.main);
        }
        self.open
            .iter()
            .find(|(open, _)| *open == zone)
            .map(|(_, rect)| *rect)
    }

    /// Each open drawer and where it lies.
    pub(crate) fn drawers(&self) -> &[(Zone, Rect)] {
        &self.open
    }

    /// The zone a window let go at `at` lands in: an open drawer there,
    /// however clear, over the main area it lies on (`plan/28` §7d.5: a
    /// drop *"targets what the pointer visually sits on"*).
    pub(crate) fn at(&self, at: Pos2) -> Zone {
        self.open
            .iter()
            .find(|(_, rect)| rect.contains(at))
            .map_or(Zone::Main, |(zone, _)| *zone)
    }

    /// Where a window kept at `kept` in zone `zone` is drawn: from the
    /// zone's corner, squeezed with the main area, and inside the zone
    /// ([`Self::inside`]). `None` when the zone is a drawer that is shut.
    pub(crate) fn fit(&self, kept: Rect, zone: Zone) -> Option<Rect> {
        let within = self.rect(zone)?;
        let share = self.share(zone);
        let drawn =
            Rect::from_min_size(within.min + kept.min.to_vec2() * share, kept.size() * share);
        self.inside(drawn, zone)
    }

    /// `at`, from the play area's top left, made no bigger than zone `zone`
    /// and moved inside it; `None` when the zone is a drawer that is shut.
    pub(crate) fn inside(&self, at: Rect, zone: Zone) -> Option<Rect> {
        let within = self.rect(zone)?;
        let size = at.size().min(within.size());
        let furthest = (within.max - size).max(within.min);
        Some(Rect::from_min_size(
            at.min.clamp(within.min, furthest),
            size,
        ))
    }

    /// A rect `at` from the play area's top left, as kept in zone `zone`:
    /// from the zone's corner, unsqueezed.
    pub(crate) fn keep(&self, at: Rect, zone: Zone) -> Rect {
        let corner = self.rect(zone).map_or(Pos2::ZERO, |rect| rect.min);
        let share = self.share(zone);
        Rect::from_min_size(((at.min - corner) / share).to_pos2(), at.size() / share)
    }

    /// The share of its kept size a window in zone `zone` is drawn at.
    fn share(&self, zone: Zone) -> Vec2 {
        if zone == Zone::Main {
            self.squeeze
        } else {
            Vec2::splat(1.0)
        }
    }

    /// The edge a drawer is resized by, as a strip `width` thick on its
    /// side toward the main area.
    pub(crate) fn edge(zone: Zone, rect: Rect, width: f32) -> Rect {
        let half = width / 2.0;
        match zone {
            Zone::Top => Rect::from_min_max(
                pos2(rect.min.x, rect.max.y - half),
                pos2(rect.max.x, rect.max.y + half),
            ),
            Zone::Bottom => Rect::from_min_max(
                pos2(rect.min.x, rect.min.y - half),
                pos2(rect.max.x, rect.min.y + half),
            ),
            Zone::Left => Rect::from_min_max(
                pos2(rect.max.x - half, rect.min.y),
                pos2(rect.max.x + half, rect.max.y),
            ),
            Zone::Right => Rect::from_min_max(
                pos2(rect.min.x - half, rect.min.y),
                pos2(rect.min.x + half, rect.max.y),
            ),
            Zone::Main => Rect::NOTHING,
        }
    }

    /// How a drag of `by` on drawer `zone`'s edge changes its size: toward
    /// the main area, larger.
    pub(crate) fn grown(zone: Zone, by: Vec2) -> f32 {
        match zone {
            Zone::Top => by.y,
            Zone::Bottom => -by.y,
            Zone::Left => by.x,
            Zone::Right => -by.x,
            Zone::Main => 0.0,
        }
    }

    /// Whether drawer `zone` is resized along x.
    pub(crate) fn sideways(zone: Zone) -> bool {
        !zone.across()
    }
}

/// Two opposite drawers `a` and `b` thick, in a play area `whole` long,
/// drawn so the main area keeps `least`: each made thinner by the same
/// share when they would not fit. A shut drawer, 0, stays 0.
fn squeezed(whole: f32, least: f32, a: f32, b: f32) -> (f32, f32) {
    let room = (whole - least).max(0.0);
    let total = a + b;
    if total <= room || total <= 0.0 {
        return (a, b);
    }
    let share = room / total;
    (a * share, b * share)
}

/// A window's place among the zones: where it is drawn, moved and let go
/// (moved here from `layout.rs` at its cap).
impl super::Layout {
    /// Where window `id` is drawn in `zones`, from the play area's top left:
    /// inside its zone ([`Zones::fit`]); `None` when it is not here or its
    /// drawer is shut.
    pub(crate) fn shown(&self, id: u32, zones: &Zones) -> Option<Rect> {
        let holder = self.holder(id)?;
        zones.fit(holder.rect(), holder.zone)
    }

    /// Put window `id` at `at`, from the play area's top left, in the zone
    /// it lives in: while a gesture lasts, which may carry it anywhere.
    pub(crate) fn set_shown(&mut self, id: u32, at: Rect, zones: &Zones) {
        if let Some(holder) = self.holders.iter_mut().find(|holder| holder.id == id) {
            let kept = zones.keep(at, holder.zone);
            holder.set(kept);
        }
    }

    /// Window `id` let go at `at`, from the play area's top left, into zone
    /// `zone`: made to fit there, and kept from its corner.
    pub(crate) fn put(&mut self, id: u32, zone: Zone, at: Rect, zones: &Zones) {
        let Some(holder) = self.holders.iter_mut().find(|holder| holder.id == id) else {
            return;
        };
        let Some(inside) = zones.inside(at, zone) else {
            return;
        };
        holder.zone = zone;
        holder.set(zones.keep(inside, zone));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    const AREA: Vec2 = Vec2::new(1000.0, 700.0);

    fn open(size: f32, mode: Mode) -> Drawer {
        Drawer {
            open: true,
            size,
            mode,
            opacity: 1.0,
        }
    }

    /// Shut, the drawers take nothing: the main area is the play area.
    #[test]
    fn shut_drawers_leave_the_main_area_whole() {
        let zones = Zones::of(&Drawers::default(), AREA);
        assert_eq!(
            zones.rect(Zone::Main),
            Some(Rect::from_min_size(Pos2::ZERO, AREA))
        );
        assert!(zones.drawers().is_empty());
        assert_eq!(zones.rect(Zone::Left), None);
    }

    /// A push drawer takes its strip from the main area; a clip drawer
    /// lies over it, which keeps its size.
    #[test]
    fn push_takes_room_and_clip_lies_over() {
        let drawers = Drawers {
            left: open(300.0, Mode::Push),
            right: open(200.0, Mode::Clip),
            top: open(100.0, Mode::Push),
            ..Drawers::default()
        };
        let zones = Zones::of(&drawers, AREA);
        assert_eq!(
            zones.rect(Zone::Main),
            Some(Rect::from_min_max(pos2(300.0, 100.0), pos2(1000.0, 700.0)))
        );
        assert_eq!(
            zones.rect(Zone::Left),
            Some(Rect::from_min_max(pos2(0.0, 100.0), pos2(300.0, 700.0))),
            "a side drawer runs between the drawers across"
        );
        assert_eq!(
            zones.rect(Zone::Right),
            Some(Rect::from_min_max(pos2(800.0, 100.0), pos2(1000.0, 700.0)))
        );
        assert_eq!(
            zones.rect(Zone::Top),
            Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 100.0)))
        );
    }

    /// Two drawers too wide for the play area are drawn thinner by one
    /// share, the main area keeping the smallest window; what is kept is
    /// the drawers' own business.
    #[test]
    fn drawers_too_wide_are_drawn_thinner_alike() {
        let drawers = Drawers {
            left: open(600.0, Mode::Push),
            right: open(600.0, Mode::Push),
            ..Drawers::default()
        };
        let zones = Zones::of(&drawers, AREA);
        let main = zones.rect(Zone::Main).unwrap_or(Rect::NOTHING);
        assert!((main.width() - SMALLEST.x).abs() < 0.01, "{main:?}");
        let left = zones.rect(Zone::Left).unwrap_or(Rect::NOTHING);
        let right = zones.rect(Zone::Right).unwrap_or(Rect::NOTHING);
        assert!((left.width() - right.width()).abs() < 0.01);
        assert!((drawers.left.size - 600.0).abs() < 0.01, "kept as it was");
    }

    /// A drop lands in the drawer under it, however clear, before the main
    /// area it lies on.
    #[test]
    fn a_drop_lands_in_the_drawer_it_is_over() {
        let drawers = Drawers {
            right: Drawer {
                opacity: 0.0,
                ..open(200.0, Mode::Clip)
            },
            ..Drawers::default()
        };
        let zones = Zones::of(&drawers, AREA);
        assert_eq!(zones.at(pos2(900.0, 300.0)), Zone::Right);
        assert_eq!(zones.at(pos2(500.0, 300.0)), Zone::Main);
    }

    /// Whether two rects are the same, to a hundredth.
    fn same(a: Option<Rect>, b: Rect) -> bool {
        a.is_some_and(|a| (a.min - b.min).length() < 0.01 && (a.max - b.max).length() < 0.01)
    }

    /// A push drawer squeezes the main area's windows into what is left:
    /// two that tiled the play area tile the main area, and kept back
    /// they are as they were.
    #[test]
    fn a_push_drawer_squeezes_the_main_areas_windows() {
        let drawers = Drawers {
            left: open(300.0, Mode::Push),
            ..Drawers::default()
        };
        let zones = Zones::of(&drawers, AREA);
        let story = Rect::from_min_size(pos2(0.0, 0.0), vec2(660.0, 700.0));
        let side = Rect::from_min_size(pos2(660.0, 0.0), vec2(340.0, 200.0));
        let (drawn_story, drawn_side) = (zones.fit(story, Zone::Main), zones.fit(side, Zone::Main));
        assert!(same(
            drawn_story,
            Rect::from_min_size(pos2(300.0, 0.0), vec2(462.0, 700.0))
        ));
        assert!(
            same(
                drawn_side,
                Rect::from_min_size(pos2(762.0, 0.0), vec2(238.0, 200.0))
            ),
            "still beside the story, and to the edge: {drawn_side:?}"
        );
        let back = drawn_side.map(|drawn| zones.keep(drawn, Zone::Main));
        assert!(same(back, side), "{back:?}");
    }

    /// A window is drawn from its drawer's corner, no bigger than the
    /// drawer and inside it; a shut drawer draws none.
    #[test]
    fn a_window_is_drawn_inside_its_drawer() {
        let drawers = Drawers {
            right: open(300.0, Mode::Clip),
            ..Drawers::default()
        };
        let zones = Zones::of(&drawers, AREA);
        let kept = Rect::from_min_size(pos2(20.0, 30.0), vec2(200.0, 100.0));
        assert!(same(
            zones.fit(kept, Zone::Right),
            kept.translate(vec2(700.0, 0.0))
        ));
        let wide = Rect::from_min_size(pos2(50.0, 0.0), vec2(900.0, 100.0));
        assert!(
            same(
                zones.fit(wide, Zone::Right),
                Rect::from_min_size(pos2(700.0, 0.0), vec2(300.0, 100.0))
            ),
            "no wider than the drawer, and inside it"
        );
        assert_eq!(
            zones.fit(kept, Zone::Left),
            None,
            "a shut drawer shows nothing"
        );
    }

    /// Opacity is a clip drawer's; a push drawer is solid, and so is a
    /// value no slider gives. Only a clear clip drawer lets a press by.
    #[test]
    fn a_press_passes_only_a_clear_clip_drawer() {
        let clear = Drawer {
            opacity: 0.05,
            ..open(200.0, Mode::Clip)
        };
        assert!(clear.clear());
        assert!(
            !Drawer {
                mode: Mode::Push,
                ..clear
            }
            .clear(),
            "a push drawer is solid"
        );
        assert!(
            !Drawer {
                opacity: 0.6,
                ..clear
            }
            .clear(),
            "seen, so it stops a press"
        );
        let garbled = Drawer {
            opacity: f32::NAN,
            ..clear
        };
        assert!((garbled.shown_opacity() - 1.0).abs() < f32::EPSILON);
        assert!(!garbled.clear());
    }
}
