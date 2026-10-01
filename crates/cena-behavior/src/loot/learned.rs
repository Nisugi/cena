//! What looting learns that the profile keeps. eloot saves its profile each
//! time it learns a bag that closes itself (`eloot.lic:4090-4093`,
//! `:4120-4124`), a thing that crumbles (`:4149-4153`), one it cannot hold
//! when it is told to remember those (`ELoot.unlootable`, `:2951-2959`), or a
//! creature it cannot skin (`:5846`), so the next run knows it. Here the
//! planner collects the names a visit learned ([`Learned`]) and whoever holds
//! the profile's file writes them in ([`remember`]).

use super::profile::LootProfile;

/// Names a visit learned that its profile does not hold yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Learned {
    /// Creatures the game said cannot be skinned.
    pub unskinnable: Vec<String>,
    /// Things that crumbled when stowed.
    pub crumbly: Vec<String>,
    /// Things the game would not let the character hold, of no kind the
    /// object table knows: only when the profile remembers them.
    pub unlootable: Vec<String>,
    /// Bags that close themselves, by name.
    pub autoclose: Vec<String>,
}

impl Learned {
    /// Whether nothing was learned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.unskinnable.is_empty()
            && self.crumbly.is_empty()
            && self.unlootable.is_empty()
            && self.autoclose.is_empty()
    }

    /// Add `name` to `list` once.
    pub(super) fn add(list: &mut Vec<String>, name: &str) {
        if !list.iter().any(|held| held == name) {
            list.push(name.to_owned());
        }
    }

    /// What was learned, a line for each kind, for the player.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        [
            (
                &self.unskinnable,
                "cannot be skinned; none is skinned again",
            ),
            (&self.crumbly, "crumbled; none is taken again"),
            (&self.unlootable, "could not be held; none is taken again"),
            (
                &self.autoclose,
                "closes itself; it is opened first from now on",
            ),
        ]
        .into_iter()
        .filter(|(names, _)| !names.is_empty())
        .map(|(names, what)| format!("{}: {what}.", names.join(", ")))
        .collect()
    }
}

/// The profile file's text with what was learned added to its lists, the
/// comments at its head kept, as `;hunt set` keeps them. `Ok(None)` when
/// every name is already there.
///
/// # Errors
///
/// The text is not a loot profile, or cannot be written back as one.
pub fn remember(text: &str, learned: &Learned) -> Result<Option<String>, String> {
    // Written back from the profile alone, the first name learned took the
    // importer's notes of what it dropped out of the file (the review of
    // 2026-09-29).
    let (head, _) = crate::settings::split(text)?;
    let mut profile = LootProfile::parse(text)?;
    let before = profile.clone();
    for (list, names) in [
        (&mut profile.skin.unskinnable, &learned.unskinnable),
        (&mut profile.crumbly, &learned.crumbly),
        (&mut profile.unlootable, &learned.unlootable),
        (&mut profile.autoclose, &learned.autoclose),
    ] {
        for name in names {
            Learned::add(list, name);
        }
    }
    if profile == before {
        return Ok(None);
    }
    Ok(Some(format!("{head}{}", profile.to_toml()?)))
}

/// The profile file's text with these creatures added to what it has learned
/// cannot be skinned: [`remember`], for skinning alone. `Ok(None)` when every
/// name is already there.
///
/// # Errors
///
/// As [`remember`].
pub fn remember_unskinnable(text: &str, names: &[String]) -> Result<Option<String>, String> {
    remember(
        text,
        &Learned {
            unskinnable: names.to_vec(),
            ..Learned::default()
        },
    )
}

/// `loot reset unskinnable [creature]`, eloot's `manage_unskinnable`
/// (`eloot.lic:2169-2193`): the profile's text with every creature learned
/// unskinnable forgotten, or the one named, without regard to case; and what
/// to say. `None` for the text when nothing changed.
///
/// # Errors
///
/// The text is not a loot profile, or the creature named is not on the list:
/// the reason, for the player.
pub fn forget_unskinnable(
    text: &str,
    creature: Option<&str>,
) -> Result<(Option<String>, String), String> {
    let (head, _) = crate::settings::split(text)?;
    let mut profile = LootProfile::parse(text)?;
    let list = &mut profile.skin.unskinnable;
    let said = match creature.map(str::trim).filter(|name| !name.is_empty()) {
        None if list.is_empty() => {
            return Ok((None, "the unskinnable list is already empty.".to_owned()));
        }
        None => {
            let removed = list.len();
            list.clear();
            let what = if removed == 1 {
                "creature"
            } else {
                "creatures"
            };
            format!("cleared {removed} {what} from the unskinnable list.")
        }
        Some(name) => {
            let before = list.len();
            list.retain(|held| !held.eq_ignore_ascii_case(name));
            if list.len() == before {
                return Err(format!("\"{name}\" is not on the unskinnable list."));
            }
            format!("removed \"{name}\" from the unskinnable list.")
        }
    };
    Ok((Some(format!("{head}{}", profile.to_toml()?)), said))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# imported\n\ntake = [\"gem\"]\ncrumbly = [\"dried flower\"]\n\n[skin]\nenable = true\nunskinnable = [\"cave troll\", \"Krag Dweller\"]\n";

    /// Each kind into its own list, once, the head kept; nothing new is no
    /// write.
    #[test]
    fn what_was_learned_goes_into_its_list_once() {
        let learned = Learned {
            unskinnable: vec!["glacial morph".to_owned()],
            crumbly: vec!["dried flower".to_owned(), "dusty tome".to_owned()],
            unlootable: vec!["boulder".to_owned()],
            autoclose: vec!["leather backpack".to_owned()],
        };
        let written = remember(FILE, &learned).unwrap().unwrap();
        assert!(written.starts_with("# imported\n"), "{written}");
        let profile = LootProfile::parse(&written).unwrap();
        assert_eq!(profile.crumbly, ["dried flower", "dusty tome"]);
        assert_eq!(profile.unlootable, ["boulder"]);
        assert_eq!(profile.autoclose, ["leather backpack"]);
        assert_eq!(
            profile.skin.unskinnable,
            ["cave troll", "Krag Dweller", "glacial morph"]
        );
        assert_eq!(remember(&written, &learned).unwrap(), None);
        assert!(Learned::default().is_empty());
        assert_eq!(learned.lines().len(), 4);
    }

    /// eloot's four answers: cleared, removed by name in any case, not
    /// there, already empty.
    #[test]
    fn the_unskinnable_list_is_cleared_or_one_is_taken_off() {
        let (text, said) = forget_unskinnable(FILE, Some("krag dweller")).unwrap();
        let text = text.unwrap();
        assert_eq!(said, "removed \"krag dweller\" from the unskinnable list.");
        assert_eq!(
            LootProfile::parse(&text).unwrap().skin.unskinnable,
            ["cave troll"]
        );
        assert!(text.starts_with("# imported\n"));
        assert!(forget_unskinnable(&text, Some("wolf")).is_err());
        let (cleared, said) = forget_unskinnable(&text, None).unwrap();
        assert_eq!(said, "cleared 1 creature from the unskinnable list.");
        let cleared = cleared.unwrap();
        assert!(
            LootProfile::parse(&cleared)
                .unwrap()
                .skin
                .unskinnable
                .is_empty()
        );
        assert_eq!(
            forget_unskinnable(&cleared, Some("  ")).unwrap(),
            (None, "the unskinnable list is already empty.".to_owned())
        );
    }
}
