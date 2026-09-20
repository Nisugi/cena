//! One creature the feed has shown: its statuses, wounds, damage and UCS
//! state, and the questions a hunter asks of it.
//!
//! Ports `CreatureInstance` (`lib/gemstone/creature.rb:325-716`) and the
//! per-instance half of `CreatureBase` (`creature_base.rb:531-712`).
//!
//! # The clock is the server's
//!
//! Every Lich timer here reads `Time.now`. The model has no wall clock -- it
//! has [`GameState::game_time_now`](crate::GameState::game_time_now), the
//! server epoch extrapolated from the last prompt -- so every timestamp is a
//! server second and every method that must compare one takes `now:
//! Option<u32>`. With no clock (`None`, before the first prompt) a timed
//! status simply does not expire and an estimate is not active: `plan/12`
//! §5.2, an unknown clock reports unknown rather than a guess.

use std::collections::{BTreeMap, BTreeSet};

use super::body::BodyPart;
use crate::state::combat::status::StatusName;
use crate::state::combat::ucs::{PositionTier, UcsAttack};
use crate::state::creature::status::{Classification, CreatureStatus, Status};
use crate::state::creature::{Stat, by_name};

/// Seconds per stun round. The crit tables express stun in ROUNDS while
/// their `roundtime` is already seconds (`creature.rb:333-338`).
pub const STUN_ROUND_SECONDS: u32 = 5;
/// UCS positioning expires after this many seconds (`creature.rb:331`).
pub const UCS_TTL: u32 = 120;
/// A smite holds for this many seconds (`creature.rb:332`).
pub const UCS_SMITE_TTL: u32 = 15;
/// Max HP when the bestiary has no template for the name (`creature.rb:592`).
pub const FALLBACK_MAX_HP: u32 = 400;

/// `BOON_ADJECTIVES` (`creature.rb:20-27`): a prefix the bestiary strips
/// before looking a name up. Lich builds one regex of these; the check here
/// is a prefix compare, longest first so `sickly green` beats nothing.
const BOON_ADJECTIVES: &[&str] = &[
    "adroit",
    "afflicted",
    "apt",
    "barbed",
    "belligerent",
    "blurry",
    "canny",
    "combative",
    "dazzling",
    "deft",
    "diseased",
    "drab",
    "dreary",
    "ethereal",
    "flashy",
    "flexile",
    "flickering",
    "flinty",
    "frenzied",
    "ghastly",
    "ghostly",
    "gleaming",
    "glittering",
    "glorious",
    "glowing",
    "grotesque",
    "hardy",
    "illustrious",
    "indistinct",
    "keen",
    "lanky",
    "luminous",
    "lustrous",
    "muculent",
    "nebulous",
    "oozing",
    "pestilent",
    "radiant",
    "raging",
    "ready",
    "resolute",
    "robust",
    "rune-covered",
    "shadowy",
    "shielded",
    "shifting",
    "shimmering",
    "shining",
    "sickly green",
    "sinuous",
    "slimy",
    "sparkling",
    "spindly",
    "spiny",
    "stalwart",
    "steadfast",
    "stout",
    "tattoed",
    "tattooed",
    "tenebrous",
    "tough",
    "twinkling",
    "unflinching",
    "unyielding",
    "wavering",
    "wispy",
];

/// Statuses that satisfy the coup de grace's "incapacitated" requirement
/// (`COUP_INCAP_STATUSES`, `creature.rb:616`). Positions are deliberately
/// excluded.
const COUP_INCAP: [StatusName; 4] = [
    StatusName::Stunned,
    StatusName::Immobilized,
    StatusName::Webbed,
    StatusName::Sleeping,
];

/// The bestiary's max HP for a display name, boon adjective stripped.
///
/// `CreatureTemplate[]` (`creature.rb:151-162`) keys templates by lowercased
/// name, so the first template of that name answers; a range answers its
/// high end, so a creature is not declared dead early.
fn template_max_hp(name: &str) -> Option<u32> {
    let hp = |n: &str| {
        by_name(n)
            .first()
            .and_then(|c| c.max_hp)
            .map(|s: Stat| u32::try_from(s.high()).unwrap_or(0))
            .filter(|&h| h > 0)
    };
    if let Some(h) = hp(name) {
        return Some(h);
    }
    let lower = name.to_ascii_lowercase();
    let stripped = BOON_ADJECTIVES
        .iter()
        .find_map(|adj| lower.strip_prefix(adj).and_then(|r| r.strip_prefix(' ')))?;
    hp(stripped.trim())
}

/// One tracked creature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatureInstance {
    /// The `exist` id.
    pub id: i64,
    /// The link's noun, when the feed gave one.
    pub noun: Option<String>,
    /// The display name.
    pub name: String,
    /// Active statuses, each with its expiry (server second) when timed.
    statuses: BTreeMap<StatusName, Option<u32>>,
    /// `<crtrStatus>` classifications, as last stated.
    flags: BTreeMap<Classification, bool>,
    /// Wound rank per part.
    injuries: BTreeMap<BodyPart, u8>,
    /// Parts that are gone.
    amputated: BTreeSet<BodyPart>,
    /// Damage this session has seen it take.
    damage_taken: u32,
    /// A fatal crit landed.
    fatal_crit: bool,
    ucs_position: Option<PositionTier>,
    ucs_tierup: Option<UcsAttack>,
    ucs_smote_at: Option<u32>,
    ucs_updated_at: Option<u32>,
    stun_rounds: Option<u16>,
    stun_until: Option<u32>,
    /// The bestiary's max HP, looked up once by name.
    ///
    /// The **fallback**. The server now states real hit points for hostile
    /// creatures, and [`Self::stated_health`] outranks this wherever both
    /// exist -- see [`Self::max_hp`].
    template_max_hp: Option<u32>,
    /// `<crtrStatus health=>`: current hit points, as the server last stated.
    ///
    /// **Signed**: a dead creature reads `health="-10"`.
    stated_health: Option<i32>,
    /// `<crtrStatus maxhealth=>`: maximum hit points, as the server last
    /// stated. `Some(0)` means the creature has no HP model.
    stated_max_health: Option<u32>,
    /// When the feed first and last showed it.
    pub first_seen_at: Option<u32>,
    pub last_seen_at: Option<u32>,
}

impl CreatureInstance {
    /// A new instance, seen now.
    #[must_use]
    pub fn new(id: i64, noun: Option<&str>, name: &str, now: Option<u32>) -> Self {
        Self {
            id,
            noun: noun.map(str::to_owned),
            name: name.to_owned(),
            statuses: BTreeMap::new(),
            flags: BTreeMap::new(),
            injuries: BTreeMap::new(),
            amputated: BTreeSet::new(),
            damage_taken: 0,
            fatal_crit: false,
            ucs_position: None,
            ucs_tierup: None,
            ucs_smote_at: None,
            ucs_updated_at: None,
            stun_rounds: None,
            stun_until: None,
            template_max_hp: template_max_hp(name),
            stated_health: None,
            stated_max_health: None,
            first_seen_at: now,
            last_seen_at: now,
        }
    }

    /// The feed showed it again.
    pub fn touch_seen(&mut self, now: Option<u32>) {
        if now.is_some() {
            self.last_seen_at = now;
        }
    }

    /// Does the bestiary know this creature?
    #[must_use]
    pub const fn has_template(&self) -> bool {
        self.template_max_hp.is_some()
    }

    // --- statuses -------------------------------------------------------

    /// Add a status, with an explicit duration or the status's default.
    ///
    /// Re-adding with a duration EXTENDS the lock and never shortens it: a
    /// 3s roundtime followed by an overlapping 20s one keeps the 20s
    /// (`creature_base.rb:562-594`).
    pub fn add_status(&mut self, status: StatusName, now: Option<u32>, duration: Option<u32>) {
        let duration = duration.or_else(|| status.duration());
        let expires = match (now, duration) {
            (Some(n), Some(d)) => Some(n.saturating_add(d)),
            _ => None,
        };
        match self.statuses.get_mut(&status) {
            Some(current) => {
                if let Some(e) = expires
                    && current.is_none_or(|c| e > c)
                {
                    *current = Some(e);
                }
            }
            None => {
                self.statuses.insert(status, expires);
            }
        }
    }

    /// Remove a status. A stun removal retires the crit-table estimate with
    /// it, however much time that estimate had left.
    pub fn remove_status(&mut self, status: StatusName) {
        self.statuses.remove(&status);
        if status == StatusName::Stunned {
            self.clear_stun_estimate();
        }
    }

    /// Drop timed statuses whose expiry has passed.
    pub fn expire_statuses(&mut self, now: Option<u32>) {
        let Some(now) = now else {
            return;
        };
        self.statuses
            .retain(|_, expires| expires.is_none_or(|e| e > now));
    }

    /// Is the status active now?
    #[must_use]
    pub fn has_status(&self, status: StatusName, now: Option<u32>) -> bool {
        self.statuses
            .get(&status)
            .is_some_and(|expires| match (expires, now) {
                (Some(e), Some(n)) => *e > n,
                _ => true,
            })
    }

    /// Every active status, now.
    #[must_use]
    pub fn statuses(&self, now: Option<u32>) -> Vec<StatusName> {
        self.statuses
            .keys()
            .copied()
            .filter(|s| self.has_status(*s, now))
            .collect()
    }

    /// Reconcile from a `<crtrStatus>` (`sync_crtr_status`,
    /// `creature_base.rb:647-671`).
    ///
    /// Scoped to the flags the tag can carry: the tag is a full snapshot of
    /// exactly those, so an absent one is cleared -- and it must not touch
    /// the rest. Blind, poisoned and the other message-only effects never
    /// appear in the tag; reconciling them here would clear them on the
    /// next one.
    pub fn sync_crtr_status(&mut self, status: &CreatureStatus, now: Option<u32>) {
        self.touch_seen(now);
        for s in Status::ALL {
            let canonical = StatusName::from_crtr(s);
            if status.is(s) {
                self.add_status(canonical, now, None);
            } else if self.statuses.contains_key(&canonical) {
                self.remove_status(canonical);
            }
        }
        for c in Classification::ALL {
            self.flags.insert(c, status.is_classified(c));
        }
        // Absolute hit points, when the server states them. A frame that
        // omits them says nothing, so the last stated value stands -- unlike
        // the flags above, which the tag re-sends in full every time.
        if status.health.is_some() {
            self.stated_health = status.health;
        }
        if status.max_health.is_some() {
            self.stated_max_health = status.max_health;
        }
    }

    /// Current hit points as the server last stated them.
    ///
    /// Negative for a dead creature.
    #[must_use]
    pub const fn stated_health(&self) -> Option<i32> {
        self.stated_health
    }

    /// Maximum hit points as the server last stated them.
    ///
    /// `Some(0)` means the creature has no HP model -- a shopkeeper rather
    /// than a combatant.
    #[must_use]
    pub const fn stated_max_health(&self) -> Option<u32> {
        self.stated_max_health
    }

    /// **Does our damage tally agree with what the server says?**
    ///
    /// > **AUTHOR, 2026-09-20:** *"damage_taken becomes a check on our combat
    /// > parser, if damage_taken and max_health - health are not the same,
    /// > then we know our tracking missed something."*
    ///
    /// Returns the signed discrepancy in hit points: `damage_taken` minus the
    /// server's `maxhealth - health`. Zero is agreement. **Positive** means we
    /// counted damage the creature did not take; **negative** means it lost
    /// health we never saw -- another hunter's blow, a damage-over-time tick,
    /// or a message form the parser does not recognise. That last case is the
    /// one worth surfacing: it is the combat parser silently missing a line.
    ///
    /// `None` when the server has stated no HP model for this creature, when
    /// `maxhealth` is 0, or when nothing has been stated at all -- there is
    /// nothing to reconcile against.
    #[must_use]
    pub fn damage_discrepancy(&self) -> Option<i64> {
        let (health, max) = (self.stated_health?, self.stated_max_health?);
        if max == 0 {
            return None;
        }
        let server_lost = i64::from(max) - i64::from(health);
        Some(i64::from(self.damage_taken) - server_lost)
    }

    /// A classification flag. Always-sent booleans: unseen is `false`.
    #[must_use]
    pub fn flag(&self, class: Classification) -> bool {
        self.flags.get(&class).copied().unwrap_or(false)
    }

    /// Has any `<crtrStatus>` been seen? A ridden mount carries a bold link
    /// but never a tag of its own, so "not hostile" and "not told" differ.
    #[must_use]
    pub fn flags_known(&self) -> bool {
        !self.flags.is_empty()
    }

    // --- wounds and damage -----------------------------------------------

    /// Add wound rank to a part. An amputated limb cannot be wounded further.
    pub fn add_injury(&mut self, part: BodyPart, rank: u8) {
        if self.amputated.contains(&part) {
            return;
        }
        let entry = self.injuries.entry(part).or_insert(0);
        *entry = entry.saturating_add(rank);
    }

    /// The wound rank at a part.
    #[must_use]
    pub fn injury(&self, part: BodyPart) -> u8 {
        self.injuries.get(&part).copied().unwrap_or(0)
    }

    /// Every part wounded to at least `threshold`.
    #[must_use]
    pub fn injured_parts(&self, threshold: u8) -> Vec<BodyPart> {
        self.injuries
            .iter()
            .filter(|(_, r)| **r >= threshold)
            .map(|(p, _)| *p)
            .collect()
    }

    /// A limb came off: gone rather than wounded, and it stays gone.
    pub fn amputate(&mut self, part: BodyPart) {
        self.amputated.insert(part);
    }

    /// Is the part gone?
    #[must_use]
    pub fn is_amputated(&self, part: BodyPart) -> bool {
        self.amputated.contains(&part)
    }

    /// Record damage taken.
    pub fn add_damage(&mut self, amount: u32) {
        self.damage_taken = self.damage_taken.saturating_add(amount);
    }

    /// Damage seen this session.
    #[must_use]
    pub const fn damage_taken(&self) -> u32 {
        self.damage_taken
    }

    /// Forget the damage: healed or respawned.
    pub const fn reset_damage(&mut self) {
        self.damage_taken = 0;
    }

    /// A fatal crit landed.
    pub const fn mark_fatal_crit(&mut self) {
        self.fatal_crit = true;
    }

    /// Did a fatal crit land?
    #[must_use]
    pub const fn fatal_crit(&self) -> bool {
        self.fatal_crit
    }

    /// Max HP: **the server's when it has stated one**, else the bestiary
    /// template's, else [`FALLBACK_MAX_HP`].
    ///
    /// The stated value wins because it is the answer rather than an
    /// estimate. A `maxhealth` of 0 is not an answer -- it means the creature
    /// has no HP model -- so it falls through to the template.
    #[must_use]
    pub fn max_hp(&self) -> u32 {
        self.stated_max_health
            .filter(|max| *max > 0)
            .or(self.template_max_hp)
            .unwrap_or(FALLBACK_MAX_HP)
    }

    /// Current HP: **the server's when it has stated one**, else max less the
    /// damage this session counted.
    ///
    /// The inferred form is what a combat tracker had to do before the game
    /// reported hit points, and it is still right for every creature the
    /// server says nothing about.
    #[must_use]
    pub fn current_hp(&self) -> u32 {
        if self.stated_max_health.is_some_and(|max| max > 0)
            && let Some(health) = self.stated_health
        {
            // Negative is a dead creature; this accessor is unsigned because
            // "how many hit points does it have" floors at none.
            // `stated_health` keeps the sign for anything that needs it.
            return health.max(0).unsigned_abs();
        }
        self.max_hp().saturating_sub(self.damage_taken)
    }

    /// Whether [`Self::current_hp`] is the server's number rather than ours.
    #[must_use]
    pub fn hp_is_stated(&self) -> bool {
        self.stated_health.is_some() && self.stated_max_health.is_some_and(|max| max > 0)
    }

    /// Current HP as a percentage, 0..=100.
    #[must_use]
    pub fn hp_percent(&self) -> f64 {
        f64::from(self.current_hp()) * 100.0 / f64::from(self.max_hp())
    }

    /// Does it qualify for a coup de grace at the given trained rank?
    ///
    /// At or below `rank * 10`% of max HP when incapacitated, `rank * 5`%
    /// otherwise, hard-capped at 200 HP either way (`creature.rb:622-629`).
    #[must_use]
    pub fn coup_eligible(&self, rank: u32, now: Option<u32>) -> bool {
        if rank == 0 {
            return false;
        }
        let incap = COUP_INCAP.iter().any(|s| self.has_status(*s, now));
        let pct = rank * if incap { 10 } else { 5 };
        let threshold = (f64::from(self.max_hp()) * f64::from(pct) / 100.0).min(200.0);
        f64::from(self.current_hp()) <= threshold
    }

    /// Out of hit points (`creature.rb:631`).
    #[must_use]
    pub fn dead(&self) -> bool {
        self.current_hp() == 0
    }

    /// Should this be attacked? (`valid_target?`, `creature.rb:642-651`):
    /// not dead by flag or HP, not an animated decoy (slush excepted), not
    /// a bare appendage (kraken tentacles excepted).
    #[must_use]
    pub fn valid_target(&self) -> bool {
        if self.flag(Classification::Dead) || self.dead() {
            return false;
        }
        let lower = self.name.to_ascii_lowercase();
        if lower.starts_with("animated") && !lower.starts_with("animated slush") {
            return false;
        }
        let appendage = self.noun.as_deref().is_some_and(|n| {
            matches!(
                n.to_ascii_lowercase().trim_end_matches('s'),
                "arm" | "appendage" | "claw" | "limb" | "pincer" | "tentacle"
            ) || matches!(n.to_ascii_lowercase().as_str(), "palpus" | "palpi")
        });
        if appendage
            && !["amaranthine", "ghostly", "grizzled", "ancient"]
                .iter()
                .any(|a| lower.contains(&format!("{a} kraken tentacle")))
        {
            return false;
        }
        true
    }

    /// Can it not act right now? (`muckled?`, `creature.rb:658-661`.)
    /// Narrower than every tracked status: not penalties, not positions.
    #[must_use]
    pub fn muckled(&self, now: Option<u32>) -> bool {
        self.flag(Classification::Dead)
            || self.dead()
            || [
                StatusName::Webbed,
                StatusName::Stunned,
                StatusName::Sleeping,
                StatusName::Immobilized,
                StatusName::Rooted,
            ]
            .iter()
            .any(|s| self.has_status(*s, now))
    }

    // --- stun estimate ----------------------------------------------------

    /// Record a crit-table stun estimate in rounds, anchored to `at`.
    /// Advisory: the boolean stays owned by the feed and the messages.
    /// Extends, never shortens.
    pub fn add_stun_estimate(&mut self, rounds: u16, at: Option<u32>) {
        let Some(at) = at else {
            return;
        };
        if rounds == 0 {
            return;
        }
        let until = at.saturating_add(u32::from(rounds) * STUN_ROUND_SECONDS);
        if self.stun_until.is_some_and(|u| u >= until) {
            return;
        }
        self.stun_rounds = Some(rounds);
        self.stun_until = Some(until);
    }

    fn stun_estimate_active(&self, now: Option<u32>) -> bool {
        match (self.stun_until, now) {
            (Some(u), Some(n)) => u > n && self.has_status(StatusName::Stunned, Some(n)),
            _ => false,
        }
    }

    /// Estimated stun rounds from the last crit, while the estimate holds.
    #[must_use]
    pub fn stun_rounds(&self, now: Option<u32>) -> Option<u16> {
        self.stun_estimate_active(now)
            .then_some(self.stun_rounds)
            .flatten()
    }

    /// Estimated seconds of stun remaining; 0 with no active estimate.
    #[must_use]
    pub fn stunned_for(&self, now: Option<u32>) -> u32 {
        if !self.stun_estimate_active(now) {
            return 0;
        }
        match (self.stun_until, now) {
            (Some(u), Some(n)) => u.saturating_sub(n),
            _ => 0,
        }
    }

    /// The creature shook off the stun.
    pub const fn clear_stun_estimate(&mut self) {
        self.stun_rounds = None;
        self.stun_until = None;
    }

    // --- UCS ----------------------------------------------------------------

    fn ucs_expired(&self, now: Option<u32>) -> bool {
        match (self.ucs_updated_at, now) {
            (Some(u), Some(n)) => n.saturating_sub(u) > UCS_TTL,
            (None, _) => true,
            (Some(_), None) => false,
        }
    }

    /// Our positioning tier against it. A tier change clears the tierup.
    pub fn set_ucs_position(&mut self, tier: PositionTier, now: Option<u32>) {
        if self.ucs_position != Some(tier) {
            self.ucs_tierup = None;
        }
        self.ucs_position = Some(tier);
        self.ucs_updated_at = now;
    }

    /// A followup opened.
    pub fn set_ucs_tierup(&mut self, attack: UcsAttack, now: Option<u32>) {
        self.ucs_tierup = Some(attack);
        self.ucs_updated_at = now;
    }

    /// The crimson mist landed.
    pub fn smite(&mut self, now: Option<u32>) {
        self.ucs_smote_at = now;
        self.ucs_updated_at = now;
    }

    /// The mist is gone.
    pub fn clear_smote(&mut self, now: Option<u32>) {
        self.ucs_smote_at = None;
        self.ucs_updated_at = now;
    }

    /// Is it smote, within the smite TTL?
    #[must_use]
    pub fn smote(&self, now: Option<u32>) -> bool {
        match (self.ucs_smote_at, now) {
            (Some(at), Some(n)) => n.saturating_sub(at) <= UCS_SMITE_TTL,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    /// Our positioning tier, unless expired.
    #[must_use]
    pub fn ucs_position(&self, now: Option<u32>) -> Option<PositionTier> {
        (!self.ucs_expired(now))
            .then_some(self.ucs_position)
            .flatten()
    }

    /// The open followup, unless expired.
    #[must_use]
    pub fn ucs_tierup(&self, now: Option<u32>) -> Option<UcsAttack> {
        (!self.ucs_expired(now))
            .then_some(self.ucs_tierup)
            .flatten()
    }
}
