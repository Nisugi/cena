//! What a hunt is doing, for a frontend's hunt panel (`plan/47` step 8):
//! where it is in its cycle, what it did last, the creature it fights, and
//! why it is waiting -- *"resting: mana 30%, wants 50%"*.
//!
//! The hunt told the player only in lines of text (`Hunt: resting.`), which
//! scroll away; a stuck live run was read by scrolling back. The driver now
//! also reports after every turn ([`Reports`]), the latest replacing the
//! last, and the binary hands that to the window.

use std::fmt;
use std::sync::{Mutex, PoisonError};

use tokio::sync::watch;

use super::said::Said;

/// What a hunt is doing now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// The profile hunted on, by name.
    pub profile: String,
    /// Where the hunt is in its cycle, in words: `hunting`, `resting (out
    /// of mana)`.
    pub phase: String,
    /// What it did this turn, in words: the line it sent, the room it walks
    /// to, how long it waits.
    pub doing: String,
    /// The creature it fights, by what the room calls it, when it has one.
    pub target: Option<String>,
    /// Why it is waiting, with the numbers, when it is: `mana 30%, wants
    /// 50%`.
    pub waiting: Option<String>,
}

/// Where a session's hunts say what they are doing: the latest, replaced
/// each turn; `None` while no hunt runs. One per hunt desk.
#[derive(Debug)]
pub struct Reports {
    latest: watch::Sender<Option<Status>>,
    /// What is running, as the desk named it: the profile, or `heal`.
    running: Mutex<String>,
}

impl Default for Reports {
    fn default() -> Self {
        Self {
            latest: watch::channel(None).0,
            running: Mutex::default(),
        }
    }
}

impl Reports {
    /// Hear each report from now on, starting with the latest.
    #[must_use]
    pub fn follow(&self) -> watch::Receiver<Option<Status>> {
        self.latest.subscribe()
    }

    /// What the reports from now on are of: a profile's name, or the
    /// command that started a run with no profile to name.
    pub(crate) fn running(&self, what: &str) {
        what.clone_into(&mut self.running.lock().unwrap_or_else(PoisonError::into_inner));
    }

    /// Replace the latest report; the profile is what is running.
    pub(crate) fn tell(&self, status: Option<Status>) {
        let status = status.map(|mut status| {
            self.running
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone_into(&mut status.profile);
            status
        });
        self.latest.send_replace(status);
    }
}

/// Why a rest is not over yet, with the numbers it was judged on; `None`
/// is a vital the game has not stated, which keeps the rest going.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unrested {
    /// A `rest.when` condition still holds.
    Wounded,
    /// Encumbrance is not yet under the profile's.
    Encumbered { now: Option<u32>, under: u32 },
    /// The mind is not yet down to the profile's.
    Mind { now: Option<u32>, most: u32 },
    /// Mana is not yet up to the profile's percent.
    Mana { now: Option<u32>, wants: u32 },
    /// Spirit is not yet up to the profile's points.
    Spirit { now: Option<i32>, wants: u32 },
    /// Stamina is not yet up to the profile's percent.
    Stamina { now: Option<u32>, wants: u32 },
}

impl Unrested {
    /// The group board's word for it: what a leader's followers are told
    /// they are waiting on (`hunt/party.rs`).
    pub(super) fn word(self) -> &'static str {
        match self {
            Self::Wounded => "wounded",
            Self::Encumbered { .. } => "encumbrance not yet cleared or known",
            Self::Mind { .. } => "mind still above threshold",
            Self::Mana { .. } => "mana still below threshold",
            Self::Spirit { .. } => "spirit still below threshold",
            Self::Stamina { .. } => "stamina still below threshold",
        }
    }
}

impl fmt::Display for Unrested {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let now = |now: Option<u32>, unit: &str| {
            now.map_or_else(|| "not yet known".to_owned(), |now| format!("{now}{unit}"))
        };
        match *self {
            Self::Wounded => f.write_str("wounded"),
            Self::Encumbered { now: at, under } => {
                write!(f, "encumbrance {}, wants under {under}%", now(at, "%"))
            }
            Self::Mind { now: at, most } => {
                write!(f, "mind {}, wants {most}% or less", now(at, "%"))
            }
            Self::Mana { now: at, wants } => write!(f, "mana {}, wants {wants}%", now(at, "%")),
            Self::Spirit { now: at, wants } => write!(
                f,
                "spirit {}, wants {wants}",
                at.map_or_else(|| "not yet known".to_owned(), |at| at.to_string())
            ),
            Self::Stamina { now: at, wants } => {
                write!(f, "stamina {}, wants {wants}%", now(at, "%"))
            }
        }
    }
}

/// What the driver does with `said`, in words for the panel.
pub(super) fn doing(said: &Said) -> String {
    match said {
        Said::Send { line, .. } => format!("sending: {line}"),
        Said::Walk(room) => format!("walking to room {}", room.0),
        Said::Wait(seconds) => format!("waiting {seconds}s"),
        Said::Done(ending) => format!("ending: {ending}"),
        Said::Loot(corpses) if corpses.is_empty() => "looting the floor".to_owned(),
        Said::Loot(_) => "looting".to_owned(),
        Said::Sell => "selling".to_owned(),
        Said::Heal => "healing".to_owned(),
        Said::Stock(_) => "stocking herbs".to_owned(),
        Said::Errand(errand) => match errand {
            crate::loot::Errand::Room => "looting",
            crate::loot::Errand::Skin => "skinning",
            crate::loot::Errand::Box => "emptying a box",
            crate::loot::Errand::Sell => "selling",
            crate::loot::Errand::Pool { .. } => "at the locksmith pool",
            crate::loot::Errand::Deposit => "at the bank",
        }
        .to_owned(),
        Said::Waggle(people) => format!("casting on {}", people.join(", ")),
        Said::Nothing => "watching".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rest_says_what_it_waits_for_with_the_numbers() {
        assert_eq!(
            Unrested::Mana {
                now: Some(30),
                wants: 50
            }
            .to_string(),
            "mana 30%, wants 50%"
        );
        assert_eq!(
            Unrested::Mind {
                now: None,
                most: 50
            }
            .to_string(),
            "mind not yet known, wants 50% or less"
        );
        assert_eq!(
            Unrested::Spirit {
                now: Some(4),
                wants: 7
            }
            .to_string(),
            "spirit 4, wants 7"
        );
        assert_eq!(
            Unrested::Mana {
                now: Some(30),
                wants: 50
            }
            .word(),
            "mana still below threshold",
            "the group board's words are unchanged"
        );
    }

    #[test]
    fn what_the_driver_does_is_said_in_words() {
        assert_eq!(
            doing(&Said::Send {
                line: "attack #123".to_owned(),
                target: Some(123)
            }),
            "sending: attack #123"
        );
        assert_eq!(doing(&Said::Wait(3)), "waiting 3s");
        assert_eq!(
            doing(&Said::Walk(cena_map::RoomId(4521))),
            "walking to room 4521"
        );
    }

    #[test]
    fn a_follower_hears_the_latest_report() {
        let reports = Reports::default();
        let heard = reports.follow();
        reports.running("ojandhaart");
        reports.tell(Some(Status::default()));
        assert_eq!(
            heard
                .borrow()
                .as_ref()
                .map(|status| status.profile.as_str()),
            Some("ojandhaart")
        );
        reports.tell(None);
        assert!(heard.borrow().is_none(), "no hunt runs");
    }
}
