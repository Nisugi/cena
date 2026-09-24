//! What a player types about hunt profiles, while playing.
//!
//! ```text
//! ;hunt import <path>             read a bigshot profile; write it as a Hydra one, named after the file
//! ;hunt import <path> as <name>   ...under another name
//! ;hunt check <name>              read a profile the way this character would run it, and say what is held
//! ;hunt list                      the profiles there are
//! ```
//!
//! Running one (`;hunt <name>`) and stopping it are M6b's, when there is an
//! engine to run it (`plan/30` §4, §7).
//!
//! **The symbol is not this module's.** A line reaches here already marked
//! as Hydra's and stripped of its symbol (`cena_session::command::claimant`),
//! as travel's do. A line this module does not know answers `None`, which
//! means *not hunt's*, and never *the game's*.

/// One thing asked about hunt profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Read a bigshot profile at `path` and write it as `name`, or as the
    /// file's own name.
    Import {
        /// The bigshot YAML, as typed. It may hold spaces.
        path: String,
        /// The Hydra name, when one was given after `as`.
        name: Option<String>,
    },
    /// Read a profile through the chain and report on it.
    Check(String),
    /// List the profiles.
    List,
    /// Hunt's, and already answered: said wrongly. Nothing to do.
    Nothing,
}

/// What a wrongly said command is answered with.
pub const USAGE: &str = "hunt import <bigshot yaml> [as <name>], hunt check <name>, or hunt list";

/// The hunt command a line is, **the command symbol already gone**. `None`:
/// not hunt's. `Some(Err(_))`: hunt's, said wrongly.
#[must_use]
pub fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("hunt") {
        return None;
    }
    let rest: Vec<&str> = words.collect();
    Some(match rest.split_first() {
        Some((word, args)) if word.eq_ignore_ascii_case("import") => import(args),
        Some((word, [name])) if word.eq_ignore_ascii_case("check") => {
            Ok(Command::Check((*name).to_owned()))
        }
        Some((word, [])) if word.eq_ignore_ascii_case("list") => Ok(Command::List),
        _ => Err(USAGE.to_owned()),
    })
}

/// `import <path...> [as <name>]`: everything before `as` is the path.
fn import(args: &[&str]) -> Result<Command, String> {
    let (path, name) = match args.iter().position(|w| w.eq_ignore_ascii_case("as")) {
        Some(at) => {
            let (path, tail) = args.split_at(at);
            match tail {
                [_, name] => (path, Some((*name).to_owned())),
                _ => return Err(USAGE.to_owned()),
            }
        }
        None => (args, None),
    };
    if path.is_empty() {
        return Err(USAGE.to_owned());
    }
    Ok(Command::Import {
        path: path.join(" "),
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::{Command, parse};

    #[test]
    fn the_words() {
        assert_eq!(
            parse("hunt import C:\\some dir\\ojandhaart.yaml"),
            Some(Ok(Command::Import {
                path: "C:\\some dir\\ojandhaart.yaml".to_owned(),
                name: None,
            }))
        );
        assert_eq!(
            parse("HUNT import x.yaml as archer"),
            Some(Ok(Command::Import {
                path: "x.yaml".to_owned(),
                name: Some("archer".to_owned()),
            }))
        );
        assert_eq!(
            parse("hunt check archer"),
            Some(Ok(Command::Check("archer".to_owned())))
        );
        assert_eq!(parse("hunt list"), Some(Ok(Command::List)));
    }

    #[test]
    fn said_wrongly_is_hunts_and_refused() {
        for line in [
            "hunt",
            "hunt import",
            "hunt import x as",
            "hunt check",
            "hunt check a b",
            "hunt list all",
        ] {
            assert!(matches!(parse(line), Some(Err(_))), "{line}");
        }
    }

    #[test]
    fn not_hunts_is_nobodys_yet() {
        assert_eq!(parse("go2 bank"), None);
        assert_eq!(parse(""), None);
    }
}
