//! What an agent may never send to the game, whatever its level (`plan/35`
//! §3, "The denylist, even at Commands").
//!
//! > **AUTHOR, 2026-09-24:** start from LAB's list, *"we can start with that
//! > one"*. It grows the way the hunt's guard vocabulary does, from a real
//! > case, never ahead of one.
//!
//! # LAB's list
//!
//! `reference/lich-agent-bridge` at `016bcc9`, `src/lich_agent_bridge/actions.py`
//! (`_FORBIDDEN` and `evaluate`): no line with a chaining or control
//! character (`;`, `|`, `&`, a line break) or starting with `,`; and never a
//! line whose verb is **drop, discard, trash, sell, give, offer, exchange,
//! trade, mail, place, throw, hurl, empty, destroy or sacrifice**; nor
//! `unmark`, nor `mark ... remove`; nor `set nomarkeddrop off` or `set
//! saferdrop off`; nor a `put` onto the ground, the floor or the room. Here,
//! as there, a line that starts with the command symbol is a Hydra command,
//! which an agent runs through `perform` at its own level.
//!
//! # Three holes, each closed on a documented case
//!
//! - **`put` with no container drops.** The wiki's own example
//!   (`reference/wiki_clean/Verb_DROP.txt`): `>put my topaz` answers *You
//!   drop a clear topaz.* LAB denies only a `put` that names the ground; a
//!   `put` that names no container at all is denied here too.
//! - **Verbs are abbreviated.** `reference/wiki_clean/Verb_EXPERIENCE.txt`:
//!   the verb is *"commonly abbreviated to 'EXP'"*. A whole-word list lets
//!   `dro sword` through, so a first word that begins any denied verb is
//!   denied as that verb -- except the directions that happen to (`d`, `e`,
//!   `o`, `s`, `se`, `u`), which move the character and are what they look
//!   like. Which verb the game would take an abbreviation for is the game's;
//!   nothing here guesses, so any denied verb it could be is enough.
//! - **`_drag` onto `drop` drops.** `_drag #<item> <onto>` is the markup's
//!   own move, and Hydra sends `_drag #<item> drop` itself to put an item on
//!   the ground (`cena-behavior`'s `batch/build.rs` and `travel/routines/
//!   day_pass.rs`; the GUI's carry to the floor). A `_drag` whose destination
//!   is `drop`, or could be, or names the ground, is denied as a drop (the
//!   integrated crate review of 2026-09-28, I4). A `_drag` to a hand
//!   (`left`, `right`) or into a container (`#<id>`, `my cloak`) goes, as a
//!   `put` into a container does: Hydra's own behaviors stow with it.
//!
//! **What this is not**: a sandbox. It refuses the verbs that give things
//! away or destroy them, as LAB's does, and it is not a list of what is
//! safe. `plan/35` §3: the line that matters is Behaviors, below which
//! everything goes through curated code.

/// The verbs no agent line may begin with, nor with the start of one.
const VERBS: &[&str] = &[
    "drop",
    "discard",
    "trash",
    "sell",
    "give",
    "offer",
    "exchange",
    "trade",
    "mail",
    "place",
    "throw",
    "hurl",
    "empty",
    "destroy",
    "sacrifice",
    "unmark",
];

/// Movement words that begin a denied verb, and are movement all the same:
/// down, east, out, south, southeast, up.
const DIRECTIONS: &[&str] = &["d", "e", "o", "s", "se", "u"];

/// The drop guards `set` must not turn off.
const GUARDS: &[&str] = &["nomarkeddrop", "saferdrop"];

/// Where a `put` puts something that is a drop.
const GROUND: &[&str] = &["ground", "floor", "room"];

/// The words that name a container after `put`.
const INTO: &[&str] = &["in", "into", "on", "under", "behind"];

/// Why an agent may not send `line`, or `None` when the list does not deny
/// it. `symbol` is the character's command symbol.
#[must_use]
pub fn refused(line: &str, symbol: char) -> Option<String> {
    if line.chars().any(char::is_control) {
        return Some("one line, with no control characters".to_owned());
    }
    let folded = line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if folded.starts_with(symbol) {
        return Some(format!(
            "`{symbol}` begins a Hydra command, which `perform` runs at the behaviors level"
        ));
    }
    if folded.starts_with(',') {
        return Some("a line starting with `,` is a client's command, not the game's".to_owned());
    }
    if folded.contains([';', '|', '&']) {
        return Some("no `;`, `|` or `&`: one command, never a chain".to_owned());
    }
    let words: Vec<&str> = folded.split(' ').collect();
    let first = *words.first()?;
    let begins = |verb: &str| !DIRECTIONS.contains(&first) && verb.starts_with(first);
    if let Some(verb) = VERBS.iter().find(|verb| begins(verb)) {
        return Some(format!(
            "`{first}` may be {verb}, which an agent never sends: it gives away or destroys"
        ));
    }
    if begins("mark") && words.last() == Some(&"remove") {
        return Some("removing a mark takes away a drop guard".to_owned());
    }
    let guard = words
        .get(1)
        .is_some_and(|option| option.len() >= 3 && GUARDS.iter().any(|g| g.starts_with(option)));
    if first == "set" && guard && words.last() != Some(&"on") {
        return Some("that turns off a drop guard".to_owned());
    }
    if begins("put") {
        return put(&words[1..]);
    }
    if first == "_drag" {
        return drag(words.get(2..).unwrap_or_default());
    }
    None
}

/// Why a `_drag` whose destination is `onto` (the words after the item) is
/// a drop, if it is.
fn drag(onto: &[&str]) -> Option<String> {
    onto.iter()
        .any(|word| "drop".starts_with(word) || GROUND.contains(word))
        .then(|| "a `_drag` onto `drop` or the ground drops what it drags".to_owned())
}

/// Why a `put` with `rest` after its verb is a drop, if it is.
fn put(rest: &[&str]) -> Option<String> {
    let Some(at) = rest.iter().position(|word| INTO.contains(word)) else {
        return Some("a `put` that names no container drops what it puts".to_owned());
    };
    let mut after = rest[at + 1..].iter().copied();
    let target = match after.next() {
        Some("the") => after.next(),
        word => word,
    };
    match target {
        None => Some("a `put` that names no container drops what it puts".to_owned()),
        Some(place) if GROUND.contains(&place) => {
            Some(format!("putting something on the {place} drops it"))
        }
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::refused;

    fn denied(line: &str) -> bool {
        refused(line, ';').is_some()
    }

    /// LAB's list, whole.
    #[test]
    fn labs_list_is_denied() {
        for line in [
            "drop sword",
            "discard my gem",
            "trash pack",
            "sell gem",
            "give sword to Nerten",
            "offer 500 to Nerten",
            "exchange sword",
            "trade Nerten",
            "mail gem to Nerten",
            "place sword on table",
            "throw dagger at troll",
            "hurl rock",
            "empty my pack",
            "destroy note",
            "sacrifice gem",
            "unmark sword",
            "mark sword remove",
            "set nomarkeddrop off",
            "set saferdrop off",
            "put sword on the ground",
            "put sword in floor",
            "put sword into room",
            ",e echo",
            ";go2 bank",
            "look; drop sword",
            "look | drop",
            "look & drop",
            "look\ndrop sword",
            "DROP Sword",
        ] {
            assert!(denied(line), "{line:?}");
        }
    }

    /// The two holes: a `put` with no container, and an abbreviated verb.
    #[test]
    fn a_bare_put_and_an_abbreviation_are_denied() {
        for line in [
            "put my topaz",
            "put topaz in",
            "dro sword",
            "gi sword to Nerten",
            "sel gem",
            "tr Nerten",
            "unm sword",
            "set nomark off",
            "_drag #123 drop",
            "_DRAG  #123   DROP",
            "_drag #123 dro",
            "_drag #123 ground",
        ] {
            assert!(denied(line), "{line:?}");
        }
    }

    /// What the list does not deny: the directions that begin a denied verb,
    /// a `put` into a container, a guard turned on, and ordinary lines.
    #[test]
    fn what_is_not_on_the_list_goes() {
        for line in [
            "d",
            "e",
            "o",
            "s",
            "se",
            "u",
            "look",
            "exp",
            "say hello",
            "put topaz in my reticule",
            "put gem on the table",
            "stow sword",
            "get gem",
            "set nomarkeddrop on",
            "attack troll",
            "go gate",
            "_drag #123 #456",
            "_drag #123 right",
            "_drag #123 left",
            "_drag #77 my cloak",
        ] {
            assert!(!denied(line), "{line:?}: {:?}", refused(line, ';'));
        }
        // Another character's symbol is theirs to choose.
        assert!(refused("/go2 bank", '/').is_some());
        assert!(
            refused(";go2 bank", '/').is_some(),
            "`;` chains, whatever the symbol"
        );
    }
}
