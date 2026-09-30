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

use cena_ui::theme::{Outfit, Palette, Rgb, Shape, Token};
use egui::{Color32, CornerRadius, Stroke, Visuals};

pub(crate) use cena_ui::theme::Token as T;

/// Where the outfit worn is kept in egui's data.
fn key() -> egui::Id {
    egui::Id::new("theme")
}

/// The outfit in force for what `ctx` is drawing: the one worn for its
/// viewport ([`wearing`]), else Hydra's ([`wear`]).
pub(crate) fn outfit(ctx: &egui::Context) -> Outfit {
    let viewport = ctx.viewport_id();
    ctx.data(|data| {
        data.get_temp::<Outfit>(key().with(viewport))
            .or_else(|| data.get_temp::<Outfit>(key()))
    })
    .unwrap_or_default()
}

/// The palette in force for what `ctx` is drawing.
pub(crate) fn palette(ctx: &egui::Context) -> Palette {
    outfit(ctx).palette
}

/// The shape in force for what `ctx` is drawing.
pub(crate) fn shape(ctx: &egui::Context) -> Shape {
    outfit(ctx).shape
}

/// A control's corner radius in force for what `ctx` is drawing, as a
/// painter takes it.
pub(crate) fn corner(ctx: &egui::Context) -> f32 {
    f32::from(shape(ctx).corner)
}

/// `token`'s colour, in the palette in force for `ctx`.
pub(crate) fn color(ctx: &egui::Context, token: Token) -> Color32 {
    rgb(palette(ctx).get(token))
}

/// Wear `outfit`: every token answers from its palette, and egui's style
/// is set from it, the visuals from its surfaces and text, the spacing and
/// corners from its shape.
pub(crate) fn wear(ctx: &egui::Context, outfit: &Outfit) {
    ctx.data_mut(|data| data.insert_temp(key(), *outfit));
    let style = style(outfit);
    // egui keeps a style for dark and one for light and picks by its own
    // theme; ours decides which, so the computer's mode changing does not
    // swap the visuals out from under the palette.
    let theme = if style.visuals.dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(theme);
    ctx.set_style_of(theme, style);
}

/// egui's style from `outfit`: egui's own, its visuals from the palette
/// ([`visuals`]) and its spacing and corners from the shape.
pub(crate) fn style(outfit: &Outfit) -> egui::Style {
    let mut style = egui::Style {
        visuals: visuals(&outfit.palette),
        ..egui::Style::default()
    };
    let shape = &outfit.shape;
    let factor = shape.density.factor();
    let spacing = &mut style.spacing;
    spacing.item_spacing *= factor;
    spacing.button_padding *= factor;
    spacing.scroll.bar_width = shape.scrollbar;
    let corner = CornerRadius::same(shape.corner);
    let visuals = &mut style.visuals;
    visuals.window_corner_radius = CornerRadius::same(shape.corner.saturating_mul(2));
    visuals.menu_corner_radius = corner;
    visuals.window_stroke.width = shape.stroke;
    if !shape.shadows {
        visuals.window_shadow = egui::Shadow::NONE;
        visuals.popup_shadow = egui::Shadow::NONE;
    }
    for state in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        state.corner_radius = corner;
        state.bg_stroke.width = shape.stroke;
    }
    style
}

/// `outfit` worn for the viewport `ctx` is showing until the [`Worn`] is
/// dropped: the outfit answers under that viewport, and egui's style is
/// the outfit's meanwhile and put back after, so a play window wears its
/// character's own theme while the rest wear Hydra's (`plan/57` §3c). With
/// no outfit, nothing changes.
pub(crate) fn wearing(ctx: &egui::Context, outfit: Option<&Outfit>) -> Option<Worn> {
    let outfit = outfit?;
    let viewport = ctx.viewport_id();
    let theme = ctx.theme();
    let before = ctx.global_style();
    ctx.data_mut(|data| data.insert_temp(key().with(viewport), *outfit));
    ctx.set_style_of(theme, style(outfit));
    Some(Worn {
        ctx: ctx.clone(),
        viewport,
        theme,
        before,
    })
}

/// An outfit worn for one viewport, put off when dropped.
pub(crate) struct Worn {
    ctx: egui::Context,
    viewport: egui::ViewportId,
    theme: egui::Theme,
    before: std::sync::Arc<egui::Style>,
}

impl Drop for Worn {
    fn drop(&mut self) {
        self.ctx
            .set_style_of(self.theme, std::sync::Arc::clone(&self.before));
        self.ctx
            .data_mut(|data| data.remove::<Outfit>(key().with(self.viewport)));
    }
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
        let despana = themes.outfit(Theme::DEFAULT).unwrap();
        wear(&ctx, &despana);
        assert_eq!(color(&ctx, T::Canvas), Color32::from_rgb(0x0d, 0x11, 0x15));
        assert!(
            (corner(&ctx) - 3.0).abs() < f32::EPSILON,
            "the default shape"
        );
        let visuals = ctx.global_style().visuals.clone();
        assert!(visuals.dark_mode);
        assert_eq!(visuals.panel_fill, Color32::from_rgb(0x0d, 0x11, 0x15));
        assert_eq!(
            visuals.override_text_color,
            Some(Color32::from_rgb(0xdd, 0xdc, 0xd7))
        );
        // The light theme is light.
        let light = themes.outfit(Theme::LIGHT).unwrap();
        wear(&ctx, &light);
        assert!(!ctx.global_style().visuals.dark_mode);
        assert_eq!(color(&ctx, T::Canvas), rgb(light.palette.get(T::Canvas)));
    }

    #[test]
    fn the_shape_reaches_egui_and_hydras_corners() {
        let mut outfit = Outfit::default();
        outfit.shape.corner = 0;
        outfit.shape.density = cena_ui::theme::Density::Roomy;
        outfit.shape.scrollbar = 12.0;
        outfit.shape.shadows = false;
        let style = style(&outfit);
        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            CornerRadius::ZERO
        );
        assert_eq!(style.visuals.window_corner_radius, CornerRadius::ZERO);
        assert_eq!(style.visuals.window_shadow, egui::Shadow::NONE);
        assert!((style.spacing.scroll.bar_width - 12.0).abs() < f32::EPSILON);
        let plain = egui::Style::default();
        assert!(
            style.spacing.item_spacing.x > plain.spacing.item_spacing.x,
            "roomier"
        );
        let ctx = egui::Context::default();
        wear(&ctx, &outfit);
        assert!(corner(&ctx).abs() < f32::EPSILON);
    }

    #[test]
    fn a_palette_worn_for_a_viewport_is_put_off_when_dropped() {
        let ctx = egui::Context::default();
        let themes = Themes::built_in();
        let (despana, light) = (
            themes.outfit(Theme::DEFAULT).unwrap(),
            themes.outfit(Theme::LIGHT).unwrap(),
        );
        wear(&ctx, &despana);
        assert!(
            wearing(&ctx, None).is_none(),
            "nothing to wear: nothing changes"
        );
        let worn = wearing(&ctx, Some(&light));
        assert_eq!(color(&ctx, T::Canvas), rgb(light.palette.get(T::Canvas)));
        assert!(!ctx.global_style().visuals.dark_mode);
        drop(worn);
        assert_eq!(
            color(&ctx, T::Canvas),
            rgb(despana.palette.get(T::Canvas)),
            "Hydra's again"
        );
        assert!(ctx.global_style().visuals.dark_mode);
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
