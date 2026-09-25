//! Lines the player types **for Hydra, not for the game** (author,
//! 2026-09-21: *"there should be a command symbol that indicates it's a
//! command and don't send it to the game"*).
//!
//! # The symbol decides, not the word
//!
//! A line that begins with the symbol is Hydra's, **whether or not anything
//! knows the word**. `;go22 bank` is a mistyped `;go2`, and sending it to the
//! game says `;go22 bank` in the room. So an unrecognised command is answered
//! with "I do not know that", and the game never hears it. The alternative --
//! each command deciding for itself, and anything unclaimed falling through --
//! is what this replaces.
//!
//! Lich's symbol is `;` (`$lich_char`, `lich.rbw`), and it is the one in
//! everyone's fingers, so it is the default. It is **configurable**, because a
//! game command could begin with it one day and because a player may already
//! use it for something else.
//!
//! # A line without the symbol, taken on purpose
//!
//! One exception, and the player's to switch on: a behavior may take a line
//! that has no symbol ([`Bare`]), as spellcaster takes a typed `401 bob`
//! (author, 2026-09-25: *"I don't want to have to type ;sc to cast a spell
//! with just the number"*). It sees the line only after the symbol has
//! passed on it, and a line it does not take is the game's as before.
//!
//! # This crate knows nothing about commands
//!
//! It holds the symbol, splits the line, and asks whoever registered. What
//! `;go2 bank` *means* is the behavior's (`cena_behavior::travel::Desk`); the
//! binary is what joins the two, which is the same layering as
//! `plan/12` §3a's "one parser, N classifiers".

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

/// Lich's `$lich_char`, and every `GemStone` player's habit.
pub const DEFAULT_SYMBOL: char = ';';

/// The `commands` section of a character's settings (`settings_store`):
///
/// ```json
/// { "commands": { "symbol": "/" } }
/// ```
///
/// A section with no symbol, or one that is not a single character, keeps
/// [`DEFAULT_SYMBOL`] -- a player who mistypes their own preference gets the
/// one their fingers know, not a session where nothing is a command.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// The command symbol as the file spells it. Only a single character is
    /// used; `None` or anything else means `DEFAULT_SYMBOL`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// The name of [`Settings`]' section.
pub const SECTION: &str = "commands";

impl Settings {
    /// The symbol these settings choose. See [`Settings`] for what a symbol
    /// that is not one character does.
    #[must_use]
    pub fn symbol(&self) -> char {
        self.symbol
            .as_deref()
            .and_then(|said| {
                let mut letters = said.chars();
                letters.next().filter(|_| letters.next().is_none())
            })
            .unwrap_or(DEFAULT_SYMBOL)
    }
}

/// What a claimant did with a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claimed {
    /// It was run, or answered. The game never sees it.
    Done,
    /// Nothing knows this one. **Still not the game's**: the caller says so.
    Unknown,
}

/// Whoever runs the player's commands. One per session, registered by the
/// binary once it has built the behaviors.
///
/// Called with the line **less its symbol**, trimmed of nothing else: a
/// command's own arguments are its own business.
pub type Runner = Arc<dyn Fn(&str) -> Claimed + Send + Sync>;

/// Who may take a line typed **without** the symbol: `true` when it took
/// the line, which then never reaches the game.
pub type Bare = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// The symbol, and who runs what it marks. Shared by a handle and all its
/// clones, filled once the behaviors exist -- the same shape as the player
/// log's `Slot`, for the same reason: a session is built before the things
/// that hang off it.
pub type Slot = Arc<OnceLock<Desk>>;

/// What a session does with typed lines.
#[derive(Clone)]
pub struct Desk {
    /// A `char` as its `u32`, so it can change after the desk is installed:
    /// the desk goes in when the session is built, and the character's own
    /// symbol is only known once the login says who it is.
    symbol: Arc<AtomicU32>,
    runner: Runner,
    /// Who takes a line with no symbol, once registered.
    bare: Arc<OnceLock<Bare>>,
}

impl std::fmt::Debug for Desk {
    /// By hand, because a [`Runner`] is a closure and has no `Debug`. The
    /// symbol is the only part anyone could print.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desk")
            .field("symbol", &self.symbol())
            .finish_non_exhaustive()
    }
}

impl Desk {
    /// `symbol` is what marks a command; `None` is [`DEFAULT_SYMBOL`].
    #[must_use]
    pub fn new(symbol: Option<char>, runner: Runner) -> Desk {
        Desk {
            symbol: Arc::new(AtomicU32::new(u32::from(symbol.unwrap_or(DEFAULT_SYMBOL)))),
            runner,
            bare: Arc::new(OnceLock::new()),
        }
    }

    /// Let `bare` look at lines typed without the symbol. Once: `false` if
    /// something already does.
    #[must_use]
    pub fn set_bare(&self, bare: Bare) -> bool {
        self.bare.set(bare).is_ok()
    }

    /// The symbol this session marks commands with.
    #[must_use]
    pub fn symbol(&self) -> char {
        char::from_u32(self.symbol.load(Ordering::Relaxed)).unwrap_or(DEFAULT_SYMBOL)
    }

    /// Mark commands with `symbol` from now on: the character's preference,
    /// learned after the desk was installed (author, 2026-09-23: the command
    /// line is "loaded on startup by default", not when a feature is ready).
    pub fn set_symbol(&self, symbol: char) {
        self.symbol.store(u32::from(symbol), Ordering::Relaxed);
    }

    /// What to do with a typed line. `None`: it is the game's.
    ///
    /// A line without the symbol is offered to [`Self::set_bare`]'s taker,
    /// if one is registered, and is the game's unless it takes it.
    ///
    /// **Leading whitespace is allowed before the symbol** and nothing else
    /// is: `  ;go2 bank` is a command, `say ;go2 bank` is speech.
    #[must_use]
    pub fn claim(&self, line: &str) -> Option<Claimed> {
        let Some(rest) = line.trim_start().strip_prefix(self.symbol()) else {
            let taken = self.bare.get().is_some_and(|bare| bare(line.trim()));
            return taken.then_some(Claimed::Done);
        };
        // The symbol alone is not a command, and is not the game's either.
        if rest.trim().is_empty() {
            return Some(Claimed::Unknown);
        }
        Some((self.runner)(rest))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// A desk that remembers what it was given and knows only `go2`.
    fn desk(symbol: Option<char>) -> (Desk, Arc<Mutex<Vec<String>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&seen);
        let runner: Runner = Arc::new(move |line: &str| {
            kept.lock().map(|mut seen| seen.push(line.to_owned())).ok();
            if line.split_whitespace().next() == Some("go2") {
                Claimed::Done
            } else {
                Claimed::Unknown
            }
        });
        (Desk::new(symbol, runner), seen)
    }

    /// The author's rule: `;go22 bank` must never reach the game.
    #[test]
    fn a_line_with_the_symbol_is_hydras_known_or_not() {
        let (desk, seen) = desk(None);
        assert_eq!(desk.claim(";go2 bank"), Some(Claimed::Done));
        assert_eq!(desk.claim(";go22 bank"), Some(Claimed::Unknown));
        assert_eq!(desk.claim(";"), Some(Claimed::Unknown));
        assert_eq!(desk.claim(";   "), Some(Claimed::Unknown));
        // The symbol is gone, and nothing else is.
        assert_eq!(
            *seen.lock().unwrap(),
            ["go2 bank".to_owned(), "go22 bank".to_owned()]
        );
    }

    #[test]
    fn a_line_without_it_is_the_games_and_is_not_even_looked_at() {
        let (desk, seen) = desk(None);
        for line in ["north", "say ;go2 bank", "", "  ", "go2 bank"] {
            assert_eq!(desk.claim(line), None, "{line:?}");
        }
        assert!(seen.lock().unwrap().is_empty());
    }

    /// A taker for bare lines sees them, and only what it takes leaves the
    /// game's stream.
    #[test]
    fn a_bare_line_is_offered_to_a_taker_and_kept_only_if_taken() {
        let (desk, seen) = desk(None);
        assert!(desk.set_bare(Arc::new(|line: &str| line == "401")));
        assert!(!desk.set_bare(Arc::new(|_: &str| true)), "once");
        assert_eq!(desk.claim("  401 "), Some(Claimed::Done));
        assert_eq!(desk.claim("north"), None);
        assert_eq!(desk.claim(";go2 bank"), Some(Claimed::Done));
        assert_eq!(*seen.lock().unwrap(), ["go2 bank".to_owned()]);
    }

    /// Leading space is a typo, not speech.
    #[test]
    fn the_symbol_may_be_led_up_to_by_whitespace() {
        let (desk, _) = desk(None);
        assert_eq!(desk.claim("  ;go2 bank"), Some(Claimed::Done));
        assert_eq!(desk.claim("\t;go2 bank"), Some(Claimed::Done));
    }

    #[test]
    fn the_symbol_a_settings_section_chooses() {
        let said = |symbol: Option<&str>| {
            Settings {
                symbol: symbol.map(str::to_owned),
            }
            .symbol()
        };
        assert_eq!(said(Some("/")), '/');
        assert_eq!(said(None), DEFAULT_SYMBOL);
        // Neither of these is a symbol, and neither leaves the player without
        // one: `;` is what their fingers know.
        assert_eq!(said(Some("")), DEFAULT_SYMBOL);
        assert_eq!(said(Some("//")), DEFAULT_SYMBOL);
        // One character, whatever it is made of.
        assert_eq!(said(Some("\u{a7}")), '\u{a7}');
    }

    #[test]
    fn the_symbol_is_the_players_to_choose() {
        let (desk, _) = desk(Some('/'));
        assert_eq!(desk.claim("/go2 bank"), Some(Claimed::Done));
        assert_eq!(desk.symbol(), '/');
        // ...and then the old one is the game's again, as it was before.
        assert_eq!(desk.claim(";go2 bank"), None);
    }
}
