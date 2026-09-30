//! `plan/57` §4: **no colour literal outside the GUI's theme module.**
//!
//! Every colour Hydra draws is a token of the palette
//! (`crates/cena-ui/src/theme.rs`), and `crates/cena-gui/src/theme.rs` is the
//! one place that turns a token into egui's `Color32`. A widget that writes
//! `Color32::from_rgb(...)` draws a colour no theme can change, which is how
//! `cena-gui` came to hold 76 of them in 14 files before the sweep of
//! `plan/57` step 0. Written with the sweep, not after (`plan/05` §0).
//!
//! # What it checks
//!
//! Every code line (comments stripped) of `crates/cena-gui/src/` for
//! `Color32`'s constructors from numbers and its named colours, outside the
//! theme module and outside test code. Three names are not colours and are
//! allowed anywhere: `WHITE` as an image's tint (drawn as it is),
//! `TRANSPARENT` (no colour) and `PLACEHOLDER` (egui's "the caller's colour
//! goes here").
//!
//! # What it does not
//!
//! `Color32::from_hex(text)` and `Color32::from_rgb(r, g, b)` over a saved
//! or a trigger's colour are conversions, not literals, and a lexical scan
//! cannot tell `from_rgb(0xcd, 0x4d, 0x4d)` from `from_rgb(red, green, blue)`
//! without reading the arguments. So the constructors are banned whatever
//! they take, and a conversion goes through `theme::rgb`. Test code (from a
//! file's `#[cfg(test)]` on, and the `tests` folders) asserts colours by
//! their numbers and is skipped.

use cena_arch_tests::harness::{relative, workspace_sources};
use cena_arch_tests::lexical::code_lines;
use std::path::PathBuf;

/// The one file that may write a colour.
const THEME_MODULE: &str = "crates/cena-gui/src/theme.rs";

/// Constructors that make a colour from numbers.
const CONSTRUCTORS: &[&str] = &[
    "Color32::from_rgb(",
    "Color32::from_rgb_additive(",
    "Color32::from_rgba_premultiplied(",
    "Color32::from_rgba_unmultiplied(",
    "Color32::from_gray(",
    "Color32::from_black_alpha(",
    "Color32::from_white_alpha(",
    "Color32::from_additive_luminance(",
    "hex_color!(",
];

/// Named colours that mean no colour, allowed anywhere.
const NOT_COLOURS: &[&str] = &["WHITE", "TRANSPARENT", "PLACEHOLDER"];

/// Whether `path` is test code by its place: a `tests.rs`, or under a
/// `tests` folder.
fn test_file(rel: &str) -> bool {
    rel.ends_with("/tests.rs") || rel.contains("/tests/")
}

/// A `Color32::NAME` on `line` that is a colour, if any.
fn named_colour(line: &str) -> Option<String> {
    let mut rest = line;
    while let Some(at) = rest.find("Color32::") {
        let after = &rest[at + "Color32::".len()..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .collect();
        let is_constant = !name.is_empty()
            && name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && !after[name.len()..].starts_with(|c: char| c.is_ascii_alphanumeric());
        if is_constant && !NOT_COLOURS.contains(&name.as_str()) {
            return Some(name);
        }
        rest = after;
    }
    None
}

/// Every colour literal in `sources` that the rule does not allow.
fn colour_literals(sources: &[(PathBuf, String)]) -> Vec<String> {
    let mut found = Vec::new();
    for (path, text) in sources {
        let rel = relative(path);
        if !rel.starts_with("crates/cena-gui/src/") || rel == THEME_MODULE || test_file(&rel) {
            continue;
        }
        for (at, line) in code_lines(text).iter().enumerate() {
            if line.contains("#[cfg(test)]") {
                break;
            }
            let constructor = CONSTRUCTORS.iter().find(|needle| line.contains(*needle));
            let named = named_colour(line);
            if let Some(what) = constructor.map(|c| (*c).to_owned()).or(named) {
                found.push(format!("{rel}:{}: {what}", at + 1));
            }
        }
    }
    found
}

#[test]
fn no_colour_literal_outside_the_theme_module() {
    let sources = workspace_sources();
    assert!(
        sources
            .iter()
            .filter(|(p, _)| relative(p).starts_with("crates/cena-gui/src/"))
            .count()
            > 20,
        "the scan is vacuous"
    );
    let found = colour_literals(&sources);
    assert!(
        found.is_empty(),
        "A colour written in cena-gui is one no theme can change (plan/57 §4). \
         Name what it means as a token in crates/cena-ui/src/theme.rs and ask \
         theme::color(ctx, Token::...) for it; a saved or a trigger's colour \
         goes through theme::rgb.\n{}",
        found.join("\n")
    );
}

/// The scan's own cases: what it must find and what it must let by.
#[test]
fn the_scan_finds_a_literal_and_lets_the_allowed_by() {
    let widget =
        cena_arch_tests::harness::workspace_root().join("crates/cena-gui/src/widget/fixture.rs");
    let theme = cena_arch_tests::harness::workspace_root().join(THEME_MODULE);
    let tests =
        cena_arch_tests::harness::workspace_root().join("crates/cena-gui/src/widget/tests.rs");
    let found = |path: &PathBuf, code: &str| colour_literals(&[(path.clone(), code.to_owned())]);
    // Found: a constructor, a named colour, a colour built from variables.
    assert_eq!(
        found(&widget, "let c = Color32::from_rgb(1, 2, 3);\n").len(),
        1
    );
    assert_eq!(found(&widget, "let c = egui::Color32::BLACK;\n").len(), 1);
    assert_eq!(
        found(&widget, "let c = Color32::YELLOW.gamma_multiply(0.5);\n").len(),
        1
    );
    assert_eq!(
        found(&widget, "Color32::from_rgb(red, green, blue)\n").len(),
        1
    );
    assert_eq!(
        found(&widget, "let c = Color32::from_black_alpha(170);\n").len(),
        1
    );
    // Let by: the theme module, test code, a comment, the three non-colours,
    // a conversion from text, and a type or function that is not a constant.
    assert!(found(&theme, "let c = Color32::from_rgb(1, 2, 3);\n").is_empty());
    assert!(found(&tests, "let c = Color32::from_rgb(1, 2, 3);\n").is_empty());
    assert!(
        found(
            &widget,
            "#[cfg(test)]\nmod tests { let c = Color32::from_rgb(1, 2, 3); }\n"
        )
        .is_empty()
    );
    assert!(found(&widget, "// Color32::from_rgb(1, 2, 3) once lived here\n").is_empty());
    assert!(found(&widget, "painter.image(id, rect, uv, Color32::WHITE);\n").is_empty());
    assert!(found(&widget, "let c = Color32::TRANSPARENT;\n").is_empty());
    assert!(found(&widget, "let c = Color32::PLACEHOLDER;\n").is_empty());
    assert!(found(&widget, "let c = Color32::from_hex(text).ok();\n").is_empty());
    assert!(found(&widget, "let c: Color32 = theme::color(ctx, T::Health);\n").is_empty());
}
