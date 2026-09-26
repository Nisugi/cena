//! Bounty mode (`;hunt <name> bounty`; bigshot's `;bigshot bounty`,
//! `bigshot.lic:3345-3350`): hunt until the bounty is done or a new one can
//! be taken, rest, and end there, leaving the bounty's errand to whoever
//! runs it next (`should_hunt?`, `:8980-8986`).
//!
//! When it is done is bigshot's `bounty_eval` (`set_bounty_eval`,
//! `:3813-3856`, and `bounty_check?`, `:8929-8943`), read off the task the
//! model keeps (`state.bounty`):
//!
//! | The task | Done when | bigshot |
//! |---|---|---|
//! | succeeded, located, failed | the mind is not saturated, or the task is a bandit one | `:3823-3824` |
//! | none assigned | no `Next Bounty` cooldown runs | `:3825` |
//! | the child found | a child is in the room | `:3827-3828` |
//! | anything, while bandits are here on a bandit bounty | never | `:8931` |
//!
//! **Not built:** bigshot's counts of skins and gems in every container
//! (`:3829-3852`), which look in each one. A skin or gem bounty ends the
//! hunt when the guild says the task is done, as the others do.

use cena_session::{GameState, TaskKind, gameobj};

use super::engine::Hunt;

/// Bounty mode, and whether it began on a bandit bounty.
#[derive(Debug, Default)]
pub(super) struct BountyMode {
    /// `;hunt <name> bounty`.
    pub(super) on: bool,
    /// bigshot's `@BANDIT_HUNTING`: the bounty named bandits when first read.
    bandits: Option<bool>,
}

impl Hunt {
    /// This hunt in bounty mode: it ends at the rest once the bounty is done
    /// or a new one can be taken.
    #[must_use]
    pub fn bounty(mut self) -> Self {
        self.bounty_mode.on = true;
        self
    }

    /// Whether the bounty is done or a new one is ready: bigshot's
    /// `bounty_check?`. Never outside bounty mode, nor before the bounty has
    /// been read.
    pub(super) fn bounty_done(&mut self, state: &GameState) -> bool {
        if !self.bounty_mode.on {
            return false;
        }
        let Some(kind) = state.bounty.kind() else {
            return false;
        };
        let bandits = *self.bounty_mode.bandits.get_or_insert(matches!(
            kind,
            TaskKind::Bandit | TaskKind::BanditAssignment
        ));
        // bigshot's `target.type =~ /bandit/`: the object table's type.
        let bandit_here = || {
            self.could_fight(state).any(|creature| {
                creature
                    .noun
                    .as_deref()
                    .is_some_and(|noun| gameobj::classify(noun, &creature.name).is("bandit"))
            })
        };
        if bandits && bandit_here() {
            return false;
        }
        match kind {
            TaskKind::Taskmaster | TaskKind::HeirloomFound | TaskKind::Guard | TaskKind::Failed => {
                bandits
                    || state
                        .character
                        .experience
                        .mind_percent
                        .is_none_or(|mind| mind < 100)
            }
            TaskKind::None => !next_bounty_cooling(state),
            TaskKind::RescueSpawned => state.creatures().in_room().any(|creature| {
                creature
                    .name
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .any(|word| word.eq_ignore_ascii_case("child"))
            }),
            _ => false,
        }
    }
}

/// The guild's `Next Bounty` cooldown is running.
fn next_bounty_cooling(state: &GameState) -> bool {
    let Some(now) = state.game_time_now() else {
        return true;
    };
    state.effects.in_category("Cooldowns").any(|(id, effect)| {
        effect.text.starts_with("Next Bounty") && state.effects.active(id, now) == Some(true)
    })
}
