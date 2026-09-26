//! Maneuvers the game says are on cooldown.
//!
//! # Why this is model work and not a behavior's
//!
//! Lich's PSM machinery is almost entirely about **sending**: `psms.rb:260`
//! builds a per-command `results_regex` so a script that sends `cman bullrush`
//! can recognise its own reply, and the eleven `FAILURES_REGEXES` beside it are
//! the other half of that. All of it needs the authority token and a roundtime,
//! so it is M6 -- the same reading/sending split `stash.rb`, `bank.rb` and
//! `fog.rb` took (`plan/20` section 0b).
//!
//! One line in that set is different: **`<Name> is still in cooldown.` is a
//! fact about the character, not an answer to a command.** It arrives whether
//! or not anything was waiting for it, and it names the maneuver, so a model
//! can hold it and a behavior can ask rather than guess.
//!
//! MEASURED over the author's combat logs (`GST-Nisugi`, 390 files):
//!
//! ```text
//! 15 cooldown lines, naming 5 distinct maneuvers:
//!   Volley, Barrage, Whirlwind, Guardant Thrusts, Pulverize
//! ```
//!
//! and verified on the raw wire, so it is game text rather than a client
//! artifact (`2025-09-04_05-28-18.xml`):
//!
//! ```text
//! Barrage is still in cooldown.
//! <prompt time="1756984481">&gt;</prompt>
//! ```
//!
//! # What this does NOT know, and will not pretend to
//!
//! **How long is left.** The line carries no duration and the game sends no
//! other statement of one -- so this records *"the game said this was on
//! cooldown, at this server second"* and nothing more. A caller wanting "is it
//! ready" has [`Maneuvers::said_at`] and the game clock and can decide how stale a
//! reading it will trust; inventing a duration here would be the
//! `plan/18` `pbarStance` mistake, an interface over data nobody has measured.
//!
//! # That it has ended: the game does say so
//!
//! > **CORRECTED 2026-09-25** (`inventory/12` §1.3). This said *"Nothing
//! > says so"*, and the workspace's own fixture refutes it: `Volley is ready
//! > for use.` (`cena-behavior/tests/fixtures/smithy_engage.xml:271`), which
//! > `Combatical.lic:4553-4556` reads as the end, `^(\w[\w\s]+?) is ready
//! > for use\.$`. [`ready_line`] reads it, and [`Maneuvers::note_ready`]
//! > forgets the refusal: a maneuver the game has said is ready is no longer
//! > listed as cooling.
//!
//! Still no `is_cooling`: a refusal with no ready line after it may be
//! stale, and *"when was I last told"* stays the honest question.

use std::collections::BTreeMap;

/// What the game has said about maneuver cooldowns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Maneuvers {
    /// Maneuver name as the game spelled it -> the server second we were told.
    ///
    /// The game's own spelling and capitalisation (`Guardant Thrusts`), not a
    /// normalised mnemonic: this crate has no PSM name table and Lich's
    /// `name_normal` belongs with the sending half. A caller matching against
    /// `psm.rs`'s rows can normalise at the point of comparison.
    cooling: BTreeMap<String, Option<u32>>,
}

impl Maneuvers {
    /// Record that the game refused a maneuver as still cooling.
    ///
    /// `now` is the server second from the last prompt, or `None` when no clock
    /// is known -- which is a real state after a reconnect, and is why the
    /// value is an `Option` rather than a defaulted zero.
    ///
    /// Returns whether this changed anything.
    pub fn note_cooling(&mut self, name: &str, now: Option<u32>) -> bool {
        self.cooling.insert(name.to_owned(), now) != Some(now)
    }

    /// The game said this maneuver is ready for use: forget its refusal.
    /// Returns whether one was held.
    pub fn note_ready(&mut self, name: &str) -> bool {
        self.cooling.remove(name).is_some()
    }

    /// When the game last said this maneuver was cooling.
    ///
    /// `None`: never said. `Some(None)`: said, with no clock known.
    #[must_use]
    pub fn said_at(&self, name: &str) -> Option<Option<u32>> {
        self.cooling.get(name).copied()
    }

    /// Every maneuver the game has refused, in name order.
    pub fn cooling(&self) -> impl Iterator<Item = (&str, Option<u32>)> {
        self.cooling.iter().map(|(n, at)| (n.as_str(), *at))
    }

    /// How many have been refused.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cooling.len()
    }

    /// Whether the game has refused any.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cooling.is_empty()
    }

    /// Forget everything: a reconnect.
    ///
    /// **Kept deliberately simple, and cleared rather than kept**, unlike
    /// `roundtime_ends`: a cooldown with no known duration cannot be reasoned
    /// about across a gap of unknown length, and a stale "was cooling" is worse
    /// than no reading because it cannot expire.
    pub fn clear(&mut self) {
        self.cooling.clear();
    }
}

/// The maneuver a `<Name> is ready for use.` line names, if the line is one
/// (`Combatical.lic:4553-4556`: a word, then words and spaces).
#[must_use]
pub fn ready_line(line: &str) -> Option<&str> {
    let name = line.trim().strip_suffix(" is ready for use.")?;
    let mut chars = name.chars();
    let first = chars.next()?;
    let wordy = |c: char| c.is_alphanumeric() || c == '_';
    (wordy(first) && chars.all(|c| wordy(c) || c.is_whitespace())).then_some(name)
}

/// The maneuver named by a cooldown refusal, if the line is one.
///
/// `psms.rb:260`: `/^#{name} is still in cooldown\./i`. Lich builds that per
/// command because it knows the name it sent; read the other way round, the
/// line itself names the maneuver.
///
/// **A leading client timestamp is tolerated.** MEASURED: the author's logs
/// carry both `Barrage is still in cooldown.` and `23:53:12: Volley is still in
/// cooldown.` -- the second from a client with per-line timestamps on. The
/// prefix is not game text and never reaches the model through
/// `GameState`, but a classifier that could not survive one would be brittle
/// for no reason.
#[must_use]
pub fn cooldown_refusal(line: &str) -> Option<&str> {
    let text = line.trim();
    let name = text.strip_suffix(" is still in cooldown.")?;
    // Strip an `HH:MM:SS: ` prefix if one is there.
    let name = match name.split_once(": ") {
        Some((stamp, rest))
            if stamp.len() == 8 && stamp.chars().all(|c| c.is_ascii_digit() || c == ':') =>
        {
            rest
        }
        _ => name,
    };
    // **No empty-name guard, and that is MEASURED rather than assumed.** One
    // was here; a mutation removing it left every test green, because `trim`
    // reduces `" is still in cooldown."` to the suffix itself and
    // `strip_suffix` then fails first. A guard no input can reach is a line
    // that cannot be tested, which `plan/19`'s standing finding says to delete
    // rather than cover with a test that proves nothing.
    Some(name)
}
