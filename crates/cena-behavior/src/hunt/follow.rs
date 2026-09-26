//! What a step does once it is sent, for bigshot's handlers that wait or
//! read the answer: a **hold**, nothing sent until a line is heard, a time
//! passes or the creature is gone; and an **answer**, the next line chosen
//! by what the game said to the last.
//!
//! | Step | After it is sent | bigshot |
//! |---|---|---|
//! | `wait N` | nothing until the creature swings at the character, N s pass, or it is down or gone | `wait_for_swing`, `:6919-6962` |
//! | `sleep N` | nothing for N s, or until the creature is gone | `cmd_sleep`, `:6529-6538` |
//! | `efury` | nothing until the fury ends, 12 s, or the creature is gone | `cmd_efury`, `:6080-6120` |
//! | `tether` | nothing until the tether completes, breaks or passes on, 12 s, or the creature is gone | `cmd_tether`, `:6630-6693` |
//! | `berserk` | nothing while Berserk (9607) is up, 60 s at most | `cmd_berserk`, `:6495-6506` |
//! | `depress` | `incant 1015` when the song is not being sung | `cmd_depress`, `:4870-4892` |
//! | `unravel` | `stop 1013` once the song resonates or gains mana; `stop 1013` and again when already singing or the tendril wends on, five tries; `release` when there is no target | `cmd_unravel`, `:4915-4957` |
//! | `sacrifice` | `sacrifice #id` when `appraise #id` reads enticingly frail | `cmd_sacrifice`, `:6611-6622` |
//! | `dhurl` | `recover hurl`, again while the weapon is around here somewhere, ten tries | `cmd_dhurl`, `cmd_recover`, `:6280-6354` |
//!
//! Also kept here, from every line heard: whether the character is rooted,
//! for bigshot's `kick` sent as `punch` (`bigshot.lic:2830-2833`, `:3974`).
//! bigshot never clears it but for a snake's coils breaking, and neither
//! does this, within one hunt.
//!
//! An answer reads the reply to a step's **last** line: a spell's `cast`,
//! not its `prepare`.

use std::collections::VecDeque;

use cena_session::{GameState, StatusName};

use super::engine::Hunt;
use super::said::Said;

/// A hold's longest, whatever it waits for (`berserk`).
const HOLD_CAP: u32 = 60;

/// How long a hold on an effect waits before it reads the effect.
const EFFECT_GRACE: u32 = 5;

/// How many times an answer sends its line again.
const UNRAVEL_TRIES: u8 = 5;
const RECOVER_TRIES: u8 = 10;

/// Elemental Fury's ends (`cmd_efury`'s `complete_regex`).
pub(super) const EFURY_ENDS: &[&str] = &[
    "suddenly calms.",
    "causing a brief swelter.",
    "as the ground rumbles.",
    "absorbs the essence of the spell, dissipating it harmlessly.",
];

/// Tether's ends: complete, broken, or passed to another creature
/// (`cmd_tether`).
pub(super) const TETHER_ENDS: &[&str] = &[
    "dissolve into black mist",
    "You struggle to maintain control of the dark force, but you feel it break away!",
    "You feel your connection to the dark presence fade away.",
    "begin to vibrate and emit a sinister thrum that emanates through the surrounding area.",
];

/// What a hold waits for, besides its time.
#[derive(Debug)]
pub(super) enum End {
    /// Only the time.
    Time,
    /// A line naming the creature (by this noun) and the character.
    Swing(String),
    /// A line containing one of these.
    Heard(&'static [&'static str]),
    /// This spell's effect to end.
    Effect(u16),
}

/// Nothing sent for a while after a step.
#[derive(Debug)]
pub(super) struct Hold {
    seconds: u32,
    until: Option<u32>,
    target: i64,
    end: End,
    over: bool,
}

impl Hold {
    /// Hold for `seconds` at most, on creature `target`, until `end`.
    pub(super) fn new(seconds: u32, target: i64, end: End) -> Self {
        Self {
            seconds: seconds.min(HOLD_CAP),
            until: None,
            target,
            end,
            over: false,
        }
    }
}

/// The next line, by the game's answer.
#[derive(Debug)]
pub(super) enum Answer {
    /// `renew 1015`.
    Depress,
    /// Unravel's cast, and the lines to cast it again.
    Unravel { again: VecDeque<String>, tries: u8 },
    /// `appraise #id`.
    Appraised { target: i64 },
    /// `hurl #id <part>`.
    Hurled,
    /// `recover hurl`.
    Recovering { tries: u8 },
    /// `cman dislodge #id <part>`: that part is free once it works.
    Dislodged { part: String },
}

/// What a handler leaves to happen after its lines.
#[derive(Debug)]
pub(super) enum Next {
    /// Hold.
    Hold(Hold),
    /// Read the answer.
    Answer(Answer),
}

/// A step's hold and answer, and whether the character is rooted.
#[derive(Debug, Default)]
pub(super) struct Follow {
    hold: Option<Hold>,
    answer: Option<Answer>,
    /// bigshot's `$bigshot_rooted`.
    pub(super) rooted: bool,
}

impl Hunt {
    /// What happens after the step now going, at game second `now`.
    pub(super) fn follow_with(&mut self, next: Next, now: Option<u32>) {
        match next {
            Next::Hold(mut hold) => {
                hold.until = now.map(|now| now + hold.seconds);
                self.follow.hold = Some(hold);
            }
            Next::Answer(answer) => self.follow.answer = Some(answer),
        }
    }

    /// While a hold runs, a second's wait; `None` once it is over.
    pub(super) fn holding(&mut self, state: &GameState, now: Option<u32>) -> Option<Said> {
        let alone = self.fightable(state).next().is_none();
        let hold = self.follow.hold.as_mut()?;
        let Some(now) = now else {
            self.follow.hold = None;
            return None;
        };
        let until = *hold.until.get_or_insert(now + hold.seconds);
        let creature = state.creatures().get(hold.target);
        let gone = creature.is_none_or(|creature| !creature.valid_target());
        let down = matches!(hold.end, End::Swing(_))
            && creature.is_some_and(|creature| creature.has_status(StatusName::Prone, Some(now)));
        // bigshot pauses five seconds before it watches the effect: it is
        // not listed the moment the command goes.
        let lapsed = match hold.end {
            End::Effect(spell) => {
                now + hold.seconds >= until + EFFECT_GRACE
                    && state.effects.active(&spell.to_string(), now) != Some(true)
            }
            _ => false,
        };
        if hold.over || now >= until || gone || down || lapsed || alone {
            self.follow.hold = None;
            return None;
        }
        Some(Said::Wait(1))
    }

    /// A main-window line: what a hold waits for, and the character rooted.
    pub(super) fn follow_heard(&mut self, line: &str) {
        if line.starts_with("You don't seem to be able to move")
            || line.contains("coils tightly around you, holding you in place!")
        {
            self.follow.rooted = true;
        } else if line.starts_with("You're finally able to break free of")
            && line.contains("coils!")
        {
            self.follow.rooted = false;
        }
        let Some(hold) = self.follow.hold.as_mut() else {
            return;
        };
        let heard = match &hold.end {
            End::Swing(noun) => {
                line.contains(noun.as_str())
                    && [" you.", " you!", " your ", " you "]
                        .iter()
                        .any(|me| line.contains(me))
            }
            End::Heard(ends) => ends.iter().any(|end| line.contains(end)),
            End::Time | End::Effect(_) => false,
        };
        if heard {
            hold.over = true;
        }
    }

    /// The game's answer to the step's last line: the next line, if any.
    pub(super) fn follow_replied(&mut self, lines: &[&str]) {
        if !self.followups.is_empty() {
            return;
        }
        let Some(answer) = self.follow.answer.take() else {
            return;
        };
        let said = |text: &str| lines.iter().any(|line| line.contains(text));
        match answer {
            Answer::Depress => {
                if said("But you are not singing that spellsong.") {
                    self.followups.push_back("incant 1015".to_owned());
                }
            }
            Answer::Unravel { again, tries } => {
                if said("You feel your song resonate around")
                    || said("and begin to resonate, pulling at the threads")
                    || lines.iter().any(|line| gained_mana(line))
                {
                    self.followups.push_back("stop 1013".to_owned());
                } else if said("You are already singing that spellsong.")
                    || said("The silvery tendril continues to wend its way away from the")
                {
                    self.followups.push_back("stop 1013".to_owned());
                    if tries + 1 < UNRAVEL_TRIES {
                        self.followups.extend(again.iter().cloned());
                        self.follow.answer = Some(Answer::Unravel {
                            again,
                            tries: tries + 1,
                        });
                    }
                } else if said("What were you referring to?")
                    || said("A little bit late for that don't you think?")
                {
                    self.followups.push_back("release".to_owned());
                }
            }
            Answer::Appraised { target } => {
                if cena_session::inspect::appraised_frail(lines.iter().copied()) == Some(true) {
                    self.followups.push_back(format!("sacrifice #{target}"));
                }
            }
            Answer::Hurled => {
                let thrown = said("You take aim and throw")
                    || said("You throw")
                    || said("you deftly send")
                    || said("That's not going to do much.  Try using a weapon")
                    || said("You find nothing recoverable");
                if thrown {
                    self.recover(0);
                }
            }
            Answer::Dislodged { part } => {
                if said("You manage to dislodge") || said("You skillfully wrench") {
                    self.aiming
                        .stuck
                        .retain(|stuck| !stuck.eq_ignore_ascii_case(&part));
                }
            }
            Answer::Recovering { tries } => {
                if said("is around here somewhere, but you don't see it.")
                    && tries + 1 < RECOVER_TRIES
                {
                    self.recover(tries + 1);
                }
            }
        }
    }

    fn recover(&mut self, tries: u8) {
        self.followups.push_back("recover hurl".to_owned());
        self.follow.answer = Some(Answer::Recovering { tries });
    }

    /// The link dropped, or the target went: nothing held or awaited.
    pub(super) fn follow_gone(&mut self) {
        self.follow.hold = None;
        self.follow.answer = None;
    }
}

/// `You gain N mana!`.
fn gained_mana(line: &str) -> bool {
    line.strip_prefix("You gain ")
        .and_then(|rest| rest.strip_suffix(" mana!"))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}
