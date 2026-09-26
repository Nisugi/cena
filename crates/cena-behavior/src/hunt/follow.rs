//! What a step does once it is sent, for bigshot's handlers that wait or
//! read the answer: a **hold**, nothing sent until a line is heard, a time
//! passes or the creature is gone; and an **answer**, the next line chosen
//! by what the game said to the last.
//!
//! | Step | After it is sent | bigshot |
//! |---|---|---|
//! | `wait N` | nothing until the creature swings at the character, N s pass, or it is down or gone | `wait_for_swing`, `:6919-6962` |
//! | `barrage`, `flurry`, `fury`, `gthrusts`, `pummel`, `thrash` | nothing until the assault completes or is refused, 12 s, or the creature is gone | `cmd_assault`, `:4591-4667` |
//! | `bearhug` | nothing until the hold ends, 17 s, or the creature is gone | `cmd_bearhug`, `:5230-5279` |
//! | `sleep N` | nothing for N s, or until the creature is gone | `cmd_sleep`, `:6529-6538` |
//! | `efury` | nothing until the fury ends, 12 s, or the creature is gone | `cmd_efury`, `:6080-6120` |
//! | `tether` | nothing until the tether completes, breaks or passes on, 12 s, or the creature is gone | `cmd_tether`, `:6630-6693` |
//! | `berserk` | nothing while Berserk (9607) is up, 60 s at most | `cmd_berserk`, `:6495-6506` |
//! | `depress` | `incant 1015` when the song is not being sung | `cmd_depress`, `:4870-4892` |
//! | `unravel` | `stop 1013` once the song resonates or gains mana; `stop 1013` and again when already singing or the tendril wends on, five tries; `release` when there is no target | `cmd_unravel`, `:4915-4957` |
//! | `sacrifice` | `sacrifice #id` when `appraise #id` reads enticingly frail | `cmd_sacrifice`, `:6611-6622` |
//! | `dhurl` | `recover hurl`, again while the weapon is around here somewhere, ten tries | `cmd_dhurl`, `cmd_recover`, `:6280-6354` |
//! | `briar` | `raise #id` when `measure #id` reads 100 percent, then the next weapon | `cmd_briar`, `:5650-5673` |
//! | `wandolier` | `rub my <fresh>` when it has no wand to give; the rest for an injury, `reserve list` when the wand is not found | `cmd_wandolier`, `:5980-6021` |
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

/// An assault's ends: complete, or refused (`cmd_assault`'s
/// `complete_regex` and `error_regex`, `bigshot.lic:4595-4618`).
pub(super) const ASSAULT_ENDS: &[&str] = &[
    "Distracted, you hesitate",
    "glides to its inevitable end with one final twirl",
    "You feel a fair amount more durable.",
    "With a final snap of your wrist",
    "You complete your assault",
    "to the ready, your assault complete.",
    "Upon firing your last",
    "With a final, explosive breath",
    "recentering yourself for the fight",
    "You don't seem to be able to move your legs to do that",
    "too injured",
    "already dead",
    "little bit late",
    "could not find",
    "can not be used with attack as the attack type",
    "may not be activated within 60 seconds of a Multi-Strike.",
    "is still in cooldown.",
    "Your mind clouds with confusion",
    "You can't reach",
];

/// A bearhug's ends (`cmd_bearhug`'s `complete_regex`, `:5234-5249`).
pub(super) const BEARHUG_ENDS: &[&str] = &[
    "You release your grip",
    "You feel a fair amount stronger.",
    "avoids your grasp",
    "fend off your grasp",
    "leaving you flailing",
    "Your concentration lapses",
    "You don't seem to be able to move your legs to do that",
    "too injured",
    "already dead",
    "little bit late",
    "could not find",
    "but you stumble and completely miss",
    "is out of reach",
    "You cannot bearhug",
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
    /// `get <wand> from my <fresh>`: rubbed when it has none.
    WandGot { fresh: String },
    /// `wave #id` from the wandolier.
    Waved,
    /// `measure #id`: raised at 100 percent, then the next weapon measured.
    Measured { id: String, rest: VecDeque<String> },
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
    /// `reserve list` has been sent for the wandolier.
    pub(super) reserve_asked: bool,
    /// No Adrenal Surge (1107) before this game second (bigshot's
    /// `$bigshot_adrenal_surge`, 301 s after the last).
    pub(super) adrenal_until: Option<u32>,
    /// This rest's waggle has run (`rest.waggle`).
    pub(super) rest_waggled: bool,
    /// What went last, to send again if the game says `...wait`. Every
    /// line sent is answered (`Hunt::replied`), which takes it.
    pub(super) resend: Option<Resend>,
}

/// What the last line was, to put back on `...wait N seconds.`: bigshot's
/// handlers wait the roundtime out and send again (`cmd_cmans`,
/// `bigshot.lic:5084-5087`, and the others).
#[derive(Debug)]
pub(super) enum Resend {
    /// A line queued after a step's first.
    Line(String),
    /// A routine step's first line.
    Step(super::profile::Step),
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

    /// `...wait N seconds.`: the line that met it goes again, after the
    /// roundtime the driver waits out before every line.
    pub(super) fn resend_replied(&mut self, lines: &[&str]) {
        let waited = lines.iter().any(|line| {
            line.trim()
                .strip_prefix("...wait ")
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        });
        match self.follow.resend.take().filter(|_| waited) {
            Some(Resend::Line(line)) => self.followups.push_front(line),
            Some(Resend::Step(step)) => self.queue.push_front(step),
            None => {}
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
            Answer::Measured { id, mut rest } => {
                let full = lines
                    .iter()
                    .any(|line| cena_session::inspect::measured_percent(line) == Some(100));
                if full {
                    self.followups.push_back(format!("raise #{id}"));
                }
                if let Some(next) = rest.pop_front() {
                    self.followups.push_back(format!("measure #{next}"));
                    self.follow.answer = Some(Answer::Measured { id: next, rest });
                }
            }
            Answer::WandGot { fresh } => {
                if said("Get what?") {
                    self.followups.push_back(format!("rub my {fresh}"));
                }
            }
            Answer::Waved => {
                if said("You are in no condition") {
                    self.must_rest = Some(super::said::Why::Injured);
                } else if said("What were you referring to?") {
                    self.followups.push_back("reserve list".to_owned());
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
