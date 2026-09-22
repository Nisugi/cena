//! Character status indicators: what the game says is true of you right now.
//!
//! **Ported from `reference/VellumFE/src/core/state.rs:339-410`**, nearly
//! whole. `plan/17` §2 is the reasoning; this is the code, and three of its
//! decisions are load-bearing enough to restate:
//!
//! 1. **Ids are normalised.** `Icon` stripped, lowercased, so a caller may
//!    pass `IconSTUNNED`, `stunned` or `STUNNED` and get the same answer.
//! 2. **An unreported id reads `false`** -- "the game never told us, so it is
//!    not happening."
//! 3. **[`StatusInfo::is_known`] exists separately**, distinguishing *reported
//!    inactive* from *never reported*.
//!
//! Point 3 is `plan/12` §5.2 arriving independently: "`Unknown` is a
//! first-class value, not a default." After a reconnect nothing has been
//! reported yet, and a confident `false` would be a lie -- the exact failure
//! §5.2 exists to prevent. Vellum reached the same shape for a different
//! reason (a multi-account display must render unknown as unknown), which is
//! decent evidence it is right.
//!
//! # What the wire actually sends, MEASURED
//!
//! `plan/15` §2a.4a, from Cena's own capture of 2026-09-18: **all ten ids
//! arrive in one burst at login**, `IconSTANDING` the only `visible="y"`, and
//! **no further indicator traffic in 75 seconds** because nothing changed.
//!
//! So these are **state declarations, not an event stream**. Absence of an
//! update means unchanged; it never means "not happening". A design that
//! expected a stream would treat a quiet session as a session with no status.
//!
//! # These have both edges, which prompt flags do not
//!
//! `<indicator>` carries `visible='y'` *and* `visible='n'`, so it reports
//! onset **and offset**. The prompt's status letters
//! (`W I i P S s J K U H C R !`) report onset only -- a prompt is sent when
//! something happens, and nothing is sent when a condition merely ends
//! (`plan/15` §2a.1, the author). Anything that waits on a prompt flag to
//! learn a condition ended waits forever in a quiet room.
//!
//! Four prompt codes have no indicator at all -- Immobilized, Unconscious,
//! Calmed, and roundtime. Roundtime is computed instead ([`crate::GameState`]);
//! the other three are text-derived, and `plan/15` §2a.3 records how Lich
//! guards them.

use std::collections::BTreeMap;

/// Every indicator the game has reported, and whether it is active.
///
/// `BTreeMap`, not `HashMap`: iteration order is part of criterion 7's
/// determinism, and `crates/cena-arch-tests` already states the preference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatusInfo {
    /// Normalised id -> active. Absent means never reported.
    flags: BTreeMap<String, bool>,
}

impl StatusInfo {
    /// Normalise an id to its map key.
    ///
    /// Strips the `Icon` prefix the wire uses, so callers may pass either
    /// form, and lowercases so `IconSTUNNED` and `stunned` agree.
    fn key(id: &str) -> String {
        id.strip_prefix("Icon").unwrap_or(id).to_ascii_lowercase()
    }

    /// Set an indicator. Returns `true` if the value **changed**.
    ///
    /// The return is what lets a caller avoid emitting a no-op event: the
    /// login burst sets ten indicators at once and nine of them are already
    /// what they were.
    pub fn set(&mut self, id: &str, active: bool) -> bool {
        self.flags.insert(Self::key(id), active) != Some(active)
    }

    /// Forget one id, so it reads *never reported* again.
    ///
    /// For a reconnect, and only for the statuses nothing re-states: the login
    /// burst re-declares every `<indicator>`, so those are kept
    /// (`reconnect.rs`), while `afflictions.rs`'s six have no indicator and
    /// would otherwise keep a belief forever.
    pub fn forget(&mut self, id: &str) {
        self.flags.remove(&Self::key(id));
    }

    /// Read an indicator. An id the game has never mentioned reads `false`.
    #[must_use]
    pub fn get(&self, id: &str) -> bool {
        self.flags.get(&Self::key(id)).copied().unwrap_or(false)
    }

    /// Whether the game has ever reported this id.
    ///
    /// **Distinguishes "reported inactive" from "never reported"**, which
    /// [`Self::get`] deliberately collapses. `plan/12` §5.2: after a reconnect
    /// an unreported indicator is `Unknown`, and rendering it as a confident
    /// "no" is the stale-belief failure that rule exists to prevent.
    #[must_use]
    pub fn is_known(&self, id: &str) -> bool {
        self.flags.contains_key(&Self::key(id))
    }

    /// Every id the game has reported, with its current value.
    pub fn iter(&self) -> impl Iterator<Item = (&str, bool)> {
        self.flags.iter().map(|(k, v)| (k.as_str(), *v))
    }

    /// Forget everything reported.
    ///
    /// For `plan/12` §5.2's reconnect path: a new generation has been told
    /// nothing yet, so every indicator returns to *never reported* rather than
    /// keeping a belief from a connection that has ended.
    pub fn clear(&mut self) {
        self.flags.clear();
    }
}

/// Typed accessors for the indicators worth naming.
///
/// Generated so the list lives in one place. [`StatusInfo::set`] still accepts
/// any id the game invents -- this is a convenience over the map, never a
/// filter on it, which is what lets `IconPOISONED` and `IconDISEASED` work
/// despite being absent from the wiki's list (`plan/15` §2, note 1).
macro_rules! status_accessors {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {
        impl StatusInfo {
            $(
                $(#[$m])*
                ///
                /// **Reads `false` when the game has never reported this id**,
                /// including after a reconnect until the login burst re-teaches
                /// it. See [`Self::known`] for the three-state reading, and the
                /// type docs for why both exist.
                #[must_use]
                pub fn $name(&self) -> bool {
                    self.get(stringify!($name))
                }
            )*

            /// Every indicator, as a **three-state** reading.
            ///
            /// `None` is `plan/12` §5.2's `Unknown`: the game has not said.
            /// `Some(false)` is the game having said no.
            ///
            /// # Why this exists beside the `bool` accessors
            ///
            /// The typed accessors collapse those two, and they are what a
            /// caller reaches for -- `status.stunned()` reads like a question
            /// with an answer. After a reconnect it answers a confident
            /// `false` for every indicator, because `reconnect.rs` clears the
            /// map and `indicator` is absent from the login burst
            /// (MEASURED: unanimous across all seven captured logins).
            ///
            /// A behavior that casts on "not stunned" would act on a belief
            /// nothing supports, which is the stale-belief failure §5.2 exists
            /// to prevent (review MO-4).
            ///
            /// The collapsing accessors are kept rather than replaced: for a
            /// renderer, "no icon" is the right treatment of both states, and
            /// forcing every display site through an `Option` would add noise
            /// where there is no decision to make. The rule is the one §5.2
            /// already states -- **anything that ACTS reads this; anything
            /// that merely DISPLAYS may read the bool.**
            #[must_use]
            pub fn known(&self) -> StatusReading<'_> {
                StatusReading { info: self }
            }
        }

        /// A three-state view of [`StatusInfo`]. See [`StatusInfo::known`].
        #[derive(Debug, Clone, Copy)]
        pub struct StatusReading<'a> {
            info: &'a StatusInfo,
        }

        impl StatusReading<'_> {
            $(
                $(#[$m])*
                ///
                /// `None` when the game has never reported this id
                /// (`plan/12` §5.2's `Unknown`).
                #[must_use]
                pub fn $name(&self) -> Option<bool> {
                    self.info
                        .is_known(stringify!($name))
                        .then(|| self.info.get(stringify!($name)))
                }
            )*
        }
    };
}

status_accessors! {
    /// The default posture, and the only one the login burst reports active.
    standing,
    kneeling,
    sitting,
    prone,
    stunned,
    bleeding,
    hidden,
    invisible,
    webbed,
    /// True while grouped.
    ///
    /// Ported with Vellum's own note: before its map refactor this "was never
    /// written -- the parser had no arm for it -- so any code reading it saw a
    /// permanent `false`." A typed accessor over a map cannot have that bug,
    /// because the map is filled by id rather than by field.
    joined,
    dead,
    // --- text-derived, and there is NO indicator for any of these ----------
    //
    // `afflictions.rs` classifies the lines; these are the accessors, here
    // rather than in a second type because a caller asking "is this character
    // silenced" should not need to know whether an indicator or a sentence
    // said so. `infomon/status.rb` is the split CLAUDE.md names, and Lich
    // stores both in one place for the same reason.
    /// Immobilised (Bind, moonbeam traps). Text only.
    bound,
    /// Calmed, so unable to attack. Text only.
    calmed,
    /// Vocal cords cut, so unable to cast. Text only.
    cutthroat,
    /// Silenced by a pall of silence. Text only.
    silenced,
    /// Asleep. Text only.
    sleeping,
    /// Thorn-poisoned. Text only.
    thorned,
    /// On the wire and **absent from the wiki's list** (`plan/15` §2, note 1).
    poisoned,
    /// See [`Self::poisoned`].
    diseased,
}
