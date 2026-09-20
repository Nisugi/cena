//! Can this character act? The four injury predicates.
//!
//! Ported from `lib/gemstone/injured.rb` (195 lines). The rules are the
//! valuable part -- they encode game mechanics written down nowhere else, and
//! `plan/13` §4a's "port aggressively where knowledge lives in code" is
//! exactly this case.
//!
//! # What is NOT ported: the cache, the mutex, the mode-switching
//!
//! Lich's version is expensive, and says so:
//!
//! > *"one cache miss can block for roughly 15 seconds if the round trips time
//! > out. These are not instantaneous checks; avoid calling them in tight
//! > loops."*
//!
//! That cost buys **the scars**, not the rules. `Scars.all_scars` toggles the
//! game's injury mode and waits on `_injury` round trips, because the dialog
//! in mode `both` cannot show a scar underneath an active wound. The
//! fingerprint cache, the double-checked mutex and the 7.5-second timeouts all
//! exist to make that affordable.
//!
//! None of it belongs here. A model crate does not send commands
//! (`model_does_no_file_io` makes the same point about the filesystem), and
//! the rules themselves are arithmetic over values a caller already has.
//!
//! # How wounds and scars relate -- settled by the wiki
//!
//! `reference/wiki_clean/Wound.txt` is the primary source and it states the
//! whole model. Three things it settles that this module first got wrong:
//!
//! **1. A part carries a wound AND a scar at once, and both are readable.**
//!
//! > *"Scars are the remnants of old wounds that have been partially healed
//! > through herbs or empathic self-healing. ... if there is both a scar and a
//! > fresh wound on the same location then **the wound must be healed
//! > first**."*
//!
//! The author's progression is the same fact from the player's side:
//!
//! > **AUTHOR, 2026-09-20:** *"R2W -> R1W & R2S -> R0W & R2S -> R1S ->
//! > Healthy."*
//!
//! `character.rs` used to write `scar: 0` on every wound image, erasing a
//! known scar whenever a new wound arrived. It now retains it, as Lich does
//! (`xmlparser.rb:811-815`).
//!
//! **2. Scars come only from healing**, so one cannot appear mid-hunt:
//!
//! > **AUTHOR:** *"You have to heal to get the scar. So it's not like you're
//! > out hunting and oh no I got a R2W and R3S."*
//!
//! > *"A character who is healed by an empath will not have a scar."*
//!
//! **3. `effective = max(wound, scar unless rank 1)` is the wiki's own rule:**
//!
//! > *"Rank 1 scars never have any mechanical penalties. More significant
//! > scars have penalties **similar to wounds of the same level**."*
//!
//! # An `Unknown` answer was designed and then removed
//!
//! This module briefly carried an `Able::Unknown`, on the reading that Lich's
//! `Scars.all_scars` mode-toggling meant a scar under a wound was
//! unobservable. It is not: `eherbs.lic:2887` reads `h['scar']` and
//! `h['wound']` from one hash for every part, and the wiki describes them as
//! two coexisting facts. The uncertainty was mine, not the game's.
//!
//! # What is NOT ported: the cache, the mutex, the mode-switching
//!
//! Lich's version is expensive, and says so:
//!
//! > *"one cache miss can block for roughly 15 seconds if the round trips time
//! > out. These are not instantaneous checks; avoid calling them in tight
//! > loops."*
//!
//! None of it belongs here. A model crate does not send commands
//! (`model_does_no_file_io` makes the same point about the filesystem), and
//! the rules are arithmetic over values the dialog already gave us.
//!
//! # A second implementation exists and disagrees
//!
//! `eherbs.lic:2885-2914` has its own `able_to_cast`, and it is not the same
//! rule: it sums wound and scar **separately** across an arm/hand pair, where
//! `injured.rb` takes `max(arm, hand)` after merging them, and it counts rank-1
//! scars that `injured.rb` and the wiki both discard.
//!
//! **AUTHOR, 2026-09-20: "I would say injured.rb implementation is the right
//! one."** Recorded in `inventory/10` rather than reconciled here.
//!
use std::collections::BTreeMap;

use super::Injury;

/// A body part's wire id, as the `injuries` dialog names it.
///
/// Not an enum: the wire's part list is the game's, `nsys` sits in it beside
/// anatomical parts, and a part we have never seen must still be storable
/// (Rule 2.2). The groups below name the ones the rules care about.
pub type PartId = str;

/// The parts each rule reasons about, from `injured.rb:7-14`.
pub mod groups {
    /// Both eyes.
    pub const EYES: [&str; 2] = ["leftEye", "rightEye"];
    /// Both arms.
    pub const ARMS: [&str; 2] = ["leftArm", "rightArm"];
    /// Both hands.
    pub const HANDS: [&str; 2] = ["leftHand", "rightHand"];
    /// Both legs.
    pub const LEGS: [&str; 2] = ["leftLeg", "rightLeg"];
    /// Both feet.
    pub const FEET: [&str; 2] = ["leftFoot", "rightFoot"];
    /// The head and the nervous system.
    ///
    /// `nsys` is a body part on this wire, not a separate mechanism --
    /// `character.rs`'s module doc records the same thing.
    pub const HEAD_AND_NERVES: [&str; 2] = ["head", "nsys"];
}

/// Why an action is blocked, or that it is not.
///
/// **Three-valued, and the third is the point.** A predicate returning `bool`
/// would have to answer `true` or `false` for a character whose scars are
/// hidden under wounds, and both answers can be wrong. `Unknown` is the same
/// distinction `PsmSet::has_table` and `Effects::active_in` draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Able {
    /// The character can perform the action.
    Yes,
    /// A rank-3 injury blocks it, and Sigil of Determination cannot bypass.
    ///
    /// Certain regardless of hidden scars: a wound at rank 3 blocks whatever
    /// is underneath it.
    NoCritical,
    /// Blocked by a rank-2 injury or a cumulative total.
    NoInjured,
}

impl Able {
    /// Can the character do it?
    ///
    /// The two `No` variants are kept distinct because Sigil of Determination
    /// bypasses one and not the other, and a caller deciding whether to cast
    /// Sigil needs to know which it is looking at.
    #[must_use]
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }

    /// Would Sigil of Determination unblock this?
    ///
    /// The wiki: *"Sigil of Determination allows you to ignore penalties from
    /// rank 2 and lower wounds, including combined penalties from multiple
    /// wounds. It does not allow you to ignore penalties from rank 3 wounds."*
    #[must_use]
    pub const fn sigil_would_help(self) -> bool {
        matches!(self, Self::NoInjured)
    }
}

/// The injury state the predicates read.
///
/// Borrows the model's map rather than copying it, and carries an optional
/// overlay of scars a `_injury` round trip revealed.
#[derive(Debug, Clone, Copy)]
pub struct Injuries<'a> {
    parts: &'a BTreeMap<String, Injury>,
    /// Whether Sigil of Determination is active.
    sigil: bool,
}

impl<'a> Injuries<'a> {
    /// Read the predicates against what the `injuries` dialog showed.
    ///
    /// Scars under wounds are invisible; see the module docs.
    #[must_use]
    pub const fn new(parts: &'a BTreeMap<String, Injury>) -> Self {
        Self {
            parts,
            sigil: false,
        }
    }

    /// Whether Sigil of Determination is active.
    ///
    /// `injured.rb:90-92` reads `Effects::Buffs.active?("Sigil of
    /// Determination")`. Passed in rather than read here, because this crate's
    /// effects live beside it and a classifier that reached for them would be
    /// the stateful consumer `plan/12` §3a forbids.
    #[must_use]
    pub const fn with_sigil(mut self, active: bool) -> Self {
        self.sigil = active;
        self
    }

    /// The rank that governs one part.
    ///
    /// `injured.rb:66-72`: **rank-1 scars are ignored**, and the worse of
    /// wound and scar wins. Both facts are the game's, not Lich's convenience.
    #[must_use]
    pub fn effective_rank(&self, part: &PartId) -> u8 {
        let injury = self.parts.get(part).copied().unwrap_or_default();
        // A rank-1 scar carries no penalty (the wiki, "Wound Penalties").
        let scar = if injury.scar == 1 { 0 } else { injury.scar };
        injury.wound.max(scar)
    }

    /// The worse of a matched pair, as the arm/hand rules require.
    ///
    /// `injured.rb:110-117` takes `max(arm, hand)` **per side** rather than
    /// summing them: a rank-2 arm and a rank-2 hand on one side is a rank-2
    /// side, not a rank-4 one.
    fn side(&self, arm: &PartId, hand: &PartId) -> u8 {
        self.effective_rank(arm).max(self.effective_rank(hand))
    }

    /// Any of these parts at exactly this rank.
    fn any_at(&self, rank: u8, parts: &[&PartId]) -> bool {
        parts.iter().any(|p| self.effective_rank(p) == rank)
    }

    /// Any of these parts at this rank or worse.
    fn any_at_or_above(&self, rank: u8, parts: &[&PartId]) -> bool {
        parts.iter().any(|p| self.effective_rank(p) >= rank)
    }

    /// Can the character cast spells? `injured.rb:101-135`.
    ///
    /// The most involved of the four, and the only one with a cumulative rule.
    #[must_use]
    pub fn able_to_cast(&self) -> Able {
        let critical: Vec<&PartId> = ["head", "nsys"].into_iter().chain(groups::EYES).collect();
        if self.any_at(3, &critical) {
            return Able::NoCritical;
        }

        let left = self.side("leftArm", "leftHand");
        let right = self.side("rightArm", "rightHand");
        if left == 3 || right == 3 {
            return Able::NoCritical;
        }

        if self.sigil {
            return Able::Yes;
        }

        if self.any_at_or_above(2, &groups::HEAD_AND_NERVES) {
            return Able::NoInjured;
        }

        // **Cumulative**, and this is where a hidden scar bites: two rank-1
        // wounds each covering a rank-2 scar total 2 here and 4 in the game.
        let eyes: u8 = groups::EYES.iter().map(|p| self.effective_rank(p)).sum();
        if eyes >= 3 {
            return Able::NoInjured;
        }
        if left.saturating_add(right) >= 3 {
            return Able::NoInjured;
        }

        Able::Yes
    }

    /// Can the character sneak? `injured.rb:138-153`.
    ///
    /// # A Lich quirk, flagged rather than reproduced as fact
    ///
    /// Lich's comment reads: *"Rank 3 leg/foot injuries always prevent
    /// sneaking, but these are NOT critical (Sigil cannot bypass, but they're
    /// not in the critical list for other actions)"*.
    ///
    /// The parenthesis describes the code and not a game rule. Rank-3
    /// legs/feet block sneaking and Sigil does not help -- which is what every
    /// other action's "critical" means -- so calling them not-critical
    /// distinguishes nothing observable. **AUTHOR, 2026-09-20: "Lich quirk --
    /// flag it."**
    ///
    /// Recorded in `inventory/10`. Ported as [`Able::NoCritical`], because
    /// that is what the behaviour is; the naming is the only thing that
    /// differs.
    #[must_use]
    pub fn able_to_sneak(&self) -> Able {
        let parts: Vec<&PartId> = groups::LEGS.into_iter().chain(groups::FEET).collect();
        if self.any_at(3, &parts) {
            return Able::NoCritical;
        }
        if self.sigil {
            return Able::Yes;
        }
        if self.any_at_or_above(2, &parts) {
            return Able::NoInjured;
        }
        Able::Yes
    }

    /// Can the character search? `injured.rb:156-171`.
    #[must_use]
    pub fn able_to_search(&self) -> Able {
        let critical: Vec<&PartId> = ["head", "nsys"].into_iter().chain(groups::EYES).collect();
        if self.any_at(3, &critical) {
            return Able::NoCritical;
        }
        if self.sigil {
            return Able::Yes;
        }
        if self.any_at_or_above(2, &groups::HEAD_AND_NERVES) {
            return Able::NoInjured;
        }
        Able::Yes
    }

    /// Can the character use ranged weapons? `injured.rb:174-189`.
    ///
    /// The widest critical list: head, nerves, eyes, arms **and** hands.
    #[must_use]
    pub fn able_to_use_ranged(&self) -> Able {
        let critical: Vec<&PartId> = ["head", "nsys"]
            .into_iter()
            .chain(groups::EYES)
            .chain(groups::ARMS)
            .chain(groups::HANDS)
            .collect();
        if self.any_at(3, &critical) {
            return Able::NoCritical;
        }
        if self.sigil {
            return Able::Yes;
        }
        // **`nsys` is in this list and not in `HEAD_AND_NERVES`'s usage
        // above** -- the rank-2 check here is arms, hands and nerves, with the
        // head excluded. Faithful to `injured.rb:187`, and **VERIFIED in the
        // game by the author** (2026-09-20) against a wiki table that lists
        // only spellcasting and searching for rank-2 nerves. The table is
        // incomplete, not contradictory (`inventory/10` §9b).
        let blocking: Vec<&PartId> = groups::ARMS
            .into_iter()
            .chain(groups::HANDS)
            .chain(["nsys"])
            .collect();
        if self.any_at_or_above(2, &blocking) {
            return Able::NoInjured;
        }
        Able::Yes
    }
}
