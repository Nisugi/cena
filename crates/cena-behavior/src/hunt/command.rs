//! What a player types about hunt profiles, while playing.
//!
//! ```text
//! ;hunt import <path>             read a bigshot profile; write it as a Hydra one, named after the file
//! ;hunt import <path> as <name>   ...under another name
//! ;hunt check <name>              read a profile the way this character would run it, and say what is held
//! ;hunt list                      the profiles there are
//! ;hunt <name>                    hunt on that profile, as this character
//! ;hunt stop                      stop hunting
//! ;heal [spellcast] [ranged] [blood]   heal with herbs by the heal profile (`plan/36`)
//! ```
//!
//! `;heal` is here rather than beside a desk of its own because it runs in the
//! hunt's driver, which is where the herbs are eaten during a rest.
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
    /// `import-loot <path>`: eloot's settings in, the character's loot
    /// profile out (`plan/31` §6).
    ImportLoot {
        /// The `eloot.yaml`, as typed. It may hold spaces.
        path: String,
    },
    /// Read a profile through the chain and report on it.
    Check(String),
    /// List the profiles.
    List,
    /// Hunt on this profile.
    Run(String),
    /// `;heal`: heal with herbs once, by the character's heal profile, with
    /// eherbs' `--spellcast`, `--ranged` and `blood` for this run.
    Heal {
        /// Only what stops a cast.
        spellcast: bool,
        /// Only what stops a shot.
        ranged: bool,
        /// Only blood.
        blood: bool,
    },
    /// `;heal stock` or `;heal fill`: stock the herb container at the
    /// herbalist; `fill` buys one of each kind it lacks.
    Stock {
        /// eherbs' `fill` rather than `stock`.
        fill: bool,
    },
    /// `;waggle [names]`: the waggle profile's spells cast on these people,
    /// or yourself.
    Waggle(Vec<String>),
    /// `;keep`: keep the keep profile's spells up until stopped.
    Keep,
    /// `;keep <words>`: change or show the keep profile.
    KeepEdit(Vec<String>),
    /// Stop the hunt under way.
    Stop,
    /// Hunt's, and already answered: said wrongly. Nothing to do.
    Nothing,
}

/// The words that are hunt's own, and so never a profile's name.
const RESERVED: &[&str] = &["import", "import-loot", "check", "list", "stop"];

/// What a wrongly said command is answered with.
pub const USAGE: &str = "hunt <name>, hunt stop, hunt import <bigshot yaml> [as <name>], hunt import-loot <eloot yaml>, hunt check <name>, or hunt list";

/// The hunt command a line is, **the command symbol already gone**. `None`:
/// not hunt's. `Some(Err(_))`: hunt's, said wrongly.
#[must_use]
pub fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.split_whitespace();
    let first = words.next()?;
    if first.eq_ignore_ascii_case("heal") {
        return Some(heal(words));
    }
    if first.eq_ignore_ascii_case("waggle") {
        return Some(Ok(Command::Waggle(words.map(str::to_owned).collect())));
    }
    if first.eq_ignore_ascii_case("keep") {
        let rest: Vec<String> = words.map(str::to_ascii_lowercase).collect();
        return Some(Ok(if rest.is_empty() {
            Command::Keep
        } else {
            Command::KeepEdit(rest)
        }));
    }
    if !first.eq_ignore_ascii_case("hunt") {
        return None;
    }
    let rest: Vec<&str> = words.collect();
    Some(match rest.split_first() {
        Some((word, args)) if word.eq_ignore_ascii_case("import") => import(args),
        Some((word, args)) if word.eq_ignore_ascii_case("import-loot") && !args.is_empty() => {
            Ok(Command::ImportLoot {
                path: args.join(" "),
            })
        }
        Some((word, [name])) if word.eq_ignore_ascii_case("check") => {
            Ok(Command::Check((*name).to_owned()))
        }
        Some((word, [])) if word.eq_ignore_ascii_case("list") => Ok(Command::List),
        Some((word, [])) if word.eq_ignore_ascii_case("stop") => Ok(Command::Stop),
        // `hunt check` with nothing after it is a check said wrongly, not a
        // profile named "check": the words above are not profile names.
        Some((word, _)) if RESERVED.iter().any(|r| word.eq_ignore_ascii_case(r)) => {
            Err(USAGE.to_owned())
        }
        Some((name, [])) => Ok(Command::Run((*name).to_owned())),
        _ => Err(USAGE.to_owned()),
    })
}

/// `import <path...> [as <name>]`: everything before `as` is the path.
/// `;heal`'s flags, with or without eherbs' dashes.
fn heal<'a>(words: impl Iterator<Item = &'a str>) -> Result<Command, String> {
    let (mut spellcast, mut ranged, mut blood) = (false, false, false);
    let words: Vec<&str> = words.collect();
    match words.as_slice() {
        [word] if word.eq_ignore_ascii_case("stock") => return Ok(Command::Stock { fill: false }),
        [word] if word.eq_ignore_ascii_case("fill") => return Ok(Command::Stock { fill: true }),
        _ => {}
    }
    for word in words {
        match word.trim_start_matches('-').to_ascii_lowercase().as_str() {
            "spellcast" => spellcast = true,
            "ranged" => ranged = true,
            "blood" => blood = true,
            _ => {
                return Err(
                    "heal, heal spellcast, heal ranged, heal blood, heal stock or heal fill"
                        .to_owned(),
                );
            }
        }
    }
    Ok(Command::Heal {
        spellcast,
        ranged,
        blood,
    })
}

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
        assert_eq!(parse("hunt stop"), Some(Ok(Command::Stop)));
        assert_eq!(
            parse("hunt ojandhaart"),
            Some(Ok(Command::Run("ojandhaart".to_owned())))
        );
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
            "hunt a b",
        ] {
            assert!(matches!(parse(line), Some(Err(_))), "{line}");
        }
    }

    #[test]
    fn heal_and_its_flags() {
        assert_eq!(
            parse("heal"),
            Some(Ok(Command::Heal {
                spellcast: false,
                ranged: false,
                blood: false
            }))
        );
        assert_eq!(
            parse("heal --spellcast blood"),
            Some(Ok(Command::Heal {
                spellcast: true,
                ranged: false,
                blood: true
            }))
        );
        assert!(matches!(parse("heal everyone"), Some(Err(_))));
        assert_eq!(
            parse("heal stock"),
            Some(Ok(Command::Stock { fill: false }))
        );
        assert_eq!(parse("heal fill"), Some(Ok(Command::Stock { fill: true })));
    }

    #[test]
    fn not_hunts_is_nobodys_yet() {
        assert_eq!(parse("go2 bank"), None);
        assert_eq!(parse(""), None);
    }
}
