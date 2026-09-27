//! Where a pane lands when it is dragged or resized (`plan/47` step 6): its
//! moving edges pulled to the play window's edges, to the other panes'
//! edges, and to the grid, when one is near.
//!
//! A port of `VellumFE`'s snap engine (`reference/VellumFE/src/frontend/gui/app/snap.rs`),
//! its pure half: the gesture read per axis from the totals since it began
//! (`classify_axis`), the candidates (`axis_candidates`), and the nearest
//! legal one winning, ties to the more meaningful target (`snap_1d`,
//! `kind_priority`). Two of its field lessons come with it: a window's
//! centre pairs only with centre lines, never with an edge or the grid, or
//! a move lands with both edges off the grid; and a resize that would take
//! a pane below its smallest is skipped before the nearest is chosen, so an
//! illegal near candidate never hides a legal one. Left out: its anchors
//! (the author's container window replaces what they served, `plan/28` §7d)
//! and centre lines, which are off by default there.

use egui::{Rect, Vec2};

/// How close an edge must come to a target to snap to it, in points.
pub(crate) const RADIUS: f32 = 8.0;

/// What an edge can snap to, in the order ties are given: the play
/// window's edge beats a pane flush with it, which beats a grid line
/// running through both (`VellumFE`'s `kind_priority`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Target {
    /// The area's own edge.
    Bound,
    /// Another pane's edge.
    Sibling,
    /// A grid line.
    Grid,
}

/// How a gesture moves a rect along one axis, read from the totals since
/// the gesture began: a move shifts both edges, a resize pins one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gesture {
    /// This axis has not moved.
    Idle,
    /// Both edges together: a move.
    Translate,
    /// The left or top edge: a resize from that side.
    MinEdge,
    /// The right or bottom edge.
    MaxEdge,
}

/// Movement below this is rounding, not a gesture (`VellumFE`'s
/// `MOVED_EPS`).
const MOVED: f32 = 0.6;

/// Read one axis's gesture from how far its two edges have moved in all.
pub(crate) fn classify(dmin: f32, dmax: f32) -> Gesture {
    match (dmin.abs() > MOVED, dmax.abs() > MOVED) {
        (false, false) => Gesture::Idle,
        (true, false) => Gesture::MinEdge,
        (false, true) => Gesture::MaxEdge,
        (true, true) => Gesture::Translate,
    }
}

/// One engaged snap: a line to draw while the gesture lasts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Guide {
    /// A vertical line at x = `at`, or a horizontal one at y = `at`.
    pub(crate) vertical: bool,
    /// Where.
    pub(crate) at: f32,
    /// What it snapped to.
    pub(crate) target: Target,
}

/// Snap `now`, which the gesture has made of `start`, against `bounds`,
/// the other panes' rects, and a grid of `grid` points (0 for none). Axes
/// are independent; a pane is never resized below `smallest`.
pub(crate) fn snap(
    start: Rect,
    now: Rect,
    bounds: Rect,
    siblings: &[Rect],
    smallest: Vec2,
    grid: f32,
) -> (Rect, Vec<Guide>) {
    let mut rect = now;
    let mut guides = Vec::new();
    let axes = [
        (
            true,
            classify(now.min.x - start.min.x, now.max.x - start.max.x),
        ),
        (
            false,
            classify(now.min.y - start.min.y, now.max.y - start.max.y),
        ),
    ];
    for (vertical, gesture) in axes {
        let along = |r: Rect| {
            if vertical {
                (r.min.x, r.max.x)
            } else {
                (r.min.y, r.max.y)
            }
        };
        let (lo, hi) = along(now);
        let (bound_lo, bound_hi) = along(bounds);
        let mut candidates = vec![(bound_lo, Target::Bound), (bound_hi, Target::Bound)];
        for sibling in siblings {
            let (sibling_lo, sibling_hi) = along(*sibling);
            candidates.push((sibling_lo, Target::Sibling));
            candidates.push((sibling_hi, Target::Sibling));
        }
        let smallest = if vertical { smallest.x } else { smallest.y };
        let Some((delta, at, target)) =
            snap_1d(gesture, (lo, hi), smallest, &candidates, (bound_lo, grid))
        else {
            continue;
        };
        let (new_lo, new_hi) = match gesture {
            Gesture::MinEdge => (lo + delta, hi),
            Gesture::MaxEdge => (lo, hi + delta),
            Gesture::Translate | Gesture::Idle => (lo + delta, hi + delta),
        };
        if vertical {
            rect.min.x = new_lo;
            rect.max.x = new_hi;
        } else {
            rect.min.y = new_lo;
            rect.max.y = new_hi;
        }
        guides.push(Guide {
            vertical,
            at,
            target,
        });
    }
    (rect, guides)
}

/// The nearest legal snap along one axis within [`RADIUS`], as how far to
/// move, the line it meets and what that is; `None` when nothing is near.
/// `grid` is its origin and pitch.
fn snap_1d(
    gesture: Gesture,
    (lo, hi): (f32, f32),
    smallest: f32,
    candidates: &[(f32, Target)],
    (origin, pitch): (f32, f32),
) -> Option<(f32, f32, Target)> {
    let moving: &[f32] = match gesture {
        Gesture::Idle => return None,
        Gesture::Translate => &[lo, hi],
        Gesture::MinEdge => &[lo],
        Gesture::MaxEdge => &[hi],
    };
    let mut best: Option<(f32, f32, f32, Target)> = None;
    for &position in moving {
        let grid_line = (pitch > 0.0)
            .then(|| origin + ((position - origin) / pitch).round() * pitch)
            .map(|line| (line, Target::Grid));
        for &(value, target) in candidates.iter().chain(grid_line.iter()) {
            let distance = (value - position).abs();
            if distance > RADIUS {
                continue;
            }
            let delta = value - position;
            let extent = match gesture {
                Gesture::MinEdge => hi - (lo + delta),
                Gesture::MaxEdge => (hi + delta) - lo,
                Gesture::Translate | Gesture::Idle => hi - lo,
            };
            if extent < smallest - 0.01 {
                continue;
            }
            let wins = best.is_none_or(|(best_distance, _, _, best_target)| {
                distance + 0.001 < best_distance
                    || ((distance - best_distance).abs() <= 0.001 && target < best_target)
            });
            if wins {
                best = Some((distance, delta, value, target));
            }
        }
    }
    best.map(|(_, delta, at, target)| (delta, at, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), Vec2::new(w, h))
    }

    const AREA: Rect = Rect {
        min: pos2(0.0, 0.0),
        max: pos2(600.0, 400.0),
    };
    const SMALLEST: Vec2 = Vec2::new(80.0, 40.0);

    /// Whether two points are the same, to a hundredth.
    fn same(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn a_gesture_is_read_from_its_totals() {
        assert_eq!(classify(0.2, -0.3), Gesture::Idle, "rounding");
        assert_eq!(classify(12.0, 12.0), Gesture::Translate);
        assert_eq!(classify(-7.0, 0.0), Gesture::MinEdge);
        assert_eq!(classify(0.0, 9.0), Gesture::MaxEdge);
    }

    /// Moved to within the radius of the area's right edge, a pane butts
    /// it; nothing near, it stays where it was dropped.
    #[test]
    fn a_move_meets_the_edge_it_comes_near() {
        let start = rect(100.0, 100.0, 200.0, 100.0);
        let (snapped, guides) = snap(
            start,
            start.translate(Vec2::new(295.0, 0.0)),
            AREA,
            &[],
            SMALLEST,
            0.0,
        );
        assert_eq!(snapped, rect(400.0, 100.0, 200.0, 100.0));
        assert_eq!(
            guides,
            [Guide {
                vertical: true,
                at: 600.0,
                target: Target::Bound
            }]
        );
        let (free, none) = snap(
            start,
            start.translate(Vec2::new(40.0, 0.0)),
            AREA,
            &[],
            SMALLEST,
            0.0,
        );
        assert_eq!(free, start.translate(Vec2::new(40.0, 0.0)));
        assert!(none.is_empty());
    }

    /// A pane moved beside another meets its edge; on the grid, a move
    /// keeps both edges on it when the pane is a whole number of cells.
    #[test]
    fn a_move_meets_a_neighbour_and_the_grid() {
        let other = rect(0.0, 0.0, 300.0, 400.0);
        let start = rect(320.0, 50.0, 200.0, 100.0);
        let (beside, _) = snap(
            start,
            start.translate(Vec2::new(-16.0, 0.0)),
            AREA,
            &[other],
            SMALLEST,
            0.0,
        );
        assert!(same(beside.min.x, 300.0), "{beside:?}");
        let (gridded, guides) = snap(
            start,
            start.translate(Vec2::new(0.0, 13.0)),
            AREA,
            &[],
            SMALLEST,
            20.0,
        );
        assert!(same(gridded.min.y, 60.0), "{gridded:?}");
        assert!(same(gridded.height(), 100.0), "a move keeps the size");
        assert_eq!(guides[0].target, Target::Grid);
    }

    /// A resize moves only the edge being dragged, and never takes the
    /// pane below its smallest: the illegal snap is skipped, not taken.
    #[test]
    fn a_resize_moves_one_edge_and_keeps_its_smallest() {
        let start = rect(100.0, 100.0, 200.0, 100.0);
        let mut dragged = start;
        dragged.max.x = 594.0;
        let (wider, _) = snap(start, dragged, AREA, &[], SMALLEST, 0.0);
        assert_eq!(wider, rect(100.0, 100.0, 500.0, 100.0));

        // Shrinking to 83 wide: a neighbour's near edge, 4 away, would make
        // it 79, under the smallest, so its far edge, 6 away, is taken.
        let start = rect(100.0, 100.0, 100.0, 100.0);
        let mut shrunk = start;
        shrunk.max.x = 183.0;
        let wall = rect(179.0, 300.0, 10.0, 50.0);
        let (kept, _) = snap(start, shrunk, AREA, &[wall], SMALLEST, 0.0);
        assert!(same(kept.max.x, 189.0), "{kept:?}");
    }

    /// Exact ties go to the area's edge before a pane flush with it, and a
    /// pane before a grid line through both.
    #[test]
    fn a_tie_goes_to_the_more_meaningful_target() {
        let start = rect(300.0, 100.0, 200.0, 100.0);
        let flush = rect(500.0, 0.0, 100.0, 400.0);
        let (_, guides) = snap(
            start,
            start.translate(Vec2::new(95.0, 0.0)),
            AREA,
            &[flush],
            SMALLEST,
            50.0,
        );
        assert_eq!(guides[0].target, Target::Bound);
        let (_, guides) = snap(
            start,
            start.translate(Vec2::new(-6.0, 0.0)),
            AREA,
            &[rect(0.0, 0.0, 294.0, 400.0)],
            SMALLEST,
            50.0,
        );
        assert_eq!(guides[0].target, Target::Sibling);
    }
}
