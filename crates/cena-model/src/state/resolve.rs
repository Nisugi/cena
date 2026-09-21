//! What does a name mean? Turning `"kron"` into an item the game will accept.
//!
//! Ports the **pure half** of `lib/stash.rb`. See the module's own note below
//! on why only half.
//!
//! # stash.rb is two files wearing one name
//!
//! MEASURED over its 642 lines and 27 entry points: **45 send/wait/retry
//! calls** (`fput`, `dothistimeout`, `issue_command`, `wait_until`, `sleep`).
//! Splitting by whether a function sends anything:
//!
//! | | Functions | Lines | What it does |
//! |---|---:|---:|---|
//! | **Resolution** | 13 | ~140 | "which item does this name mean?" |
//! | **Manipulation** | 12 | ~460 | get it, wear it, swap hands, retry |
//!
//! `plan/20`'s audit budgeted stash as "~24 patterns", and that count is
//! real but misleading: **not one of them is a classifier.** Every regex in
//! the file is either a terminator for an `issue_command` wait
//! (`stash.rb:28`, `:98`, `:165`) or a guard inside a retry loop (`:46`,
//! `:55`, `:68`). There is no line the game sends that stash reads for a
//! fact; it sends commands and watches for the reply to stop.
//!
//! So the manipulation half is **`cena-behavior` work at M6**, where the
//! authority token and roundtime live. Building it here would put `fput` in
//! a crate that has no socket, which the crate graph forbids anyway. The
//! resolution half is model work, needs nothing but state, and is what
//! everything above it will ask first.
//!
//! # The ordering is the port's real content
//!
//! `find_items` (`stash.rb:309`) sorts by two keys, and the comment there
//! says why: it is "the order the game itself resolves a bare noun in".
//!
//! 1. **How specific the match is** -- the whole name beats part of it beats
//!    a noun-only hit (`match_specificity`, `:584`).
//! 2. **Where the item is** -- hands, then the ready list, then worn, then
//!    inside a container (`known_items_ranked`, `:573`).
//!
//! Both matter, and the second is why this cannot live below `GameState`: it
//! reads the hands, the ready list and the inventory tree at once.
//!
//! # One Lich behaviour NOT ported
//!
//! `find_container` (`stash.rb:13`) interpolates the caller's string straight
//! into a regex: `container.name =~ %r[#{param.strip}]i`. A profile naming a
//! bag `"pack (old)"` raises `RegexpError` and takes the script with it.
//! `name_matches?` (`:605`) fixes exactly this for items -- its comment says
//! "the pattern is escaped, so an item name or profile string carrying regex
//! metacharacters cannot raise" -- but `find_container` was never given the
//! same treatment. Here there is no regex at all: matching is over words, so
//! the question does not arise.

use crate::state::GameState;
use crate::state::containers::ReadySlot;

/// How well a name matched an item, best first.
///
/// `stash.rb:584`'s `match_specificity`, as a type rather than `0 | 1 | 2`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Specificity {
    /// The name is the item's whole name.
    Exact,
    /// The name is a run of whole words inside the item's name.
    Phrase,
    /// The name matched loosely -- a single word, or words in order but apart.
    Loose,
}

/// Where an item is, in the order the game resolves a bare noun.
///
/// `stash.rb:573`'s `known_items_ranked`, whose ranks are `0` hands, `1`
/// ready list, `2` worn, `3` in a container. The fourth rank there is
/// `inventory_matches`'s `4` (`:311`), for items known only from the
/// inventory tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Location {
    /// In a hand. Already held, so nothing needs fetching.
    Hand,
    /// Named by the ready list.
    Ready,
    /// Worn. Needs `remove`, not `get`.
    Worn,
    /// Inside a container that has been looked in.
    Container,
    /// Known only from the inventory snapshot -- possibly in a container
    /// nobody has opened.
    Inventory,
}

/// One candidate for a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    /// The item's exist id. **What a command targets.**
    pub id: String,
    /// Its display name.
    pub name: String,
    /// How well the name matched.
    pub specificity: Specificity,
    /// Where it is.
    pub location: Location,
}

/// Whether `name` matches `item`, and how well.
///
/// Ports `match_specificity` (`stash.rb:584`) and `name_matches?` (`:605`)
/// together, because in Lich they are two functions asking one question and
/// the second is only ever called to filter for the first.
///
/// **Word-based, not regex.** Lich builds a pattern per comparison; here the
/// three cases are tested directly, so no caller's string is ever compiled.
#[must_use]
pub fn match_of(item_name: &str, name: &str) -> Option<Specificity> {
    let wanted = name.trim().to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let actual = item_name.trim().to_lowercase();
    if actual == wanted {
        return Some(Specificity::Exact);
    }
    // A run of whole words: `"long bow"` in `"a scorched glowbark long bow"`,
    // but not `"ow"` in `"bow"`. Lich's `(?:\A|\s)...(?:\s|\z)` guard.
    if contains_words(&actual, &wanted) {
        return Some(Specificity::Phrase);
    }
    // Lich's loose form: the first word, then anything, then the rest
    // (`name_matches?`'s `split(/ /, 2)` joined with ` .*`). Words in order
    // but not adjacent.
    words_in_order(&actual, &wanted).then_some(Specificity::Loose)
}

/// Whether `haystack` contains `needle` on whole-word boundaries.
fn contains_words(haystack: &str, needle: &str) -> bool {
    haystack
        .match_indices(needle)
        .any(|(at, _)| starts_word(haystack, at) && ends_word(haystack, at + needle.len()))
}

/// Whether position `at` begins a word.
fn starts_word(text: &str, at: usize) -> bool {
    at == 0 || text[..at].ends_with(char::is_whitespace)
}

/// Whether position `at` ends a word.
fn ends_word(text: &str, at: usize) -> bool {
    at == text.len() || text[at..].starts_with(char::is_whitespace)
}

/// Whether every word of `needle` appears in `haystack`, in order, on word
/// boundaries.
fn words_in_order(haystack: &str, needle: &str) -> bool {
    let mut rest = haystack;
    for word in needle.split_whitespace() {
        let Some(at) = rest
            .match_indices(word)
            .find(|&(at, _)| starts_word(rest, at) && ends_word(rest, at + word.len()))
            .map(|(at, _)| at)
        else {
            return false;
        };
        rest = &rest[at + word.len()..];
    }
    true
}

impl GameState {
    /// Every item this name could mean, best first.
    ///
    /// Ports `find_items` (`stash.rb:309`). **Sends nothing** -- it reads the
    /// hands, the ready list and the inventory this state already holds, so
    /// the answer is only as good as what the game has said. A caller that
    /// needs certainty refreshes inventory first, which is a behavior's job.
    ///
    /// Ordered by [`Specificity`] then [`Location`], which `stash.rb:309`
    /// records as "the order the game itself resolves a bare noun in".
    /// Duplicate ids collapse; items with the **same name** collapse too,
    /// because Lich's comment at `:307` is right that they are
    /// interchangeable -- and a caller picking between two identical names
    /// has no basis to choose.
    #[must_use]
    pub fn find_items(&self, name: &str) -> Vec<Match> {
        let mut found: Vec<Match> = self
            .candidates()
            .filter_map(|(id, item_name, location)| {
                match_of(&item_name, name).map(|specificity| Match {
                    id,
                    name: item_name,
                    specificity,
                    location,
                })
            })
            .collect();
        // `sort_by_key` is stable, so equal (specificity, location) pairs keep
        // the order `candidates` yielded -- which is Lich's `with_index` tie
        // break (`stash.rb:311`), and what makes a replay deterministic.
        found.sort_by_key(|m| (m.specificity, m.location));
        let mut seen_ids = Vec::new();
        let mut seen_names = Vec::new();
        found.retain(|m| {
            let fresh = !seen_ids.contains(&m.id) && !seen_names.contains(&m.name);
            if fresh {
                seen_ids.push(m.id.clone());
                seen_names.push(m.name.clone());
            }
            fresh
        });
        found
    }

    /// The single best item this name means, or `None`.
    ///
    /// `find_item` (`stash.rb:287`), minus its `fail`: a caller here decides
    /// what an unresolvable name means, because at this layer it is a
    /// question, not an error.
    #[must_use]
    pub fn find_item(&self, name: &str) -> Option<Match> {
        self.find_items(name).into_iter().next()
    }

    /// Which hand holds this id, if either.
    ///
    /// `hand_holding` (`stash.rb:367`). **By id, not name** -- which is what
    /// [`Hand`](super::hands::Hand) carrying the wire's `exist=` bought.
    #[must_use]
    pub fn hand_holding(&self, id: &str) -> Option<Held> {
        if self.right_hand.holds(id) {
            return Some(Held::Right);
        }
        self.left_hand.holds(id).then_some(Held::Left)
    }

    /// Every item the game has named, with where it is.
    ///
    /// `known_items_ranked` (`stash.rb:573`) plus `inventory_matches`'s
    /// fallback rank, as one pass. Yielded in [`Location`] order so the sort
    /// above is stable against it.
    fn candidates(&self) -> impl Iterator<Item = (String, String, Location)> + '_ {
        let hands = [&self.right_hand, &self.left_hand]
            .into_iter()
            .filter_map(|hand| {
                Some((
                    hand.id()?.to_owned(),
                    hand.name()?.to_owned(),
                    Location::Hand,
                ))
            });
        let ready = ReadySlot::ALL.into_iter().filter_map(|slot| {
            let item = self.containers.ready(slot)?;
            Some((item.id.clone(), item.text.clone(), Location::Ready))
        });
        let carried = self.inventory_snapshot.all().map(|item| {
            // `worn,player` is worn; anything else the snapshot lists is
            // inside something. The distinction is Lich's `worn?`
            // (`stash.rb:534`) and it decides `remove` against `get`.
            let where_it_is = if item.relation == "worn" {
                Location::Worn
            } else {
                Location::Inventory
            };
            (item.id.clone(), item.name.clone(), where_it_is)
        });
        let in_containers = self.inventory.containers().flat_map(|(_, container)| {
            container
                .items
                .iter()
                .map(|item| (item.id.clone(), item.text.clone(), Location::Container))
        });
        hands.chain(ready).chain(carried).chain(in_containers)
    }
}

/// Which hand holds something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// The right hand.
    Right,
    /// The left hand.
    Left,
}
