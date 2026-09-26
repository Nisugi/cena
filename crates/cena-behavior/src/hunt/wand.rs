//! Wands: a routine step `wand` waves the profile's wand at the target,
//! bigshot's `cmd_wand` (`bigshot.lic:5935-5973`), and with `wand.if_oom` a
//! spell step the character cannot afford waves one instead (`:5866-5869`).
//!
//! One step a tick. A wand not in hand is got from `wand.fresh` (`get
//! <wand> from my <fresh>`); `Get what?` moves to the next wand on the
//! list, and past the last the hunt ends, where bigshot rests (a rest does
//! not refill the container). In hand, `wave my <wand> at #<id>`. A wave
//! the game answers with none of bigshot's outcomes -- a roll, `You hurl`,
//! already dead, not here, no condition, not found -- is a dead wand, put
//! in `wand.dead` or dropped. `You are in no condition` rests for the
//! injury, as the other injury refusals do (`hunt/replies.rs`).
//!
//! bigshot's `wandolier`, which draws from the wandolier's reserve, is a
//! verb (`hunt/verbs/gated.rs`), sharing the wand list and its matching.

use cena_session::GameState;

use super::engine::Hunt;
use super::said::{Ending, Why};

/// What the last wand line was, to read its reply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Sent {
    #[default]
    Nothing,
    Get,
    Wave,
}

/// Where the wand list stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Wanding {
    /// The wand on the list in use.
    at: usize,
    sent: Sent,
    /// A dead wand to put away or drop, sent at the next wand step.
    dead: Option<String>,
}

impl Hunt {
    /// The line for a `wand` step against `target`. `None`: the profile
    /// names no wand or no fresh container, and the step is skipped.
    pub(super) fn wand_line(&mut self, state: &GameState, target: i64) -> Option<String> {
        let wands = &self.profile.wand;
        let fresh = wands.fresh.as_deref()?;
        if let Some(name) = self.wanding.dead.take() {
            self.wanding.sent = Sent::Nothing;
            return Some(match wands.dead.as_deref() {
                Some(dead) => format!("put my {name} in my {dead}"),
                None => format!("drop my {name}"),
            });
        }
        let Some(name) = wands.names.get(self.wanding.at).cloned() else {
            self.heard.ending = Some(Ending::NoWands);
            return None;
        };
        if held(state, &name) {
            self.wanding.sent = Sent::Wave;
            Some(format!("wave my {name} at #{target}"))
        } else {
            self.wanding.sent = Sent::Get;
            Some(format!("get {name} from my {fresh}"))
        }
    }

    /// Whether a spell step should be a wand instead: `wand.if_oom`, and
    /// the spell unaffordable now.
    pub(super) fn wand_instead(&self, state: &GameState, send: &str) -> bool {
        if !self.profile.wand.if_oom {
            return false;
        }
        let mut words = send.split_whitespace();
        let first = words.next().unwrap_or_default();
        let number = if first.eq_ignore_ascii_case("incant") {
            words.next()
        } else {
            Some(first)
        };
        number
            .and_then(|n| n.parse::<u16>().ok())
            .is_some_and(|n| crate::cast::ready(state, n, 1, 0).is_err())
    }

    /// Read the reply to the last wand line.
    pub(super) fn wand_replied(&mut self, lines: &[&str]) {
        let sent = std::mem::take(&mut self.wanding.sent);
        let has = |needle: &str| lines.iter().any(|line| line.contains(needle));
        match sent {
            Sent::Get if has("Get what?") => {
                self.wanding.at += 1;
            }
            Sent::Nothing | Sent::Get => {}
            Sent::Wave if has("You are in no condition") => {
                self.must_rest = Some(Why::Injured);
            }
            Sent::Wave => {
                let outcome = has("d100")
                    || has("You hurl")
                    || has("is already dead")
                    || has("You do not see that here")
                    || has("I could not find");
                if !outcome {
                    self.wanding.dead = self.profile.wand.names.get(self.wanding.at).cloned();
                    self.notes.push("the wand is spent.".to_owned());
                }
            }
        }
    }
}

impl Wanding {
    /// The wand on the list in use.
    pub(super) const fn at(&self) -> usize {
        self.at
    }
}

/// Whether a hand holds the wand ([`named_like`]).
fn held(state: &GameState, name: &str) -> bool {
    [&state.right_hand, &state.left_hand]
        .iter()
        .any(|hand| hand.name().is_some_and(|text| named_like(name, text)))
}

/// Whether `text` names the wand `name`: every word of it, in order
/// (bigshot's `split(' ').join('.*?')`, `:5939`).
pub(super) fn named_like(name: &str, text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    let mut from = 0;
    name.to_ascii_lowercase().split_whitespace().all(|word| {
        text.get(from..)
            .and_then(|rest| rest.find(word))
            .is_some_and(|at| {
                from += at + word.len();
                true
            })
    })
}
