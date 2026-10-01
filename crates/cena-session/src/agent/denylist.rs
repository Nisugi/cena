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
//! # The holes, each closed on a documented case
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
//!   integrated crate review of 2026-09-28, I4).
//! - **A `_drag` or a `put` into what is not the character's.** `_drag
//!   #<item> #<player>` gives the item away and `_drag #<item> #<bin>`
//!   destroys it (the author, 2026-10-01); a `put` into a bin is TRASH's own
//!   example (`reference/wiki_clean/Verb_TRASH.txt`). So either goes only to
//!   a hand (`left`, `right`, for a `_drag`), to `my <container>`, or to an
//!   `#id` the model shows the character carries
//!   (`cena_model::GameState::carries`), which the session checks as it
//!   writes the line (`destination`, `actor/io.rs`): an id it does not know
//!   is refused, never sent. Anything else -- a noun, which the game may
//!   find in the room, `drop`, the ground -- is refused here (the crate
//!   review of 2026-10-01, L-1).
//! - **More verbs that drop, give or destroy** (the same review, SE-B-1):
//!   `toss`, `break`, `tear`, `share` and `pay`, each cited at `VERBS`.
//! - **A quit by another spelling.** The game takes `qui` as `quit` (the
//!   author, 2026-10-01). A certain quit (`quit`, `qui`, `exit`, `exi`) goes
//!   on to the session's quit gate, which lets an agent log the character
//!   out only holding a takeover; one that only may be (`q`, `qu`, `ex`) is
//!   refused here, since the game, not the session, would decide it
//!   (`actor/ending.rs`, the same review, L-2).
//!
//! **What this is not**: a sandbox. It refuses the verbs that give things
//! away or destroy them, as LAB's does, and it is not a list of what is
//! safe. `plan/35` §3: the line that matters is Behaviors, below which
//! everything goes through curated code.

use crate::actor::ending::{is_exit_intent, may_be_exit};

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
    // Beyond LAB's list, each on the wiki's own words (the crate review of
    // 2026-10-01, SE-B-1): `toss emerald` answers *"You toss your emerald
    // onto the floor"* (`Verb_TOSS.txt`); BREAK *"is used to destroy an
    // item"* (`Verb_BREAK.txt`); TEAR makes pages and cards into confetti
    // (`Verb_TEAR.txt`); SHARE gives coins to the group (`Verb_SHARE.txt`);
    // PAY gives them to a clerk (`Verb_PAY.txt`).
    "toss",
    "break",
    "tear",
    "share",
    "pay",
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
    if may_be_exit(&folded) && !is_exit_intent(&folded) {
        return Some(format!(
            "`{folded}` may be quit; an agent that may log the character out says `quit`"
        ));
    }
    let words: Vec<&str> = folded.split(' ').collect();
    let first = *words.first()?;
    if let Some(verb) = VERBS.iter().find(|verb| begins(first, verb)) {
        return Some(format!(
            "`{first}` may be {verb}, which an agent never sends: it gives away or destroys"
        ));
    }
    if begins(first, "mark") && words.last() == Some(&"remove") {
        return Some("removing a mark takes away a drop guard".to_owned());
    }
    let guard = words
        .get(1)
        .is_some_and(|option| option.len() >= 3 && GUARDS.iter().any(|g| g.starts_with(option)));
    if first == "set" && guard && words.last() != Some(&"on") {
        return Some("that turns off a drop guard".to_owned());
    }
    match onto(&words) {
        Some(Onto::Not(why)) => Some(why),
        Some(Onto::Safe | Onto::Id(_)) | None => None,
    }
}

/// The `#id` an agent's `_drag` or `put` puts something into, which the
/// session lets it write only while the model shows the character carries
/// it (`GameState::carries`, checked at the write in `actor/io.rs`). `None`
/// for any other line, and for one [`refused`] denies or lets go on its
/// words alone.
#[must_use]
pub(crate) fn destination(line: &str) -> Option<String> {
    let folded = line.to_lowercase();
    let words: Vec<&str> = folded.split_whitespace().collect();
    match onto(&words) {
        Some(Onto::Id(id)) => Some(id.to_owned()),
        _ => None,
    }
}

/// Where a `_drag` or a `put` puts what it moves, as far as its words say.
enum Onto<'a> {
    /// A hand, or `my` something: the character's own, by the game's word.
    Safe,
    /// An object by id: the character's own only if the model says so.
    Id(&'a str),
    /// Anywhere else, and why it is refused.
    Not(String),
}

/// Where the `_drag` or `put` in `words` (folded, lowercase) puts what it
/// moves; `None` for any other line.
///
/// Only the character's own hands and containers are safe: a `_drag` onto
/// a player gives the item away, and onto a bin destroys it (the author,
/// 2026-10-01), and a `put` into a bin is the trash verb's own example
/// (`reference/wiki_clean/Verb_TRASH.txt`). A noun names whatever the game
/// finds first, the room's included, so only `my <noun>` and an `#id` the
/// model knows are trusted.
fn onto<'a>(words: &[&'a str]) -> Option<Onto<'a>> {
    let first = *words.first()?;
    if first == "_drag" {
        let not = || {
            Onto::Not(
                "a `_drag` goes only to a hand, `my <container>`, or a container the character carries by `#id`: anywhere else it may drop, give or destroy what it drags"
                    .to_owned(),
            )
        };
        return Some(match words.get(2..).unwrap_or_default() {
            ["left" | "right"] | ["my", _, ..] => Onto::Safe,
            [place] => by_id(place).map_or_else(not, Onto::Id),
            // `_drag #1 #2` names one place; more words are not one.
            _ => not(),
        });
    }
    if !begins(first, "put") {
        return None;
    }
    let rest = &words[1..];
    let Some(at) = rest.iter().position(|word| INTO.contains(word)) else {
        return Some(Onto::Not(
            "a `put` that names no container drops what it puts".to_owned(),
        ));
    };
    let after = &rest[at + 1..];
    let target = after.strip_prefix(&["the"]).unwrap_or(after);
    if let [place] = target
        && let Some(id) = by_id(place)
    {
        return Some(Onto::Id(id));
    }
    Some(match target {
        [] => Onto::Not("a `put` that names no container drops what it puts".to_owned()),
        ["my", _, ..] => Onto::Safe,
        [place, ..] if GROUND.contains(place) => {
            Onto::Not(format!("putting something on the {place} drops it"))
        }
        _ => Onto::Not(
            "a `put` goes only into `my <container>`, or a container the character carries by `#id`: a bin or another's takes what is put in it"
                .to_owned(),
        ),
    })
}

/// The id `word` names, `#123` or `#-123`.
fn by_id(word: &str) -> Option<&str> {
    word.strip_prefix('#').filter(|id| !id.is_empty())
}

/// Whether a first word `first` may be `verb` abbreviated: it begins it,
/// and is not one of the directions that happen to.
fn begins(first: &str, verb: &str) -> bool {
    !DIRECTIONS.contains(&first) && verb.starts_with(first)
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

    /// The crate review of 2026-10-01: verbs that drop, give or destroy
    /// beyond LAB's list (SE-B-1), a `_drag` or `put` onto a player, a bin
    /// or anything not known to be the character's own (L-1), and a line
    /// that may be a quit but is not certainly one (L-2).
    #[test]
    fn what_else_drops_gives_destroys_or_may_quit_is_denied() {
        for line in [
            "toss emerald",
            "tos emerald",
            "break clod",
            "tear card",
            "share 500",
            "pay 100",
            "_drag #123 Nerten",
            "_drag #123 bin",
            "_drag #123 in barrel",
            "_drag #123",
            "_drag #123 #456 drop",
            "_drag #123 the barrel",
            "put gem in barrel",
            "put gem in the barrel",
            "put gem on table",
            "put gem in Nerten",
            "put gem in my",
            "q",
            "qu",
            "ex",
            "<c>qu",
        ] {
            assert!(denied(line), "{line:?}");
        }
    }

    /// Where an agent's `_drag` or `put` may go: a hand, `my` something, or
    /// an id, which the session checks against the model as it writes
    /// ([`super::destination`]).
    #[test]
    fn a_destination_by_id_is_left_to_the_model() {
        use super::destination;
        assert_eq!(destination("_drag #123 #456"), Some("456".to_owned()));
        assert_eq!(destination("_DRAG #123  #-77"), Some("-77".to_owned()));
        assert_eq!(destination("put #1 in #456"), Some("456".to_owned()));
        assert_eq!(destination("put gem in the #456"), Some("456".to_owned()));
        for line in [
            "_drag #1 right",
            "_drag #1 my cloak",
            "put gem in my sack",
            "look",
        ] {
            assert_eq!(destination(line), None, "{line:?}");
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
            "put gem on my belt",
            "put gem in #456",
            "stow sword",
            "get gem",
            "set nomarkeddrop on",
            "attack troll",
            "go gate",
            "_drag #123 #456",
            "_drag #123 right",
            "_drag #123 left",
            "_drag #77 my cloak",
            // A certain quit goes to the session's quit gate, which lets an
            // agent log out only holding a takeover (`actor/ending.rs`).
            "quit",
            "qui",
            "exit",
            "exi",
            "exp",
            "tell",
            "bow",
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
