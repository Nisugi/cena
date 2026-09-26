//! bigshot's two switches for trouble and death (`dead_man_switch`,
//! `bigshot.lic:6755-6790`).
//!
//! - **The dead man's switch**, on the Shattered instance only (bigshot's
//!   `XMLData.game =~ /GSF/`): dead, or below 40 percent health, and the
//!   character quits the game. Here the hunt ends with
//!   [`Ending::Trouble`] and the driver quits through the session, which
//!   saves before the connection closes.
//! - **The depart switch**: dead, the character departs (`depart` twice,
//!   `depart confirm` twice), waggles itself when it has a waggle profile
//!   (bigshot starts `ewaggle`), sends `info` once a minute for fifteen
//!   minutes, waits for full spirit, and hunts again (bigshot restarts
//!   itself `solo`): here the walk back to the hunting room.
//!
//! Without either switch, death ends the hunt as it always has.

use std::collections::VecDeque;

use cena_session::GameState;

use super::engine::Hunt;
use super::said::{Ending, Phase, Said};

/// The instance bigshot's dead man's switch runs on (`XMLData.game`
/// `GSF`, which the character's instance names so).
const SHATTERED: &str = "Shattered";
/// Health below which the dead man's switch quits.
const TROUBLE_HEALTH: u32 = 40;
/// Seconds bigshot waits after departing, `info` each minute of it.
const MOURNING: u32 = 15 * 60;
/// Seconds between the `info` lines.
const INFO_EVERY: u32 = 60;

/// Where recovering from a death stands.
#[derive(Debug, Default)]
pub(super) enum Mourning {
    /// Alive, or dead without the depart switch.
    #[default]
    None,
    /// Departing: these lines still to send.
    Departing(VecDeque<String>),
    /// Departed: the waggle is asked for once, then the wait begins.
    Waggling,
    /// Waiting out the time bigshot waits, with `info` each minute.
    Waiting {
        /// Not before this game second.
        until: u32,
        /// The next `info` at this game second.
        info_at: u32,
    },
}

impl Hunt {
    /// The switches, ahead of everything else each tick; `None` when
    /// neither has anything to do.
    pub(super) fn death(&mut self, state: &GameState, now: Option<u32>) -> Option<Said> {
        let dead = state.status.known().dead() == Some(true);
        if self.profile.react.dead_man_switch
            && state.character.instance.as_deref() == Some(SHATTERED)
        {
            let low = state
                .health()
                .is_some_and(|health| health.percent < TROUBLE_HEALTH);
            if dead || low {
                return Some(Said::Done(Ending::Trouble));
            }
        }
        if !self.profile.react.depart_switch {
            return None;
        }
        match &mut self.mourning {
            Mourning::None if dead => {
                self.notes
                    .push("dead: departing, as depart_switch asks.".to_owned());
                let mut lines: VecDeque<String> =
                    ["depart", "depart", "depart confirm", "depart confirm"]
                        .into_iter()
                        .map(str::to_owned)
                        .collect();
                let first = lines.pop_front()?;
                self.mourning = Mourning::Departing(lines);
                Some(send(first))
            }
            Mourning::None => None,
            Mourning::Departing(lines) => {
                if let Some(line) = lines.pop_front() {
                    Some(send(line))
                } else {
                    self.mourning = Mourning::Waggling;
                    Some(Said::Wait(1))
                }
            }
            Mourning::Waggling => {
                let now = now?;
                self.mourning = Mourning::Waiting {
                    until: now + MOURNING,
                    info_at: now + INFO_EVERY,
                };
                Some(if self.waggle_profile.is_some() {
                    Said::Waggle(Vec::new())
                } else {
                    Said::Wait(1)
                })
            }
            Mourning::Waiting { until, info_at } => {
                let now = now?;
                if now >= *info_at && now < *until {
                    *info_at = now + INFO_EVERY;
                    return Some(send("info".to_owned()));
                }
                let whole = state.spirit().is_some_and(|spirit| spirit.percent >= 100);
                if now < *until || !whole {
                    return Some(Said::Wait(5));
                }
                self.mourning = Mourning::None;
                self.notes
                    .push("recovered from the death: back to the hunt.".to_owned());
                self.phase = Phase::Returning;
                None
            }
        }
    }
}

/// A line with no target.
fn send(line: String) -> Said {
    Said::Send { line, target: None }
}
