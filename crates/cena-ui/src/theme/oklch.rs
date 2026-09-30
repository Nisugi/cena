//! Colour maths for the harmony generator: sRGB to `OKLab` and OKLCH and
//! back (Ottosson's transform), a fit into the sRGB gamut that keeps hue and
//! lightness, WCAG contrast and a perceptual distance. Niffy's, from
//! `vellum-palette-harmony.html` through `VellumFE`'s `core/harmony.rs`;
//! over [`Rgb`] rather than hex strings, since that is Hydra's colour.
//!
//! Equal OKLCH lightness reads equally bright across hues, which is why the
//! generator works in it: a palette built there keeps every role readable.

use super::Rgb;

/// A colour in OKLCH: lightness 0 to 1, chroma 0 to about 0.33, hue in
/// degrees.
pub type Lch = [f64; 3];

/// A colour in sRGB, each channel 0 to 1.
type Srgb = [f64; 3];

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn to_srgb([red, green, blue]: Rgb) -> Srgb {
    [
        f64::from(red) / 255.0,
        f64::from(green) / 255.0,
        f64::from(blue) / 255.0,
    ]
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0..=1 and rounded, so the value is a byte"
)]
fn channel(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn to_rgb(srgb: Srgb) -> Rgb {
    [channel(srgb[0]), channel(srgb[1]), channel(srgb[2])]
}

/// Ottosson's transform: linear sRGB to the cone response `lms`, cube
/// rooted, then to `Lab`.
fn rgb_to_oklab(rgb: Srgb) -> [f64; 3] {
    let red = srgb_to_linear(rgb[0]);
    let green = srgb_to_linear(rgb[1]);
    let blue = srgb_to_linear(rgb[2]);
    let long = (0.412_221_470_8 * red + 0.536_332_536_3 * green + 0.051_445_992_9 * blue).cbrt();
    let medium = (0.211_903_498_2 * red + 0.680_699_545_1 * green + 0.107_396_956_6 * blue).cbrt();
    let short = (0.088_302_461_9 * red + 0.281_718_837_6 * green + 0.629_978_700_5 * blue).cbrt();
    [
        0.210_454_255_3 * long + 0.793_617_785 * medium - 0.004_072_046_8 * short,
        1.977_998_495_1 * long - 2.428_592_205 * medium + 0.450_593_709_9 * short,
        0.025_904_037_1 * long + 0.782_771_766_2 * medium - 0.808_675_766 * short,
    ]
}

/// The transform back: `Lab` to the cone response, cubed, to linear sRGB.
fn oklab_to_rgb(lab: [f64; 3]) -> Srgb {
    let [light, a_axis, b_axis] = lab;
    let long = (light + 0.396_337_777_4 * a_axis + 0.215_803_757_3 * b_axis).powi(3);
    let medium = (light - 0.105_561_345_8 * a_axis - 0.063_854_172_8 * b_axis).powi(3);
    let short = (light - 0.089_484_177_5 * a_axis - 1.291_485_548 * b_axis).powi(3);
    [
        linear_to_srgb(4.076_741_662_1 * long - 3.307_711_591_3 * medium + 0.230_969_929_2 * short),
        linear_to_srgb(
            -1.268_438_004_6 * long + 2.609_757_401_1 * medium - 0.341_319_396_5 * short,
        ),
        linear_to_srgb(-0.004_196_086_3 * long - 0.703_418_614_7 * medium + 1.707_614_701 * short),
    ]
}

fn lab_to_lch(lab: [f64; 3]) -> Lch {
    let [l, a, b] = lab;
    [l, a.hypot(b), (b.atan2(a).to_degrees() + 360.0) % 360.0]
}

fn lch_to_lab(lch: Lch) -> [f64; 3] {
    let [l, c, h] = lch;
    [l, c * h.to_radians().cos(), c * h.to_radians().sin()]
}

fn in_gamut(rgb: Srgb) -> bool {
    rgb.iter().all(|v| (-0.001..=1.001).contains(v))
}

/// `rgb` in OKLCH.
#[must_use]
pub fn to_lch(rgb: Rgb) -> Lch {
    lab_to_lch(rgb_to_oklab(to_srgb(rgb)))
}

/// `lch` as a colour. A hue turned often lands outside sRGB; the chroma is
/// then reduced by bisection until it fits, keeping the hue and the
/// lightness, which are what the eye keys on.
#[must_use]
pub fn from_lch(lch: Lch) -> Rgb {
    let [l, c, h] = lch;
    let rgb = oklab_to_rgb(lch_to_lab([l, c, h]));
    if in_gamut(rgb) {
        return to_rgb(rgb);
    }
    let (mut lo, mut hi) = (0.0, c);
    for _ in 0..24 {
        let mid = f64::midpoint(lo, hi);
        if in_gamut(oklab_to_rgb(lch_to_lab([l, mid, h]))) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    to_rgb(oklab_to_rgb(lch_to_lab([l, lo, h])))
}

/// WCAG 2.x relative-luminance contrast ratio between two colours, 1 to 21.
#[must_use]
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let lum = |rgb: Rgb| {
        let [r, g, b] = to_srgb(rgb);
        0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
    };
    let (l1, l2) = (lum(a), lum(b));
    (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
}

/// Perceptual distance: Euclidean in `OKLab`. About 0.02 is barely different;
/// over 0.1 is clearly distinct.
#[must_use]
pub fn delta_e(a: Rgb, b: Rgb) -> f64 {
    let la = rgb_to_oklab(to_srgb(a));
    let lb = rgb_to_oklab(to_srgb(b));
    ((la[0] - lb[0]).powi(2) + (la[1] - lb[1]).powi(2) + (la[2] - lb[2]).powi(2)).sqrt()
}

/// The shorter way round the hue circle between two hues, in degrees.
#[must_use]
pub fn hue_distance(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::parse_hex;

    fn hex(text: &str) -> Rgb {
        parse_hex(text).unwrap()
    }

    #[test]
    fn oklch_roundtrip_stays_close() {
        for text in [
            "#ff0000", "#00ff00", "#0000ff", "#808080", "#bf616a", "#123456",
        ] {
            let rgb = hex(text);
            let back = from_lch(to_lch(rgb));
            assert!(delta_e(rgb, back) < 0.01, "{text} -> {back:?} drifted");
        }
    }

    #[test]
    fn known_oklch_values() {
        // White: L=1, C~0. Black: L=0.
        let white = to_lch([0xff, 0xff, 0xff]);
        assert!(
            (white[0] - 1.0).abs() < 0.01 && white[1] < 0.01,
            "{white:?}"
        );
        let black = to_lch([0, 0, 0]);
        assert!(black[0].abs() < 0.01, "{black:?}");
        // sRGB red: Ottosson's reference gives L~0.628, C~0.258, h~29.2.
        let red = to_lch([0xff, 0, 0]);
        assert!((red[0] - 0.628).abs() < 0.01, "{red:?}");
        assert!((red[1] - 0.258).abs() < 0.01, "{red:?}");
        assert!((red[2] - 29.23).abs() < 1.0, "{red:?}");
    }

    #[test]
    fn gamut_fit_preserves_hue_and_lightness() {
        // Chroma 0.4 at this hue is far outside sRGB; the fit must land in
        // gamut with hue and lightness intact.
        let lch = to_lch(from_lch([0.6, 0.4, 145.0]));
        assert!((lch[0] - 0.6).abs() < 0.02, "lightness held: {lch:?}");
        assert!((lch[2] - 145.0).abs() < 2.0, "hue held: {lch:?}");
    }

    #[test]
    fn wcag_contrast_known_values() {
        assert!((contrast([0xff; 3], [0; 3]) - 21.0).abs() < 0.01);
        assert!((contrast([0; 3], [0xff; 3]) - 21.0).abs() < 0.01);
        assert!((contrast([0x77; 3], [0x77; 3]) - 1.0).abs() < 0.01);
    }

    #[test]
    fn hue_distance_goes_the_short_way_round() {
        assert!((hue_distance(10.0, 350.0) - 20.0).abs() < 1e-9);
        assert!((hue_distance(350.0, 10.0) - 20.0).abs() < 1e-9);
        assert!((hue_distance(90.0, 270.0) - 180.0).abs() < 1e-9);
    }
}
