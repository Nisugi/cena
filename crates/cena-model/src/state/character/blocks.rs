//! The command-output block machine: which multi-line report is open.
//!
//! M3 step 4. `info`, `skill`, `experience` and the rest arrive as **blobs** --
//! a command echo, some lines of prose, and a terminating prompt (`plan/15`
//! §2a.4b). One line of that prose means nothing on its own: `115 (32)` needs to
//! know it is inside `info`, and `  Ranger....|  162` needs to know it is inside
//! `skill`.
//!
//! That memory is what makes this a **consumer** rather than a classifier
//! (`plan/12` §3a): classifiers are stateless, so anything needing to remember
//! which report is open lives here.
//!
//! # Where it sits
//!
//! Below `GameState::route_text`, which already reassembles runs into lines via
//! `pending`. A completed line is offered here; the machine decides whether it
//! belongs to an open block and hands it to the right classifier.
//!
//! Reassembly matters more than it looks: an enhancive stat line is **five
//! frames** with one `ends_line` (`stats.rs`'s module doc measures it), so a
//! machine fed frames rather than lines would see `"106"` on its own.
//!
//! # What Lich does, and the two places this differs
//!
//! `infomon/parser.rb` has two mechanisms and they are worth separating:
//!
//! 1. Four `@*_hold` accumulator arrays, opened by a start pattern and committed
//!    by a terminator, with **the mutex being held as the only "am I inside a
//!    block" signal** -- and only the skills arm actually checks it
//!    (`parser.rb:304`).
//! 2. An explicit `State` FSM (`parser.rb:165-203`) for `Goals`, `Profile` and
//!    the six enhancive sections, which **raises** on an illegal transition.
//!
//! **Difference one: a block that never terminates.** In Lich, a disconnect
//! mid-`info` leaves the mutex locked and the hold array dirty; the next block's
//! start reassigns the array and calls `mutex_lock`, which is a no-op when
//! already held (`infomon.rb:54`). So the previous block's rows are silently
//! discarded and the mutex unlocks one level too shallow. Here, [`Blocks::end`]
//! is called at the terminating prompt **and** on reconnect, and an unterminated
//! block is dropped explicitly rather than by accident.
//!
//! **Difference two: no partial commit.** Lich pushes rows as it reads them and
//! commits at the terminator. This accumulates into a typed value and applies it
//! whole, so a truncated `info` cannot leave four stats updated and six stale.

use super::stats::{Identity, Stat, StatKind, StatLine};

/// Which multi-line command report is currently open.
///
/// **A named enum, not an `Option<&str>`.** The set is closed -- these are the 15
/// commands `Infomon.sync` issues (`infomon/cli.rb:19-33`) -- and a typo in a
/// string would silently open a block nothing ever closes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Block {
    /// No report open; ordinary game text.
    #[default]
    None,
    /// `info` or `info full`: identity and the ten statistics.
    Info,
}

/// The block machine, plus whatever the open block has accumulated.
///
/// Lives in [`super::Character`], not in `GameState` directly, because it is
/// character knowledge and because `state.rs` is a split parent with little
/// headroom (Rule 4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blocks {
    /// Which report is open.
    open: Block,
    /// Stat lines seen since the block opened, in wire order, each with
    /// whether the wire bolded its enhanced column.
    ///
    /// Held rather than applied line-by-line so a truncated block applies
    /// nothing -- see the module doc's "no partial commit".
    stats: Vec<(StatKind, StatLine, bool)>,
    /// Identity fields seen since the block opened.
    identity: Identity,
}

impl Blocks {
    /// Which block is open, if any.
    #[must_use]
    pub const fn open(&self) -> Block {
        self.open
    }

    /// Offer one **reassembled** line.
    ///
    /// Returns `true` if the line was recognised as part of a report -- either
    /// opening one or contributing to the open one. The caller still routes the
    /// line for display: recognising a line must not consume it, the rule
    /// `state.rs:320-324` records for the idle warning ("observing a line must
    /// not consume it").
    pub fn offer(&mut self, line: &str) -> bool {
        self.offer_with_bold(line, &[])
    }

    /// [`Self::offer`], plus the bold fragments the caller saw on this line.
    ///
    /// The caller did the reassembly, so it is the only thing that knows which
    /// runs were bold -- and bold is the wire's own enhancement signal
    /// (`stats.rs`'s module doc measures it). A caller with no bold information
    /// passes `&[]` and every stat reads as unenhanced, which is the honest
    /// answer when nothing observed it.
    pub fn offer_with_bold(&mut self, line: &str, bold: &[&str]) -> bool {
        // An opener is recognised whatever is open, because a report can follow
        // another with no prompt between when the user types two commands fast.
        if let Some(identity) = classify_identity(line) {
            self.open = Block::Info;
            self.stats.clear();
            self.identity = identity;
            return true;
        }
        if self.open == Block::None {
            return false;
        }
        if let Some((stat, bolded)) = StatLine::classify_with_bold(line, bold) {
            self.stats.push((stat.kind, stat, bolded));
            return true;
        }
        if let Some((gender, age)) = classify_gender_age(line) {
            self.identity.gender = Some(gender);
            self.identity.age = Some(age);
            return true;
        }
        false
    }

    /// End the open block at a prompt, returning what it accumulated.
    ///
    /// `None` when no block was open, or when the block carried nothing --
    /// **a caller must not be handed an empty update to apply.**
    ///
    /// Called at the terminating prompt, which is what `plan/15` §2a.4b
    /// establishes as a blob's end, and on reconnect, where an unterminated
    /// block is dropped.
    pub fn end(&mut self) -> Option<InfoReport> {
        let was = std::mem::replace(&mut self.open, Block::None);
        let stats = std::mem::take(&mut self.stats);
        let identity = std::mem::take(&mut self.identity);
        if was != Block::Info || stats.is_empty() {
            return None;
        }
        Some(InfoReport { stats, identity })
    }

    /// Drop any open block without applying it.
    ///
    /// For a reconnect: a block opened before the drop describes a session that
    /// is gone, and its remaining lines will never arrive.
    pub fn abandon(&mut self) {
        self.open = Block::None;
        self.stats.clear();
        self.identity = Identity::default();
    }
}

/// A complete `info` report, ready to fold into typed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoReport {
    /// The stat lines, in wire order, each with whether its enhanced column
    /// arrived bolded.
    pub stats: Vec<(StatKind, StatLine, bool)>,
    /// The identity fields the blob carried.
    pub identity: Identity,
}

impl InfoReport {
    /// Fold one stat line into a [`Stat`], preserving columns the line did not
    /// carry.
    ///
    /// **`normal` is only overwritten by a line that had it.** `info` sends two
    /// columns and `info full` three, so a plain `info` run after an `info full`
    /// must not erase the base values it never mentioned. That is the difference
    /// between "unknown" and "unchanged", and merging rather than replacing is
    /// what keeps it.
    #[must_use]
    pub fn merge_into(line: &StatLine, bolded: bool, previous: Stat) -> Stat {
        Stat {
            normal: line.normal().or(previous.normal),
            ascended: line.ascended().or(previous.ascended),
            enhanced: line.enhanced().or(previous.enhanced),
            // **Replaced, not `or`-ed.** Bold is a per-report observation: a
            // stat that stopped arriving bolded stopped being enhanced, because
            // the item was removed or paused. Carrying the old `true` forward
            // would make an expired enhancive permanent.
            enhanced_is_bolded: bolded,
        }
    }
}

/// Classify `info`'s opening line, which is also where race and profession live.
///
/// ```text
/// Name: Ashryn Race: Half-Elf  Profession: Ranger (shown as: Hero)
/// ```
///
/// This is Lich's own opener -- `parser.rb:10`'s `CharRaceProf`, used at `:238`
/// to reset the accumulator.
///
/// **Not the command echo.** `<c>info` arrives as `Frame::ClientCommand`, and
/// keying on that would open a block for a player who typed `info` into a chat
/// channel. This triple is server prose.
///
/// # Three things deliberately not taken from this line
///
/// * **The name.** `parser.rb:237` says so outright: *"name captured here, but
///   do not rely on it - use XML instead"*. `<playerID>` and the `<a exist>` link
///   are authoritative.
/// * **`(shown as: Hero)`.** A title, not a profession, and Lich strips it too.
/// * **Anything under Shroud of Deception.** See [`Identity`]'s docs -- the
///   caller decides, because only it knows whether 1212 is active.
fn classify_identity(line: &str) -> Option<Identity> {
    let rest = line.strip_prefix("Name: ")?;
    let (_name, rest) = rest.split_once(" Race: ")?;
    let (race, profession) = rest.split_once(" Profession: ")?;
    let race = race.trim();
    // `(shown as: X)` and `(not shown)` are display titles; the profession is
    // whatever precedes the parenthesis.
    let profession = profession
        .split_once('(')
        .map_or(profession, |(before, _)| before)
        .trim();
    if race.is_empty() || profession.is_empty() {
        return None;
    }
    Some(Identity {
        race: Some(race.to_owned()),
        profession: Some(profession.to_owned()),
        ..Identity::default()
    })
}

/// Classify `info`'s second line: gender and age.
///
/// ```text
/// Gender: Male    Age: 36    Expr: 43,904,921    Level:  100
/// ```
///
/// **`Expr:` and `Level:` are read and discarded**, as Lich discards them
/// (`parser.rb:246`: *"level captured here, but do not rely on it - use XML
/// instead"*). They change continuously and `info` is a snapshot;
/// `<dialogData id='expr'>` carries the live values.
fn classify_gender_age(line: &str) -> Option<(String, u32)> {
    let rest = line.strip_prefix("Gender: ")?;
    let (gender, rest) = rest.split_once(" Age: ")?;
    let age = rest
        .split_whitespace()
        .next()?
        .replace(',', "")
        .parse()
        .ok()?;
    let gender = gender.trim();
    if gender.is_empty() {
        return None;
    }
    Some((gender.to_owned(), age))
}
