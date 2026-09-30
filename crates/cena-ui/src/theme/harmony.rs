//! The harmony generator: a whole palette from one seed colour against a
//! background (`plan/57` §2b, §3a).
//!
//! **Niffy's work**: the prototype `vellum-palette-harmony.html`, then its
//! port in `VellumFE` (`reference/VellumFE/src/core/harmony.rs`), brought here
//! whole and widened from eleven game-text roles to every token of the
//! palette. The engine is pure: a [`Recipe`] in, a [`Palette`] out, the same
//! every time.
//!
//! The idea: the seed's hue anchors a [`Scheme`] of hues round the OKLCH
//! circle, and each *free* token takes a slot in it, nudged in lightness and
//! chroma so two on one hue stay apart. A token that *carries meaning* by
//! its hue (health red, mana blue, a bank's mark gold) is *anchored*: it
//! keeps the hue it has always had and harmony moves only its lightness and
//! chroma. Every generated colour is lifted until it clears a contrast floor
//! against the background, which is how a light theme costs one input, and
//! kept a least distance from the others in its group. A *pin* is kept as
//! written; the rest harmonise round it.

use std::collections::BTreeMap;

use super::oklch::{Lch, contrast, delta_e, from_lch, to_lch};
use super::{Palette, Rgb, Token};

/// Hue offsets in degrees round the OKLCH hue circle. The golden angle is
/// the honest answer to "give me N distinct hues": 137.5 degrees never
/// repeats and spreads evenly however many are taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// One hue, varied in lightness and chroma.
    Monochrome,
    /// Neighbours: calm, low contrast.
    Analogous,
    /// Opposites: a strong pairing.
    Complementary,
    /// Softer than complementary.
    Split,
    /// Even thirds: vivid, balanced.
    Triadic,
    /// Two complementary pairs.
    Tetradic,
    /// The golden angle: the most separation.
    Golden,
    /// An analogous pair plus its opposite.
    Compound,
}

impl Scheme {
    /// Every scheme, in the order the editor offers them.
    pub const ALL: [Scheme; 8] = [
        Scheme::Monochrome,
        Scheme::Analogous,
        Scheme::Complementary,
        Scheme::Split,
        Scheme::Triadic,
        Scheme::Tetradic,
        Scheme::Golden,
        Scheme::Compound,
    ];

    /// Its hue offsets from the seed's hue, in degrees.
    #[must_use]
    pub const fn offsets(self) -> &'static [f64] {
        match self {
            Scheme::Monochrome => &[0.0],
            Scheme::Analogous => &[0.0, 30.0, -30.0, 60.0, -60.0],
            Scheme::Complementary => &[0.0, 180.0],
            Scheme::Split => &[0.0, 150.0, 210.0],
            Scheme::Triadic => &[0.0, 120.0, 240.0],
            Scheme::Tetradic => &[0.0, 90.0, 180.0, 270.0],
            Scheme::Golden => &[0.0, 137.5, 275.0, 52.5, 190.0, 327.5],
            Scheme::Compound => &[0.0, 30.0, 180.0, 210.0],
        }
    }

    /// Its name, as a theme file writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Scheme::Monochrome => "monochrome",
            Scheme::Analogous => "analogous",
            Scheme::Complementary => "complementary",
            Scheme::Split => "split",
            Scheme::Triadic => "triadic",
            Scheme::Tetradic => "tetradic",
            Scheme::Golden => "golden",
            Scheme::Compound => "compound",
        }
    }

    /// What it does, in a few words.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Scheme::Monochrome => "one hue, varied lightness and chroma",
            Scheme::Analogous => "neighbours: calm, low contrast",
            Scheme::Complementary => "opposites: strong pairing",
            Scheme::Split => "softer than complementary",
            Scheme::Triadic => "even thirds: vivid, balanced",
            Scheme::Tetradic => "two complementary pairs",
            Scheme::Golden => "golden angle: most separation",
            Scheme::Compound => "analogous pair plus its opposite",
        }
    }

    /// The scheme named `name`, ignoring case.
    #[must_use]
    pub fn parse(name: &str) -> Option<Scheme> {
        Scheme::ALL
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// How a token takes its colour from the harmony.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    /// A slot in the scheme's hues, nudged in lightness and chroma.
    Free {
        /// Which of the scheme's offsets.
        slot: usize,
        /// Added to the seed's lightness.
        dl: f64,
        /// Added to the seed's chroma.
        dc: f64,
    },
    /// The hue the token has always had ([`Token::bare`]'s), its lightness
    /// and chroma the seed's, nudged.
    Anchored {
        /// Added to the seed's lightness.
        dl: f64,
        /// Added to the seed's chroma.
        dc: f64,
    },
    /// A surface: the background, its lightness moved by `dl`. Never lifted
    /// to the contrast floor, which is for what is drawn *on* a surface.
    Surface {
        /// Added to the background's lightness.
        dl: f64,
    },
    /// The plate under the room's name: its hue, the lightness solved so the
    /// name reads on it at the recipe's spread.
    Plate,
    /// Kept as it is ([`Token::bare`]) unless pinned: a veil or a grid line
    /// drawn part clear, whose colour is not a colour.
    Fixed,
}

/// Every group of tokens keeps its distances within itself: a vital and a
/// map line are never on one screen at once, so they need not differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// The story's and the room's text, the accents.
    Text,
    /// The bars.
    Vitals,
    /// The doll.
    Injuries,
    /// The indicators and the lists of effects.
    Status,
    /// The map's drawing.
    Map,
    /// The map's marks.
    Marks,
    /// What is not a colour.
    Chrome,
}

/// What a theme generates from: the seed, the background, a scheme and the
/// dials, and the pins kept as written.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    /// Its OKLCH hue anchors the scheme; its lightness and chroma anchor the
    /// candidate ladder.
    pub seed: Rgb,
    /// What every generated colour must read against.
    pub background: Rgb,
    /// The hues.
    pub scheme: Scheme,
    /// A multiplier on the scheme's offsets: low 0.7, medium 1.0, high 1.4.
    pub variance: f64,
    /// The WCAG contrast floor against the background: low 3.0, medium 4.5,
    /// high 7.0.
    pub contrast: f64,
    /// The least `OKLab` distance wanted between two tokens of one group: low
    /// 0.04, medium 0.09, high 0.15.
    pub separation: f64,
    /// The room's name's contrast against its plate: 2.5 reads as a subtle
    /// plate, 7.0 as a hard label.
    pub room_spread: f64,
    /// Tokens kept as written; the rest harmonise round them.
    pub pins: BTreeMap<Token, Rgb>,
}

impl Default for Recipe {
    /// Despana's link blue on its canvas, triadic, every dial at medium.
    fn default() -> Self {
        Self {
            seed: [0x47, 0x7a, 0xb3],
            background: [0x0d, 0x11, 0x15],
            scheme: Scheme::Triadic,
            variance: 1.0,
            contrast: 4.5,
            separation: 0.09,
            room_spread: 2.5,
            pins: BTreeMap::new(),
        }
    }
}

/// A seed that does not parse as a colour would still be one; this is the
/// ladder's start when nothing better is known.
const FALLBACK_SEED: Lch = [0.7, 0.12, 250.0];

fn hue_for(token: Token, recipe: &Recipe, seed_hue: f64) -> f64 {
    match token.role() {
        Role::Free { slot, .. } => {
            let off = recipe.scheme.offsets();
            (seed_hue + off[slot % off.len()] * recipe.variance).rem_euclid(360.0)
        }
        _ => to_lch(token.bare())[2],
    }
}

/// Raise (or lower, on a light background) the lightness until the colour
/// clears the contrast floor against the background. Cheaper and less
/// destructive than desaturating.
fn lift_to_contrast(mut l: f64, c: f64, h: f64, recipe: &Recipe) -> Rgb {
    let mut rgb = from_lch([l, c, h]);
    if contrast(rgb, recipe.background) >= recipe.contrast {
        return rgb;
    }
    let step = if to_lch(recipe.background)[0] < 0.5 {
        0.015
    } else {
        -0.015
    };
    for _ in 0..40 {
        l = (l + step).clamp(0.05, 0.99);
        rgb = from_lch([l, c, h]);
        if contrast(rgb, recipe.background) >= recipe.contrast {
            break;
        }
    }
    rgb
}

/// A ladder of lightness and chroma round a nominal point, each rung lifted
/// to the contrast floor. Tokens sharing a hue separate along lightness,
/// which is what the eye reads first. Nearest the nominal point first.
fn candidates_at(hue: f64, dl: f64, dc: f64, recipe: &Recipe, seed: Lch) -> Vec<Rgb> {
    let mut out = Vec::new();
    for step_l in [0.0, 0.09, -0.09, 0.18, -0.18, 0.27, -0.27] {
        for step_c in [0.0, 0.05, -0.05, 0.10] {
            let l = (seed[0] + dl + step_l).clamp(0.28, 0.94);
            let c = (seed[1] + dc + step_c).clamp(0.015, 0.33);
            let rgb = lift_to_contrast(l, c, hue, recipe);
            if !out.contains(&rgb) {
                out.push(rgb);
            }
        }
    }
    out
}

/// The first candidate (nearest the nominal point) that clears the
/// separation floor and is not lost in the background; else the one
/// farthest from what is used, so two tokens on one hue never come out the
/// same.
fn pick_color(cands: &[Rgb], used: &[Rgb], recipe: &Recipe) -> Rgb {
    let min_dist = |rgb: Rgb| {
        used.iter()
            .map(|u| delta_e(*u, rgb))
            .fold(f64::INFINITY, f64::min)
    };
    let visible = |rgb: &&Rgb| delta_e(**rgb, recipe.background) >= 0.06;
    cands
        .iter()
        .find(|c| visible(c) && min_dist(**c) >= recipe.separation)
        .or_else(|| {
            cands
                .iter()
                .filter(visible)
                .max_by(|a, b| min_dist(**a).total_cmp(&min_dist(**b)))
        })
        .or_else(|| cands.first())
        .copied()
        .unwrap_or(recipe.seed)
}

/// The plate under the room's name: the name's hue, a little of its chroma,
/// the lightness solved by bisection so the pair hits the recipe's spread.
fn plate(name: Rgb, recipe: &Recipe) -> Rgb {
    let rl = to_lch(name);
    let c = (rl[1] * 0.7).clamp(0.02, 0.2);
    let (mut lo, mut hi) = (0.05, (rl[0] - 0.02).max(0.05));
    let mut plate = from_lch([lo, c, rl[2]]);
    for _ in 0..26 {
        let mid = f64::midpoint(lo, hi);
        plate = from_lch([mid, c, rl[2]]);
        if contrast(name, plate) > recipe.room_spread {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    plate
}

/// The whole palette from `recipe`. The same recipe gives the same palette.
#[must_use]
pub fn generate(recipe: &Recipe) -> Palette {
    let seed = to_lch(recipe.seed);
    let seed = if seed[1].is_finite() {
        seed
    } else {
        FALLBACK_SEED
    };
    let background = to_lch(recipe.background);
    let mut palette = Palette::bare();
    let mut used: BTreeMap<Group, Vec<Rgb>> = BTreeMap::new();
    for token in Token::ALL {
        if let Some(pinned) = recipe.pins.get(&token) {
            used.entry(token.group()).or_default().push(*pinned);
            palette.set(token, *pinned);
            continue;
        }
        let (dl, dc) = match token.role() {
            Role::Free { dl, dc, .. } | Role::Anchored { dl, dc } => (dl, dc),
            Role::Surface { dl } => {
                let l = (background[0] + dl).clamp(0.0, 1.0);
                palette.set(token, from_lch([l, background[1], background[2]]));
                continue;
            }
            Role::Plate => {
                palette.set(token, plate(palette.get(Token::RoomName), recipe));
                continue;
            }
            Role::Fixed => continue,
        };
        let cands = candidates_at(hue_for(token, recipe, seed[2]), dl, dc, recipe, seed);
        let group = used.entry(token.group()).or_default();
        let pick = pick_color(&cands, group, recipe);
        group.push(pick);
        palette.set(token, pick);
    }
    palette
}

/// For the editor's swatch explorer: `token`'s lightness and chroma held,
/// its hue turned by noticeable steps round the wheel, so the player
/// chooses between hues rather than shades of one. Empty for a token that
/// is not free.
#[must_use]
pub fn hue_variants(token: Token, recipe: &Recipe) -> Vec<Rgb> {
    let Role::Free { dl, dc, .. } = token.role() else {
        return Vec::new();
    };
    let seed = to_lch(recipe.seed);
    let base_h = hue_for(token, recipe, seed[2]);
    let l = (seed[0] + dl).clamp(0.30, 0.92);
    let c = (seed[1] + dc).clamp(0.03, 0.30);
    let mut out = Vec::new();
    for d in [
        0.0, 30.0, -30.0, 60.0, -60.0, 90.0, -90.0, 140.0, 180.0, 220.0,
    ] {
        let rgb = lift_to_contrast(l, c, (base_h + d).rem_euclid(360.0), recipe);
        if !out.contains(&rgb) {
            out.push(rgb);
        }
    }
    out
}

/// Seeds worth offering from `raw` colours (a theme's own, say): near-greys
/// and colours lost in `background` dropped, perceptual duplicates dropped,
/// the most vivid first, at most `cap`. Every swatch offered must be a safe
/// seed, or choosing by eye is guesswork.
#[must_use]
pub fn seed_swatches(raw: &[Rgb], background: Rgb, cap: usize) -> Vec<Rgb> {
    let pass = |min_chroma: f64| {
        let mut swatches: Vec<(Rgb, f64)> = Vec::new();
        for rgb in raw {
            let [_, chroma, _] = to_lch(*rgb);
            let lost = delta_e(*rgb, background) < 0.15;
            let seen = swatches.iter().any(|(s, _)| delta_e(*s, *rgb) < 0.02);
            if chroma >= min_chroma && !lost && !seen {
                swatches.push((*rgb, chroma));
            }
        }
        swatches
    };
    // An all-grey set has nothing vivid; a grey seed is then coherent, so
    // the chroma floor is relaxed rather than nothing offered.
    let mut swatches = pass(0.04);
    if swatches.is_empty() {
        swatches = pass(0.0);
    }
    swatches.sort_by(|a, b| b.1.total_cmp(&a.1));
    swatches.truncate(cap);
    swatches.into_iter().map(|(rgb, _)| rgb).collect()
}

#[cfg(test)]
mod tests;
