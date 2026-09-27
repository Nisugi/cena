//! Carrying out a dead member (`plan/39` §8, question 10; Stage 7): the
//! author's *"end the hunt, add the dead person to your group (have to empty
//! hands), cast 130 to teleport away after adding them to your group, or
//! drag them if you can't, if you can't drag them probably try to alert the
//! human"*. Every member's hunt ends; one carries the dead out first
//! ([`crate::group::recoverer`]).
//!
//! | Step | Sent | Then |
//! |---|---|---|
//! | 1 | the hands emptied (`travel/hands.rs`, `store_commands`) | |
//! | 2 | `hold <name>`: taking the dead member's hand adds it to the group (Lich's `HOLD_*`, `group.rb:468-505`) | wait for the group to hold it, a few seconds at most |
//! | 3 | `incant 130` (Spirit Guide) when known and affordable | the hunt ends |
//! | 4 | else `drag <name>`, then the walk to the resting room | the hunt ends there |
//! | 5 | else an alert to the player | the hunt ends |
//!
//! UNVERIFIED, for the author's live run: where Spirit Guide goes from a
//! hunting ground and that it takes a held dead member along; `drag`'s
//! conditions and messages. No source here records them (`plan/39` §8).

use cena_map::RoomId;
use cena_session::GameState;

use super::super::engine::Hunt;
use super::super::said::{Ending, Here, Said};
use crate::cast;
use crate::group;
use crate::travel::store_commands;

/// Spirit Guide.
const SPIRIT_GUIDE: u16 = 130;
/// Game seconds the group is given to hold the dead member.
const HOLD_WAIT: u32 = 5;

/// A recovery under way.
#[derive(Clone, Copy, Debug)]
pub(in crate::hunt) struct Recovery {
    step: Step,
    held_at: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Hands,
    Hold,
    Carry,
    Home(RoomId),
    Guided,
}

impl Hunt {
    /// Carry `dead` out, one step a tick; `Done` when it is out, or cannot
    /// be carried.
    pub(in crate::hunt) fn carry_out(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        dead: &str,
        now: Option<u32>,
    ) -> Said {
        let recovery = *self.grouping.recovery.get_or_insert(Recovery {
            step: Step::Hands,
            held_at: None,
        });
        if let Some(line) = self.pending.pop_front() {
            return Said::Send { line, target: None };
        }
        let mut next = recovery;
        let said = match recovery.step {
            Step::Hands => {
                self.notes
                    .push(format!("{dead} is dead: carrying them out."));
                self.pending = store_commands(state)
                    .into_iter()
                    .map(|(_, line)| line)
                    .collect();
                next.step = Step::Hold;
                if let Some(line) = self.pending.pop_front() {
                    Said::Send { line, target: None }
                } else {
                    // Hands already empty: the hand is taken at once.
                    next.held_at = Some(now.unwrap_or(0));
                    Said::Send {
                        line: format!("hold {dead}"),
                        target: None,
                    }
                }
            }
            Step::Hold => {
                let now = now.unwrap_or(0);
                match recovery.held_at {
                    None => {
                        next.held_at = Some(now);
                        Said::Send {
                            line: format!("hold {dead}"),
                            target: None,
                        }
                    }
                    Some(at) if !group::in_group(state, dead) && now < at + HOLD_WAIT => {
                        Said::Wait(1)
                    }
                    Some(_) => {
                        next.step = Step::Carry;
                        Said::Wait(1)
                    }
                }
            }
            Step::Carry => {
                let guide = state.known_spells.knows(u32::from(SPIRIT_GUIDE)) == Some(true)
                    && cast::ready(state, SPIRIT_GUIDE, 1, 0).is_ok();
                if guide {
                    next.step = Step::Guided;
                    Said::Send {
                        line: format!("incant {SPIRIT_GUIDE}"),
                        target: None,
                    }
                } else if let Some(room) = self.profile.rooms.resting {
                    next.step = Step::Home(RoomId(room));
                    Said::Send {
                        line: format!("drag {dead}"),
                        target: None,
                    }
                } else {
                    self.alerts.push(format!(
                        "{dead} is dead here, and there is neither Spirit Guide nor a resting \
                         room to drag them to: they need you."
                    ));
                    self.grouping.recovery = None;
                    return Said::Done(Ending::MemberDied);
                }
            }
            Step::Guided => {
                self.grouping.recovery = None;
                return Said::Done(Ending::MemberDied);
            }
            Step::Home(room) => {
                if here.room == Some(room) {
                    self.grouping.recovery = None;
                    return Said::Done(Ending::MemberDied);
                }
                Said::Walk(room)
            }
        };
        self.grouping.recovery = Some(next);
        said
    }
}
