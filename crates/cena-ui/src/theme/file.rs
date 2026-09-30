//! A theme as a file, and the themes Hydra has (`plan/57` §3b): a name, a
//! base it takes what it does not say from, a recipe, and pins. The palette
//! is computed from them when the theme is looked up, never saved.
//!
//! Two themes are built in: **Despana**, the web page's palette, which is
//! what Hydra wore before it had themes with the page's own chrome round it,
//! and **Light**, generated from a light background. A file in the `themes`
//! folder adds one more, holding only what it says.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::shape::{Shape, ShapeFile};
use super::{Palette, Recipe, Rgb, Scheme, Token, generate, hex, parse_hex};

/// What a theme resolves to when it is worn: its palette and its shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outfit {
    /// Every token's colour.
    pub palette: Palette,
    /// The corners, edges, density, scrollbar and shadows.
    pub shape: Shape,
}

impl Default for Outfit {
    /// Hydra before it had themes: the bare palette, the default shape.
    fn default() -> Self {
        Self {
            palette: Palette::bare(),
            shape: Shape::default(),
        }
    }
}

/// What one character chose for its own window (`plan/57` §3c): a theme in
/// place of Hydra's, and an accent pinned over whichever it wears. The
/// `theme` section of its settings file, which the binary's *Theme* page
/// writes and the GUI reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Chosen {
    /// The theme's name; none, Hydra's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// The accent, `#rrggbb`; none, the theme's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

impl Chosen {
    /// The section's name in the settings file.
    pub const SECTION: &str = "theme";
}

/// One theme: what it says, over its base.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// What the player calls it.
    pub name: String,
    /// The theme it takes what it does not say from; none, the defaults.
    pub base: Option<String>,
    /// The parts of the recipe it sets.
    pub recipe: RecipeFile,
    /// Tokens kept as written.
    pub pins: BTreeMap<Token, Rgb>,
    /// The parts of the shape it sets.
    pub shape: ShapeFile,
}

/// The recipe as a file writes it: a dial left out is the base's.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeFile {
    /// The seed colour, `#rrggbb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    /// The background, `#rrggbb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// The scheme by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    /// The hue spread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variance: Option<f64>,
    /// The contrast floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f64>,
    /// The least distance between two tokens of a group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separation: Option<f64>,
    /// The room's name's contrast on its plate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_spread: Option<f64>,
}

/// The file's whole form.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base: Option<String>,
    #[serde(default, skip_serializing_if = "RecipeFile::is_empty")]
    recipe: RecipeFile,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pins: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "ShapeFile::is_empty")]
    shape: ShapeFile,
}

impl RecipeFile {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// `recipe` with what this file sets laid over it.
    ///
    /// # Errors
    ///
    /// A colour or a scheme that does not read, named.
    fn over(&self, mut recipe: Recipe) -> Result<Recipe, String> {
        if let Some(seed) = &self.seed {
            recipe.seed =
                parse_hex(seed).ok_or_else(|| format!("seed `{seed}` is not a colour"))?;
        }
        if let Some(background) = &self.background {
            recipe.background = parse_hex(background)
                .ok_or_else(|| format!("background `{background}` is not a colour"))?;
        }
        if let Some(scheme) = &self.scheme {
            recipe.scheme =
                Scheme::parse(scheme).ok_or_else(|| format!("`{scheme}` is not a scheme"))?;
        }
        recipe.variance = self.variance.unwrap_or(recipe.variance);
        recipe.contrast = self.contrast.unwrap_or(recipe.contrast);
        recipe.separation = self.separation.unwrap_or(recipe.separation);
        recipe.room_spread = self.room_spread.unwrap_or(recipe.room_spread);
        Ok(recipe)
    }
}

impl Theme {
    /// The name of the theme worn unless one is chosen.
    pub const DEFAULT: &str = "Despana";
    /// The name of the built-in light theme.
    pub const LIGHT: &str = "Light";

    /// **Despana**: the web page's palette (`crates/cena-web/assets/style.css`)
    /// round the colours Hydra drew before it had themes, every one pinned.
    #[must_use]
    pub fn despana() -> Self {
        let mut pins: BTreeMap<Token, Rgb> =
            Token::ALL.into_iter().map(|t| (t, t.bare())).collect();
        for (token, css) in [
            (Token::Canvas, [0x0d, 0x11, 0x15]),
            (Token::Surface, [0x18, 0x1e, 0x24]),
            (Token::Raised, [0x20, 0x27, 0x2e]),
            (Token::Inset, [0x11, 0x16, 0x1b]),
            (Token::Line, [0x35, 0x40, 0x4a]),
            (Token::LineStrong, [0x76, 0x5f, 0x39]),
            (Token::Text, [0xdd, 0xdc, 0xd7]),
            (Token::Muted, [0xa4, 0xae, 0xb7]),
            (Token::Selection, [0x3b, 0x4a, 0x5a]),
        ] {
            pins.insert(token, css);
        }
        Self {
            name: Self::DEFAULT.to_owned(),
            base: None,
            recipe: RecipeFile {
                seed: Some(hex([0xd7, 0xad, 0x63])),
                background: Some(hex([0x0d, 0x11, 0x15])),
                scheme: Some(Scheme::Compound.name().to_owned()),
                ..RecipeFile::default()
            },
            pins,
            shape: ShapeFile::default(),
        }
    }

    /// **Light**: generated from Despana's link blue on a warm white.
    #[must_use]
    pub fn light() -> Self {
        Self {
            name: Self::LIGHT.to_owned(),
            base: None,
            recipe: RecipeFile {
                seed: Some(hex([0x47, 0x7a, 0xb3])),
                background: Some(hex([0xf4, 0xf1, 0xea])),
                scheme: Some(Scheme::Triadic.name().to_owned()),
                ..RecipeFile::default()
            },
            pins: BTreeMap::new(),
            shape: ShapeFile::default(),
        }
    }

    /// Read from `text`, a theme file; `stem` names it when it does not
    /// name itself.
    ///
    /// # Errors
    ///
    /// Why it does not read.
    pub fn parse(text: &str, stem: &str) -> Result<Self, String> {
        let file: File = toml::from_str(text).map_err(|why| why.to_string())?;
        let mut pins = BTreeMap::new();
        for (name, colour) in &file.pins {
            let token = Token::named(name).ok_or_else(|| format!("`{name}` is not a token"))?;
            let rgb =
                parse_hex(colour).ok_or_else(|| format!("{name} = `{colour}` is not a colour"))?;
            pins.insert(token, rgb);
        }
        file.recipe.over(Recipe::default())?;
        Ok(Self {
            name: file.name.unwrap_or_else(|| stem.to_owned()),
            base: file.base,
            recipe: file.recipe,
            pins,
            shape: file.shape,
        })
    }

    /// As a theme file writes it.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let file = File {
            name: Some(self.name.clone()),
            base: self.base.clone(),
            recipe: self.recipe.clone(),
            pins: self
                .pins
                .iter()
                .map(|(token, rgb)| (token.name().to_owned(), hex(*rgb)))
                .collect(),
            shape: self.shape.clone(),
        };
        toml::to_string(&file).unwrap_or_default()
    }
}

/// The theme Hydra wears, as its own settings choose it: the `theme` key
/// of `window.toml` in `data`, the data folder, else the default. The file
/// is the GUI's to write (`cena-gui`'s `own.rs`); the web reads the one
/// key here, so a page opened later wears what the window does.
#[must_use]
pub fn chosen_for_hydra(data: &Path) -> String {
    std::fs::read_to_string(data.join("window.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|table| table.get("theme")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| Theme::DEFAULT.to_owned())
}

/// The themes Hydra has: the built-ins, and each file in the `themes`
/// folder.
#[derive(Clone, Debug, Default)]
pub struct Themes {
    themes: Vec<Theme>,
    /// Files that did not read, each with why.
    pub problems: Vec<String>,
}

impl Themes {
    /// The built-ins alone.
    #[must_use]
    pub fn built_in() -> Self {
        Self {
            themes: vec![Theme::despana(), Theme::light()],
            problems: Vec::new(),
        }
    }

    /// The built-ins and every `.toml` in `folder`. A file named as a
    /// built-in is is read after it and wins.
    #[must_use]
    pub fn load(folder: &Path) -> Self {
        let mut themes = Self::built_in();
        let Ok(entries) = std::fs::read_dir(folder) else {
            return themes;
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        files.sort();
        for path in files {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let read = std::fs::read_to_string(&path)
                .map_err(|why| why.to_string())
                .and_then(|text| Theme::parse(&text, &stem));
            match read {
                Ok(theme) => themes.add(theme),
                Err(why) => themes.problems.push(format!("{}: {why}", path.display())),
            }
        }
        themes
    }

    /// Add `theme`, in place of one of its name.
    pub fn add(&mut self, theme: Theme) {
        self.themes
            .retain(|t| !t.name.eq_ignore_ascii_case(&theme.name));
        self.themes.push(theme);
    }

    /// Every theme's name, the built-ins first.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.themes.iter().map(|t| t.name.clone()).collect()
    }

    /// The theme named `name`, ignoring case.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// The recipe and pins of the theme named `name`, its base's under it,
    /// and its shape the same way.
    ///
    /// # Errors
    ///
    /// No theme of that name, a base that is missing or circular, or a
    /// value that does not read.
    pub fn resolve(&self, name: &str) -> Result<(Recipe, Shape), String> {
        let mut chain: Vec<&Theme> = Vec::new();
        let mut next = Some(name);
        while let Some(name) = next {
            if chain.len() >= 8 {
                return Err(format!("`{name}`'s bases go round in a circle"));
            }
            let theme = self
                .get(name)
                .ok_or_else(|| format!("there is no theme `{name}`"))?;
            next = theme.base.as_deref();
            chain.push(theme);
        }
        let mut recipe = Recipe::default();
        let mut shape = Shape::default();
        for theme in chain.iter().rev() {
            recipe = theme
                .recipe
                .over(recipe)
                .map_err(|why| format!("{}: {why}", theme.name))?;
            recipe.pins.extend(theme.pins.iter().map(|(t, c)| (*t, *c)));
            shape = theme.shape.over(shape);
        }
        Ok((recipe, shape))
    }

    /// What the theme named `name` is worn as.
    ///
    /// # Errors
    ///
    /// As [`Themes::resolve`].
    pub fn outfit(&self, name: &str) -> Result<Outfit, String> {
        let (recipe, shape) = self.resolve(name)?;
        Ok(Outfit {
            palette: generate(&recipe),
            shape,
        })
    }

    /// What a character's window wears: `chosen`'s theme, or `hydras`,
    /// with `chosen`'s accent pinned over it.
    ///
    /// # Errors
    ///
    /// As [`Themes::resolve`], or an accent that is not a colour.
    pub fn outfit_for(&self, hydras: &str, chosen: &Chosen) -> Result<Outfit, String> {
        let (mut recipe, shape) = self.resolve(chosen.theme.as_deref().unwrap_or(hydras))?;
        if let Some(accent) = &chosen.accent {
            let rgb =
                parse_hex(accent).ok_or_else(|| format!("accent `{accent}` is not a colour"))?;
            recipe.pins.insert(Token::Accent, rgb);
        }
        Ok(Outfit {
            palette: generate(&recipe),
            shape,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn despana_is_what_hydra_drew_with_the_pages_chrome_round_it() {
        let themes = Themes::built_in();
        let palette = themes.outfit(Theme::DEFAULT).unwrap().palette;
        assert_eq!(palette.get(Token::Health), Token::Health.bare());
        assert_eq!(palette.get(Token::Accent), Token::Accent.bare());
        assert_eq!(palette.get(Token::Canvas), [0x0d, 0x11, 0x15]);
        assert_eq!(palette.get(Token::Text), [0xdd, 0xdc, 0xd7]);
    }

    #[test]
    fn light_is_light() {
        let palette = Themes::built_in().outfit(Theme::LIGHT).unwrap().palette;
        let [l, ..] = super::super::oklch::to_lch(palette.get(Token::Canvas));
        assert!(l > 0.9, "canvas lightness {l}");
        let [l, ..] = super::super::oklch::to_lch(palette.get(Token::Text));
        assert!(l < 0.5, "text lightness {l}");
    }

    #[test]
    fn a_file_says_only_what_it_changes_over_its_base() {
        let theme = Theme::parse(
            "base = \"Despana\"\n[recipe]\nseed = \"#c9733a\"\n[pins]\nhealth = \"#112233\"\n[shape]\ncorner = 0\n",
            "ember",
        )
        .unwrap();
        assert_eq!(theme.name, "ember");
        let mut themes = Themes::built_in();
        themes.add(theme.clone());
        let (recipe, shape) = themes.resolve("Ember").unwrap();
        assert_eq!(shape.corner, 0);
        assert!(shape.shadows, "the base's");
        assert_eq!(recipe.seed, [0xc9, 0x73, 0x3a]);
        assert_eq!(recipe.background, [0x0d, 0x11, 0x15], "the base's");
        assert_eq!(recipe.scheme, Scheme::Compound, "the base's");
        assert_eq!(recipe.pins.get(&Token::Health), Some(&[0x11, 0x22, 0x33]));
        assert_eq!(
            recipe.pins.get(&Token::Mana),
            Some(&Token::Mana.bare()),
            "the base's pin"
        );
        // Round trip.
        let again = Theme::parse(&theme.to_toml(), "x").unwrap();
        assert_eq!(again, theme);
    }

    #[test]
    fn what_does_not_read_is_said() {
        assert!(
            Theme::parse("[pins]\nhealt = \"#112233\"\n", "x")
                .unwrap_err()
                .contains("healt")
        );
        assert!(
            Theme::parse("[recipe]\nseed = \"red\"\n", "x")
                .unwrap_err()
                .contains("red")
        );
        assert!(Theme::parse("volume = 3\n", "x").is_err());
        let mut themes = Themes::built_in();
        assert!(themes.resolve("Nope").unwrap_err().contains("Nope"));
        themes.add(Theme {
            name: "a".to_owned(),
            base: Some("b".to_owned()),
            recipe: RecipeFile::default(),
            pins: BTreeMap::new(),
            shape: ShapeFile::default(),
        });
        themes.add(Theme {
            name: "b".to_owned(),
            base: Some("a".to_owned()),
            recipe: RecipeFile::default(),
            pins: BTreeMap::new(),
            shape: ShapeFile::default(),
        });
        assert!(themes.resolve("a").unwrap_err().contains("circle"));
    }

    #[test]
    fn hydras_choice_is_read_from_its_own_file() {
        let dir = std::env::temp_dir().join(format!("cena-chosen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(chosen_for_hydra(&dir), Theme::DEFAULT, "no file");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("window.toml"),
            "card_width = 500.0\ntheme = \"Light\"\n",
        )
        .unwrap();
        assert_eq!(chosen_for_hydra(&dir), "Light");
        std::fs::write(dir.join("window.toml"), "theme = 3\n").unwrap();
        assert_eq!(chosen_for_hydra(&dir), Theme::DEFAULT, "not a name");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_of_files_is_read_and_a_bad_one_reported() {
        let dir = std::env::temp_dir().join(format!("cena-themes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ember.toml"), "base = \"Light\"\n").unwrap();
        std::fs::write(dir.join("bad.toml"), "name = 3\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a theme").unwrap();
        let themes = Themes::load(&dir);
        assert_eq!(themes.names(), ["Despana", "Light", "ember"]);
        assert_eq!(themes.problems.len(), 1, "{:?}", themes.problems);
        assert!(themes.problems[0].contains("bad.toml"));
        assert!(Themes::load(&dir.join("missing")).get("Light").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
