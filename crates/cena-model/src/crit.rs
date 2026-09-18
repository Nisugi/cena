//! Critical-hit tables: what a crit did, and the message that announces it.
//!
//! Ported from `reference/lich-5/lib/<game>/critranks/` per `plan/13` §4a,
//! which names the crit tables as a port target -- "port aggressively where
//! knowledge lives in code". 2,394 entries across 21 damage types, each with
//! the regex that recognises its message. This is two years of accumulated
//! game knowledge that would otherwise have to be re-derived by being hit.
//!
//! # Shape
//!
//! - `crit/types.rs` -- the vocabulary: the enums an entry is built from.
//! - `crit/entry.rs` -- `CritEntry`, one row of one table.
//! - `crit/load.rs` -- parsing `data/crit_tables.tsv`, the shipped data.
//! - `crit/match_index.rs` -- the first-word bucket index, and the one
//!   pattern Rust's `regex` cannot compile.
//!
//! The data is a checked-in TSV rather than generated Rust, which is what
//! `plan/13:125` says for this row ("static data; ships as data files") and
//! what the measured 21x rustfmt expansion forces. `crit/load.rs`'s module
//! docs carry that measurement.
//!
//! # Regenerating the data
//!
//! ```text
//! ruby crates/cena-model/tools/extract_crit_tables.rb \
//!      reference/lich-5/lib/<game>/critranks \
//!      crates/cena-model/data/crit_tables.tsv
//! ```
//!
//! The extractor is committed and needs no Lich runtime. `cargo test` never
//! runs it and never needs Ruby: the TSV is the checked-in ground truth, and
//! `tests/crit_parity.rs` is what holds it to the Ruby it came from.
//!
//! # No process global
//!
//! `CritTables` is an ordinary value. Build it once and share it as
//! `Arc<CritTables>`; there is no `static`, no `OnceLock` and therefore no
//! entry in `ALLOWED_STATICS` (plan/05 Rule 5.2, :400-408). That rule is the
//! one multi-session depends on, and a 2,394-pattern table is precisely the
//! thing that "just this once" gets made global.

pub mod entry;
pub mod load;
pub mod match_index;
pub mod types;

pub use entry::{CritEntry, STUN_UNKNOWN};
pub use load::LoadError;
pub use match_index::PatternError;
pub use types::{DamageType, Location, Position, SecondaryWound, WoundLocation};

use match_index::MatchIndex;

/// Why the shipped tables could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// The data file did not parse.
    Data(LoadError),
    /// A pattern did not compile.
    Pattern(PatternError),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data(e) => write!(f, "{e}"),
            Self::Pattern(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Every critical-hit table, with a compiled matcher over their messages.
pub struct CritTables {
    /// Sorted by `(damage_type, location, rank)`, which `get` binary searches.
    ///
    /// A sorted array and not a map: ranks are **sparse** -- `acid/right_eye`
    /// holds ranks [4, 7, 8] -- so a dense `[rank]` index would be mostly
    /// holes. And a perfect-hash crate buys nothing over a binary search on
    /// 2,394 entries while costing two dependencies (`plan/05` §-1).
    entries: Vec<CritEntry>,
    index: MatchIndex,
}

impl Default for CritTables {
    /// An empty table: no entries, no patterns.
    ///
    /// Not a fallback anything should ship with -- `load` is the constructor.
    /// It exists so a caller that cannot use the shipped data has a total
    /// function to fall back to rather than an `unwrap`, which the workspace
    /// denies (`plan/06` §1.5: the parser never panicking is non-negotiable).
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: MatchIndex::empty(),
        }
    }
}

impl CritTables {
    /// Build the tables from the shipped data file.
    ///
    /// Costs ~94ms, almost all of it compiling 2,394 regexes. Pay it once at
    /// startup and share the result.
    ///
    /// # Errors
    ///
    /// `BuildError::Data` if the shipped TSV does not parse, naming the line;
    /// `BuildError::Pattern` if a pattern does not compile, naming the index.
    pub fn load() -> Result<Self, BuildError> {
        let entries = load::shipped_entries().map_err(BuildError::Data)?;
        Self::from_entries(entries)
    }

    /// Build from entries already parsed. Used by the behaviour test, which
    /// feeds in a subset with one damage type removed.
    ///
    /// # Errors
    ///
    /// `BuildError::Pattern` if one of the entries carries a pattern that does
    /// not compile.
    pub fn from_entries(mut entries: Vec<CritEntry>) -> Result<Self, BuildError> {
        entries.sort_by_key(CritEntry::key);
        let patterns: Vec<String> = entries.iter().map(|e| e.pattern.clone()).collect();
        let index = MatchIndex::build(&patterns).map_err(BuildError::Pattern)?;
        Ok(Self { entries, index })
    }

    /// Every entry whose message pattern matches this line.
    ///
    /// Returns a slice of entries rather than one `Option`, because **18 lines
    /// in the table match more than one entry and that ambiguity is in the
    /// game data, not a porting bug**. "Steam billows around X." is both
    /// `steam/head/0` and `steam/chest/0`; "X right leg jerks momentarily."
    /// is `disruption/right_leg/1` *and* `unbalance/right_leg/1` -- different
    /// damage types, indistinguishable from the message alone.
    ///
    /// **Ordered most-specific first: longest pattern wins.** Every caller in
    /// the reference corpus takes the first
    /// (`reference/lich-5/lib/<game>/combat/processor.rb:1707` does
    /// `CritRanks.parse(line).values.first`), so the order decides the answer
    /// -- and in key order "first" means *lowest rank*, i.e. least severe.
    ///
    /// That is wrong where one pattern is a literal prefix of another. Four
    /// pairs in the tables are such subsumptions: `^Burst of flames to chest.`
    /// (fire/chest/0, damage 0) is a prefix of `^Burst of flames to chest
    /// toasts skin nicely.` (fire/chest/2, damage 10, wound 1), so key order
    /// always returned the weaker one. Sorting by descending pattern length
    /// puts the more specific entry first, which fixes all four pairs and both
    /// `generic/unspecified/0` collisions (`.*? is stunned.` is the shortest
    /// pattern in every collision it takes part in) without touching the data.
    ///
    /// This is a deliberate divergence from Lich, which preserves Ruby hash
    /// insertion order. Ties are broken by key so the order stays total and
    /// deterministic.
    ///
    /// `line` must be **de-tagged text**. Lich's caller strips markup itself
    /// (`gsub(/<.+?>/, '')`); in Cena that is automatic, because Rule 2.1
    /// (`plan/05:270-274`) means nothing above `cena-protocol` ever sees
    /// markup in the first place.
    #[must_use]
    pub fn parse(&self, line: &str) -> Vec<&CritEntry> {
        let mut hits: Vec<&CritEntry> = self
            .index
            .matches(line)
            .into_iter()
            .filter_map(|i| self.entries.get(i))
            .collect();
        // Most specific first. Length is the proxy for specificity that fixes
        // the prefix-subsumption pairs; the key tiebreak keeps it total.
        hits.sort_by(|a, b| {
            b.pattern
                .len()
                .cmp(&a.pattern.len())
                .then_with(|| a.key().cmp(&b.key()))
        });
        hits
    }

    /// One entry by its key.
    ///
    /// VERIFIED that `(damage_type, location, rank)` is unique across all
    /// 2,394 entries -- the extractor refuses to write a TSV with a duplicate
    /// -- so this is well defined.
    #[must_use]
    pub fn get(&self, damage_type: DamageType, location: Location, rank: u8) -> Option<&CritEntry> {
        let key = (damage_type, location, rank);
        self.entries
            .binary_search_by(|e| e.key().cmp(&key))
            .ok()
            .and_then(|i| self.entries.get(i))
    }

    /// Every entry, sorted by key.
    #[must_use]
    pub fn entries(&self) -> &[CritEntry] {
        &self.entries
    }

    /// How many entries carry a rewritten lookahead. Exactly 1; the parity
    /// test holds it there.
    #[must_use]
    pub fn exclusion_count(&self) -> usize {
        self.index.exclusion_count()
    }

    /// `(buckets, residual, largest bucket)` of the match index.
    #[must_use]
    pub fn index_shape(&self) -> (usize, usize, usize) {
        self.index.shape()
    }
}
