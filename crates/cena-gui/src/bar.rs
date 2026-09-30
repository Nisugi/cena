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

use egui::{Align2, Color32, Sense, Vec2, WidgetInfo, WidgetType};

/// Health's colour: `VellumFE`'s, so a bar is the same red wherever it is.
pub const HEALTH: Color32 = Color32::from_rgb(0xcd, 0x4d, 0x4d);
/// Mana's colour, `VellumFE`'s.
pub const MANA: Color32 = Color32::from_rgb(0x47, 0x84, 0xd9);
/// Stamina's colour, `VellumFE`'s.
pub const STAMINA: Color32 = Color32::from_rgb(0x55, 0xb8, 0x6c);
/// Spirit's colour, `VellumFE`'s.
pub const SPIRIT: Color32 = Color32::from_rgb(0xcb, 0xa9, 0x42);
/// The betrayer's Blood Points' colour: darker than health's.
pub const BLOOD: Color32 = Color32::from_rgb(0x8b, 0x1a, 0x1a);
/// Stance's colour.
pub const STANCE: Color32 = Color32::from_rgb(0x4f, 0xa3, 0xa5);
/// Encumbrance's colour.
pub const ENCUMBRANCE: Color32 = Color32::from_rgb(0xb0, 0x7a, 0x3c);
/// The mind's colour.
pub const MIND: Color32 = Color32::from_rgb(0x8e, 0x6b, 0xc9);
/// The next level's colour.
pub const LEVEL: Color32 = Color32::from_rgb(0xc9, 0xa2, 0x3c);

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
    /// A circle, filling from the bottom as a flask fills: a health or mana
    /// orb.
    Orb,
    /// A ring, filling clockwise from the top.
    Ring,
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

/// What a bar's text says, in this order: its label, the game's word for
/// its state, its numbers (`current/max`), its percent. Any combination;
/// none is no text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "what a bar's text says: each part on or off on its own, as the player picks"
)]
pub struct Says {
    /// The bar's label: "HP".
    pub label: bool,
    /// Its value out of its most: "350/400". Left out when either is unknown.
    pub numbers: bool,
    /// Its percent: "87%".
    pub percent: bool,
    /// What the game calls its state, after the label: "offensive",
    /// "Light", "clear as a bell". Only a bar the game words has any: stance,
    /// encumbrance, the mind and the next level (the author, 2026-09-29:
    /// *"just percentage, stance word (offensive/defensive), label
    /// (stance:), or any combination of the three"*).
    #[serde(default)]
    pub words: bool,
}

impl Default for Says {
    /// The label and the percent, as the hub's cards have them.
    fn default() -> Self {
        Self {
            label: true,
            numbers: false,
            percent: true,
            words: false,
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
    /// An image under its fill, in place of its trough, by its file's path:
    /// an empty orb's glass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// An image its fill uncovers as it fills, in place of its colour, by its
    /// file's path: a liquid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_image: Option<String>,
    /// A ring's thickness, in percent of its radius.
    #[serde(default = "Look::default_ring")]
    pub ring: u8,
}

impl Look {
    /// A ring's thickness unless the player chose: a quarter of its radius.
    pub const RING: u8 = 25;

    const fn default_ring() -> u8 {
        Self::RING
    }
}

impl Fills {
    /// Whether it is round, an orb or a ring: drawn in the square in the
    /// middle of its space.
    #[must_use]
    pub const fn round(self) -> bool {
        matches!(self, Self::Orb | Self::Ring)
    }
}

/// The least a bar is, either way, however little room it is given.
const LEAST: f32 = 8.0;

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
    state: Option<&'a str>,
    value: Option<Amount>,
    fill: Option<Color32>,
    fills: Fills,
    size: Option<Vec2>,
    place: Place,
    says: Says,
    overlay: Option<Overlay>,
    background: Option<egui::TextureId>,
    fill_image: Option<egui::TextureId>,
    ring: u8,
}

/// The gap between a bar and text outside it.
const GAP: f32 = 4.0;

impl<'a> Bar<'a> {
    /// A bar labelled `label` at `value`; `None` is unknown, drawn empty
    /// with `?` for its numbers and percent, never as a guess.
    pub fn new(label: &'a str, value: Option<Amount>) -> Self {
        Self {
            label,
            state: None,
            value,
            fill: None,
            fills: Fills::default(),
            size: None,
            place: Place::default(),
            says: Says::default(),
            overlay: None,
            background: None,
            fill_image: None,
            ring: Look::RING,
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

    /// What the game calls its state, `words`, said when [`Says::words`]
    /// asks for it.
    pub fn state(mut self, words: Option<&'a str>) -> Self {
        self.state = words;
        self
    }

    /// What its text says.
    pub fn says(mut self, says: Says) -> Self {
        self.says = says;
        self
    }

    /// Drawn as `look` says: which way it fills, where its text goes, what
    /// the text says, its colour and a ring's thickness. Its images are the
    /// caller's to load.
    pub fn look(self, look: &Look) -> Self {
        let [red, green, blue] = look.color;
        let mut drawn = self
            .fills(look.fills)
            .text(look.place)
            .says(look.says)
            .fill(Color32::from_rgb(red, green, blue));
        drawn.ring = look.ring;
        drawn
    }

    /// An image under the fill, in place of the trough.
    pub fn background(mut self, image: egui::TextureId) -> Self {
        self.background = Some(image);
        self
    }

    /// An image the fill uncovers as it fills, in place of its colour.
    pub fn fill_image(mut self, image: egui::TextureId) -> Self {
        self.fill_image = Some(image);
        self
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
        let state = self
            .state
            .filter(|words| self.says.words && !words.is_empty());
        if self.says.label && !self.label.is_empty() {
            // `Stance: offensive`, and `Stance 80%` with no word after it.
            parts.push(match state {
                Some(_) => format!("{}:", self.label),
                None => self.label.to_owned(),
            });
        }
        if let Some(state) = state {
            parts.push(state.to_owned());
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
            Fills::Orb | Fills::Ring => Vec2::splat(48.0),
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
        let painter = ui.painter();
        let share = self
            .value
            .map_or(0.0, |amount| shape::share(amount.percent));
        let paint = shape::Paint {
            trough,
            fill,
            background: self.background,
            fill_image: self.fill_image,
            ring: f32::from(self.ring) / 100.0,
        };
        let covered = shape::paint(painter, self.fills, bar_rect, share, &paint);
        if let Some(overlay) = self.overlay {
            overlay.paint(painter, shape::frame(self.fills, bar_rect));
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
                let behind = if covered { fill } else { trough };
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

mod overlay;
mod shape;

pub use overlay::Overlay;

#[cfg(test)]
mod tests;
