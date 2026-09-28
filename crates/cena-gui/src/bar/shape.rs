//! What a bar paints under its overlay and its text: its trough and its fill,
//! in its shape. Across and upright, a rectangle filled from one edge; an
//! orb, a circle filled from the bottom as a flask fills; a ring, a band
//! filled clockwise from the top (the author, 2026-09-28: *"we also need
//! circle progress bars (health/mana orbs!) The goal being to give all the
//! options possible for custom overlays."*).
//!
//! Each shape takes art of its own: an image under the fill in place of its
//! trough (an empty orb's glass), an image the fill uncovers in place of its
//! colour (a liquid, uncovered as it rises rather than squeezed), and the
//! overlay over both ([`super::Bar::overlay`]). An image for a round shape is
//! laid over the square the circle sits in, so art drawn round stays round.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, TextureId, Vec2, pos2};

use super::Fills;

/// How a bar paints its trough and its fill.
#[derive(Clone, Copy, Debug)]
pub(super) struct Paint {
    /// The trough's colour, where no image is under the fill.
    pub(super) trough: Color32,
    /// The fill's colour, where no image is its own.
    pub(super) fill: Color32,
    /// An image under the fill, in place of the trough.
    pub(super) background: Option<TextureId>,
    /// An image the fill uncovers, in place of its colour.
    pub(super) fill_image: Option<TextureId>,
    /// A ring's thickness, as a share of its radius.
    pub(super) ring: f32,
}

/// Points along any arc: enough that an orb a window tall still looks round.
const STEPS: u16 = 96;

/// The whole of an image.
const WHOLE: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

/// Where the shape sits in `area`: the whole of it across or upright; the
/// square in its middle for an orb or a ring, so a circle stays round.
pub(super) fn frame(fills: Fills, area: Rect) -> Rect {
    if fills.round() {
        Rect::from_center_size(area.center(), Vec2::splat(area.width().min(area.height())))
    } else {
        area
    }
}

/// The share of the way full a value of `percent` is: past 100 is full.
pub(super) fn share(percent: u32) -> f32 {
    f32::from(u16::try_from(percent.min(100)).unwrap_or(100)) / 100.0
}

/// Paint the trough and a fill `share` of the way full in `area`, shaped as
/// `fills` says. Whether the fill covers the middle, where text inside sits.
pub(super) fn paint(
    painter: &Painter,
    fills: Fills,
    area: Rect,
    share: f32,
    paint: &Paint,
) -> bool {
    let frame = frame(fills, area);
    if !frame.is_positive() {
        return false;
    }
    match fills {
        Fills::Orb => orb(painter, frame, share, paint),
        Fills::Ring => ring(painter, frame, share, paint),
        Fills::Right | Fills::Left | Fills::Up | Fills::Down => {
            across(painter, fills, frame, share, paint)
        }
    }
}

/// A rectangle filled from the edge `fills` says.
fn across(painter: &Painter, fills: Fills, frame: Rect, share: f32, paint: &Paint) -> bool {
    let corners = CornerRadius::same(3);
    match paint.background {
        Some(image) => painter.image(image, frame, WHOLE, Color32::WHITE),
        None => painter.rect_filled(frame, corners, paint.trough),
    };
    let part = filled(frame, fills, share);
    if part.width() <= 0.0 || part.height() <= 0.0 {
        return false;
    }
    match paint.fill_image {
        // The part of the image under the fill, uncovered, not squeezed.
        Some(image) => painter.image(image, part, within(part, frame), Color32::WHITE),
        None => painter.rect_filled(part, corners, paint.fill),
    };
    part.contains(frame.center())
}

/// The part of `bar` a fill `share` of the way full covers, from the side
/// `fills` says. An orb and a ring fill by [`segment`] and an arc instead.
pub(super) fn filled(bar: Rect, fills: Fills, share: f32) -> Rect {
    let mut part = bar;
    match fills {
        Fills::Right => part.max.x = bar.min.x + bar.width() * share,
        Fills::Left => part.min.x = bar.max.x - bar.width() * share,
        Fills::Up => part.min.y = bar.max.y - bar.height() * share,
        Fills::Down => part.max.y = bar.min.y + bar.height() * share,
        Fills::Orb | Fills::Ring => {}
    }
    part
}

/// A circle filled from the bottom, as a flask fills.
fn orb(painter: &Painter, frame: Rect, share: f32, paint: &Paint) -> bool {
    let (centre, radius) = (frame.center(), frame.width() / 2.0);
    match paint.background {
        Some(image) => painter.image(image, frame, WHOLE, Color32::WHITE),
        None => painter.circle_filled(centre, radius, paint.trough),
    };
    if share > 0.0 {
        let outline = segment(centre, radius, share);
        painter.add(fan(&outline, frame, (paint.fill_image, paint.fill)));
    }
    share >= 0.5
}

/// A band filled clockwise from the top.
fn ring(painter: &Painter, frame: Rect, share: f32, paint: &Paint) -> bool {
    let outer = frame.width() / 2.0;
    let widths = (outer * (1.0 - paint.ring.clamp(0.05, 1.0)), outer);
    let top = -FRAC_PI_2;
    match paint.background {
        Some(image) => {
            painter.image(image, frame, WHOLE, Color32::WHITE);
        }
        None => {
            painter.add(band(frame, widths, (top, top + TAU), (None, paint.trough)));
        }
    }
    if share > 0.0 {
        let sweep = (top, top + TAU * share);
        painter.add(band(frame, widths, sweep, (paint.fill_image, paint.fill)));
    }
    false
}

/// The part of a circle below a level `share` of the way up it, as points
/// around its edge: from where the level meets its right side, round the
/// bottom, to where it meets its left.
pub(super) fn segment(centre: Pos2, radius: f32, share: f32) -> Vec<Pos2> {
    // Down the screen is positive: the level is where sin θ = 1 - 2 × share.
    let from = (1.0 - 2.0 * share.clamp(0.0, 1.0)).asin();
    around(centre, radius, (from, PI - from))
}

/// Points on a circle from one angle to another, clockwise on the screen.
fn around(centre: Pos2, radius: f32, (from, to): (f32, f32)) -> Vec<Pos2> {
    (0..=STEPS)
        .map(|step| {
            let angle = from + (to - from) * f32::from(step) / f32::from(STEPS);
            centre + radius * Vec2::angled(angle)
        })
        .collect()
}

/// A convex outline filled: with a colour, or an image uncovered.
fn fan(outline: &[Pos2], frame: Rect, (image, color): (Option<TextureId>, Color32)) -> Mesh {
    let mut mesh = image.map_or_else(Mesh::default, Mesh::with_texture);
    for at in outline {
        mesh.vertices.push(vertex(*at, frame, image, color));
    }
    let count = u32::try_from(outline.len()).unwrap_or(0);
    for next in 1..count.saturating_sub(1) {
        mesh.add_triangle(0, next, next + 1);
    }
    mesh
}

/// The band between two circles about `frame`'s middle, from one angle to
/// another: with a colour, or an image uncovered.
fn band(
    frame: Rect,
    (inner, outer): (f32, f32),
    sweep: (f32, f32),
    (image, color): (Option<TextureId>, Color32),
) -> Mesh {
    let mut mesh = image.map_or_else(Mesh::default, Mesh::with_texture);
    let centre = frame.center();
    for (out, inside) in around(centre, outer, sweep)
        .into_iter()
        .zip(around(centre, inner, sweep))
    {
        mesh.vertices.push(vertex(out, frame, image, color));
        mesh.vertices.push(vertex(inside, frame, image, color));
    }
    for step in 0..u32::from(STEPS) {
        let at = step * 2;
        mesh.add_triangle(at, at + 1, at + 2);
        mesh.add_triangle(at + 1, at + 3, at + 2);
    }
    mesh
}

/// A corner of a mesh: coloured, or showing the image at its place.
fn vertex(at: Pos2, frame: Rect, image: Option<TextureId>, color: Color32) -> Vertex {
    match image {
        Some(_) => Vertex {
            pos: at,
            uv: uv(at, frame),
            color: Color32::WHITE,
        },
        None => Vertex {
            pos: at,
            uv: WHITE_UV,
            color,
        },
    }
}

/// `part`'s place in `whole`, as an image's coordinates.
fn within(part: Rect, whole: Rect) -> Rect {
    Rect::from_min_max(uv(part.min, whole), uv(part.max, whole))
}

/// `at`'s place in `whole`, 0 to 1 each way.
fn uv(at: Pos2, whole: Rect) -> Pos2 {
    pos2(
        (at.x - whole.min.x) / whole.width(),
        (at.y - whole.min.y) / whole.height(),
    )
}
