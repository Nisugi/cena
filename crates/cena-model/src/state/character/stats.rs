//! The ten statistics, and the classifier that reads an `info` line.
//!
//! M3 step 3. `research/04-inherited-decisions.md:1878` (C21) asks for typed
//! named fields; these are ten of them, plus the four scalars `info` carries
//! alongside.
//!
//! # Three columns, and the wire sends two or three of them
//!
//! `info` prints two value columns and `info full` prints three:
//!
//! ```text
//!                 Ascended (Bonus)  ...  Enhanced (Bonus)          <- `info`
//!     Strength (STR):   115 (32)    ...  115 (32)
//!
//!        Normal (Bonus) ... Ascended (Bonus) ... Enhanced (Bonus)  <- `info full`
//!     Strength (STR):  110 (30)    ...  115 (32)    ...  115 (32)
//! ```
//!
//! Lich handles both with one regex whose FIRST group is optional
//! (`infomon/parser.rb:14`), so a 2-column line matches with `normal` absent.
//! That is worth stating because it looks like a bug and is not: I believed the
//! corpus showed a layout change that would break the port, tested it, and the
//! regex handled both forms. The port is faithful, not patched.
//!
//! **So `normal` is only known after `info full`.** `Option` per column, not one
//! `Option` for the row.
//!
//! # Enhancement is marked by BOLD, not by arithmetic
//!
//! The wire bolds an enhanced number:
//!
//! ```text
//!    Intuition (INT):    98 (24)    ...  <pushBold/>106<popBold/> (<pushBold/>28<popBold/>)
//! ```
//!
//! MEASURED through Cena's parser -- that line is **five frames**, only the last
//! carrying `ends_line`:
//!
//! ```text
//! bold=0 ends=false "   Intuition (INT):    98 (24)    ...  "
//! bold=1 ends=false "106"
//! bold=0 ends=false " ("
//! bold=1 ends=false "28"
//! bold=0 ends=true  ")"
//! ```
//!
//! A plain stat line is ONE frame with `ends_line=true`. So:
//!
//! * **Reassembly is the consumer's job** -- see `state.rs`'s `pending`, whose
//!   doc records that this same split "printed the author's worn inventory down
//!   the screen". A classifier may not assume one frame is one line.
//! * **`bold_depth` is the enhancement signal**, available without a regex.
//!   Lich has to infer enhancement by comparing columns; Cena is told. That is
//!   `plan/12` §3a's bargain paying off where nobody designed it.
//!
//! [`StatLine::classify`] takes the reassembled text because that is what a
//! classifier is (§3a: stateless, one line in, typed answer out). The bold runs
//! are carried separately, by [`StatLine::classify_runs`], for the caller that
//! has them.

use super::vocabulary::AccountType;

/// One value column: the stat and its bonus.
///
/// **Both signed.** `infomon/parser.rb:14` captures `(?<bonus>-?[0-9]+)`, and
/// Lich's own fixture asserts `Aura (AUR): 100 (-35)` -> `-35`
/// (`spec/lib/gemstone/infomon_spec.rb:91`). A bonus is genuinely negative for a
/// stat below the racial mean, so `u16` would wrap it into nonsense.
///
/// `i16` rather than `i32`: stats cap well under 32,767 and the type says so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StatValue {
    /// The stat itself, e.g. 115.
    pub value: i16,
    /// The bonus it confers, e.g. 32. Negative below the racial mean.
    pub bonus: i16,
}

/// One statistic, across every column the wire has shown.
///
/// # Why three `Option`s and not one
///
/// The columns arrive from different commands and are independently unknown.
/// `info` gives `ascended` and `enhanced`; only `info full` adds `normal`. A
/// single `Option<...>` for the row could not express "we ran `info` but not
/// `info full`", which is the common case.
///
/// `plan/12` §5.2: "`Unknown` is a first-class value, not a default." A zeroed
/// `StatValue` is indistinguishable from a real stat of 0 with no bonus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stat {
    /// The base value, before ascension and enhancives. `info full` only.
    pub normal: Option<StatValue>,
    /// After ascension. `info`'s first column.
    pub ascended: Option<StatValue>,
    /// After enhancive items. `info`'s last column.
    pub enhanced: Option<StatValue>,
    /// Whether the wire **bolded** the enhanced column.
    ///
    /// The game's own signal that an enhancive is contributing, so this is
    /// observed rather than derived. Comparing `ascended` to `enhanced` would
    /// answer the same question most of the time and be wrong whenever an
    /// enhancive contributes exactly zero -- which happens while an item is
    /// depleted or paused.
    pub enhanced_is_bolded: bool,
}

impl Stat {
    /// The most authoritative value the wire has given: enhanced, else
    /// ascended, else normal.
    ///
    /// This is what a behavior reading "how strong am I" wants, and it is
    /// `Option` because a character whose `info` has never been run has no
    /// answer -- not a zero.
    #[must_use]
    pub const fn effective(&self) -> Option<StatValue> {
        match (self.enhanced, self.ascended, self.normal) {
            (Some(v), _, _) | (None, Some(v), _) | (None, None, Some(v)) => Some(v),
            (None, None, None) => None,
        }
    }

    /// Has the wire said anything at all about this stat?
    ///
    /// The `is_known` half of the C21 pattern (`status.rs:88`), at the field
    /// rather than at a map: a caller that must not guess reads this first.
    #[must_use]
    pub const fn is_known(&self) -> bool {
        self.normal.is_some() || self.ascended.is_some() || self.enhanced.is_some()
    }
}

/// The ten statistics, in the order `info` prints them.
///
/// `attributes/stats.rs:30`'s `@@stats`, which is also the wire's print order --
/// verified against the fixture, where STR..INF appear in exactly this sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatKind {
    Strength,
    Constitution,
    Dexterity,
    Agility,
    Discipline,
    Aura,
    Logic,
    Intuition,
    Wisdom,
    Influence,
}

impl StatKind {
    /// All ten, in the wire's print order.
    pub const ALL: [Self; 10] = [
        Self::Strength,
        Self::Constitution,
        Self::Dexterity,
        Self::Agility,
        Self::Discipline,
        Self::Aura,
        Self::Logic,
        Self::Intuition,
        Self::Wisdom,
        Self::Influence,
    ];

    /// The Lich key spelling, e.g. `strength` (`attributes/stats.rs:30`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strength => "strength",
            Self::Constitution => "constitution",
            Self::Dexterity => "dexterity",
            Self::Agility => "agility",
            Self::Discipline => "discipline",
            Self::Aura => "aura",
            Self::Logic => "logic",
            Self::Intuition => "intuition",
            Self::Wisdom => "wisdom",
            Self::Influence => "influence",
        }
    }

    /// The three-letter code the wire prints, e.g. `STR`.
    ///
    /// The wire gives both this and the long name on every line
    /// (`Strength (STR):`). The code is the reliable half -- it is fixed width
    /// and unambiguous -- so [`StatLine::classify`] keys on it.
    #[must_use]
    pub const fn abbrev(self) -> &'static str {
        match self {
            Self::Strength => "STR",
            Self::Constitution => "CON",
            Self::Dexterity => "DEX",
            Self::Agility => "AGI",
            Self::Discipline => "DIS",
            Self::Aura => "AUR",
            Self::Logic => "LOG",
            Self::Intuition => "INT",
            Self::Wisdom => "WIS",
            Self::Influence => "INF",
        }
    }

    /// The long name as the wire prints it, e.g. `Strength`.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Strength => "Strength",
            Self::Constitution => "Constitution",
            Self::Dexterity => "Dexterity",
            Self::Agility => "Agility",
            Self::Discipline => "Discipline",
            Self::Aura => "Aura",
            Self::Logic => "Logic",
            Self::Intuition => "Intuition",
            Self::Wisdom => "Wisdom",
            Self::Influence => "Influence",
        }
    }

    /// Parse the Lich key spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == text)
    }

    /// Parse the three-letter wire code, case-insensitively.
    #[must_use]
    pub fn parse_abbrev(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.abbrev().eq_ignore_ascii_case(text))
    }
}

impl std::fmt::Display for StatKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What one line of `info` said about one statistic.
///
/// The classifier's answer: stateless, total, and holding only what that line
/// carried. Folding it into a [`Stat`] is the consumer's job, because only the
/// consumer knows whether a missing column means "not sent" or "not asked for".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatLine {
    /// Which statistic.
    pub kind: StatKind,
    /// The columns, left to right, as the line carried them.
    ///
    /// **Two or three.** A 2-column line is `info` and has no `normal`; the
    /// consumer decides which column is which, since that depends on the count.
    pub columns: [Option<StatValue>; 3],
    /// How many columns the line actually carried.
    pub column_count: u8,
}

impl StatLine {
    /// Classify one **reassembled** line of `info` output.
    ///
    /// Returns `None` for any line that is not a stat line, which is most of
    /// them -- the header, the `Name:` line, `Mana:`, and every other line in
    /// the blob.
    ///
    /// # Why this is hand-written and not a regex
    ///
    /// `cena-model` already depends on `regex` (for `crit`), so a regex was
    /// available. The line's shape is fixed-width and positional --
    /// `<name> (<ABBR>): <n> (<b>)` then ` ... <n> (<b>)` repeated -- and
    /// splitting on `...` then reading two integers per field is both shorter
    /// than the pattern and reports WHICH column failed. A regex would answer
    /// only "no match" on a malformed line.
    #[must_use]
    pub fn classify(line: &str) -> Option<Self> {
        let (head, rest) = line.split_once("):")?;
        let open = head.rfind(" (")?;
        let kind = StatKind::parse_abbrev(head.get(open + 2..)?.trim())?;
        // The long name must agree with the code, or this is a line that merely
        // looks like one -- a player can type `Strength (INT): 1 (1) ... 1 (1)`.
        if !head
            .get(..open)?
            .trim()
            .eq_ignore_ascii_case(kind.display_name())
        {
            return None;
        }

        let mut columns = [None; 3];
        let mut count = 0u8;
        for field in rest.split("...") {
            let field = field.trim();
            if field.is_empty() {
                continue;
            }
            let idx = usize::from(count);
            *columns.get_mut(idx)? = Some(parse_column(field)?);
            count = count.saturating_add(1);
        }
        // Two columns (`info`) or three (`info full`). Anything else is not this
        // shape, and guessing would put a value in the wrong field.
        if count < 2 {
            return None;
        }
        Some(Self {
            kind,
            columns,
            column_count: count,
        })
    }

    /// The `normal` column, present only on an `info full` line.
    #[must_use]
    pub const fn normal(&self) -> Option<StatValue> {
        if self.column_count >= 3 {
            self.columns[0]
        } else {
            None
        }
    }

    /// The `ascended` column: the first of two, or the second of three.
    #[must_use]
    pub const fn ascended(&self) -> Option<StatValue> {
        if self.column_count >= 3 {
            self.columns[1]
        } else {
            self.columns[0]
        }
    }

    /// The `enhanced` column: always the last one sent.
    #[must_use]
    pub const fn enhanced(&self) -> Option<StatValue> {
        if self.column_count >= 3 {
            self.columns[2]
        } else {
            self.columns[1]
        }
    }
}

/// Parse one `<value> (<bonus>)` field.
///
/// Tolerates the bold markers having been stripped, because by the time a line
/// is reassembled the markup is gone and only the digits remain.
fn parse_column(field: &str) -> Option<StatValue> {
    let (value, bonus) = field.split_once('(')?;
    let bonus = bonus.strip_suffix(')')?;
    Some(StatValue {
        value: value.trim().parse().ok()?,
        bonus: bonus.trim().parse().ok()?,
    })
}

/// The scalars `info` carries beside the stat table.
///
/// # Race and profession are `String` on purpose
///
/// Lich enumerates neither -- `infomon/parser.rb:10` captures race as
/// `[A-z]+|[A-z]+(?: |-)[A-z]+` and profession as `[-A-z]+` -- and profession is
/// compared against a literal exactly once in the whole tree (`spellsong.rb:27`,
/// `Stats.prof != 'Bard'`). An enum would turn a new Simutronics race into a
/// parse failure.
///
/// # What `info` says that must NOT be believed
///
/// The line also carries `Expr:` and `Level:`. Lich captures both and
/// **deliberately discards them** (`parser.rb:238`, `:246`: *"level captured
/// here, but do not rely on it - use XML instead"*), because they change
/// continuously and `info` is a snapshot. `<dialogData id='expr'>` carries the
/// live value, so this struct holds neither.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identity {
    /// `Race: Half-Elf`.
    pub race: Option<String>,
    /// `Profession: Ranger`, with any `(shown as: ...)` disguise removed.
    pub profession: Option<String>,
    /// `Gender: Male`.
    pub gender: Option<String>,
    /// `Age: 36`.
    pub age: Option<u32>,
    /// The account's subscription tier, from `profile`, not `info`.
    pub account: Option<AccountType>,
}
