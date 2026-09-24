//! The creature registry: every creature the feed has shown, keyed by
//! `exist` id, with the current room's roster and what combat did to each.
//!
//! Ports the class half of `CreatureBase` (`lib/common/creature/
//! creature_base.rb:190-527`) -- the id-keyed registry, the room roster and
//! housekeeping -- and `processor.rb`'s `persist_event` (in [`apply`]), the
//! consumer that turns [`ChunkFacts`](crate::state::combat::ChunkFacts) into
//! creature state. [`instance`] is one creature; [`body`] is where a wound
//! lands.
//!
//! # Where the registry is fed from
//!
//! Lich registers from the XML parser's text path (`xmlparser.rb:1081`): a
//! bolded `<a exist>` in `room objs` whose id has a stashed `<crtrStatus>`
//! from earlier in the same body. On the wire the tag precedes the link it
//! describes, so Lich holds the tag until the link arrives.
//!
//! **The frames arrive the other way round.** Cena's parser emits the
//! `Frame::Component` first and the tags its body carried after it
//! (`parser/dispatch.rs`, `inline_paired`: *"After the frame, in wire
//! order"*, review PR-1). So the pairing runs link-first: the `room objs`
//! body's bold links are held in [`Creatures::apply_room_objs`], and each
//! `Frame::CreatureStatus` that follows claims its link in
//! [`Creatures::note_status`]. The condition is Lich's either way -- a
//! creature is registered when a tag vouches for its link. `<nav>` and each
//! `room objs` refresh clear the roster first (`xmlparser.rb:410`, `:456`).
//!
//! **Two deviations, both toward keeping a fact (Rule 2.2):**
//!
//! 1. A `<crtrStatus>` for a creature already registered is applied at once,
//!    not held for the next room refresh. Lich holds every one and applies
//!    it only from the link path, so a standalone tag for a known creature
//!    lags until the next `room objs` (`xmlparser.rb:485-495` explains why
//!    Lich chose that; the roster-marking it protects is done here on both
//!    paths).
//! 2. A combat event that names a creature by `exist` id registers it if the
//!    feed has not yet (`Creatures::ensure`). Lich's `persist_event` skips an
//!    unknown id -- *"No creature found for ID"* -- and the damage is lost.
//!    An attack on it is proof it exists and is here.
//!
//! # Bounds
//!
//! `MAX_SIZE` instances; housekeeping every `HOUSEKEEPING_INTERVAL` server
//! seconds drops creatures unseen for `CLEANUP_MAX_AGE`, never one on the
//! live roster or the roster just replaced (the mid-refresh shelter,
//! `creature_base.rb:314-323`). Evictions and refusals are counted.

pub mod apply;
pub mod body;
pub mod instance;

use std::collections::{BTreeMap, VecDeque};

pub use body::BodyPart;
pub use instance::CreatureInstance;

use crate::state::combat::target::Actor;
use crate::state::creature::status::{Classification, CreatureStatus};
use crate::state::room::RoomItem;

/// Most instances retained (`configure`'s default, `creature_base.rb:400`).
pub const MAX_SIZE: usize = 1000;
/// Seconds unseen before housekeeping removes a creature.
pub const CLEANUP_MAX_AGE: u32 = 600;
/// Seconds between housekeeping passes.
pub const HOUSEKEEPING_INTERVAL: u32 = 60;
/// How many touched creatures the death watch remembers
/// (`processor.rb`'s `DEATH_WATCH_MAX`: bounded FIFO).
pub const DEATH_WATCH_MAX: usize = 256;
/// How many announced deaths are remembered, so each is reported once.
pub const DEATH_ANNOUNCED_MAX: usize = 1000;

/// The registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Creatures {
    instances: BTreeMap<i64, CreatureInstance>,
    /// The current room's creatures, in feed order.
    roster: Vec<i64>,
    /// The roster `clear_room` just replaced: sheltered from housekeeping
    /// while the refresh re-marks creatures one at a time.
    previous_roster: Vec<i64>,
    /// `<crtrStatus>` frames no link has vouched for: a standalone tag for
    /// a creature not yet seen. Cleared by the next `room objs` or `<nav>`.
    pending_status: BTreeMap<i64, CreatureStatus>,
    /// Bold `room objs` links awaiting the tag that follows their component:
    /// `(name, noun)` by id.
    pending_links: BTreeMap<i64, (String, String)>,
    /// Creatures an event touched, awaiting a `dead` flag; oldest first.
    death_watch: VecDeque<i64>,
    /// Creatures the FEED said left, by id, with the direction if one was
    /// named: `creature_message.rs`'s flee lines.
    ///
    /// **This is the third condition of the author's hiding rule** -- "not seen
    /// to leave". A creature off the roster that is in here walked out; one
    /// that is not, and did not die, is unaccounted for.
    ///
    /// Bounded by the roster's own eviction: an id is dropped when the registry
    /// drops it, so this cannot outgrow the thing it annotates.
    ///
    /// > **CORRECTED 2026-09-23 (review).** The paragraph above was a wish:
    /// > nothing ever removed an id. So it grew for the session, and -- the
    /// > real defect -- a creature that fled, came BACK, and then hid was
    /// > still "seen to leave", and `vanished_unaccounted` excluded it: the
    /// > one case the inference exists for. An id now leaves this map when
    /// > it reappears on the roster ([`Self::mark_in_room`]), when the
    /// > registry drops it (eviction and `cleanup_old`), and on a reconnect.
    seen_to_leave: std::collections::BTreeMap<i64, Option<String>>,
    /// Deaths already announced.
    death_announced: VecDeque<i64>,
    /// Per chunk: creatures a message stood up, and the line it was read
    /// on (`None`: after every crit).
    position_recovered: BTreeMap<i64, Option<usize>>,
    last_housekeeping: Option<u32>,
    evicted: u64,
    refused: u64,
}

impl Creatures {
    // --- lookup ---------------------------------------------------------

    /// A creature by id.
    #[must_use]
    pub fn get(&self, id: i64) -> Option<&CreatureInstance> {
        self.instances.get(&id)
    }

    /// A creature by id, to change it.
    pub fn get_mut(&mut self, id: i64) -> Option<&mut CreatureInstance> {
        self.instances.get_mut(&id)
    }

    /// Every registered creature.
    pub fn all(&self) -> impl Iterator<Item = &CreatureInstance> {
        self.instances.values()
    }

    /// How many are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Is nothing registered?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// The current room's creature ids, in feed order.
    #[must_use]
    pub fn room_ids(&self) -> &[i64] {
        &self.roster
    }

    /// Every tracked creature in the room (`in_room`).
    pub fn in_room(&self) -> impl Iterator<Item = &CreatureInstance> {
        self.roster.iter().filter_map(|id| self.instances.get(id))
    }

    /// Attackable hostile creatures in the room (`targets`,
    /// `creature_base.rb:442-448`): room membership from the roster, not a
    /// stale client target; `valid_target` and the `hostile` flag.
    pub fn targets(&self) -> impl Iterator<Item = &CreatureInstance> {
        self.in_room()
            .filter(|c| c.valid_target() && c.flag(Classification::Hostile))
    }

    /// Instances dropped to the size bound.
    #[must_use]
    pub const fn evicted(&self) -> u64 {
        self.evicted
    }

    /// Registrations refused because the room alone filled the registry.
    #[must_use]
    pub const fn refused(&self) -> u64 {
        self.refused
    }

    /// `<crtrStatus>` frames still waiting for their link.
    #[must_use]
    pub fn pending_status_count(&self) -> usize {
        self.pending_status.len()
    }

    /// Is this creature on the death watch?
    #[must_use]
    pub fn is_watching(&self, id: i64) -> bool {
        self.death_watch.contains(&id)
    }

    // --- registration ---------------------------------------------------

    /// Register or look up a creature, marking it present in the room.
    ///
    /// `None` when the registry is full of creatures in the current room --
    /// counted in [`Self::refused`].
    pub fn register(
        &mut self,
        id: i64,
        name: &str,
        noun: Option<&str>,
        now: Option<u32>,
    ) -> Option<&mut CreatureInstance> {
        self.mark_in_room(id);
        self.housekeep(now);
        if self.instances.contains_key(&id) {
            let c = self.instances.get_mut(&id)?;
            c.touch_seen(now);
            return Some(c);
        }
        if self.instances.len() >= MAX_SIZE {
            self.evict_stalest();
            if self.instances.len() >= MAX_SIZE {
                self.refused = self.refused.saturating_add(1);
                return None;
            }
        }
        self.instances
            .insert(id, CreatureInstance::new(id, noun, name, now));
        self.instances.get_mut(&id)
    }

    /// Mark an id present in the room. `true` when newly added.
    ///
    /// **Arriving cancels a departure.** A creature on the roster again is
    /// here, so whatever the feed said about it leaving is history -- and if
    /// it now vanishes, that departure must not account for it.
    pub fn mark_in_room(&mut self, id: i64) -> bool {
        if self.roster.contains(&id) {
            return false;
        }
        self.roster.push(id);
        self.seen_to_leave.remove(&id);
        true
    }

    /// Ids that were in the room and are not now.
    ///
    /// Valid only between a roster rebuild and the next one:
    /// [`Self::clear_room`] moves the roster to `previous_roster`, so this is
    /// the difference the last restatement made.
    ///
    /// **Says nothing about WHY** — dead, walked off, or hidden. The caller
    /// decides, and `GameState::note_vanished` is the one that does.
    pub fn departed(&self) -> impl Iterator<Item = i64> + '_ {
        self.previous_roster
            .iter()
            .copied()
            .filter(|id| !self.roster.contains(id))
    }

    /// Record a departure the feed showed, if this line shows one.
    ///
    /// **Reads the markup** (`departure.rs`): the creature's `exist` id and the
    /// `<d>` direction link are both in the line, so there is no name lookup
    /// and no bestiary dependency. Only a creature this room knows is recorded
    /// -- a line about someone else's fight is not this room's news.
    pub fn read_departure(&mut self, line: &crate::state::chunks::ChunkLine) -> Option<i64> {
        let departure = crate::state::departure::classify(line)?;
        self.instances.contains_key(&departure.id).then(|| {
            self.note_fled(departure.id, Some(departure.direction));
            departure.id
        })
    }

    /// The feed said this creature left, and which way if it said.
    ///
    /// Recorded per id rather than as a one-shot flag: two creatures can flee
    /// in one chunk, and a behavior asking about the second must not get the
    /// first's answer.
    pub fn note_fled(&mut self, id: i64, direction: Option<String>) {
        self.seen_to_leave.insert(id, direction);
    }

    /// Whether the feed said this creature left, and which way.
    ///
    /// `None`: nothing said so. `Some(None)`: it left and no direction was
    /// named, which is a real case -- *"slowly backs away, its teeth bared"*
    /// is a flee line with no direction in it.
    #[must_use]
    pub fn fled(&self, id: i64) -> Option<Option<&str>> {
        self.seen_to_leave.get(&id).map(|d| d.as_deref())
    }

    /// Creatures that left the room with nothing accounting for it: **gone, not
    /// dead, and not seen to leave.**
    ///
    /// The author's rule, all three conditions (2026-09-20):
    ///
    /// > *"but gone just means not in the room, doesn't mean hid. We have
    /// > creature arrival and leaving messaging though, which would get tagged
    /// > somewhere along the way and get pushed to the creature."*
    ///
    /// A two-condition version of this was written and deleted, because
    /// departed-and-not-dead is satisfied by a creature that simply walked out.
    /// `creature_message.rs` supplies the third condition, so the inference can
    /// exist now.
    ///
    /// **Still evidence rather than proof.** A creature can leave by a route
    /// the bestiary has no line for, and this reports it as unaccounted for; a
    /// consumer should treat the answer as "worth looking" rather than "it is
    /// certainly hiding".
    pub fn vanished_unaccounted(&self) -> impl Iterator<Item = i64> + '_ {
        self.departed().filter(move |id| {
            self.fled(*id).is_none()
                && !self
                    .instances
                    .get(id)
                    .is_some_and(instance::CreatureInstance::dead)
        })
    }

    /// Empty the roster; the registry is untouched.
    pub fn clear_room(&mut self) {
        self.previous_roster = std::mem::take(&mut self.roster);
    }

    /// A `<nav>`: the roster and anything held belong to the room left.
    pub fn on_nav(&mut self) {
        self.clear_room();
        self.pending_status.clear();
        self.pending_links.clear();
    }

    /// A `<crtrStatus>` arrived. A known creature takes it at once
    /// (deviation 1 in the module doc); a held `room objs` link is claimed,
    /// registering the creature; otherwise the tag is held.
    pub fn note_status(&mut self, status: CreatureStatus, now: Option<u32>) {
        let Ok(id) = status.id.parse::<i64>() else {
            return;
        };
        if let Some(c) = self.instances.get_mut(&id) {
            c.sync_crtr_status(&status, now);
        } else if let Some((name, noun)) = self.pending_links.remove(&id) {
            if let Some(c) = self.register(id, &name, Some(&noun), now) {
                c.sync_crtr_status(&status, now);
            }
        } else {
            self.pending_status.insert(id, status);
        }
    }

    /// A `room objs` body arrived: rebuild the roster from its bold links.
    ///
    /// A creature already known is re-marked present. One with a held tag is
    /// registered now. The rest are held for the tags that follow the
    /// component -- a bold link no tag ever vouches for (a ridden mount) is
    /// never registered, as in Lich (`xmlparser.rb:1081`).
    pub fn apply_room_objs(&mut self, items: &[RoomItem], now: Option<u32>) {
        self.clear_room();
        self.pending_links.clear();
        for item in items {
            let Ok(id) = item.id.parse::<i64>() else {
                continue;
            };
            if self.instances.contains_key(&id) {
                self.register(id, &item.text, Some(&item.noun), now);
            } else if let Some(status) = self.pending_status.remove(&id) {
                if let Some(c) = self.register(id, &item.text, Some(&item.noun), now) {
                    c.sync_crtr_status(&status, now);
                }
            } else {
                self.pending_links
                    .insert(id, (item.text.clone(), item.noun.clone()));
            }
        }
        self.pending_status.clear();
    }

    /// A combat event named this creature: register it if the feed has not
    /// (deviation 2 in the module doc). Players and fixtures (non-positive
    /// ids) are never creatures.
    pub(super) fn ensure(&mut self, actor: &Actor, now: Option<u32>) -> Option<i64> {
        let id = actor.id.filter(|&i| i > 0)?;
        if !self.instances.contains_key(&id) && actor.name.is_empty() {
            return None;
        }
        self.register(id, &actor.name, actor.noun.as_deref(), now)?;
        Some(id)
    }

    /// The one creature whose name contains `name`, if exactly one does
    /// (`apply_status_to_target`'s name lookup, `processor.rb:2357-2358`).
    pub(super) fn only_named(&self, name: &str) -> Option<i64> {
        let wanted = name.to_ascii_lowercase();
        let mut found = self
            .instances
            .values()
            .filter(|c| c.name.to_ascii_lowercase().contains(&wanted))
            .map(|c| c.id);
        let first = found.next()?;
        found.next().is_none().then_some(first)
    }

    // --- housekeeping -----------------------------------------------------

    /// Ids housekeeping never evicts: the live roster and the one before it.
    fn sheltered(&self, id: i64) -> bool {
        self.roster.contains(&id) || self.previous_roster.contains(&id)
    }

    /// Drop creatures unseen for `max_age` seconds; returns how many.
    pub fn cleanup_old(&mut self, now: Option<u32>, max_age: u32) -> usize {
        let Some(now) = now else {
            return 0;
        };
        let cutoff = now.saturating_sub(max_age);
        let before = self.instances.len();
        let shelter: Vec<i64> = self
            .instances
            .keys()
            .copied()
            .filter(|id| self.sheltered(*id))
            .collect();
        self.instances.retain(|id, c| {
            shelter.contains(id) || c.last_seen_at.is_none_or(|seen| seen >= cutoff)
        });
        // Trimmed WITH the registry, as the field doc promises.
        let instances = &self.instances;
        self.seen_to_leave
            .retain(|id, _| instances.contains_key(id));
        let removed = before - self.instances.len();
        self.evicted = self.evicted.saturating_add(removed as u64);
        removed
    }

    /// `cleanup_old` at most once per `HOUSEKEEPING_INTERVAL`.
    fn housekeep(&mut self, now: Option<u32>) -> usize {
        let Some(n) = now else {
            return 0;
        };
        if self
            .last_housekeeping
            .is_some_and(|last| n.saturating_sub(last) < HOUSEKEEPING_INTERVAL)
        {
            return 0;
        }
        self.last_housekeeping = Some(n);
        self.cleanup_old(now, CLEANUP_MAX_AGE)
    }

    /// Evict the least-recently-seen creature not in the room, preferring
    /// one outside the previous room too (`creature_base.rb:358-373`).
    fn evict_stalest(&mut self) -> Option<i64> {
        let candidates: Vec<(i64, Option<u32>)> = self
            .instances
            .values()
            .filter(|c| !self.roster.contains(&c.id))
            .map(|c| (c.id, c.last_seen_at))
            .collect();
        let preferred: Vec<&(i64, Option<u32>)> = candidates
            .iter()
            .filter(|(id, _)| !self.previous_roster.contains(id))
            .collect();
        let pool: Vec<&(i64, Option<u32>)> = if preferred.is_empty() {
            candidates.iter().collect()
        } else {
            preferred
        };
        let (id, _) = pool.into_iter().min_by_key(|(_, seen)| *seen)?;
        let id = *id;
        self.instances.remove(&id);
        self.seen_to_leave.remove(&id);
        self.evicted = self.evicted.saturating_add(1);
        Some(id)
    }

    // --- death watch ------------------------------------------------------

    /// Remember a creature an event touched, until a room refresh flags it
    /// dead or it leaves the registry.
    pub(super) fn watch_for_death(&mut self, id: i64) {
        self.death_watch.retain(|&w| w != id);
        self.death_watch.push_back(id);
        while self.death_watch.len() > DEATH_WATCH_MAX {
            self.death_watch.pop_front();
        }
    }

    /// Every watched creature now flagged dead, each once.
    pub(super) fn sweep_deaths(&mut self) -> Vec<Actor> {
        let mut dead = Vec::new();
        let watched: Vec<i64> = self.death_watch.iter().copied().collect();
        for id in watched {
            let Some(c) = self.instances.get(&id) else {
                self.death_watch.retain(|&w| w != id);
                continue;
            };
            if !c.flag(Classification::Dead) {
                continue;
            }
            self.death_watch.retain(|&w| w != id);
            if self.death_announced.contains(&id) {
                continue;
            }
            self.death_announced.push_back(id);
            while self.death_announced.len() > DEATH_ANNOUNCED_MAX {
                self.death_announced.pop_front();
            }
            dead.push(Actor {
                id: Some(id),
                noun: c.noun.clone(),
                name: c.name.clone(),
            });
        }
        dead
    }

    // --- position recovery, per chunk -------------------------------------

    /// A new chunk: recoveries are per chunk (`processor.rb:366`).
    pub(super) fn begin_chunk(&mut self) {
        self.position_recovered.clear();
    }

    /// A message stood the creature up at `line`. Keeps the LATEST.
    pub(super) fn note_recovery(&mut self, id: i64, line: Option<usize>) {
        let prev = self.position_recovered.get(&id).copied();
        let later = match (prev, line) {
            (None, _) | (Some(Some(_)), None) => true,
            (Some(None), _) => false,
            (Some(Some(p)), Some(l)) => l >= p,
        };
        if later {
            self.position_recovered.insert(id, line);
        }
    }

    /// A later knockdown MESSAGE outranks the recovery.
    pub(super) fn clear_recovery(&mut self, id: i64) {
        self.position_recovered.remove(&id);
    }

    /// Did a message stand the creature up AFTER a crit on `crit_line`?
    /// A crit with no line falls back to "any recovery this chunk".
    pub(super) fn recovered_after(&self, id: i64, crit_line: Option<usize>) -> bool {
        match (self.position_recovered.get(&id), crit_line) {
            (None, _) => false,
            (Some(None), _) | (Some(Some(_)), None) => true,
            (Some(Some(at)), Some(line)) => *at > line,
        }
    }

    /// Forget who is in the room; keep what combat learned about each.
    pub(crate) fn invalidate_for_reconnect(&mut self) {
        self.roster.clear();
        self.previous_roster.clear();
        // Departures are about the room the old connection was watching, the
        // same as the rosters they annotate.
        self.seen_to_leave.clear();
        self.pending_status.clear();
        self.pending_links.clear();
        self.position_recovered.clear();
    }
}
