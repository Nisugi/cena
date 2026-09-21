//! The two ways a crossing sends (`super::steps` has the why): a **move**,
//! which must change the room and has Lich's ladder of remedies behind it,
//! and an **exchange**, which is a command and the game's answer to it.

use cena_map::{Cond, RoomId, Walker};
use cena_session::MoveFeedback;

use super::recovery::{Attempt, First, Reaction};
use super::steps::{Deed, Out, Tick};

/// An unanswered move is sent again after this. Vellum's `STEP_TIMEOUT_MS`.
pub const STEP_TIMEOUT_MS: u64 = 8000;
/// ...this many times. Vellum's `MAX_EDGE_RETRIES`.
pub const MAX_RESENDS: u32 = 2;
/// An exchange the game never answers is taken as answered after this.
pub const EXCHANGE_TIMEOUT_MS: u64 = 3000;
const STUN_POLL_MS: u64 = 500;

/// A command, and the game's answer to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Exchange {
    sent: String,
    sent_at: u64,
    hold_until: u64,
    again: bool,
}

impl Exchange {
    /// `None` once the game has answered.
    pub(super) fn tick(&mut self, tick: &Tick<'_>, room_changed: bool) -> Option<Out> {
        if room_changed {
            return None;
        }
        // Roundtime: the command was refused, not answered. Wait, and send
        // it again, as Lich's `fput` does.
        if let Some(seconds) = tick.feedback.iter().find_map(|feedback| match feedback {
            MoveFeedback::Wait(seconds) => Some(*seconds),
            _ => None,
        }) {
            self.hold_until = tick.ms + wait_ms(seconds);
            self.again = true;
        }
        if tick.ms < self.hold_until {
            return Some(Out::Hold);
        }
        if std::mem::take(&mut self.again) {
            self.sent_at = tick.ms;
            return Some(Out::Send(self.sent.clone()));
        }
        let timed_out = tick.ms.saturating_sub(self.sent_at) >= EXCHANGE_TIMEOUT_MS;
        (!tick.prompted && !timed_out).then_some(Out::Hold)
    }
}

/// Lich sleeps N - 0.2 seconds, or 0.3 for a wait of one.
pub(super) fn wait_ms(seconds: u32) -> u64 {
    if seconds > 1 {
        u64::from(seconds) * 1000 - 200
    } else {
        300
    }
}

/// A move under way, with Lich's remedies and Vellum's rules
/// (`super` module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Mover {
    from: RoomId,
    attempt: Attempt,
    sent_at: u64,
    resends: u32,
    hold_until: u64,
    again: bool,
    unstunned: bool,
    /// Remedies to do before the move is sent again.
    first: Vec<First>,
    /// A remedy emptied the hands; they are given back when the move lands.
    pub(super) hands_emptied: bool,
}

/// How a [`Mover`] ended.
pub(super) enum Moved {
    /// Still under way: do this.
    Not(Out),
    /// The room changed, or the game said it did.
    Landed { believed: bool },
}

impl Mover {
    pub(super) fn new(from: RoomId, command: &str, ms: u64) -> Mover {
        Mover {
            from,
            attempt: Attempt::new(command),
            sent_at: ms,
            resends: 0,
            hold_until: 0,
            again: false,
            unstunned: false,
            first: Vec::new(),
            hands_emptied: false,
        }
    }

    pub(super) fn tick(&mut self, tick: &Tick<'_>) -> Moved {
        // A room change is looked at before anything that was heard.
        if tick.here != self.from {
            return Moved::Landed { believed: false };
        }
        let standing = tick
            .walker
            .posture
            .as_deref()
            .is_none_or(|is| is == "standing");
        let resolve = Cond::SpellKnown("Sigil of Resolve".to_owned()).holds(tick.walker);
        for feedback in tick.feedback {
            match self.attempt.react(*feedback, standing, resolve) {
                Reaction::Again {
                    first,
                    after_ms,
                    unstunned,
                } => {
                    self.first.extend(first);
                    self.hold_until = tick.ms + after_ms;
                    self.again = true;
                    self.unstunned = unstunned;
                }
                Reaction::GiveUp { wrong_for_map } => {
                    return Moved::Not(Out::GiveUp {
                        ban: true,
                        wrong: wrong_for_map,
                    });
                }
                Reaction::Landed => return Moved::Landed { believed: true },
            }
        }
        if !self.first.is_empty() {
            return Moved::Not(match self.first.remove(0) {
                First::Send(command) => Out::Send(command),
                First::EmptyHands => {
                    self.hands_emptied = true;
                    Out::Deed(Deed::EmptyHands)
                }
                First::Cast(spell) => Out::Deed(Deed::Cast(spell)),
            });
        }
        if tick.ms < self.hold_until {
            return Moved::Not(Out::Hold);
        }
        if self.again {
            if self.unstunned && is_stunned(tick.walker) {
                self.hold_until = tick.ms + STUN_POLL_MS;
                return Moved::Not(Out::Hold);
            }
            self.again = false;
            self.sent_at = tick.ms;
            return Moved::Not(Out::Send(self.attempt.sent.clone()));
        }
        if tick.ms.saturating_sub(self.sent_at) < STEP_TIMEOUT_MS {
            return Moved::Not(Out::Hold);
        }
        // Silence. Not a failure: the game may only be slow.
        if self.resends < MAX_RESENDS {
            self.resends += 1;
            self.sent_at = tick.ms;
            return Moved::Not(Out::Send(self.attempt.sent.clone()));
        }
        Moved::Not(Out::GiveUp {
            ban: !tick.left_first_room,
            wrong: false,
        })
    }
}

pub(super) fn is_stunned(walker: &Walker) -> bool {
    Cond::Flag("stunned".to_owned()).holds(walker)
}

pub(super) fn exchange_of(command: &str, ms: u64) -> Exchange {
    Exchange {
        sent: command.to_owned(),
        sent_at: ms,
        hold_until: 0,
        again: false,
    }
}
