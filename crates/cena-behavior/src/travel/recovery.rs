//! What to do when a move does not simply work (`plan/24` stage 2).
//!
//! The line is named by `cena_model::movement`, which is Lich's ladder. This
//! is the other half of `move.rb`: the **remedy** for each, and its budget.
//! Both are Lich's too -- `MAX_REMEDIES = 3` for a fix that either works or
//! does not (`stand`, `unhide`, `open`), `MAX_ROLLS = 20` for a skill roll
//! that may simply need another throw -- because an unbounded remedy is how
//! Lich's own stand loop used to run for ever, and how Vellum's uncapped
//! `open` became "the only outright hang in the walker" (`plan/21` §4.0).
//!
//! Pure: a line and what has been tried, in; what to do, out.

use std::collections::HashMap;

use cena_session::MoveFeedback;

/// Tries for a fix that works or does not. `move.rb:50`.
pub const MAX_REMEDIES: u32 = 3;
/// Tries for a skill roll. `move.rb:55`.
pub const MAX_ROLLS: u32 = 20;

const ORDINALS: [&str; 12] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eight", "ninth", "tenth",
    "eleventh", "twelfth",
];

/// One exit being tried: what is being sent, and what has been tried on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Attempt {
    /// What is sent, and **re-sent**: never the map's text again, because a
    /// remedy may have rewritten it (`go` to `climb`, `door` to `second
    /// door`), and resending the original would undo the fix.
    pub sent: String,
    tried: HashMap<Remedy, u32>,
    opened: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Remedy {
    Unhide,
    Door,
    Roll,
    Verb,
    Stand,
    Recover,
    TypeAhead,
    Disk,
    Feet,
    Trap,
    Held,
}

/// What the trip should do about a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Reaction {
    /// Send `first` if there is one, wait `after_ms`, then send the move
    /// again -- once the walker is no longer stunned, if `unstunned`.
    Again {
        first: Option<String>,
        after_ms: u64,
        unstunned: bool,
    },
    /// Stop trying this exit on this trip. `wrong_for_map`: the game says
    /// there is no such way, which is a fact about the map; otherwise the
    /// exit is real and only this walker, now, was refused.
    GiveUp { wrong_for_map: bool },
    /// The game says the move worked, though no room change will say so.
    Landed,
}

impl Attempt {
    pub fn new(command: &str) -> Attempt {
        Attempt {
            sent: command.to_owned(),
            tried: HashMap::new(),
            opened: false,
        }
    }

    /// `true` while this remedy still has tries left, and counts one.
    fn may(&mut self, remedy: Remedy, budget: u32) -> bool {
        let tried = self.tried.entry(remedy).or_insert(0);
        *tried += 1;
        *tried <= budget
    }

    fn again(
        &mut self,
        remedy: Remedy,
        budget: u32,
        first: Option<&str>,
        after_ms: u64,
    ) -> Reaction {
        if self.may(remedy, budget) {
            Reaction::Again {
                first: first.map(str::to_owned),
                after_ms,
                unstunned: false,
            }
        } else {
            // Lich: `finish.call(nil, cause)` -- the obstacle is about the
            // character, not the map, so the exit is kept.
            Reaction::GiveUp {
                wrong_for_map: false,
            }
        }
    }

    /// Lich's remedy for `feedback`, given what has been tried already.
    /// `standing` is whether the walker is known to be on its feet.
    pub fn react(&mut self, feedback: MoveFeedback, standing: bool) -> Reaction {
        let stand = (!standing).then_some("stand");
        match feedback {
            MoveFeedback::MustUnhide => self.again(Remedy::Unhide, MAX_REMEDIES, Some("unhide"), 0),
            MoveFeedback::WrongDoor => {
                let budget = u32::try_from(ORDINALS.len()).unwrap_or(MAX_REMEDIES);
                if self.may(Remedy::Door, budget) && self.next_door() {
                    Reaction::Again {
                        first: None,
                        after_ms: 0,
                        unstunned: false,
                    }
                } else {
                    Reaction::GiveUp {
                        wrong_for_map: true,
                    }
                }
            }
            MoveFeedback::NoSuchWay => Reaction::GiveUp {
                wrong_for_map: true,
            },
            // Refused, injured past climbing, or dragging someone who will
            // not come: real exits, closed to this walker for now. (Lich
            // casts Sigil of Resolve for the injured; casting is stage 3's.)
            MoveFeedback::Denied | MoveFeedback::TooInjured | MoveFeedback::CannotDrag => {
                Reaction::GiveUp {
                    wrong_for_map: false,
                }
            }
            MoveFeedback::FailedRoll { fell } => {
                let first = if fell { stand } else { None };
                self.again(Remedy::Roll, MAX_ROLLS, first, 1000)
            }
            // Lich empties the hands here as well; hands are stage 3's, and
            // until then the climb is simply tried again.
            MoveFeedback::FailedRollHandsFull => self.again(Remedy::Roll, MAX_ROLLS, stand, 500),
            // The hands must be emptied and given back, which is stage 3's
            // (`plan/21` §4.5). Until then the exit is left for this trip.
            MoveFeedback::HandsFull => Reaction::GiveUp {
                wrong_for_map: false,
            },
            MoveFeedback::Swam | MoveFeedback::PitchDark => Reaction::Landed,
            MoveFeedback::NeedsClimb => self.swap_verb("go", "climb"),
            MoveFeedback::NeedsGo => self.swap_verb("climb", "go"),
            MoveFeedback::Closed => {
                if std::mem::replace(&mut self.opened, true) {
                    // Opened once already and still shut: locked.
                    return Reaction::GiveUp {
                        wrong_for_map: true,
                    };
                }
                let open = self
                    .sent
                    .replacen("go", "open", 1)
                    .replacen("climb", "open", 1);
                Reaction::Again {
                    first: Some(open),
                    after_ms: 0,
                    unstunned: false,
                }
            }
            // Roundtime always ends, so waiting is not a remedy that can
            // fail and is not counted. Lich sleeps N - 0.2, or 0.3 for one.
            MoveFeedback::Wait(seconds) => Reaction::Again {
                first: None,
                after_ms: if seconds > 1 {
                    u64::from(seconds) * 1000 - 200
                } else {
                    300
                },
                unstunned: false,
            },
            MoveFeedback::MustStand => self.again(Remedy::Stand, MAX_REMEDIES, Some("stand"), 0),
            MoveFeedback::StillRecovering => self.again(Remedy::Recover, MAX_ROLLS, None, 2000),
            MoveFeedback::TypeAhead => self.again(Remedy::TypeAhead, MAX_ROLLS, None, 1000),
            MoveFeedback::Stunned => Reaction::Again {
                first: None,
                after_ms: 0,
                unstunned: true,
            },
            MoveFeedback::DiskWobbled => self.again(Remedy::Disk, MAX_REMEDIES, None, 0),
            MoveFeedback::ItemAtFeet => {
                self.again(Remedy::Feet, MAX_REMEDIES, Some("stow feet"), 1000)
            }
            MoveFeedback::Trapped => {
                if self.may(Remedy::Trap, MAX_REMEDIES) {
                    Reaction::Again {
                        first: stand.map(str::to_owned),
                        after_ms: 500,
                        unstunned: true,
                    }
                } else {
                    Reaction::GiveUp {
                        wrong_for_map: false,
                    }
                }
            }
            // Lich polls three seconds for "You regain control".
            MoveFeedback::Held => self.again(Remedy::Held, MAX_REMEDIES, None, 3000),
        }
    }

    fn swap_verb(&mut self, from: &str, to: &str) -> Reaction {
        if self.may(Remedy::Verb, MAX_REMEDIES) {
            self.sent = self.sent.replace(from, to);
            Reaction::Again {
                first: None,
                after_ms: 0,
                unstunned: false,
            }
        } else {
            Reaction::GiveUp {
                wrong_for_map: true,
            }
        }
    }

    /// `go door` -> `go second door` -> `go third door`. `false` when there
    /// is no door in the command, or no ordinal after the last.
    fn next_door(&mut self) -> bool {
        let words: Vec<&str> = self.sent.split(' ').collect();
        let at = words.iter().position(|word| ORDINALS.contains(word));
        let next = match at {
            Some(at) => {
                let Some(which) = ORDINALS
                    .iter()
                    .position(|ordinal| words.get(at) == Some(ordinal))
                else {
                    return false;
                };
                let Some(ordinal) = ORDINALS.get(which + 1) else {
                    return false;
                };
                let mut words = words;
                if let Some(word) = words.get_mut(at) {
                    *word = ordinal;
                }
                words.join(" ")
            }
            None if self.sent.contains("door") => self.sent.replacen("door", "second door", 1),
            None => return false,
        };
        self.sent = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn again(first: Option<&str>, after_ms: u64) -> Reaction {
        Reaction::Again {
            first: first.map(str::to_owned),
            after_ms,
            unstunned: false,
        }
    }

    #[test]
    fn a_closed_door_is_opened_once_and_then_called_locked() {
        let mut attempt = Attempt::new("go gate");
        assert_eq!(
            attempt.react(MoveFeedback::Closed, true),
            again(Some("open gate"), 0)
        );
        assert_eq!(
            attempt.react(MoveFeedback::Closed, true),
            Reaction::GiveUp {
                wrong_for_map: true
            }
        );
    }

    #[test]
    fn a_rewritten_command_is_what_gets_sent_again() {
        let mut attempt = Attempt::new("go wall");
        assert_eq!(
            attempt.react(MoveFeedback::NeedsClimb, true),
            again(None, 0)
        );
        assert_eq!(attempt.sent, "climb wall");

        let mut doors = Attempt::new("go door");
        doors.react(MoveFeedback::WrongDoor, true);
        assert_eq!(doors.sent, "go second door");
        doors.react(MoveFeedback::WrongDoor, true);
        assert_eq!(doors.sent, "go third door");
    }

    #[test]
    fn a_remedy_that_does_not_help_is_not_tried_for_ever() {
        let mut attempt = Attempt::new("north");
        for _ in 0..MAX_REMEDIES {
            assert_eq!(
                attempt.react(MoveFeedback::MustStand, false),
                again(Some("stand"), 0)
            );
        }
        // The exit is real; it is the walker that cannot stand.
        assert_eq!(
            attempt.react(MoveFeedback::MustStand, false),
            Reaction::GiveUp {
                wrong_for_map: false
            }
        );
    }

    #[test]
    fn roundtime_is_waited_out_and_never_counted() {
        let mut attempt = Attempt::new("north");
        for _ in 0..100 {
            assert_eq!(
                attempt.react(MoveFeedback::Wait(3), true),
                again(None, 2800)
            );
        }
        assert_eq!(attempt.react(MoveFeedback::Wait(1), true), again(None, 300));
    }

    #[test]
    fn a_fall_stands_up_only_when_the_walker_is_down() {
        let mut attempt = Attempt::new("climb wall");
        let fell = MoveFeedback::FailedRoll { fell: true };
        assert_eq!(attempt.react(fell, false), again(Some("stand"), 1000));
        assert_eq!(attempt.react(fell, true), again(None, 1000));
    }

    #[test]
    fn what_the_game_calls_no_way_is_a_fact_about_the_map() {
        let mut attempt = Attempt::new("north");
        assert_eq!(
            attempt.react(MoveFeedback::NoSuchWay, true),
            Reaction::GiveUp {
                wrong_for_map: true
            }
        );
        assert_eq!(
            attempt.react(MoveFeedback::Denied, true),
            Reaction::GiveUp {
                wrong_for_map: false
            }
        );
        assert_eq!(
            attempt.react(MoveFeedback::PitchDark, true),
            Reaction::Landed
        );
    }
}
