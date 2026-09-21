//! Which characters a spell has locked out.
//!
//! Ports the tracking half of Lich PR #1597, *"track spell cooldowns on the
//! characters a spell locks out"* (`gemstone/group.rb:168-250`).
//!
//! # Two cast mechanics, two ways of learning the target
//!
//! > *"the ones looking at member are ones that are self cast but affect your
//! > group."* — the author, 2026-09-20
//!
//! | Kind | Cast | How the target is learned |
//! |---|---|---|
//! | [`CooldownKind::Group`] | **on yourself**, lands on everyone grouped | it is not — stamp every member |
//! | [`CooldownKind::Target`] | at one character | the `target-start` message names them |
//!
//! Five spells declare one, and the invariant is checked over the whole table
//! in `spells.rs`: every `target` cooldown has a `target-start` message and no
//! `group` cooldown has one, because a self-cast names nobody.
//!
//! # The group case is optimistic, and bounded
//!
//! The caster's **only** notice that a group casting landed is a clause inside
//! their own start message — each of the three is one pattern with `your
//! group` as an optional alternation, and `parser.rb:662` tells the two views
//! apart with `line.include?('your group')`.
//!
//! So everyone grouped at the time is stamped, and Lich's comment
//! (`group.rb:170`) records what that costs and why it is acceptable:
//!
//! > *"A member who was out of range, or who was already on cooldown from an
//! > earlier casting, gets a stamp that is too early — the next casting
//! > corrects it, so the error is bounded by one cooldown and does not
//! > accumulate."*
//!
//! **A member still locked out is skipped rather than re-stamped**
//! (`group.rb:201`): they did not receive this casting, so their own cooldown
//! keeps running. Porting that wrong would extend a cooldown every time
//! someone else got a buff.
//!
//! # Keyed by noun, and the target case needs no group
//!
//! `record_target_cooldown` (`group.rb:212`) counts whoever the spell landed
//! on, in or out of the group — Lich's comment says *"seeing someone else put
//! a target on cooldown is as useful as doing it yourself"*.
//!
//! # What this does NOT port
//!
//! Lich records on the parser thread and takes care to call `_members` rather
//! than `members`, because `members` would send `GROUP` and block waiting for
//! a reply only that thread can parse (`group.rb:198`). Cena has no such
//! hazard — nothing here sends anything — but the reason is recorded because
//! it is the kind of constraint a port silently loses.

use std::collections::BTreeMap;

use crate::spells::{self, CooldownKind};
use crate::state::GameState;
use crate::state::group::Member;

/// When each character becomes castable again, by spell number then noun.
///
/// `@@spell_cooldowns` (`group.rb:24`), which is keyed the same way.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cooldowns {
    /// Spell number -> member noun -> the server second it expires.
    expiries: BTreeMap<u16, BTreeMap<String, u32>>,
}

impl Cooldowns {
    /// Stamp every member for a group casting.
    ///
    /// Returns how many were stamped. A member **already locked out is
    /// skipped**, not re-stamped — see the module doc.
    pub fn record_group(&mut self, spell: u16, members: &[Member], now: u32) -> usize {
        let Some(seconds) = spells::spell(spell).and_then(|s| s.cooldown(CooldownKind::Group))
        else {
            return 0;
        };
        let expires = now.saturating_add(seconds);
        let held = self.expiries.entry(spell).or_default();
        let mut stamped = 0;
        for member in members {
            // Still locked out: they did not receive this casting, so their
            // own cooldown keeps running rather than being extended by it.
            if held.get(&member.noun).is_some_and(|at| *at > now) {
                continue;
            }
            held.insert(member.noun.clone(), expires);
            stamped += 1;
        }
        stamped
    }

    /// Stamp one character a spell landed on.
    ///
    /// **Not restricted to the group**: the cooldown belongs to the character,
    /// so it counts whoever cast it and whether or not they are grouped.
    pub fn record_target(&mut self, spell: u16, noun: &str, now: u32) -> bool {
        let Some(seconds) = spells::spell(spell).and_then(|s| s.cooldown(CooldownKind::Target))
        else {
            return false;
        };
        self.expiries
            .entry(spell)
            .or_default()
            .insert(noun.to_owned(), now.saturating_add(seconds));
        true
    }

    /// Seconds before a character can receive this spell again.
    ///
    /// **Zero when nothing is recorded**, which is Lich's answer
    /// (`group.rb:235`) and the right one: a character nobody has seen
    /// buffed is castable, and `None` would make every caller handle a case
    /// that means "yes".
    #[must_use]
    pub fn remaining(&self, spell: u16, noun: &str, now: u32) -> u32 {
        self.expiries
            .get(&spell)
            .and_then(|held| held.get(noun))
            .map_or(0, |at| at.saturating_sub(now))
    }

    /// Whether this character can receive the spell now.
    #[must_use]
    pub fn ready(&self, spell: u16, noun: &str, now: u32) -> bool {
        self.remaining(spell, noun, now) == 0
    }

    /// Forget everything.
    ///
    /// Called on a reconnect: these are stamps on **other people**, taken
    /// against a clock this session was keeping, and both assumptions break
    /// while we are away.
    pub fn clear(&mut self) {
        self.expiries.clear();
    }

    /// Whether anything is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.expiries.is_empty()
    }
}

impl GameState {
    /// Which of the current group a spell would actually affect.
    ///
    /// `spell_cooldown_ready` (`group.rb:255`). Empty when the group is
    /// empty or every member is locked out — and **also** when the server
    /// time is unknown, because a cooldown cannot be judged without a clock.
    #[must_use]
    pub fn spell_cooldown_ready(&self, spell: u16) -> Vec<&Member> {
        let Some(now) = self.game_time_now() else {
            return Vec::new();
        };
        self.group
            .members()
            .iter()
            .filter(|member| self.cooldowns.ready(spell, &member.noun, now))
            .collect()
    }

    /// Seconds before a character can receive this spell again.
    ///
    /// `None` when the server time is unknown: §5.2, and distinct from the
    /// `0` that means "ready now".
    #[must_use]
    pub fn spell_cooldown_left(&self, spell: u16, noun: &str) -> Option<u32> {
        Some(self.cooldowns.remaining(spell, noun, self.game_time_now()?))
    }
}
