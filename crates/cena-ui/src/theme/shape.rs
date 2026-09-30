//! A theme's shape (`plan/57` §3, step 4): the corners, the edges, how
//! roomy the controls are, the scrollbar's width, and whether windows cast
//! shadows. What egui's style carries to every stock control at once, and
//! Hydra's own drawing reads for its corners. A button drawn differently,
//! not just shaped, is art, a later plan.

use serde::{Deserialize, Serialize};

/// How roomy the controls are: the gaps between them and the padding in
/// them, as a multiple of egui's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Closer than egui's: six tenths of its gaps.
    Tight,
    /// egui's own.
    #[default]
    Normal,
    /// Half again egui's gaps.
    Roomy,
}

impl Density {
    /// Every density, in order.
    pub const ALL: [Density; 3] = [Density::Tight, Density::Normal, Density::Roomy];

    /// What egui's gaps and padding are multiplied by.
    #[must_use]
    pub const fn factor(self) -> f32 {
        match self {
            Density::Tight => 0.6,
            Density::Normal => 1.0,
            Density::Roomy => 1.5,
        }
    }

    /// Its name, as a theme file writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Density::Tight => "tight",
            Density::Normal => "normal",
            Density::Roomy => "roomy",
        }
    }
}

/// A theme's shape, whole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    /// A control's corner radius, in points; a window's is twice it.
    pub corner: u8,
    /// An edge's width, in points.
    pub stroke: f32,
    /// How roomy the controls are.
    pub density: Density,
    /// The scrollbar's width, in points.
    pub scrollbar: f32,
    /// Whether windows and menus cast shadows.
    pub shadows: bool,
}

impl Default for Shape {
    /// Hydra's before it had themes: corners of 3, edges of 1, egui's own
    /// gaps and scrollbar, shadows.
    fn default() -> Self {
        Self {
            corner: 3,
            stroke: 1.0,
            density: Density::Normal,
            scrollbar: 6.0,
            shadows: true,
        }
    }
}

/// The shape as a file writes it: a part left out is the base's.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeFile {
    /// The corner radius.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner: Option<u8>,
    /// The edge width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<f32>,
    /// The density, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<Density>,
    /// The scrollbar's width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrollbar: Option<f32>,
    /// Shadows on or off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadows: Option<bool>,
}

impl ShapeFile {
    /// Whether it sets nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// What `shape` says that `base` does not, as a file writes it.
    #[must_use]
    pub fn differing(shape: &Shape, base: &Shape) -> Self {
        let differs = |a: f32, b: f32| ((a - b).abs() > f32::EPSILON).then_some(a);
        Self {
            corner: (shape.corner != base.corner).then_some(shape.corner),
            stroke: differs(shape.stroke, base.stroke),
            density: (shape.density != base.density).then_some(shape.density),
            scrollbar: differs(shape.scrollbar, base.scrollbar),
            shadows: (shape.shadows != base.shadows).then_some(shape.shadows),
        }
    }

    /// `shape` with what this file sets laid over it.
    #[must_use]
    pub fn over(&self, shape: Shape) -> Shape {
        Shape {
            corner: self.corner.unwrap_or(shape.corner).min(32),
            stroke: self.stroke.unwrap_or(shape.stroke).clamp(0.0, 8.0),
            density: self.density.unwrap_or(shape.density),
            scrollbar: self.scrollbar.unwrap_or(shape.scrollbar).clamp(2.0, 32.0),
            shadows: self.shadows.unwrap_or(shape.shadows),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_sets_what_it_says_and_keeps_the_rest_within_bounds() {
        let file: ShapeFile =
            toml::from_str("corner = 40\ndensity = \"roomy\"\nscrollbar = 1\n").unwrap();
        let shape = file.over(Shape::default());
        assert_eq!(shape.corner, 32, "at most");
        assert_eq!(shape.density, Density::Roomy);
        assert!((shape.scrollbar - 2.0).abs() < f32::EPSILON, "at least");
        assert!((shape.stroke - 1.0).abs() < f32::EPSILON, "the base's");
        assert!(shape.shadows);
        assert!(toml::from_str::<ShapeFile>("density = \"loose\"\n").is_err());
        assert!(toml::from_str::<ShapeFile>("bevel = 2\n").is_err());
        assert!(ShapeFile::default().is_empty());
    }
}
