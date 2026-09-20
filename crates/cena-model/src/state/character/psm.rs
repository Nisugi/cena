//! The PSM tables: an **open set**, per C21.
//!
//! M3 step 6. The five categories -- `cman`, `feat`, `armor`, `shield`,
//! `weapon` -- each print a table of mnemonics and ranks.
//!
//! > **AUTHOR, 2026-09-19:** *"there's a few .. cman, shield, weapon, armor,
//! > feat are psms"*
//!
//! # Why this is a `BTreeMap` where `skills` is an enum
//!
//! `research/04-inherited-decisions.md:1878` (C21) reserves typed named fields
//! for **closed** vocabularies and a map with `is_known` for open ones. The 46
//! skills are closed: the game has printed the same 46 for years. The PSMs are
//! not. Lich's own tables carry 80 combat maneuvers, 33 feats, 33 shield, 24
//! weapon and 11 armor specializations (MEASURED by counting the hash literals
//! in `lib/gemstone/psms/*.rb`), and Simutronics adds to them.
//!
//! So a mnemonic the port has never heard of must still be **stored and
//! reported**, not dropped. That is Rule 2.2, and it is the defect
//! `inventory/10` §8 records against Lich's enhancive parser, which silently
//! discards unrecognised names.
//!
//! # The table, MEASURED
//!
//! Cut from `GSIV-Nisugi/2025/03/xml/2025-03-20_06-02-48.xml:24722-24762` and
//! committed as `psm_list.xml`. The command was `cman list`:
//!
//! ```text
//!   Skill                Mnemonic        Ranks Type           Category        Subcategory
//!   -------------------------------------------------------------------------------------
//!   Acrobat's Leap       acrobatsleap    0/1   Passive
//!   Combat Focus         focus           5/5   Passive
//!   Disarm Weapon        disarm          5/5   Setup          Warrior Guild
//! ```
//!
//! ## Bold means KNOWN, not maxed
//!
//! The finding that shapes this file. MEASURED across the capture's 29 rows:
//!
//! | Bolded | Ranks seen |
//! |---|---|
//! | yes (9 rows) | `1/1`, `2/2`, `3/3`, `3/5`, `4/5`, `5/5` x4 |
//! | no (18 rows) | `0/1` x2, `0/2` x2, `0/5` x14 |
//!
//! **Every bolded row has ranks > 0; every unbolded row is exactly `0/n`.**
//! `Hamstring 4/5` is bolded and not maxed, which is what rules out the
//! "bold = maxed" reading. The game bolds what the character has actually
//! trained.
//!
//! Lich reads the fraction instead (`parser.rb:362`). Both work; bold is the
//! same structural signal [`stats`](super::stats) and [`skills`](super::skills)
//! already read, so this module takes it and checks it against the fraction --
//! a disagreement would mean one of the two readings is wrong, and the port
//! should find out rather than pick.
//!
//! ## Bold spans WRAP across rows
//!
//! Unique to this table among the fixtures. The wire emits
//! `<popBold/><pushBold/>` mid-line, so one bold span closes and the next opens
//! inside a single row's text:
//!
//! ```text
//! <popBold/>  Combat Movement      <d ...>cmovement</d>       5/5   ...
//! ```
//!
//! A consumer that reset bold state at `ends_line` would mis-attribute every
//! row after the first. Cena's parser tracks `bold_depth` across frames, so
//! this is handled -- but it is the reason the fixture exists rather than a
//! hand-written table.
//!
//! ## The mnemonic is a link, not plain text
//!
//! Each sits inside `<d cmd='cman HELP acrobatsleap'>acrobatsleap</d>`. The
//! parser strips the markup, so a classifier here sees the display text -- the
//! `plan/12` §3a bargain again, and the reason this file has no pattern for
//! markup.
//!
//! **VERIFIED that the mnemonic column IS Lich's `short_name`** (`plan/15`
//! §2b.2): 27/27 combat maneuvers match `cman.rb` exactly. So the wire's own
//! column is the key, and the port does not need Lich's table to read a table.

use std::collections::BTreeMap;

use super::vocabulary::PsmCategory;

/// One PSM's ranks, as the table printed them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PsmRanks {
    /// Ranks trained.
    pub ranks: u16,
    /// The most this PSM can be trained to.
    pub max: u16,
    /// Whether the row arrived bolded, i.e. the game marks it known.
    pub known_by_bold: bool,
}

impl PsmRanks {
    /// Trained at all.
    ///
    /// Reads the **fraction**, which is Lich's signal. [`Self::disagrees`] is
    /// what checks it against bold rather than assuming they agree.
    #[must_use]
    pub const fn is_known(&self) -> bool {
        self.ranks > 0
    }

    /// Trained to the maximum.
    #[must_use]
    pub const fn is_maxed(&self) -> bool {
        self.ranks > 0 && self.ranks == self.max
    }

    /// The two "is it known" signals disagree.
    ///
    /// MEASURED as never true in the capture, and asserted so by
    /// `tests/psm_list.rs`. It is exposed rather than `debug_assert`ed because
    /// a future table where they diverge is a fact about the wire the port
    /// should surface, not a crash -- Rule 2.2 again.
    #[must_use]
    pub const fn disagrees(&self) -> bool {
        self.is_known() != self.known_by_bold
    }
}

/// One row of a PSM table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsmLine {
    /// The mnemonic, which is the key: `acrobatsleap`, `focus`, `disarm`.
    pub mnemonic: String,
    /// The display name, **truncated by the game to 20 characters**.
    ///
    /// `Spiritual Lore - Ble` in the ascension table is the proof. Kept for
    /// display, never used as a key, because a truncated name is ambiguous.
    pub display_name: String,
    /// Ranks and max.
    pub ranks: PsmRanks,
    /// `Passive`, `Setup`, `Attack`, `Buff`, `Concentration`, `Martial Stance`.
    ///
    /// A `String` because the capture shows six values and Lich's tables carry
    /// `:type` as a free symbol -- an open set, per C21, and nothing in the
    /// port branches on it yet.
    pub kind: String,
    /// `Warrior Guild`, `Rogue Guild`, or empty.
    pub category: String,
}

/// The header that opens a PSM table, and the two forms it takes.
///
/// ```text
/// <Name>, the following Combat Maneuvers are available:     <- LIST
/// <Name>, your Combat Maneuvers are as follows:             <- INFO
/// ```
///
/// **Lich reads only the first** (`parser.rb:29`'s `PSMStart` requires
/// `the following ... are available:`; `grep -c "as follows"` over the whole
/// parser returns 0). So `CMAN INFO` output is silently dropped: its rows would
/// parse fine, but no accumulator is ever opened.
///
/// This port reads both, because they carry the same table and dropping one is
/// a defect rather than a decision.
#[must_use]
pub fn classify_header(line: &str) -> Option<PsmCategory> {
    let rest = line.split_once(", ")?.1;
    let heading = rest
        .strip_prefix("the following ")
        .and_then(|r| r.strip_suffix(" are available:"))
        .or_else(|| {
            rest.strip_prefix("your ")
                .and_then(|r| r.strip_suffix(" are as follows:"))
        })?;
    PsmCategory::parse_heading(heading)
}

/// The line that closes a `LIST`-form table.
///
/// `parser.rb:31`'s `PSMEnd`, the literal `   Subcategory: all` with three
/// leading spaces. **The `INFO` form has no terminator at all** -- it simply
/// stops -- which is why the chunk's prompt boundary is the real close and this
/// is only a hint.
#[must_use]
pub fn is_table_end(line: &str) -> bool {
    line.trim_start().starts_with("Subcategory:") && line.starts_with("   ")
}

impl PsmLine {
    /// Classify one row of a PSM table.
    ///
    /// Returns `None` for the header, the `---` rule, the filter footer, and
    /// anything else that is not a row.
    #[must_use]
    pub fn classify(line: &str) -> Option<Self> {
        // The rule under the header, and the header itself.
        if line.trim_start().starts_with('-') || line.contains("Mnemonic") {
            return None;
        }
        let mut fields = line.split_whitespace().peekable();

        // The display name runs until the mnemonic, which is the field before
        // the `x/y` fraction. Scan for the fraction first, since the name has
        // an unknown number of words (`Acrobat's Leap`, `Unarmed Specialist`).
        let parts: Vec<&str> = fields.by_ref().collect();
        let fraction_at = parts.iter().position(|p| p.contains('/'))?;
        // Name, then mnemonic, then the fraction: at least three fields.
        if fraction_at < 2 {
            return None;
        }
        let mnemonic = parts.get(fraction_at - 1)?;
        let display_name = parts.get(..fraction_at - 1)?.join(" ");
        let (ranks, max) = parts.get(fraction_at)?.split_once('/')?;
        let ranks = ranks.parse().ok()?;
        let max = max.parse().ok()?;
        // Everything after the fraction is Type, then Category. Both may be
        // absent; neither is parsed into a vocabulary.
        let kind = parts.get(fraction_at + 1).copied().unwrap_or("").to_owned();
        let category = parts
            .get(fraction_at + 2..)
            .map_or_else(String::new, |r| r.join(" "));

        if mnemonic.is_empty() || display_name.is_empty() {
            return None;
        }
        Some(Self {
            mnemonic: (*mnemonic).to_owned(),
            display_name,
            ranks: PsmRanks {
                ranks,
                max,
                known_by_bold: false,
            },
            kind,
            category,
        })
    }

    /// Classify a row and record whether it arrived bolded.
    ///
    /// Bold marks a **known** PSM -- see the module docs for the measurement.
    #[must_use]
    pub fn classify_with_bold(line: &str, bolded: bool) -> Option<Self> {
        let mut parsed = Self::classify(line)?;
        parsed.ranks.known_by_bold = bolded;
        Some(parsed)
    }
}

/// Every PSM the character has been told about, by category.
///
/// Open set: keyed by the wire's own mnemonic, so a PSM added to the game after
/// this port was written is stored and reported rather than dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PsmSet {
    by_category: BTreeMap<PsmCategory, BTreeMap<String, PsmRanks>>,
}

impl PsmSet {
    /// Ranks for one mnemonic in one category.
    ///
    /// `None` means the table has not been read, **or** the mnemonic is not in
    /// it. Those differ, and [`Self::has_table`] is what tells them apart --
    /// the distinction MO-3 exists because `Effects` lacks.
    #[must_use]
    pub fn get(&self, category: PsmCategory, mnemonic: &str) -> Option<PsmRanks> {
        self.by_category.get(&category)?.get(mnemonic).copied()
    }

    /// Has this category's table ever been read?
    #[must_use]
    pub fn has_table(&self, category: PsmCategory) -> bool {
        self.by_category.contains_key(&category)
    }

    /// Every mnemonic known in a category, ordered.
    pub fn mnemonics(&self, category: PsmCategory) -> impl Iterator<Item = (&str, PsmRanks)> {
        self.by_category
            .get(&category)
            .into_iter()
            .flat_map(|m| m.iter().map(|(k, v)| (k.as_str(), *v)))
    }

    /// How many mnemonics are recorded for a category.
    #[must_use]
    pub fn len(&self, category: PsmCategory) -> usize {
        self.by_category.get(&category).map_or(0, BTreeMap::len)
    }

    /// Nothing recorded at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_category.is_empty()
    }

    /// Replace one category's table.
    ///
    /// **Clears that category first**, the same rule
    /// [`SkillSet::clear`](super::skills::SkillSet::clear) follows: the table
    /// lists every PSM including untrained ones at `0/n`, so nothing is lost,
    /// and a PSM that was *un*learned is only ever recorded this way.
    ///
    /// Scoped to one category because the five tables arrive from five separate
    /// commands; clearing all of them on a `cman list` would discard four
    /// tables nobody asked about.
    pub fn replace_category(&mut self, category: PsmCategory, rows: &[PsmLine]) {
        let table = self.by_category.entry(category).or_default();
        table.clear();
        for row in rows {
            table.insert(row.mnemonic.clone(), row.ranks);
        }
    }

    /// Forget every table.
    pub fn clear(&mut self) {
        self.by_category.clear();
    }
}

/// Ascension is **not** a PSM, and the wire says so three ways.
///
/// It is grouped with them in `infomon/cli.rb`'s sync list -- six `<x> list
/// all` commands together -- which is where this port first got it wrong, and
/// the author corrected it. The table's *layout* is shared; nothing else is.
///
/// MEASURED in `2025-03-20_06-02-48.xml`:
///
/// | | PSM (`cman list`) | Ascension |
/// |---|---|---|
/// | header | `the following ... are available:` | `your ... are as follows:` |
/// | terminator | `   Subcategory: all` | **none** -- ends at `<output class=""/>` |
/// | vocabulary | maneuvers (`acrobatsleap`, `disarm`) | **skills** (`agility`, `edgedweapons`, `slblessings`) |
///
/// The third row is the substantive one: ascension ranks are bought against the
/// *skill* names this crate already models in [`skills`](super::skills), which
/// is why they are not maneuver mnemonics and why a shared type would be wrong.
///
/// **Lich has never stored these.** `PSMStart` matches only `are available:`,
/// so the `as follows:` form opens no accumulator. Whether
/// `ascension list all all` prints the other form is **UNVERIFIED** -- across
/// five archive files, `the following Ascension Abilities are available:`
/// appears zero times (`plan/15` §2b.2). A capture is pending from the author.
///
/// Deliberately a marker with no fields yet: the shape is known, the data is
/// not, and inventing an interface for an unmeasured table is what
/// `plan/18`'s `pbarStance` lesson warns against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AscensionTable;
