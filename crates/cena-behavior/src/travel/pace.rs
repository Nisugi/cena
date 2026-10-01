//! **Batched travel**: plain moves sent ahead of the walker, as many as the
//! account's typeahead lets the game hold, a batch every [`GAP_MS`] (the
//! author's rule, `plan/16` §5.2g, 2026-10-01).
//!
//! go2 is the reference (`go2.lic:2372-2460`): the first move of a trip is
//! never sent ahead; no more moves are unanswered than the typeahead allows;
//! a scripted crossing waits until every move sent has landed; a typeahead
//! refusal lowers the setting and plans again from where the walker is.
//! `VellumFE` left this out on purpose ("go2's most fragile code"), so this
//! keeps to go2's shape and the author's measured numbers:
//!
//! - **A batch is 1 + the typeahead**, the moves the game holds at once: 3
//!   on the author's account (typeahead 2). The typeahead is go2's own
//!   setting, `typeahead`, kept per character in the travel file; with none
//!   set, the author's 2.
//! - **A batch every 150 ms**, the measured limit for the next batch not to
//!   trip typeahead (`plan/16` §5.2f).
//! - **The game's refusal says the number** (*"Sorry, you may only type ahead
//!   2 commands."*): the typeahead becomes it, and is kept. A refusal at that
//!   size already raises the gap 50 ms.
//! - **Only plain moves**: an exit that is one command with nothing in it to
//!   fill. A scripted crossing, a routine, a pass-through or a remedy ends
//!   the batch, and the walker goes on one move at a time from where it is.
//! - **Anything the game says about a move** (a closed door, *you can't go
//!   there*, roundtime) stops sending; once the walker has stopped moving,
//!   the trip plans again from the room it is in, and the move that failed
//!   is met one at a time, with its whole ladder of remedies.

use std::collections::VecDeque;

use cena_map::{Crossing, Map, RoomId, Walker};
use cena_session::MoveFeedback;

use super::mover::STEP_TIMEOUT_MS;
use super::{Said, Trip};

/// The time between batches: the author's measured limit.
pub const GAP_MS: u64 = 150;
/// What the gap grows by when a batch of the right size still trips.
pub const GAP_STEP_MS: u64 = 50;
/// The typeahead with none set: the author's account.
pub const DEFAULT_TYPEAHEAD: u32 = 2;
/// How long the walker must be still, after the game refused a batched move,
/// before the trip plans again from where it is.
pub const SETTLE_MS: u64 = 1000;

/// How the walker's moves go ahead of it: the typeahead the game allows and
/// the time between batches. Kept for the trip; the typeahead learned from a
/// refusal is handed out to keep ([`Pace::learned`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pace {
    typeahead: u32,
    gap_ms: u64,
    learned: Option<u32>,
}

impl Pace {
    pub(super) fn new(typeahead: u32) -> Pace {
        Pace {
            typeahead,
            gap_ms: GAP_MS,
            learned: None,
        }
    }

    /// The most moves unanswered at once: one, and the typeahead's.
    pub(super) fn depth(&self) -> usize {
        1 + usize::try_from(self.typeahead).unwrap_or(0)
    }

    pub(super) fn gap_ms(&self) -> u64 {
        self.gap_ms
    }

    /// The game refused a move as typed too far ahead, with `outstanding`
    /// moves unanswered. `said` is the number the refusal named.
    pub(super) fn refused(&mut self, said: Option<u32>, outstanding: usize) {
        match said {
            Some(allowed) if allowed < self.typeahead => {
                self.typeahead = allowed;
                self.learned = Some(allowed);
            }
            // No number: one fewer, as go2 does.
            None if outstanding > 1 && self.typeahead > 0 => {
                self.typeahead -= 1;
                self.learned = Some(self.typeahead);
            }
            // The right size, and still too fast: wider apart.
            Some(_) | None => self.gap_ms += GAP_STEP_MS,
        }
    }

    /// A typeahead learned since this was last asked, to keep.
    pub(super) fn learned(&mut self) -> Option<u32> {
        self.learned.take()
    }
}

/// The number a typeahead refusal names: *"Sorry, you may only type ahead 1
/// command."*
pub(super) fn typeahead_said(line: &str) -> Option<u32> {
    let rest = line
        .trim()
        .strip_prefix("Sorry, you may only type ahead ")?;
    rest.split_whitespace().next()?.parse().ok()
}

/// Moves sent ahead and not yet landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Batch {
    /// The rooms the moves sent should land in, in order, not yet seen.
    pub(super) expected: VecDeque<RoomId>,
    /// The last room seen on the way.
    pub(super) at: RoomId,
    /// When the walker last moved on the way, or the batch began.
    pub(super) moved_at: u64,
}

/// What a batch under way comes to this tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Ahead {
    /// Moves still landing: wait.
    Landing,
    /// Every move sent has landed: on as usual from here.
    Landed,
    /// Off the way, refused, or stalled: stop, and plan again once still.
    Stop,
}

impl Batch {
    pub(super) fn new(at: RoomId, ms: u64) -> Batch {
        Batch {
            expected: VecDeque::new(),
            at,
            moved_at: ms,
        }
    }

    /// Where the walker is now, at `ms`; `stalled_ms` is how long it may go
    /// without landing anywhere.
    pub(super) fn landed(&mut self, here: RoomId, ms: u64, stalled_ms: u64) -> Ahead {
        if here != self.at {
            let Some(at) = self.expected.iter().position(|room| *room == here) else {
                return Ahead::Stop;
            };
            self.expected.drain(..=at);
            self.at = here;
            self.moved_at = ms;
        }
        if self.expected.is_empty() {
            Ahead::Landed
        } else if ms.saturating_sub(self.moved_at) >= stalled_ms {
            Ahead::Stop
        } else {
            Ahead::Landing
        }
    }
}

/// The typeahead the walker's travel settings name: go2's own `typeahead`,
/// or the author's with none. Off (`0`) while go2's `delay` is set, as go2
/// has it (`go2.lic:2372`).
fn typeahead_of(walker: &Walker) -> u32 {
    let delayed = walker
        .settings
        .get("delay")
        .and_then(|delay| delay.trim().parse::<f64>().ok())
        .is_some_and(|delay| delay > 0.0);
    if delayed {
        return 0;
    }
    walker
        .settings
        .get(super::settings::TYPEAHEAD)
        .and_then(|set| set.trim().parse().ok())
        .unwrap_or(DEFAULT_TYPEAHEAD)
}

impl Trip {
    /// A typeahead learned from the game's refusal since this was last
    /// asked: the driver keeps it as the walker's `typeahead` setting.
    pub fn typeahead_learned(&mut self) -> Option<u32> {
        self.pace.as_mut().and_then(Pace::learned)
    }

    /// Plain moves sent ahead from `here`, if the way ahead has them and the
    /// game will hold them: `None` to go on one move at a time.
    pub(super) fn send_ahead(
        &mut self,
        map: &Map,
        walker: &Walker,
        here: RoomId,
        ms: u64,
    ) -> Option<Said> {
        // Never the trip's first move (go2 1.2), never beside a routine's
        // steps, and only on its feet and its own.
        if !self.left_first_room
            || self.in_aside
            || self.run.is_some()
            || walker.posture.as_deref() != Some("standing")
            || super::steps::is_stunned(walker)
        {
            return None;
        }
        let pace = self
            .pace
            .get_or_insert_with(|| Pace::new(typeahead_of(walker)));
        let (depth, gap) = (pace.depth(), pace.gap_ms());
        let outstanding = self.batch.as_ref().map_or(0, |batch| batch.expected.len());
        let room = depth.saturating_sub(outstanding);
        let mut leaving = self
            .batch
            .as_ref()
            .and_then(|batch| batch.expected.back().copied())
            .unwrap_or(here);
        let mut lines = Vec::new();
        let mut rooms = Vec::new();
        for &next in self.ahead.iter().take(room) {
            let Some(exit) = self.exit_between(map, walker, leaving, next) else {
                break;
            };
            let Crossing::Command(command) = &exit.crossing else {
                break;
            };
            if command.contains('{') {
                break;
            }
            lines.push(command.clone());
            rooms.push(next);
            leaving = next;
        }
        // A batch of one is a move like any other: it goes with its remedies.
        if lines.is_empty() || (self.batch.is_none() && lines.len() < 2) {
            return None;
        }
        self.ahead.drain(..lines.len());
        self.batch
            .get_or_insert_with(|| Batch::new(here, ms))
            .expected
            .extend(rooms);
        Some(Said::SendAll(lines, gap))
    }

    /// A batch under way, at `here` and `ms`, with what the game said since
    /// the last tick. `None`: no batch, or every move landed; on as usual.
    pub(super) fn batch_tick(
        &mut self,
        map: &Map,
        walker: &Walker,
        here: RoomId,
        (feedback, lines): (&[MoveFeedback], &[String]),
        ms: u64,
    ) -> Option<Said> {
        if ms < self.settle_until {
            // Still until the moves the game kept have landed.
            if here != self.settled_at {
                self.settled_at = here;
                self.settle_until = ms + SETTLE_MS;
            }
            return Some(Said::Hold);
        }
        let batch = self.batch.as_mut()?;
        let outstanding = batch.expected.len();
        let refused = !feedback.is_empty();
        let ahead = if refused {
            Ahead::Stop
        } else {
            batch.landed(here, ms, STEP_TIMEOUT_MS)
        };
        match ahead {
            Ahead::Landing => Some(self.send_ahead(map, walker, here, ms).unwrap_or(Said::Hold)),
            Ahead::Landed => {
                self.batch = None;
                None
            }
            Ahead::Stop => {
                if feedback.contains(&MoveFeedback::TypeAhead)
                    && let Some(pace) = self.pace.as_mut()
                {
                    pace.refused(
                        lines.iter().find_map(|line| typeahead_said(line)),
                        outstanding,
                    );
                }
                self.batch = None;
                self.ahead.clear();
                self.settled_at = here;
                self.settle_until = ms + SETTLE_MS;
                Some(Said::Hold)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_names_the_typeahead_and_the_right_size_widens_the_gap() {
        assert_eq!(
            typeahead_said("Sorry, you may only type ahead 1 command."),
            Some(1)
        );
        assert_eq!(
            typeahead_said("Sorry, you may only type ahead 2 commands."),
            Some(2)
        );
        let mut pace = Pace::new(DEFAULT_TYPEAHEAD);
        assert_eq!(pace.depth(), 3);
        pace.refused(Some(1), 3);
        assert_eq!((pace.depth(), pace.gap_ms()), (2, GAP_MS));
        assert_eq!(pace.learned(), Some(1));
        assert_eq!(pace.learned(), None, "handed out once");
        pace.refused(Some(1), 2);
        assert_eq!((pace.depth(), pace.gap_ms()), (2, GAP_MS + GAP_STEP_MS));
    }

    #[test]
    fn a_batch_follows_the_rooms_it_expects_and_stops_off_them() {
        let mut batch = Batch::new(RoomId(1), 0);
        batch.expected.extend([RoomId(2), RoomId(3), RoomId(4)]);
        assert_eq!(batch.landed(RoomId(1), 10, 8000), Ahead::Landing);
        // A room's arrival can be missed; a later one on the way is enough.
        assert_eq!(batch.landed(RoomId(3), 20, 8000), Ahead::Landing);
        assert_eq!(batch.landed(RoomId(4), 30, 8000), Ahead::Landed);

        let mut off = Batch::new(RoomId(1), 0);
        off.expected.extend([RoomId(2), RoomId(3)]);
        assert_eq!(off.landed(RoomId(9), 10, 8000), Ahead::Stop);

        let mut stalled = Batch::new(RoomId(1), 0);
        stalled.expected.push_back(RoomId(2));
        assert_eq!(stalled.landed(RoomId(1), 8000, 8000), Ahead::Stop);
    }
}
