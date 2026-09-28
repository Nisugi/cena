//! Art laid over a bar, after its fill and before its text: a whole image
//! stretched, or a frame, `VellumFE`'s nine-slice (`skin.rs`).

use egui::{Color32, Rect, Vec2};

/// Art laid over a bar: a texture of `size` pixels, stretched whole, or
/// framed -- a nine-slice, whose corners keep their size and whose edges
/// stretch only along the bar, with the centre left clear so the fill shows
/// through, as a skin's frame is over `VellumFE`'s bars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overlay {
    texture: egui::TextureId,
    size: Vec2,
    /// Border widths in the texture's pixels, top, right, bottom, left; `None`
    /// for the whole image stretched.
    insets: Option<[f32; 4]>,
}

impl Overlay {
    /// The whole image, stretched over the bar: a gloss, a texture.
    #[must_use]
    pub fn stretched(texture: egui::TextureId, size: Vec2) -> Self {
        Self {
            texture,
            size,
            insets: None,
        }
    }

    /// A frame: `insets` pixels of each edge -- top, right, bottom, left --
    /// are border, drawn at their own size; the middle is left clear.
    #[must_use]
    pub fn framed(texture: egui::TextureId, size: Vec2, insets: [f32; 4]) -> Self {
        Self {
            texture,
            size,
            insets: Some(insets),
        }
    }

    pub(super) fn paint(&self, painter: &egui::Painter, bar: Rect) {
        let whole = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let patches = match self.insets {
            None => vec![(bar, whole)],
            Some(insets) => nine_slice(self.size, insets, bar),
        };
        for (dest, uv) in patches {
            painter.image(self.texture, dest, uv, Color32::WHITE);
        }
    }
}

/// The eight border patches of a nine-slice over `rect`, each as a
/// destination and the part of the texture it shows: `VellumFE`'s
/// `nine_slice_patches_impl` (`skin.rs`) without the hidden sides. Insets too
/// wide for `rect` shrink in proportion, so opposite borders never overlap.
pub(super) fn nine_slice(texture: Vec2, insets: [f32; 4], rect: Rect) -> Vec<(Rect, Rect)> {
    if texture.x <= 0.0 || texture.y <= 0.0 || !rect.is_positive() {
        return Vec::new();
    }
    let [top, right, bottom, left] = insets.map(|inset| inset.max(0.0));
    let shrink = |a: f32, b: f32, room: f32| {
        if a + b > room {
            let by = room / (a + b);
            (a * by, b * by)
        } else {
            (a, b)
        }
    };
    let (dt, db) = shrink(top, bottom, rect.height());
    let (dl, dr) = shrink(left, right, rect.width());
    let dx = [rect.min.x, rect.min.x + dl, rect.max.x - dr, rect.max.x];
    let dy = [rect.min.y, rect.min.y + dt, rect.max.y - db, rect.max.y];
    let ux = [
        0.0,
        (left / texture.x).min(1.0),
        1.0 - (right / texture.x).min(1.0),
        1.0,
    ];
    let uy = [
        0.0,
        (top / texture.y).min(1.0),
        1.0 - (bottom / texture.y).min(1.0),
        1.0,
    ];
    let mut patches = Vec::with_capacity(8);
    for row in 0..3 {
        for col in 0..3 {
            if row == 1 && col == 1 {
                continue;
            }
            let dest = Rect::from_min_max(
                egui::pos2(dx[col], dy[row]),
                egui::pos2(dx[col + 1], dy[row + 1]),
            );
            let uv = Rect::from_min_max(
                egui::pos2(ux[col], uy[row]),
                egui::pos2(ux[col + 1], uy[row + 1]),
            );
            if dest.width() > 0.0 && dest.height() > 0.0 && uv.width() > 0.0 && uv.height() > 0.0 {
                patches.push((dest, uv));
            }
        }
    }
    patches
}
