//! A progress bar, to the author's spec (`plan/47` step 10): *"vertical and
//! horizontal filling progress bars. We want their height and width to be
//! adjustible. We want them to accept an overlay. We want them to be able to
//! have text inside them, outside them at any of the four sides, or no text.
//! text being numbers, percents, labels, or any combination."*
//!
//! `VellumFE`'s bars are the start (`reference/VellumFE/src/frontend/gui/app/widgets/vitals.rs`):
//! its four vitals' colours, its skin frame painted over the bar
//! (`overlay_progress_frame`, a nine-slice from `skin.rs`'s
//! `nine_slice_patches_impl`, ported here as [`Overlay::framed`]), and its fix
//! for egui's bar, which paints a sliver of fill at zero. Its bars fill one
//! way and hold their text inside; these fill any of four ways and put their
//! text on any side.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Vec2, WidgetInfo, WidgetType};

/// Health's colour: `VellumFE`'s, so a bar is the same red wherever it is.
pub const HEALTH: Color32 = Color32::from_rgb(0xcd, 0x4d, 0x4d);
/// Mana's colour, `VellumFE`'s.
pub const MANA: Color32 = Color32::from_rgb(0x47, 0x84, 0xd9);
/// Stamina's colour, `VellumFE`'s.
pub const STAMINA: Color32 = Color32::from_rgb(0x55, 0xb8, 0x6c);
/// Spirit's colour, `VellumFE`'s.
pub const SPIRIT: Color32 = Color32::from_rgb(0xcb, 0xa9, 0x42);

/// Which way a bar fills as its value grows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fills {
    /// Horizontal, from the left edge.
    #[default]
    Right,
    /// Horizontal, from the right edge.
    Left,
    /// Vertical, from the bottom edge.
    Up,
    /// Vertical, from the top edge.
    Down,
}

/// Where a bar's text goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// Over the bar, centred.
    #[default]
    Inside,
    /// Above the bar.
    Above,
    /// Below the bar.
    Below,
    /// To the bar's left.
    Left,
    /// To the bar's right.
    Right,
    /// No text at all.
    Hidden,
}

/// What a bar's text says, in this order: its label, its numbers
/// (`current/max`), its percent. Any combination; none is no text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Says {
    /// The bar's label: "HP".
    pub label: bool,
    /// Its value out of its most: "350/400". Left out when either is unknown.
    pub numbers: bool,
    /// Its percent: "87%".
    pub percent: bool,
}

impl Default for Says {
    /// The label and the percent, as the hub's cards have them.
    fn default() -> Self {
        Self {
            label: true,
            numbers: false,
            percent: true,
        }
    }
}

/// How a bar widget draws its bar: which way it fills, where its text goes,
/// what the text says, its colour, and an image laid over it. The player
/// picks it on the widget's own page in the settings menu, which its
/// right-click opens, and it is kept with the layout (`plan/49` §2, a
/// widget's options).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Look {
    /// Which way it fills.
    pub fills: Fills,
    /// Where its text goes.
    pub place: Place,
    /// What its text says.
    pub says: Says,
    /// Its fill's colour, red, green and blue.
    pub color: [u8; 3],
    /// An image laid over it, stretched, by its file's path; none unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
}

impl Fills {
    /// Whether it fills up or down, standing upright.
    #[must_use]
    pub const fn upright(self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }
}

/// The least a bar is, either way, however little room it is given.
const LEAST: f32 = 8.0;

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

    fn paint(&self, painter: &egui::Painter, bar: Rect) {
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

/// A bar's value: its percent, and its numbers when the game gave them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amount {
    /// 0-100; more is shown full.
    pub percent: u32,
    /// The value now, when known; a vital can be below zero.
    pub current: Option<i32>,
    /// The most it can be, when known.
    pub max: Option<i32>,
}

/// A progress bar: `ui.add(Bar::new("HP", Some(amount)).fill(HEALTH))`.
#[derive(Clone, Debug)]
#[must_use = "a bar draws nothing until added to a Ui"]
pub struct Bar<'a> {
    label: &'a str,
    value: Option<Amount>,
    fill: Option<Color32>,
    fills: Fills,
    size: Option<Vec2>,
    place: Place,
    says: Says,
    overlay: Option<Overlay>,
}

/// The gap between a bar and text outside it.
const GAP: f32 = 4.0;

impl<'a> Bar<'a> {
    /// A bar labelled `label` at `value`; `None` is unknown, drawn empty
    /// with `?` for its numbers and percent, never as a guess.
    pub fn new(label: &'a str, value: Option<Amount>) -> Self {
        Self {
            label,
            value,
            fill: None,
            fills: Fills::default(),
            size: None,
            place: Place::default(),
            says: Says::default(),
            overlay: None,
        }
    }

    /// The fill's colour; the theme's selection colour otherwise.
    pub fn fill(mut self, color: Color32) -> Self {
        self.fill = Some(color);
        self
    }

    /// Which way it fills.
    pub fn fills(mut self, fills: Fills) -> Self {
        self.fills = fills;
        self
    }

    /// The bar's own width and height, text outside it not counted. Unset,
    /// 72 by 18 across and 18 by 72 upright.
    pub fn size(mut self, size: impl Into<Vec2>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Where its text goes.
    pub fn text(mut self, place: Place) -> Self {
        self.place = place;
        self
    }

    /// What its text says.
    pub fn says(mut self, says: Says) -> Self {
        self.says = says;
        self
    }

    /// Drawn as `look` says: which way it fills, where its text goes, what
    /// the text says, and its colour. The overlay is the caller's to load.
    pub fn look(self, look: &Look) -> Self {
        let [red, green, blue] = look.color;
        self.fills(look.fills)
            .text(look.place)
            .says(look.says)
            .fill(Color32::from_rgb(red, green, blue))
    }

    /// Sized to what `ui` has left, its text outside it counted: as wide and
    /// as tall as the room, across or upright, so the space it is given is its
    /// shape (the author, 2026-09-28: *"if I make a progress bar horizontal
    /// fill and make it taller, the bar doesn't get taller, it should"*).
    pub fn fitted(mut self, ui: &egui::Ui) -> Self {
        let room = ui.available_size();
        let words = self.words();
        let text = if self.place == Place::Inside || self.place == Place::Hidden || words.is_empty()
        {
            Vec2::ZERO
        } else {
            ui.painter()
                .layout_no_wrap(
                    words,
                    egui::TextStyle::Body.resolve(ui.style()),
                    Color32::PLACEHOLDER,
                )
                .size()
                + Vec2::splat(GAP)
        };
        let left = match self.place {
            Place::Above | Place::Below => Vec2::new(room.x, room.y - text.y),
            Place::Left | Place::Right => Vec2::new(room.x - text.x, room.y),
            Place::Inside | Place::Hidden => room,
        };
        self.size = Some(Vec2::new(left.x.max(LEAST), left.y.max(LEAST)));
        self
    }

    /// Art painted over the bar, after its fill and before its text.
    pub fn overlay(mut self, overlay: Overlay) -> Self {
        self.overlay = Some(overlay);
        self
    }

    /// The words, in order, joined by spaces; empty for none.
    #[must_use]
    pub fn words(&self) -> String {
        let mut parts = Vec::new();
        if self.says.label && !self.label.is_empty() {
            parts.push(self.label.to_owned());
        }
        match self.value {
            Some(amount) => {
                if self.says.numbers
                    && let (Some(current), Some(max)) = (amount.current, amount.max)
                {
                    parts.push(format!("{current}/{max}"));
                }
                if self.says.percent {
                    parts.push(format!("{}%", amount.percent));
                }
            }
            None if self.says.numbers || self.says.percent => parts.push("?".to_owned()),
            None => {}
        }
        parts.join(" ")
    }

    fn bar_size(&self) -> Vec2 {
        self.size.unwrap_or(match self.fills {
            Fills::Right | Fills::Left => Vec2::new(72.0, 18.0),
            Fills::Up | Fills::Down => Vec2::new(18.0, 72.0),
        })
    }
}

impl egui::Widget for Bar<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let words = self.words();
        let shown = if self.place == Place::Hidden {
            String::new()
        } else {
            words.clone()
        };
        let galley = (!shown.is_empty()).then(|| {
            ui.painter().layout_no_wrap(
                shown,
                egui::TextStyle::Body.resolve(ui.style()),
                Color32::PLACEHOLDER,
            )
        });
        let bar = self.bar_size();
        let text = galley.as_ref().map_or(Vec2::ZERO, |galley| galley.size());
        let outside = galley.is_some() && self.place != Place::Inside;
        let total = match self.place {
            Place::Above | Place::Below if outside => {
                Vec2::new(bar.x.max(text.x), bar.y + GAP + text.y)
            }
            Place::Left | Place::Right if outside => {
                Vec2::new(text.x + GAP + bar.x, bar.y.max(text.y))
            }
            _ => bar,
        };
        let (rect, response) = ui.allocate_exact_size(total, Sense::hover());
        let bar_rect = match self.place {
            Place::Above if outside => Align2::CENTER_BOTTOM.align_size_within_rect(bar, rect),
            Place::Below if outside => Align2::CENTER_TOP.align_size_within_rect(bar, rect),
            Place::Left if outside => Align2::RIGHT_CENTER.align_size_within_rect(bar, rect),
            Place::Right if outside => Align2::LEFT_CENTER.align_size_within_rect(bar, rect),
            _ => rect,
        };
        let enabled = ui.is_enabled();
        response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::ProgressIndicator, enabled, words.clone())
        });
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let visuals = ui.visuals();
        let trough = visuals.extreme_bg_color;
        let fill = self.fill.unwrap_or(visuals.selection.bg_fill);
        let radius = CornerRadius::same(3);
        let painter = ui.painter();
        painter.rect_filled(bar_rect, radius, trough);
        let filled = self
            .value
            .map(|amount| filled(bar_rect, self.fills, amount.percent))
            .filter(|filled| filled.width() > 0.0 && filled.height() > 0.0);
        if let Some(filled) = filled {
            painter.rect_filled(filled, radius, fill);
        }
        if let Some(overlay) = self.overlay {
            overlay.paint(painter, bar_rect);
        }
        if let Some(galley) = galley {
            let (at, color) = if outside {
                let anchor = match self.place {
                    Place::Above => Align2::CENTER_TOP,
                    Place::Below => Align2::CENTER_BOTTOM,
                    Place::Left => Align2::LEFT_CENTER,
                    _ => Align2::RIGHT_CENTER,
                };
                (
                    anchor.align_size_within_rect(galley.size(), rect).min,
                    visuals.text_color(),
                )
            } else {
                let at = Align2::CENTER_CENTER
                    .align_size_within_rect(galley.size(), bar_rect)
                    .min;
                // Readable over whatever is behind the text's middle.
                let behind = if filled.is_some_and(|filled| filled.contains(bar_rect.center())) {
                    fill
                } else {
                    trough
                };
                let color = readable_on(behind);
                // A one-point outline in the other colour, so the text still
                // reads where the fill's edge runs through it.
                let outline = readable_on(color);
                for offset in [
                    Vec2::new(-1.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(0.0, -1.0),
                    Vec2::new(0.0, 1.0),
                ] {
                    painter.galley(at + offset, galley.clone(), outline);
                }
                (at, color)
            };
            painter.galley(at, galley, color);
        }
        response
    }
}

/// The eight border patches of a nine-slice over `rect`, each as a
/// destination and the part of the texture it shows: `VellumFE`'s
/// `nine_slice_patches_impl` (`skin.rs`) without the hidden sides. Insets too
/// wide for `rect` shrink in proportion, so opposite borders never overlap.
fn nine_slice(texture: Vec2, insets: [f32; 4], rect: Rect) -> Vec<(Rect, Rect)> {
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

/// The part of `bar` a value of `percent` fills, from the side `fills` says.
fn filled(bar: Rect, fills: Fills, percent: u32) -> Rect {
    let share = f32::from(u16::try_from(percent.min(100)).unwrap_or(100)) / 100.0;
    let mut part = bar;
    match fills {
        Fills::Right => part.max.x = bar.min.x + bar.width() * share,
        Fills::Left => part.min.x = bar.max.x - bar.width() * share,
        Fills::Up => part.min.y = bar.max.y - bar.height() * share,
        Fills::Down => part.max.y = bar.min.y + bar.height() * share,
    }
    part
}

/// Black or white, whichever reads on `behind`: by its luminance, the ITU
/// weights.
fn readable_on(behind: Color32) -> Color32 {
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

    fn amount(percent: u32) -> Amount {
        Amount {
            percent,
            current: i32::try_from(percent * 4).ok(),
            max: Some(400),
        }
    }

    #[test]
    fn the_words_are_any_combination_in_one_order() {
        let says = |label, numbers, percent| Says {
            label,
            numbers,
            percent,
        };
        let bar = |s| Bar::new("HP", Some(amount(87))).says(s).words();
        assert_eq!(bar(says(true, true, true)), "HP 348/400 87%");
        assert_eq!(bar(says(true, false, false)), "HP");
        assert_eq!(bar(says(false, true, false)), "348/400");
        assert_eq!(bar(says(false, false, true)), "87%");
        assert_eq!(bar(says(false, false, false)), "");
    }

    #[test]
    fn unknown_is_a_question_and_missing_numbers_are_left_out() {
        let all = Says {
            label: true,
            numbers: true,
            percent: true,
        };
        assert_eq!(Bar::new("HP", None).says(all).words(), "HP ?");
        let percent_only = Some(Amount {
            percent: 50,
            current: None,
            max: None,
        });
        assert_eq!(Bar::new("MP", percent_only).says(all).words(), "MP 50%");
    }

    #[test]
    fn each_direction_fills_from_its_own_edge() {
        let bar = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(100.0, 40.0));
        assert_eq!(
            filled(bar, Fills::Right, 25),
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(25.0, 40.0))
        );
        assert_eq!(
            filled(bar, Fills::Left, 25),
            Rect::from_min_max(egui::pos2(75.0, 0.0), egui::pos2(100.0, 40.0))
        );
        assert_eq!(
            filled(bar, Fills::Up, 25),
            Rect::from_min_max(egui::pos2(0.0, 30.0), egui::pos2(100.0, 40.0))
        );
        assert_eq!(
            filled(bar, Fills::Down, 25),
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 10.0))
        );
        assert_eq!(filled(bar, Fills::Right, 250), bar, "past 100 is full");
    }

    /// A fitted bar is as tall and as wide as the space it is given, across
    /// or upright: a taller cell makes a thicker bar.
    #[test]
    fn a_fitted_bar_takes_the_space_it_is_given_either_way() {
        use egui_kittest::kittest::Queryable as _;
        for fills in [Fills::Right, Fills::Up] {
            let mut harness = egui_kittest::Harness::builder()
                .with_size((200.0, 120.0))
                .build_ui(move |ui| {
                    ui.add(Bar::new("HP", Some(amount(40))).fills(fills).fitted(ui));
                });
            harness.run();
            let drawn = harness.get_by_label("HP 40%").rect();
            assert!(
                drawn.height() > 90.0 && drawn.width() > 170.0,
                "{fills:?}: {drawn:?}"
            );
        }
    }

    /// A frame's corners keep their size and its middle is left clear,
    /// however long the bar.
    #[test]
    fn a_frame_keeps_its_corners_and_leaves_the_middle_clear() {
        let bar = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(200.0, 20.0));
        let patches = nine_slice(Vec2::new(24.0, 12.0), [2.0, 2.0, 2.0, 2.0], bar);
        assert_eq!(patches.len(), 8, "no centre");
        let (corner, uv) = patches[0];
        assert_eq!(corner.size(), Vec2::new(2.0, 2.0));
        assert_eq!(uv.max, egui::pos2(2.0 / 24.0, 2.0 / 12.0));
        assert!(
            patches.iter().all(|(dest, _)| !dest.contains(bar.center())),
            "the fill shows through"
        );
        let squeezed = nine_slice(
            Vec2::new(24.0, 12.0),
            [8.0, 2.0, 8.0, 2.0],
            Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(200.0, 8.0)),
        );
        let (top_left, _) = squeezed[0];
        assert!(
            (top_left.height() - 4.0).abs() < f32::EPSILON,
            "shrunk to fit"
        );
    }

    #[test]
    fn text_reads_on_light_and_dark() {
        assert_eq!(readable_on(Color32::WHITE), Color32::BLACK);
        assert_eq!(readable_on(Color32::from_rgb(20, 20, 20)), Color32::WHITE);
        assert_eq!(readable_on(SPIRIT), Color32::BLACK);
        assert_eq!(readable_on(HEALTH), Color32::WHITE);
    }
}
