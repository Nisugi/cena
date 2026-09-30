//! `;theme`: the themes on the command line (`plan/57` §6 item 4, steps 3
//! and 8).
//!
//! - `theme list`: every theme there is, and the one Hydra wears.
//! - `theme mine <name>` and `theme mine off`: this character's own theme,
//!   worn by its play window in place of Hydra's; `theme accent <#rrggbb>`
//!   and `theme accent off`: its own accent. Both through the *Theme* page's
//!   writer ([`crate::theme_page`]).
//! - `theme import <file> [as <name>]`: a Wrayth settings file's `<presets>`
//!   as a theme in Hydra's `themes` folder, over Despana, named for the
//!   file unless said; a theme already there is never written over.
//!
//! Hydra's own theme is chosen on the *Window* page, which is the window's
//! file's one writer; this command does not reach it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cena_session::SessionHandle;
use cena_session::command::claimant::Claimed;
use cena_session::notice::{Notice, NoticeKind};
use cena_ui::theme::{Theme, Themes};

use crate::commands::Commands;

/// What `;theme help` says.
const HELP: &str = "theme list   every theme, and the one Hydra wears (chosen on the Window page). \
                    theme mine <name> | off   this character's own theme, for its window. \
                    theme accent <#rrggbb> | off   its own accent colour. \
                    theme import <file> [as <name>]   a Wrayth settings file's presets as a theme.";

/// What a line of `;theme` asks.
#[derive(Debug, PartialEq, Eq)]
enum Asked {
    Help,
    List,
    Mine(Option<String>),
    Accent(Option<String>),
    Import { file: PathBuf, name: Option<String> },
}

/// `line`, without its symbol, as `;theme`: `None` when it is not one, an
/// error when it says what it cannot.
fn parse(line: &str) -> Option<Result<Asked, String>> {
    let line = line.trim();
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    if !word.eq_ignore_ascii_case("theme") {
        return None;
    }
    let rest = rest.trim();
    let (verb, rest) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let rest = rest.trim();
    let off = |what: &str| {
        if what.eq_ignore_ascii_case("off") {
            None
        } else {
            Some(what.to_owned())
        }
    };
    Some(match verb.to_ascii_lowercase().as_str() {
        "" | "help" => Ok(Asked::Help),
        "list" => Ok(Asked::List),
        "mine" if rest.is_empty() => {
            Err("Theme: say which, as `theme mine <name>`, or `theme mine off`.".to_owned())
        }
        "mine" => Ok(Asked::Mine(off(rest))),
        "accent" if rest.is_empty() => Err(
            "Theme: say which colour, as `theme accent #c9733a`, or `theme accent off`.".to_owned(),
        ),
        "accent" => Ok(Asked::Accent(off(rest))),
        "import" if rest.is_empty() => {
            Err("Theme: say which file, as `theme import <file>`.".to_owned())
        }
        "import" => {
            let (file, name) = match rest.rsplit_once(" as ") {
                Some((file, name)) if !name.trim().is_empty() => {
                    (file, Some(name.trim().to_owned()))
                }
                _ => (rest, None),
            };
            Ok(Asked::Import {
                file: PathBuf::from(file.trim().trim_matches('"')),
                name,
            })
        }
        other => Err(format!("Theme: there is no `theme {other}`. {HELP}")),
    })
}

/// Register `;theme` on `character`'s command line (`GAME:Name`), its
/// files in `dir`, the data folder.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, dir: &Path, character: &str) {
    let told = handle.clone();
    let (dir, character) = (dir.to_owned(), character.to_owned());
    commands.theme(Arc::new(move |line: &str| {
        let asked = match parse(line)? {
            Ok(asked) => asked,
            Err(why) => {
                told.say(Notice::line(NoticeKind::Error, why).answering());
                return Some(Claimed::Done);
            }
        };
        let said = match asked {
            Asked::Help => Ok(HELP.to_owned()),
            Asked::List => Ok(list(&dir)),
            Asked::Mine(name) => own(&dir, &character, "theme", name.as_deref()),
            Asked::Accent(colour) => own(&dir, &character, "accent", colour.as_deref()),
            Asked::Import { file, name } => import(&dir, &file, name.as_deref()),
        };
        let notice = match said {
            Ok(said) => Notice::line(NoticeKind::Info, said),
            Err(why) => Notice::line(NoticeKind::Error, why),
        };
        told.say(notice.answering());
        Some(Claimed::Done)
    }));
}

/// Every theme, and the one Hydra wears.
fn list(dir: &Path) -> String {
    let themes = Themes::load(&dir.join("themes"));
    let worn = cena_ui::theme::chosen_for_hydra(dir);
    let mut said = format!(
        "Theme: Hydra wears {worn}. There are {}.",
        themes.names().join(", ")
    );
    for problem in &themes.problems {
        let _ = write!(said, " A file does not read: {problem}.");
    }
    said
}

/// The character's own `key`, through the *Theme* page's writer.
fn own(dir: &Path, character: &str, key: &str, to: Option<&str>) -> Result<String, String> {
    let (instance, name) = crate::pages::who(character)?;
    crate::theme_page::change(dir, (instance, name), key, to)
}

/// A Wrayth settings file's presets as a theme file in the themes folder.
fn import(dir: &Path, file: &Path, name: Option<&str>) -> Result<String, String> {
    let xml = std::fs::read_to_string(file)
        .map_err(|why| format!("Theme: {} cannot be read: {why}.", file.display()))?;
    let (presets, mut notes) = cena_behavior::triggers::wrayth::presets(&xml);
    if presets.is_empty() {
        return Err(format!(
            "Theme: {} has no <presets>: not a Wrayth settings file, or one with no colours.",
            file.display()
        ));
    }
    let (pins, more) = cena_ui::theme::pins_from_wrayth(&presets);
    notes.extend(more);
    if pins.is_empty() {
        return Err(format!(
            "Theme: none of {}'s presets is a colour Hydra names. {}",
            file.display(),
            notes.join(" ")
        ));
    }
    let name = name
        .map(str::to_owned)
        .or_else(|| {
            file.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            "Theme: say what to call it, as `theme import <file> as <name>`.".to_owned()
        })?;
    let stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let folder = dir.join("themes");
    let path = folder.join(format!("{stem}.toml"));
    if path.exists() {
        return Err(format!(
            "Theme: {} is already there; import it as another name, or take that file away first.",
            path.display()
        ));
    }
    if Themes::built_in().get(name.trim()).is_some() {
        return Err(format!(
            "Theme: {} is built in; import it as another name.",
            name.trim()
        ));
    }
    let theme = Theme {
        name: name.trim().to_owned(),
        base: Some(Theme::DEFAULT.to_owned()),
        recipe: cena_ui::theme::RecipeFile::default(),
        pins,
        shape: cena_ui::theme::ShapeFile::default(),
    };
    let count = theme.pins.len();
    std::fs::create_dir_all(&folder)
        .map_err(|why| format!("Theme: the themes folder cannot be made: {why}."))?;
    cena_session::store::save_text(&folder, &path, &theme.to_toml())
        .map_err(|why| format!("Theme: {} was not written: {why}.", path.display()))?;
    let mut said = format!(
        "Theme: {} written, {count} colour(s) from {}'s presets over {}; choose it on the Window page, or with theme mine {}.",
        path.display(),
        file.display(),
        Theme::DEFAULT,
        theme.name
    );
    if !notes.is_empty() {
        said.push(' ');
        said.push_str(&notes.join(" "));
    }
    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_parse() {
        assert_eq!(parse("look"), None);
        assert_eq!(parse("theme"), Some(Ok(Asked::Help)));
        assert_eq!(parse("Theme list"), Some(Ok(Asked::List)));
        assert_eq!(
            parse("theme mine Light"),
            Some(Ok(Asked::Mine(Some("Light".to_owned()))))
        );
        assert_eq!(parse("theme mine off"), Some(Ok(Asked::Mine(None))));
        assert_eq!(
            parse("theme accent #c9733a"),
            Some(Ok(Asked::Accent(Some("#c9733a".to_owned()))))
        );
        assert_eq!(
            parse("theme import \"C:/me/wrayth.xml\" as Ember"),
            Some(Ok(Asked::Import {
                file: PathBuf::from("C:/me/wrayth.xml"),
                name: Some("Ember".to_owned())
            }))
        );
        assert_eq!(
            parse("theme import C:/me/wrayth.xml"),
            Some(Ok(Asked::Import {
                file: PathBuf::from("C:/me/wrayth.xml"),
                name: None
            }))
        );
        for bad in ["theme mine", "theme accent", "theme import", "theme fly"] {
            assert!(matches!(parse(bad), Some(Err(_))), "{bad}");
        }
    }

    #[test]
    fn an_import_writes_a_theme_over_despana_and_never_over_a_file_there() {
        let dir = std::env::temp_dir().join(format!("cena-theme-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("made");
        let xml = dir.join("Nisugi.xml");
        std::fs::write(
            &xml,
            "<settings>\n<palette>\n<i id=\"6\" color=\"#FF3300\"/>\n</palette>\n<presets>\n\
             <p id=\"roomName\" color=\"#ECC013\" bgcolor=\"skin\"/>\n<p id=\"bold\" color=\"@6\"/>\n\
             <p id=\"familiar\" color=\"#00FF00\"/>\n</presets>\n</settings>\n",
        )
        .expect("written");
        let said = import(&dir, &xml, None).expect("imported");
        assert!(said.contains("2 colour(s)"), "{said}");
        assert!(said.contains("familiar"), "the rest noted: {said}");
        let themes = Themes::load(&dir.join("themes"));
        let theme = themes.get("Nisugi").expect("there");
        assert_eq!(theme.base.as_deref(), Some("Despana"));
        let palette = themes.outfit("Nisugi").expect("worn").palette;
        assert_eq!(
            palette.get(cena_ui::theme::Token::RoomName),
            [0xec, 0xc0, 0x13]
        );
        assert_eq!(
            palette.get(cena_ui::theme::Token::Creature),
            [0xff, 0x33, 0x00]
        );
        assert_eq!(
            palette.get(cena_ui::theme::Token::Health),
            cena_ui::theme::Token::Health.bare(),
            "Despana's"
        );
        assert!(import(&dir, &xml, None).is_err(), "never written over");
        assert!(import(&dir, &xml, Some("Light")).is_err(), "built in");
        assert!(list(&dir).contains("Nisugi"));
        assert!(import(&dir, &dir.join("missing.xml"), None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
