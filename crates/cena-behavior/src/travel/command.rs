//! What a player types to travel, while playing.
//!
//! go2's own words, because they are the ones in the player's fingers:
//!
//! ```text
//! ;go2 bank                 walk to the nearest bank
//! ;go2 228   ;go2 u7120     ...to a room, by the map's number or the game's
//! ;go2 targets              the places there are to go
//! ;go2 list                 the names you have saved
//! ;go2 save den             this room is "den", for this character
//! ;go2 save den --global    ...for every character
//! ;go2 save den=228,current ...and these rooms; the nearest is meant
//! ;go2 delete den           forget it (`--global`: everyone's)
//! ;go2 stop  ;kill go2  ;k go2    stop walking
//! ;route2 bank              show the way, and send nothing
//! ```
//!
//! **This only reads the line.** Which lines are offered to it -- where a
//! frontend's typed input is looked at before it goes to the game -- is the
//! frontend's seam, not this module's; a line that is not one of these
//! answers `None` and is the game's.

/// One thing asked of the travel desk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Walk there.
    Go(String),
    /// Say the way there. Sends nothing.
    Route(String),
    /// List the places the map names, town by town.
    Places,
    /// List the names the player has saved.
    List,
    /// From now on `name` means these rooms: each a number, `u<uid>`, or
    /// `current`. None given means `current`.
    Save {
        name: String,
        rooms: Vec<String>,
        global: bool,
    },
    Forget {
        name: String,
        global: bool,
    },
    /// Stop the walk under way.
    Stop,
}

/// The travel command a typed line is. `None`: it is not one, and is the
/// game's. `Some(Err(_))`: it is one, said wrongly -- **not the game's
/// either**, so a slip of the fingers is never spoken aloud in a town square.
#[must_use]
pub fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.trim().strip_prefix(';')?.split_whitespace();
    let (script, rest): (_, Vec<&str>) = (words.next()?.to_lowercase(), words.collect());
    let global = rest.contains(&"--global");
    let rest: Vec<&str> = rest
        .into_iter()
        .filter(|word| *word != "--global")
        .collect();
    let said = rest.join(" ");
    Some(
        match (
            script.as_str(),
            rest.first().map(|word| word.to_lowercase()),
        ) {
            // `;kill hunt` is somebody else's: it falls to the last arm.
            ("kill" | "k", Some(which)) if which == "go2" || which == "route2" => Ok(Command::Stop),
            ("route2", Some(_)) => Ok(Command::Route(said)),
            ("route2", None) => Err("route2 where? -- `;route2 bank`".to_owned()),
            ("go2", None) => {
                Err("go2 where? -- `;go2 bank`, or `;go2 targets` for a list".to_owned())
            }
            ("go2", Some(first)) => match first.as_str() {
                "targets" if rest.len() == 1 => Ok(Command::Places),
                "list" if rest.len() == 1 => Ok(Command::List),
                "stop" if rest.len() == 1 => Ok(Command::Stop),
                "save" => named(&rest[1..]).map(|(name, rooms)| Command::Save {
                    name,
                    rooms,
                    global,
                }),
                "delete" => named(&rest[1..]).map(|(name, _)| Command::Forget { name, global }),
                _ => Ok(Command::Go(said)),
            },
            _ => return None,
        },
    )
}

/// `den`, or `den=228,current`: the name, and the rooms if any were given.
fn named(words: &[&str]) -> Result<(String, Vec<String>), String> {
    let said = words.join(" ");
    let (name, rooms) = said.split_once('=').unwrap_or((&said, ""));
    let name = name.trim();
    if name.is_empty() {
        return Err("it needs a name -- `;go2 save den`".to_owned());
    }
    let rooms = rooms
        .split(',')
        .map(str::trim)
        .filter(|room| !room.is_empty())
        .map(str::to_lowercase)
        .collect();
    Ok((name.to_owned(), rooms))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(line: &str) -> Command {
        parse(line).unwrap().unwrap()
    }

    #[test]
    fn what_is_not_a_travel_command_is_the_games() {
        for line in [
            "north",
            "say ;go2 bank",
            ";",
            ";hunt",
            ";kill hunt",
            "go2 bank",
        ] {
            assert_eq!(parse(line), None, "{line:?}");
        }
    }

    #[test]
    fn a_place_is_everything_after_the_word() {
        assert_eq!(ok(";go2 bank"), Command::Go("bank".into()));
        assert_eq!(
            ok("  ;GO2  general store "),
            Command::Go("general store".into())
        );
        assert_eq!(ok(";go2 u7120"), Command::Go("u7120".into()));
        assert_eq!(ok(";route2 town"), Command::Route("town".into()));
        // A place may be called what a word of go2's is, if it has more to it.
        assert_eq!(
            ok(";go2 list of kings"),
            Command::Go("list of kings".into())
        );
    }

    #[test]
    fn go2s_own_words_are_its_own() {
        assert_eq!(ok(";go2 targets"), Command::Places);
        assert_eq!(ok(";go2 list"), Command::List);
        for line in [";go2 stop", ";kill go2", ";k go2", ";k route2"] {
            assert_eq!(ok(line), Command::Stop, "{line}");
        }
    }

    #[test]
    fn a_name_is_saved_for_the_character_unless_told_global() {
        let saved = |rooms: &[&str], global| Command::Save {
            name: "my den".into(),
            rooms: rooms.iter().map(|room| (*room).to_owned()).collect(),
            global,
        };
        assert_eq!(ok(";go2 save my den"), saved(&[], false));
        assert_eq!(ok(";go2 save my den --global"), saved(&[], true));
        assert_eq!(ok(";go2 save --global my den"), saved(&[], true));
        assert_eq!(
            ok(";go2 save my den=228, Current,u7120"),
            saved(&["228", "current", "u7120"], false)
        );
        assert_eq!(
            ok(";go2 delete my den --global"),
            Command::Forget {
                name: "my den".into(),
                global: true
            }
        );
    }

    #[test]
    fn a_command_said_wrongly_is_still_not_the_games() {
        for line in [";go2", ";route2", ";go2 save", ";go2 delete --global"] {
            assert!(matches!(parse(line), Some(Err(_))), "{line}");
        }
    }
}
