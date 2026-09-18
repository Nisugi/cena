//! `CritEntry`: one row of one critical-hit table.
//!
//! Split from `crit/types.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. That file reached 401 lines against the 400
//! default, and the seam it offered was the obvious one: the enums are the
//! vocabulary, this is the record built out of them.
//!
//! The field set is Lich's, minus `:type` and `:location`, which are carried
//! by `damage_type` and `location`. Lich stores each twice -- once as a hash
//! key and once inside the record -- and the extractor asserts the two agree
//! for all 2,394 entries before dropping the duplicate (`plan/05` §-1, DRY).

use super::types::{DamageType, Location, Position, SecondaryWound};

/// The sentinel `stunned` value meaning "this crit stuns, for an unknown
/// number of rounds".
///
/// Lich's own template states the convention
/// (`generic_critical_table.rb:23`: "use 999 for unknown").
pub const STUN_UNKNOWN: u16 = 999;

/// One row of one critical-hit table: what this crit does, and the message
/// that announces it.
///
/// The field set is Lich's, minus `:type` and `:location`, which are carried
/// by `damage_type` and `location` here. Lich stores each twice -- once as a
/// hash key and once inside the record -- and the extractor asserts the two
/// agree for all 2,394 entries before dropping the duplicate
/// (`plan/05` §-1, DRY).
// Eight of Lich's nineteen fields are booleans, and clippy's
// `struct_excessive_bools` is right that this usually means a missing enum.
// Here it does not: these eight are INDEPENDENT effects that co-occur -- a
// single crit can be fatal and amputate and stun and silence at once, and 255
// entries do set `amputated` alongside other flags. Collapsing them into an
// enum would forbid combinations the data contains. The schema is also not
// ours to redesign: it is Lich's, documented at generic_critical_table.rb:19-36,
// and the parity test asserts field-for-field equality with it, so a "tidier"
// shape here would mean the port no longer matches what it ported.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CritEntry {
    pub damage_type: DamageType,
    pub location: Location,
    /// Measured 0..=11.
    pub rank: u8,
    /// Extra damage. Measured 0..=88.
    pub damage: u16,
    pub position: Option<Position>,
    pub fatal: bool,
    /// Rounds of stun. Measured 0..=20, plus `STUN_UNKNOWN`.
    ///
    /// **999 means "unknown", not 999 rounds.** Exactly one entry uses it --
    /// `generic/unspecified/0`, whose message says the target is stunned
    /// without saying for how long. A caller that treats this as a duration
    /// will wait a very long time, so `stun_is_known` exists to ask.
    pub stunned: u16,
    pub amputated: bool,
    /// **`false` in all 2,394 entries.** Kept because it is part of the schema
    /// Lich documents (`generic_critical_table.rb:27`) and a future table may
    /// set it, but it carries no information today. Measured:
    /// `cut -f9 crates/cena-model/data/crit_tables.tsv | tail -n +2 | sort -u`
    /// prints `0` and nothing else.
    pub crippled: bool,
    pub sleeping: bool,
    pub dazed: bool,
    pub limb_favored: bool,
    /// Extra roundtime in seconds. Measured domain {0, 2, 5, 10, 20}.
    pub roundtime: u8,
    pub silenced: bool,
    pub slowed: bool,
    /// Rank of the wound at `location`. Measured 0..=3.
    pub wound_rank: u8,
    pub secondary_wound: Option<SecondaryWound>,
    /// The regex source that matches this crit's message, as Lich writes it.
    ///
    /// Kept beside the compiled form so a caller can report *why* a line
    /// matched, and so the parity test can compare against the TSV.
    pub pattern: String,
}

impl CritEntry {
    /// Whether `stunned` is a real duration rather than the unknown sentinel.
    #[must_use]
    pub const fn stun_is_known(&self) -> bool {
        self.stunned != STUN_UNKNOWN
    }

    /// The key this entry is sorted and looked up by.
    ///
    /// VERIFIED unique across all 2,394 entries by the extractor, which
    /// refuses to write a TSV with a duplicate.
    #[must_use]
    pub const fn key(&self) -> (DamageType, Location, u8) {
        (self.damage_type, self.location, self.rank)
    }
}
