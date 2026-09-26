//! What the group decides together at the ends of a hunt: whether to rest
//! and how to leave, who preps first at the rest, and whether to hunt again
//! (`plan/39` §0c).
//!
//! The members counted are the leader and every follower still connected
//! ([`State::Ready`]). A follower that is not has a stale report, and is
//! [`Muster`](super::Muster)'s to wait for: eohunter decides on the members
//! whose reports are fresh (`active_names`, `scripts/eohunter/group.rb:571`),
//! and bigshot's `member_online` drops one that stops answering
//! (`bigshot.lic:954-966`). The hunt merge is the exception: a follower
//! not connected is **not ready**, because the author's answer is not to
//! hunt on without a member that has not left the group (`plan/39` §8,
//! question 7).
//!
//! # The solo hunt's reasons, with bigshot's words mapped onto them
//!
//! The group reads each member's own [`Why`], so it speaks the solo hunt's
//! reasons. bigshot's are words, and three of its words are one reason
//! here: its creeping and crushing dread, Wall of Thorns poison and
//! confusion are separate rests (`ready_to_rest?`, `bigshot.lic:9017-9020`),
//! and the solo hunt folds all four into `rest.when`, which rests
//! [`Why::Wounded`] (`crates/cena-behavior/src/hunt/rest.rs`, `wounded`). So
//! a dread rest here leaves at once and waits for a stunned member, where
//! bigshot's loots first. INFERRED from the two sources; to change it, the
//! solo hunt's `Why` would split dread from wounds.

use std::iter;

use cena_session::State;

use super::report::{Hindrance, Leading, Report};
use super::settings::{FriedTrigger, Settings};
use crate::hunt::said::Why;

/// How the group leaves for a rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestCall {
    /// A member is wounded, and a member here cannot move: the group stays
    /// (`bigshot.lic:9065-9067`). How long is [`Muster::Hold`]'s deadline,
    /// for the same member.
    ///
    /// [`Muster::Hold`]: super::Muster::Hold
    Stunned,
    /// Rest for `why`, looting the room once more first when `loot_first`.
    Rest {
        /// The rest's reason: the leader's own, else the first follower's
        /// (eohunter's *"Ours names the rest, else the first follower's"*,
        /// `scripts/eohunter/rest.rb:753-766`).
        why: Why,
        /// A last loot before leaving (`bigshot.lic:9071-9073`): some member
        /// rests for its bounty, a full mind, mana or weight, and nobody for
        /// wounds. Whether the room is the group's to loot is the loot
        /// arm's own check (`bigclaim?`, no ambusher), not this one.
        loot_first: bool,
    },
}

/// Whether the group rests now, and how it leaves: bigshot's
/// `should_rest?` (`bigshot.lic:9046-9077`). `None`: hunt on.
///
/// `dropped` is [`all_dropped`] latched by the caller: every member lost its
/// connection at once since the last rest. Then the group rests first,
/// *"whatever the thresholds say"* (`plan/39` §8, question 9; the author:
/// *"rest first. someone probably died."*).
#[must_use]
pub fn should_rest(
    leader: &Report,
    followers: &[Report],
    dropped: bool,
    settings: &Settings,
) -> Option<RestCall> {
    if dropped {
        return Some(RestCall::Rest {
            why: Why::Dropped,
            loot_first: false,
        });
    }
    let counted: Vec<&Report> = counted(leader, followers).collect();
    let reasons: Vec<(&str, Why)> = counted
        .iter()
        .filter_map(|member| Some((member.name.as_str(), member.rest?)))
        .collect();
    // The leader's reason is first when it has one: it names the rest.
    let (_, why) = reasons.first().copied()?;
    // Fried rests the group only by the trigger (`:9060`): some fried and
    // others not, with nothing else wrong, hunts on.
    if reasons.iter().all(|(_, why)| *why == Why::Fried)
        && !fried_rests(&settings.fried_trigger, &reasons, &counted)
    {
        return None;
    }
    // A wounded rest waits while a member here cannot move: the leader
    // itself, wherever the map puts it, or one in its room
    // (`group_member_stunned?`, `:6717-6730`).
    let wounded = reasons.iter().any(|(_, why)| *why == Why::Wounded);
    let stuck_here = counted.iter().any(|member| {
        member.hindrance == Some(Hindrance::Stuck)
            && (member.name == leader.name || (member.room.is_some() && member.room == leader.room))
    });
    if wounded && stuck_here {
        return Some(RestCall::Stunned);
    }
    let loot_first = !wounded
        && reasons
            .iter()
            .any(|(_, why)| matches!(why, Why::Bounty | Why::Fried | Why::Mana | Why::Encumbered));
    Some(RestCall::Rest { why, loot_first })
}

/// Whether the fried members rest the group, by the trigger: every counted
/// member (bigshot, `:9060`, whose `size` is the followers and the leader,
/// `:991-993`), any, or any named one (eohunter's `fried_rest?`,
/// `scripts/eohunter/group.rb:72-83`).
fn fried_rests(trigger: &FriedTrigger, fried: &[(&str, Why)], counted: &[&Report]) -> bool {
    match trigger {
        FriedTrigger::All => fried.len() == counted.len(),
        FriedTrigger::Any => true,
        FriedTrigger::Names(names) => fried
            .iter()
            .any(|(name, _)| names.iter().any(|named| named == name)),
    }
}

/// The leader, then every follower still connected.
fn counted<'a>(leader: &'a Report, followers: &'a [Report]) -> impl Iterator<Item = &'a Report> {
    iter::once(leader).chain(
        followers
            .iter()
            .filter(|member| member.link == State::Ready),
    )
}

/// Whether every member has lost its connection: the moment `plan/39` §8,
/// question 9 is about. One network under every member takes them all
/// down together (`plan/39` §3). The caller latches it for
/// [`should_rest`]'s `dropped` until the rest begins.
#[must_use]
pub fn all_dropped(leader: &Report, followers: &[Report]) -> bool {
    iter::once(leader)
        .chain(followers)
        .all(|member| member.link != State::Ready)
}

/// Who is not ready to hunt again, and why, the leader first: bigshot's
/// `should_hunt?` (`bigshot.lic:8991-8994`) over each member's
/// `ready_to_hunt?` (`group_should_hunt?`, `:1185-1197`), the reasons the
/// leader shows while it waits (`:7577-7593`). Empty: hunt again.
///
/// A follower whose connection is lost is not ready, whatever it last said
/// (`plan/39` §8, question 7: *"Don't just continue hunting unless they
/// leave the group"*).
#[must_use]
pub fn unready<'a>(leader: &'a Report, followers: &'a [Report]) -> Vec<(&'a str, &'static str)> {
    iter::once(leader)
        .chain(followers)
        .filter_map(|member| {
            let reason = if member.link == State::Ready {
                member.unready?
            } else {
                "its connection is lost"
            };
            Some((member.name.as_str(), reason))
        })
        .collect()
}

/// Who preps first at the resting room (`quiet_followers`,
/// `bigshot.lic:7525-7549`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepOrder {
    /// The leader's resting commands and scripts, then the followers'
    /// (`:7538-7542`): *"Prevents you from looking like you have mindless
    /// bots following while selling loot"* (tooltip, `:2214`).
    LeaderFirst,
    /// Everyone at once (`:7544-7548`).
    Together,
}

impl PrepOrder {
    /// Whether a follower may begin its own prep for the leader's rest now.
    /// Leader first, only once the leader has finished its own prep for
    /// **this** rest ([`Leading::prepared`]).
    #[must_use]
    pub fn follower_may_prep(self, leading: &Leading) -> bool {
        match self {
            Self::Together => true,
            Self::LeaderFirst => leading.prepared == Some(leading.rest),
        }
    }
}

/// The order for this rest: leader first when `quiet_followers` is on and
/// no member rests wounded (`if (@QUIET_FOLLOWERS) && !any_wounded`,
/// `bigshot.lic:7525`, where `any_wounded` is any member's reason,
/// `:7428-7430`); everyone at once otherwise.
#[must_use]
pub fn prep_order(leader: &Report, followers: &[Report], settings: &Settings) -> PrepOrder {
    let wounded = counted(leader, followers).any(|member| member.rest == Some(Why::Wounded));
    if settings.quiet_followers && !wounded {
        PrepOrder::LeaderFirst
    } else {
        PrepOrder::Together
    }
}
