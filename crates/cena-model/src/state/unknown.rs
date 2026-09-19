//! Unknown tags: criterion 8's evidence, bounded.
//!
//! Split out of `state.rs` under Rule 4.1 (`plan/05:352-353`) -- and the split
//! was **demanded by Rule 4.4's own new test** minutes after it was written:
//! `state.rs` reached 505 lines against its 500 cap when the bound below landed,
//! and `split_parents_stay_facades` said what Vellum's version says, *"move code
//! down into a submodule instead of raising the cap"*. Taken as written.

use super::GameState;

/// How many distinct unknown-tag sightings [`GameState`] keeps the raw bytes of.
///
/// # Why it is bounded at all
///
/// It was not: a bare `Vec::push` per unknown tag, forever, carried across every
/// reconnect and deep-cloned by every `subscribe()`. Same shape as the
/// `Recorder` leak, on the same production path, in a client designed for 3-25
/// simultaneous characters.
///
/// # Why 512, and why that is generous rather than tight
///
/// MEASURED 2026-09-19 over a 3 MB capture: **zero** unknown tags. The table
/// covers what the wire sends today, so nothing is growing. The failure needs
/// Simutronics to add a tag that rides every prompt -- ~86,000 a day per session
/// -- which is precisely the event Rule 2.2 exists for.
///
/// 512 distinct raws is far more than a human will read and far less than a day
/// of one tag. It is a ceiling on a cost we are not yet paying, not a trade
/// against one we are.
pub const MAX_UNKNOWN_TAGS: usize = 512;

/// A tag `cena-protocol` has no variant for, kept for display and for the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownTag {
    /// The element name.
    pub name: String,
    /// The bytes the game actually sent. The raw form IS the diagnostic: a
    /// reader has to see what arrived, not a summary of it.
    pub raw: String,
}

impl GameState {
    /// Record one unknown tag: always count it, keep its raw bytes while there
    /// is room.
    ///
    /// The count comes first and is unconditional -- it is the fact that
    /// survives the bound.
    pub(super) fn record_unknown_tag(&mut self, name: &str, raw: &str) {
        *self.unknown_tag_counts.entry(name.to_owned()).or_insert(0) += 1;
        // The raws are a CENSUS, not a transcript, so the ring keeps the FIRST
        // occurrences: the interesting fact is that a new tag appeared and what
        // it looked like. `Recorder` keeps the newest because it is a transcript
        // being replayed; this is the other kind of log and takes the other end.
        if self.unknown_tags.len() < MAX_UNKNOWN_TAGS {
            self.unknown_tags.push(UnknownTag {
                name: name.to_owned(),
                raw: raw.to_owned(),
            });
        }
    }

    /// How many times an unknown tag of this name has been seen.
    ///
    /// **Not bounded by [`MAX_UNKNOWN_TAGS`].** The ring keeps a census of raw
    /// bytes and stops; this keeps counting, so a tag on every prompt never looks
    /// as rare as a tag seen twice. Zero for a name never seen.
    #[must_use]
    pub fn unknown_tag_count(&self, name: &str) -> u64 {
        self.unknown_tag_counts.get(name).copied().unwrap_or(0)
    }
}
