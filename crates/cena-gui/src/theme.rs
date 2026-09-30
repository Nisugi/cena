//! The palette as egui draws it (`plan/57` §3d): a token's colour, and the
//! one place in this crate a colour literal may be written (the rule of
//! `plan/57` §4, held by `crates/cena-arch-tests/tests/colour_literals.rs`).
//!
//! Every widget asks here for a colour by its meaning ([`color`]) and never
//! writes one. A theme, once one is applied (`plan/57` step 2), is what
//! [`palette`] answers with; until then it answers with the colours Hydra
//! drew before it had themes ([`Palette::bare`]), over egui's own dark mode.

use cena_ui::theme::{Palette, Rgb, Token};
use egui::Color32;

pub(crate) use cena_ui::theme::Token as T;

/// The palette in force for what `ctx` is drawing.
pub(crate) fn palette(ctx: &egui::Context) -> Palette {
    ctx.data(|data| data.get_temp::<Palette>(egui::Id::new("theme")))
        .unwrap_or_default()
}

/// `token`'s colour, in the palette in force for `ctx`.
pub(crate) fn color(ctx: &egui::Context, token: Token) -> Color32 {
    rgb(palette(ctx).get(token))
}

/// A palette's colour as egui's.
pub(crate) fn rgb([red, green, blue]: Rgb) -> Color32 {
    Color32::from_rgb(red, green, blue)
}

/// Black or white, whichever reads on `behind`: by its luminance, the ITU
/// weights.
pub(crate) fn readable_on(behind: Color32) -> Color32 {
    let [r, g, b, _] = behind.to_array();
    let luminance = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    if luminance > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_theme_applied_a_token_is_its_bare_colour() {
        let ctx = egui::Context::default();
        assert_eq!(color(&ctx, T::Health), Color32::from_rgb(0xcd, 0x4d, 0x4d));
        assert_eq!(color(&ctx, T::Accent), rgb(T::Accent.bare()));
    }

    #[test]
    fn black_reads_on_a_light_ground_and_white_on_a_dark_one() {
        assert_eq!(
            readable_on(Color32::from_rgb(0xd7, 0xad, 0x63)),
            Color32::BLACK
        );
        assert_eq!(
            readable_on(Color32::from_rgb(0x11, 0x16, 0x1b)),
            Color32::WHITE
        );
    }
}
