//! `;trigger`'s words (`plan/45` §5c), parsed.
//!
//! A name is one word, or a phrase in double quotes. The words a trigger
//! matches, a value, and a line to test are the rest of the line, as typed.

/// `;trigger help`, one usage per line.
pub(crate) const HELP: [&str; 13] = [
    "trigger list -- every trigger, by category",
    "trigger show <name> -- one trigger's settings",
    "trigger add <name> <words> -- a new trigger on those words, making them bold",
    "trigger set <name> <setting> <value> -- look.color #ff4040, squelch on, substitute <text>, redirect.stream <stream>",
    "trigger unset <name> <setting> -- take one out",
    "trigger remove <name> -- delete it",
    "trigger on|off <name> -- switch one trigger",
    "trigger on|off category <category> -- every trigger in a category",
    "trigger on|off every <look|squelch|substitute|redirect> -- one kind of response, everywhere",
    "trigger test <line> -- what this character's triggers would do to that line",
    "trigger reload -- read the file again, for this character",
    "trigger import <path> -- a Wrayth settings file's highlights, names and ignores",
    "trigger approve <name> -- let a trigger that came from elsewhere send its line",
];

/// One `;trigger` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    /// `;trigger`, `;trigger help`.
    Help,
    /// `;trigger list`.
    List,
    /// `;trigger show <name>`.
    Show(String),
    /// `;trigger add <name> <words>`.
    Add {
        /// Its name.
        name: String,
        /// The words it matches.
        words: String,
    },
    /// `;trigger set <name> <setting> <value>`.
    Set {
        /// Which trigger.
        name: String,
        /// The setting, dotted: `look.color`.
        key: String,
        /// As typed.
        value: String,
    },
    /// `;trigger unset <name> <setting>`.
    Unset {
        /// Which trigger.
        name: String,
        /// The setting.
        key: String,
    },
    /// `;trigger remove <name>`.
    Remove(String),
    /// `;trigger on|off ...`.
    Switch {
        /// On, or off.
        on: bool,
        /// What.
        target: Target,
    },
    /// `;trigger test <line>`.
    Test(String),
    /// `;trigger reload`.
    Reload,
    /// `;trigger import <path>`: a Wrayth settings file.
    Import(String),
    /// `;trigger approve <name>`: let a trigger from elsewhere send.
    Approve(String),
}

/// What `on` or `off` switches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// A trigger, by name.
    Trigger(String),
    /// Every trigger in a category.
    Category(String),
    /// One kind of response, everywhere.
    Every(String),
}

/// Parse a command line, without its symbol. `None` when the word is not
/// `trigger` (or `triggers`); `Some(Err)` saying what is missing when the
/// rest is not understood.
pub(crate) fn parse(line: &str) -> Option<Result<Command, String>> {
    let (word, rest) = word(line);
    if !word.eq_ignore_ascii_case("trigger") && !word.eq_ignore_ascii_case("triggers") {
        return None;
    }
    Some(command(rest))
}

fn command(rest: &str) -> Result<Command, String> {
    let (verb, rest) = word(rest);
    let verb = verb.to_ascii_lowercase();
    let usage = |what: &str| format!("`trigger {verb}` needs {what}");
    match verb.as_str() {
        "" | "help" => Ok(Command::Help),
        "list" => Ok(Command::List),
        "reload" => Ok(Command::Reload),
        "show" | "remove" | "approve" => {
            let (name, _) = name(rest).ok_or_else(|| usage("a trigger's name"))?;
            Ok(match verb.as_str() {
                "show" => Command::Show(name),
                "remove" => Command::Remove(name),
                _ => Command::Approve(name),
            })
        }
        "add" => {
            let (name, words) = name(rest)
                .filter(|(_, words)| !words.is_empty())
                .ok_or_else(|| usage("a name and the words it matches"))?;
            Ok(Command::Add {
                name,
                words: words.to_owned(),
            })
        }
        "set" | "unset" => {
            let (name, rest) = name(rest).ok_or_else(|| usage("a trigger's name"))?;
            let (key, value) = word(rest);
            if key.is_empty() {
                return Err(usage("a setting, as `trigger show` prints it"));
            }
            if verb == "unset" {
                return Ok(Command::Unset {
                    name,
                    key: key.to_owned(),
                });
            }
            if value.is_empty() {
                return Err(usage("a value"));
            }
            Ok(Command::Set {
                name,
                key: key.to_owned(),
                value: value.to_owned(),
            })
        }
        "on" | "off" => {
            let on = verb == "on";
            let (first, after) = name(rest).ok_or_else(|| usage("a trigger's name"))?;
            let target = match (first.to_ascii_lowercase().as_str(), name(after)) {
                ("category", Some((category, _))) => Target::Category(category),
                ("every", Some((kind, _))) => Target::Every(kind.to_ascii_lowercase()),
                _ => Target::Trigger(first),
            };
            Ok(Command::Switch { on, target })
        }
        "test" if !rest.is_empty() => Ok(Command::Test(rest.to_owned())),
        "test" => Err(usage("a line to test")),
        "import" if !rest.is_empty() => Ok(Command::Import(unquoted(rest))),
        "import" => Err(usage("the path of a Wrayth settings file")),
        _ => Err(format!(
            "`{verb}` is not a word it knows; `trigger help` lists them"
        )),
    }
}

use cena_behavior::settings::unquoted;

/// The first word, and the rest with its leading space gone.
fn word(line: &str) -> (&str, &str) {
    let line = line.trim_start();
    let end = line.find(char::is_whitespace).unwrap_or(line.len());
    (&line[..end], line[end..].trim_start())
}

/// A name -- a word, or a phrase in double quotes -- and the rest.
fn name(line: &str) -> Option<(String, &str)> {
    let line = line.trim_start();
    if let Some(quoted) = line.strip_prefix('"') {
        let end = quoted.find('"')?;
        let name = &quoted[..end];
        return (!name.is_empty()).then(|| (name.to_owned(), quoted[end + 1..].trim_start()));
    }
    let (word, rest) = word(line);
    (!word.is_empty()).then(|| (word.to_owned(), rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(line: &str) -> Result<Command, String> {
        parse(line).unwrap_or_else(|| Err("not ours".into()))
    }

    #[test]
    fn only_its_own_word() {
        assert_eq!(parse("trig list"), None);
        assert_eq!(parse("sorter on"), None);
        assert_eq!(parsed("TRIGGERS"), Ok(Command::Help));
        assert_eq!(parsed("trigger list"), Ok(Command::List));
    }

    #[test]
    fn add_takes_the_rest_of_the_line_as_typed() {
        assert_eq!(
            parsed("trigger add stunned You are  stunned!"),
            Ok(Command::Add {
                name: "stunned".into(),
                words: "You are  stunned!".into(),
            })
        );
        assert_eq!(
            parsed("trigger add \"my stun\" You are stunned"),
            Ok(Command::Add {
                name: "my stun".into(),
                words: "You are stunned".into(),
            })
        );
        assert!(parsed("trigger add stunned").is_err());
    }

    #[test]
    fn set_and_unset() {
        assert_eq!(
            parsed("trigger set stunned look.color #ff4040"),
            Ok(Command::Set {
                name: "stunned".into(),
                key: "look.color".into(),
                value: "#ff4040".into(),
            })
        );
        assert_eq!(
            parsed("trigger set whisper substitute $1: $2"),
            Ok(Command::Set {
                name: "whisper".into(),
                key: "substitute".into(),
                value: "$1: $2".into(),
            })
        );
        assert!(parsed("trigger set stunned look.color").is_err());
        assert_eq!(
            parsed("trigger unset stunned squelch"),
            Ok(Command::Unset {
                name: "stunned".into(),
                key: "squelch".into(),
            })
        );
    }

    #[test]
    fn on_and_off_name_a_trigger_a_category_or_a_kind() {
        let switch = |on, target| Ok(Command::Switch { on, target });
        assert_eq!(
            parsed("trigger off stunned"),
            switch(false, Target::Trigger("stunned".into()))
        );
        assert_eq!(
            parsed("trigger off category \"Wrayth names\""),
            switch(false, Target::Category("Wrayth names".into()))
        );
        assert_eq!(
            parsed("trigger on every Squelch"),
            switch(true, Target::Every("squelch".into()))
        );
        // A trigger named `category` is still a trigger.
        assert_eq!(
            parsed("trigger on category"),
            switch(true, Target::Trigger("category".into()))
        );
    }

    #[test]
    fn test_takes_the_line_and_an_unknown_word_says_so() {
        assert_eq!(
            parsed("trigger test You are stunned!"),
            Ok(Command::Test("You are stunned!".into()))
        );
        assert!(parsed("trigger test").is_err());
        let unknown = parsed("trigger frobnicate").unwrap_err();
        assert!(unknown.contains("`frobnicate`"), "{unknown}");
    }

    /// A path pasted with Windows' "Copy as path" keeps its spaces and loses
    /// its quotes.
    #[test]
    fn import_takes_a_path_as_pasted() {
        assert_eq!(
            parsed(r#"trigger import "E:\Wrayth files\Nisugi3.xml""#),
            Ok(Command::Import(r"E:\Wrayth files\Nisugi3.xml".into()))
        );
        assert_eq!(
            parsed(r"trigger import C:\Nisugi3.xml"),
            Ok(Command::Import(r"C:\Nisugi3.xml".into()))
        );
        assert!(parsed("trigger import").is_err());
    }
}
