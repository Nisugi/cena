//! The palette as egui draws it (`plan/57` §3d): a token's colour, the
//! theme worn, and the one place in this crate a colour literal may be
//! written (the rule of `plan/57` §4, held by
//! `crates/cena-arch-tests/tests/colour_literals.rs`).
//!
//! Every widget asks here for a colour by its meaning ([`color`]) and never
//! writes one. The app [`wear`]s a theme's palette: it is kept in egui's
//! own data, where [`palette`] reads it, and egui's visuals are set from
//! its surfaces and text, so every stock control follows. Until one is
//! worn, [`palette`] answers with the colours Hydra drew before it had
//! themes ([`Palette::bare`]), over egui's own dark mode.

use cena_ui::theme::{Palette, Rgb, Token};
use egui::{Color32, Stroke, Visuals};

pub(crate) use cena_ui::theme::Token as T;

/// Where the palette worn is kept in egui's data.
fn key() -> egui::Id {
    egui::Id::new("theme")
}

/// The palette in force for what `ctx` is drawing.
pub(crate) fn palette(ctx: &egui::Context) -> Palette {
    ctx.data(|data| data.get_temp::<Palette>(key()))
        .unwrap_or_default()
}

/// `token`'s colour, in the palette in force for `ctx`.
pub(crate) fn color(ctx: &egui::Context, token: Token) -> Color32 {
    rgb(palette(ctx).get(token))
}

/// Wear `palette`: every token answers from it, and egui's visuals are set
/// from its surfaces and text.
pub(crate) fn wear(ctx: &egui::Context, palette: &Palette) {
    ctx.data_mut(|data| data.insert_temp(key(), *palette));
    let visuals = visuals(palette);
    // egui keeps a style for dark and one for light and picks by its own
    // theme; ours decides which, so the computer's mode changing does not
    // swap the visuals out from under the palette.
    ctx.set_theme(if visuals.dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.set_visuals(visuals);
}

/// egui's visuals from `palette`: dark or light by its canvas, the
/// surfaces on every control, the text and lines, the selection, the
/// link, a warning and a wrong.
pub(crate) fn visuals(palette: &Palette) -> Visuals {
    let of = |token| rgb(palette.get(token));
    let (canvas, surface, raised, inset) =
        (of(T::Canvas), of(T::Surface), of(T::Raised), of(T::Inset));
    let (line, strong, text, muted) = (of(T::Line), of(T::LineStrong), of(T::Text), of(T::Muted));
    let dark = readable_on(canvas) == Color32::WHITE;
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.override_text_color = Some(text);
    visuals.weak_text_color = Some(muted);
    visuals.window_fill = canvas;
    visuals.panel_fill = canvas;
    visuals.faint_bg_color = surface;
    visuals.extreme_bg_color = inset;
    visuals.code_bg_color = inset;
    visuals.window_stroke = Stroke::new(1.0, strong);
    visuals.hyperlink_color = of(T::Link);
    visuals.warn_fg_color = of(T::Warning);
    visuals.error_fg_color = of(T::Wrong);
    visuals.selection.bg_fill = of(T::Selection);
    visuals.selection.stroke = Stroke::new(1.0, text);
    let widgets = &mut visuals.widgets;
    for (state, fill, edge) in [
        (&mut widgets.noninteractive, canvas, line),
        (&mut widgets.inactive, surface, line),
        (&mut widgets.hovered, raised, strong),
        (&mut widgets.active, raised, strong),
        (&mut widgets.open, surface, line),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(state.bg_stroke.width.max(1.0), edge);
        state.fg_stroke = Stroke::new(state.fg_stroke.width, text);
    }
    widgets.noninteractive.fg_stroke.color = muted;
    visuals
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
    use cena_ui::theme::{Theme, Themes};

    #[test]
    fn with_no_theme_worn_a_token_is_its_bare_colour() {
        let ctx = egui::Context::default();
        assert_eq!(color(&ctx, T::Health), Color32::from_rgb(0xcd, 0x4d, 0x4d));
        assert_eq!(color(&ctx, T::Accent), rgb(T::Accent.bare()));
    }

    #[test]
    fn a_theme_worn_answers_every_token_and_sets_the_visuals() {
        let ctx = egui::Context::default();
        let themes = Themes::built_in();
        let despana = themes.palette(Theme::DEFAULT).unwrap();
        wear(&ctx, &despana);
        assert_eq!(color(&ctx, T::Canvas), Color32::from_rgb(0x0d, 0x11, 0x15));
        let visuals = ctx.global_style().visuals.clone();
        assert!(visuals.dark_mode);
        assert_eq!(visuals.panel_fill, Color32::from_rgb(0x0d, 0x11, 0x15));
        assert_eq!(
            visuals.override_text_color,
            Some(Color32::from_rgb(0xdd, 0xdc, 0xd7))
        );
        // The light theme is light.
        let light = themes.palette(Theme::LIGHT).unwrap();
        wear(&ctx, &light);
        assert!(!ctx.global_style().visuals.dark_mode);
        assert_eq!(color(&ctx, T::Canvas), rgb(light.get(T::Canvas)));
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
