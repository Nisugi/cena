//! What a player types about hunt profiles, while playing.
//!
//! ```text
//! ;hunt import <path>             read a bigshot profile; write it as a Hydra one, named after the file
//! ;hunt import <path> as <name>   ...under another name
//! ;hunt check <name>              read a profile the way this character would run it, and say what is held
//! ;hunt list                      the profiles there are
//! ;hunt <name>                    hunt on that profile, as this character
//! ;hunt stop                      stop hunting
//! ;hunt show <name> [setting]     the settings, as this character runs them
//! ;hunt set <name> <setting> <value>   change one, in the profile's file (`settings.rs`)
//! ;hunt unset <name> <setting>    take one out, so the level below decides it
//! ;hunt help, or ;hunt alone      all of these ([`HELP`])
//! ;heal [spellcast] [ranged] [blood]   heal with herbs by the heal profile (`plan/36`)
//! ;heal show | set <setting> <value> | unset <setting>   the heal profile
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
    /// `hunt <name> with <A> <B> ...`: lead these characters, each running
    /// in this Hydra, each hunting its own profile of the same name
    /// (`plan/39` §8, question 2). Who they are is the binary's to resolve:
    /// the desk hunts one session.
    Group {
        /// The profile, by the name every member's chain resolves.
        name: String,
        /// The followers, by character name.
        with: Vec<String>,
    },
    /// `hunt <name> quick`: this room, on the profile, until it is clear.
    Quick(String),
    /// `hunt <name> bounty`: hunt until the bounty is done or a new one is
    /// ready, then rest and end (`hunt/bounty.rs`).
    Bounty(String),
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
    /// `;sc <spell|alias> [target] [count]`: one spell, as set up.
    Sc(Vec<String>),
    /// `;sc alias|verb|stance|set ...`: change the spellcaster profile.
    ScEdit(Vec<String>),
    /// `;waggle [names]`: the waggle profile's spells cast on these people,
    /// or yourself.
    Waggle(Vec<String>),
    /// `;keep`: keep the keep profile's spells up until stopped.
    Keep,
    /// `;keep <words>`: change or show the keep profile.
    KeepEdit(Vec<String>),
    /// Stop the hunt under way.
    Stop,
    /// `hunt set <profile> <setting> <value>`: one setting changed in the
    /// profile's file.
    Set {
        /// The profile.
        profile: String,
        /// The setting, dotted: `rooms.resting`.
        key: String,
        /// The value, as typed (`crate::settings::typed`).
        value: String,
    },
    /// `hunt unset <profile> <setting>`: one setting taken out of the
    /// profile's file, so the level below decides it.
    Unset {
        /// The profile.
        profile: String,
        /// The setting, dotted.
        key: String,
    },
    /// `hunt show <profile> [setting]`: the settings as this character runs
    /// them, every one or those under `setting`.
    Show {
        /// The profile.
        profile: String,
        /// Only this setting, or the ones under it.
        key: Option<String>,
    },
    /// `hunt help`, or `hunt` alone: every command, and how a setting is
    /// changed.
    Help,
    /// `heal set <setting> <value>`: one heal setting changed.
    HealSet {
        /// The setting: `container`.
        key: String,
        /// The value, as typed.
        value: String,
    },
    /// `heal unset <setting>`: one heal setting back to its default.
    HealUnset(String),
    /// `heal show`: the heal settings.
    HealShow,
    /// Hunt's, and already answered: said wrongly. Nothing to do.
    Nothing,
}

/// The words that are hunt's own, and so never a profile's name.
const RESERVED: &[&str] = &[
    "import",
    "import-loot",
    "check",
    "list",
    "stop",
    "set",
    "unset",
    "show",
    "help",
];

/// What a wrongly said command is answered with.
pub const USAGE: &str = "hunt <name> [quick|bounty], hunt <name> with <character>..., hunt stop, hunt list, hunt check <name>, hunt show <name> [setting], hunt set <name> <setting> <value>, hunt unset <name> <setting>, hunt import <bigshot yaml> [as <name>], hunt import-loot <eloot yaml>. `hunt help` says more.";

/// What `hunt help` says, a line each.
pub const HELP: &[&str] = &[
    "hunt <profile>                         hunt on a profile",
    "hunt <profile> quick | bounty          clear this room | hunt until the bounty is done",
    "hunt <profile> with <name> <name>...   lead these characters, each hunting its own <profile>",
    "hunt stop                              stop",
    "hunt list                              the profiles there are",
    "hunt check <profile>                   read it as this character will run it: what is wrong, what is held",
    "hunt show <profile> [setting]          every setting, or those under one: hunt show ojandhaart rest",
    "hunt set <profile> <setting> <value>   change one: hunt set ojandhaart rooms.resting 29877",
    "hunt unset <profile> <setting>         take one out, so the default decides it",
    "hunt import <bigshot yaml> [as <name>] bring in a bigshot profile",
    "hunt import-loot <eloot yaml>          bring in eloot's settings as this character's loot profile",
    "heal [spellcast] [ranged] [blood]      heal with herbs",
    "heal show                              the heal settings",
    "heal set <setting> <value>             change one: heal set container herb pouch",
    "heal stock | fill                      stock the herb container at the herbalist",
    "A value is on or off, a number, a list [\"a\", \"b\"], a table { name = \"warg\", routine = \"a\" }, or words.",
    "A setting in a list is picked by number from 1: hunt set ojandhaart targets.2.routine c",
];

/// The hunt command a line is, **the command symbol already gone**. `None`:
/// not hunt's. `Some(Err(_))`: hunt's, said wrongly.
#[must_use]
pub fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.split_whitespace();
    let first = words.next()?;
    if first.eq_ignore_ascii_case("heal") {
        return Some(heal_words(line, words));
    }
    if first.eq_ignore_ascii_case("sc") {
        let rest: Vec<String> = words.map(str::to_owned).collect();
        let edit = rest.first().is_some_and(|w| {
            ["alias", "verb", "stance", "set"].contains(&w.to_ascii_lowercase().as_str())
        });
        return Some(Ok(if edit {
            Command::ScEdit(rest)
        } else {
            Command::Sc(rest)
        }));
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
        None => Ok(Command::Help),
        Some((word, [])) if word.eq_ignore_ascii_case("help") => Ok(Command::Help),
        Some((word, [profile, key, _, ..])) if word.eq_ignore_ascii_case("set") => {
            Ok(Command::Set {
                profile: (*profile).to_owned(),
                key: (*key).to_owned(),
                value: after_words(line, 4).to_owned(),
            })
        }
        Some((word, [profile, key])) if word.eq_ignore_ascii_case("unset") => Ok(Command::Unset {
            profile: (*profile).to_owned(),
            key: (*key).to_owned(),
        }),
        Some((word, [profile, key @ ..])) if word.eq_ignore_ascii_case("show") && key.len() < 2 => {
            Ok(Command::Show {
                profile: (*profile).to_owned(),
                key: key.first().map(|k| (*k).to_owned()),
            })
        }
        Some((word, args)) if word.eq_ignore_ascii_case("import") => import(args),
        Some((word, args)) if word.eq_ignore_ascii_case("import-loot") && !args.is_empty() => {
            Ok(Command::ImportLoot {
                path: crate::settings::unquoted(&args.join(" ")),
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
        Some((name, [quick])) if quick.eq_ignore_ascii_case("quick") => {
            Ok(Command::Quick((*name).to_owned()))
        }
        Some((name, [bounty])) if bounty.eq_ignore_ascii_case("bounty") => {
            Ok(Command::Bounty((*name).to_owned()))
        }
        Some((name, [with, members @ ..]))
            if with.eq_ignore_ascii_case("with") && !members.is_empty() =>
        {
            Ok(Command::Group {
                name: (*name).to_owned(),
                with: members.iter().map(|member| capitalized(member)).collect(),
            })
        }
        _ => Err(USAGE.to_owned()),
    })
}

/// A character's name as the game spells it: first letter up, the rest down.
fn capitalized(name: &str) -> String {
    let mut letters = name.chars();
    letters.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(letters.flat_map(char::to_lowercase))
            .collect()
    })
}

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

/// `heal` and what follows: its settings, or its flags.
fn heal_words<'a>(line: &str, words: impl Iterator<Item = &'a str>) -> Result<Command, String> {
    let words: Vec<&str> = words.collect();
    match words.as_slice() {
        [word] if word.eq_ignore_ascii_case("show") => Ok(Command::HealShow),
        [word, key, _, ..] if word.eq_ignore_ascii_case("set") => Ok(Command::HealSet {
            key: (*key).to_owned(),
            value: after_words(line, 3).to_owned(),
        }),
        [word, key] if word.eq_ignore_ascii_case("unset") => {
            Ok(Command::HealUnset((*key).to_owned()))
        }
        [word, ..]
            if ["set", "unset", "show"]
                .iter()
                .any(|w| word.eq_ignore_ascii_case(w)) =>
        {
            Err("heal show, heal set <setting> <value>, or heal unset <setting>".to_owned())
        }
        _ => heal(words.into_iter()),
    }
}

/// What follows the first `n` words of `line`, as typed: a value keeps its
/// own spacing and quotes.
fn after_words(line: &str, n: usize) -> &str {
    let mut rest = line.trim_start();
    for _ in 0..n {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        rest = rest.get(end..).unwrap_or("").trim_start();
    }
    rest.trim_end()
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
        path: crate::settings::unquoted(&path.join(" ")),
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
        assert_eq!(
            parse("hunt ojandhaart with kiyna Dicate"),
            Some(Ok(Command::Group {
                name: "ojandhaart".to_owned(),
                with: vec!["Kiyna".to_owned(), "Dicate".to_owned()],
            }))
        );
        assert!(matches!(parse("hunt ojandhaart with"), Some(Err(_))));
    }

    /// A quoted path is the path: `"` is not part of a Windows file name.
    #[test]
    fn a_quoted_path_loses_its_quotes() {
        assert_eq!(
            parse("hunt import \"C:\\some dir\\ojandhaart.yaml\" as archer"),
            Some(Ok(Command::Import {
                path: "C:\\some dir\\ojandhaart.yaml".to_owned(),
                name: Some("archer".to_owned()),
            }))
        );
        assert_eq!(
            parse("hunt import-loot 'C:\\eloot.yaml'"),
            Some(Ok(Command::ImportLoot {
                path: "C:\\eloot.yaml".to_owned(),
            }))
        );
    }

    #[test]
    fn said_wrongly_is_hunts_and_refused() {
        for line in [
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
    fn settings_are_hunts_and_keep_the_value_as_typed() {
        assert_eq!(parse("hunt"), Some(Ok(Command::Help)));
        assert_eq!(parse("hunt help"), Some(Ok(Command::Help)));
        assert_eq!(
            parse("hunt set ojandhaart sequences.volley.when expiring \"Briar Betrayer\" 7"),
            Some(Ok(Command::Set {
                profile: "ojandhaart".to_owned(),
                key: "sequences.volley.when".to_owned(),
                value: "expiring \"Briar Betrayer\" 7".to_owned(),
            }))
        );
        assert_eq!(
            parse("hunt show ojandhaart rest"),
            Some(Ok(Command::Show {
                profile: "ojandhaart".to_owned(),
                key: Some("rest".to_owned()),
            }))
        );
        assert_eq!(
            parse("hunt unset ojandhaart rooms.resting"),
            Some(Ok(Command::Unset {
                profile: "ojandhaart".to_owned(),
                key: "rooms.resting".to_owned(),
            }))
        );
        assert!(matches!(
            parse("hunt set ojandhaart rooms.resting"),
            Some(Err(_))
        ));
        assert_eq!(
            parse("heal set container  herb pouch "),
            Some(Ok(Command::HealSet {
                key: "container".to_owned(),
                value: "herb pouch".to_owned(),
            }))
        );
        assert_eq!(parse("heal show"), Some(Ok(Command::HealShow)));
        assert_eq!(
            parse("heal unset stock"),
            Some(Ok(Command::HealUnset("stock".to_owned())))
        );
        assert!(matches!(parse("heal set container"), Some(Err(_))));
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
