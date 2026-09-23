//! Crossing one exit: a flat list of guarded steps, run one at a time
//! (`plan/24` stage 3, `plan/21` §4.8).
//!
//! A plain exit is the one-step list `[move "north"]`, so there is a single
//! way across and move recovery (`super::recovery`) lives inside it.
//!
//! # Two ways of sending
//!
//! - A **move** ([`Mover`]) must change the room, and has Lich's whole
//!   ladder of remedies behind it.
//! - An **exchange** ([`Exchange`]) is a command and the game's answer to it:
//!   `open door`, one `search` of many. It is answered by the next prompt or
//!   by a room change, waits out a `...wait N`, and is otherwise not second
//!   guessed. Every loop in the vocabulary -- *until the room changes*,
//!   *while there is an exit east*, *until the game says so* -- is exchanges
//!   and a question between them.
//!
//! # A guard is asked when its step is reached
//!
//! Not when the walk is planned (`cena_map::Step`): `go north` *when Water
//! Walking is up* must see what the `cast` before it did. The trip lends the
//! walker two facts only it knows -- the room, and whether this crossing has
//! moved it yet -- and asks again at every step.
//!
//! # What is left to whoever drives
//!
//! A [`Deed`] is something the trip cannot spell as a command without facts
//! it should not hold: what is in each hand and where it stows, how this
//! character casts a spell, where memories are kept. The driver does the
//! deed, then ticks again.

use cena_map::{Action, Cond, RoomId, Step, Walker};
use cena_session::MoveFeedback;

pub(super) use super::mover::is_stunned;
pub use super::mover::{EXCHANGE_TIMEOUT_MS, MAX_RESENDS, STEP_TIMEOUT_MS};
use super::mover::{Exchange, Moved, Mover, exchange_of};
use super::replies;

/// Turns of any loop. Upstream's own bound where it has one is fifty.
pub const MAX_TURNS: u32 = 50;
/// How long a step will wait for a line, an arrival or a fact. A ship's
/// voyage is the longest thing waited for; this is a stop, not an estimate.
pub const MAX_WAIT_MS: u64 = 30 * 60 * 1000;

/// How long a spell cast to carry the walker ([`Deed::CastAt`]) is given to
/// do it, from the moment the cast is done. A move's own
/// [`STEP_TIMEOUT_MS`]: Phase moves the caster as part of the cast's answer.
///
/// It used to be the generic arrival wait, bounded only by [`MAX_WAIT_MS`] --
/// thirty minutes, sized for a ship's voyage -- so a cast that fizzled, or
/// was never made for want of mana, stood the walker in place for half an
/// hour before the exit was given up (review, 2026-09-23).
pub const CARRIED_WITHIN_MS: u64 = STEP_TIMEOUT_MS;

/// Something the driver must do for the trip, to completion, before it ticks
/// again. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deed {
    /// Put away what is in the hands, remembering what went where.
    EmptyHands,
    /// Take back what [`Deed::EmptyHands`] put away, last first.
    FillHands,
    /// Cast this, by name, and wait for it to land.
    Cast(String),
    /// Cast `.0` at `.1`, waiting for the mana and casting again if hindered.
    CastAt(String, String),
    /// Take this stance, remembering the one held.
    Stance(String),
    /// Go back to the stance [`Deed::Stance`] replaced.
    RestoreStance,
    /// Write a memory down, in the character's travel file.
    Remember(String, String),
    /// Strike one out.
    Forget(String),
    /// Wait for whoever follows the walker to arrive.
    AwaitFollowers,
    /// Speak this language, remembering the one spoken.
    Speak(String),
    /// Go back to the language [`Deed::Speak`] replaced.
    RestoreSpeech,
    /// `get my <this>`, remembering what came out of where. A walker with
    /// none says so with [`super::Trip::could_not`].
    TakeOut(String),
    /// Put what [`Deed::TakeOut`] took back where it came from.
    PutBack,
}

/// What one tick of a crossing comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Out {
    Send(String),
    Deed(Deed),
    Hold,
    /// Every step is done.
    Done,
    /// A step asked for the walk to be planned again from here.
    Replan,
    /// This exit cannot be crossed on this trip. `ban`: never offer it again.
    /// `wrong`: the game said there is no such way.
    GiveUp {
        ban: bool,
        wrong: bool,
    },
    /// The game said a move landed where no room change could show it.
    Believed(RoomId),
}

/// What the trip knows this tick, lent to the crossing.
pub(super) struct Tick<'a> {
    pub walker: &'a Walker,
    pub here: RoomId,
    pub ms: u64,
    /// Lines heard since the last tick, named by Lich's ladder.
    pub feedback: &'a [MoveFeedback],
    /// The same lines, as text.
    pub lines: &'a [String],
    /// The game has prompted since the last tick.
    pub prompted: bool,
    /// The trip has left the room it began in: see [`Mover`]'s ban rule.
    pub left_first_room: bool,
    /// A number from the trip's seeded generator.
    pub random: u64,
    /// The driver could not do the deed it was last handed.
    pub could_not: bool,
}

/// When a loop of exchanges stops.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Until {
    /// The walker is no longer in the room the loop began in.
    RoomChanges,
    /// The walker is at the exit's destination.
    There,
    /// The question no longer holds.
    NoLonger(Cond),
    /// The game has said one of these, in answer to the round's last command.
    Said(Vec<String>),
}

/// Which command a turn of a loop sends.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pick {
    /// Each in order, round and round.
    InTurn(Vec<String>),
    /// One of these, at random.
    AnyOf(Vec<String>),
    /// An obvious exit at random -- not the way just come, if there is another.
    AnyExit,
}

/// A loop of exchanges, part-way through.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Looping {
    pick: Pick,
    until: Until,
    began_in: RoomId,
    turns: u32,
    /// Upstream's own bound, where it has one.
    tries: Option<u32>,
    exchange: Option<Exchange>,
    last_way: Option<String>,
}

/// What a step is doing, when it takes more than one tick.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Job {
    Moving(Mover),
    /// A move that may well not work: answered, not insisted on.
    Trying(Exchange),
    /// Commands still to exchange, in order, then the step is done.
    Round(Vec<String>, Option<Exchange>),
    /// Moves still to make, in order.
    Moves(Vec<String>, Option<Mover>),
    Loop(Looping),
    /// Nothing sent: waiting on the clock, a line, the room, or a fact.
    Pause(u64),
    Line(Vec<String>, u64),
    Arrival(RoomId, u64),
    /// A spell cast to carry the walker ([`Deed::CastAt`]): the room it was
    /// cast in, and when the cast was done -- `None` until the tick after the
    /// deed, because the deed's own mana wait is not the carrying's.
    Carried(RoomId, Option<u64>),
    Fact(Cond, u64),
    /// A deed was handed to the driver; the step is done when it ticks again.
    Deeded,
    /// `look trail`, sent: listening for the word that follows `.1`.
    Asking(Exchange, String),
    /// `inquire`, sent: listening for the numbered line that names `.1`.
    Inquiring(Exchange, String),
}

/// One exit being crossed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Run {
    pub from: RoomId,
    /// The room the exit leaves in the map: `from`, unless the path passed
    /// through a room only the map has. It is what a ban names.
    pub leaving: RoomId,
    pub expected: RoomId,
    steps: Vec<Step>,
    at: usize,
    job: Option<Job>,
    /// The crossing has moved the walker: `Cond::StillHere` is now false.
    moved: bool,
    /// Owed back when the crossing ends, however it ends.
    pub owes: Owes,
    /// The word an [`Action::Ask`] was given, for `{told}`.
    told: Option<String>,
}

/// What a crossing has changed and not yet put back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // four things owed, each its own yes or no
pub(super) struct Owes {
    pub hands: bool,
    pub stance: bool,
    pub speech: bool,
    /// Something is out of its container ([`Deed::TakeOut`]).
    pub taken: bool,
}

impl Run {
    pub fn new(from: RoomId, leaving: RoomId, expected: RoomId, steps: Vec<Step>) -> Run {
        Run {
            from,
            leaving,
            expected,
            steps,
            at: 0,
            job: None,
            moved: false,
            owes: Owes::default(),
            told: None,
        }
    }

    pub fn tick(&mut self, tick: &Tick<'_>) -> Out {
        if tick.here != self.from {
            self.moved = true;
        }
        // The two facts only the trip knows, lent for the guards.
        let mut walker = tick.walker.clone();
        walker.room = Some(tick.here.0);
        walker.still_here = Some(!self.moved);
        loop {
            if let Some(job) = self.job.take() {
                match self.work(job, tick, &walker) {
                    Some(out) => return out,
                    None => self.at += 1,
                }
            }
            let Some(step) = self.steps.get(self.at).cloned() else {
                return Out::Done;
            };
            if step.when.as_ref().is_some_and(|when| !when.holds(&walker)) {
                self.at += 1;
                continue;
            }
            match self.begin(step.action, tick, &walker) {
                Begun::Out(out) => return out,
                Begun::Skip => self.at += 1,
            }
        }
    }

    /// Carry a job on. `None` when it has finished.
    fn work(&mut self, job: Job, tick: &Tick<'_>, walker: &Walker) -> Option<Out> {
        match job {
            Job::Moving(mover) => self.moving(mover, tick),
            Job::Trying(mut exchange) => {
                let out = exchange.tick(tick, tick.here != self.from)?;
                self.job = Some(Job::Trying(exchange));
                Some(out)
            }
            Job::Round(mut rest, exchange) => {
                if let Some(mut exchange) = exchange
                    && let Some(out) = exchange.tick(tick, false)
                {
                    self.job = Some(Job::Round(rest, Some(exchange)));
                    return Some(out);
                }
                if rest.is_empty() {
                    return None;
                }
                let command = rest.remove(0);
                self.job = Some(Job::Round(rest, Some(exchange_of(&command, tick.ms))));
                Some(Out::Send(command))
            }
            Job::Moves(mut rest, mover) => {
                if let Some(mut mover) = mover {
                    match mover.tick(tick) {
                        Moved::Not(out) => {
                            self.job = Some(Job::Moves(rest, Some(mover)));
                            return Some(out);
                        }
                        Moved::Landed { .. } => self.moved = true,
                    }
                }
                if rest.is_empty() {
                    return None;
                }
                let command = rest.remove(0);
                let mover = Mover::new(tick.here, &command, tick.ms);
                self.job = Some(Job::Moves(rest, Some(mover)));
                Some(Out::Send(command))
            }
            Job::Loop(looped) => self.turn(looped, tick, walker),
            Job::Pause(until) => (tick.ms < until).then(|| {
                self.job = Some(Job::Pause(until));
                Out::Hold
            }),
            Job::Line(lines, since) => (!said(tick.lines, &lines))
                .then(|| self.wait(Job::Line(lines, since), since, tick.ms)),
            Job::Arrival(began_in, since) => (tick.here == began_in && tick.here != self.expected)
                .then(|| self.wait(Job::Arrival(began_in, since), since, tick.ms)),
            Job::Carried(began_in, since) => {
                // The driver could not cast it: no mana in ten minutes, or
                // hindered every time. Nothing is coming to carry the walker.
                if tick.could_not {
                    return Some(Out::GiveUp {
                        ban: true,
                        wrong: false,
                    });
                }
                if tick.here != began_in || tick.here == self.expected {
                    return None;
                }
                let since = since.unwrap_or(tick.ms);
                if tick.ms.saturating_sub(since) >= CARRIED_WITHIN_MS {
                    return Some(Out::GiveUp {
                        ban: true,
                        wrong: false,
                    });
                }
                self.job = Some(Job::Carried(began_in, Some(since)));
                Some(Out::Hold)
            }
            Job::Fact(cond, since) => {
                (!cond.holds(walker)).then(|| self.wait(Job::Fact(cond, since), since, tick.ms))
            }
            // A deed the driver could not do: a walker with no key. The
            // crossing fails here, before anything is unlocked.
            Job::Deeded => tick.could_not.then_some(Out::GiveUp {
                ban: true,
                wrong: false,
            }),
            Job::Asking(mut exchange, after) => {
                if let Some(word) = replies::word_after(tick.lines, &after) {
                    self.told = Some(word);
                    return None;
                }
                let out = exchange.tick(tick, false)?;
                self.job = Some(Job::Asking(exchange, after));
                Some(out)
            }
            Job::Inquiring(mut exchange, named) => {
                if let Some(number) = replies::numbered(tick.lines, &named) {
                    let command = format!("order {number}");
                    self.job = Some(Job::Round(Vec::new(), Some(exchange_of(&command, tick.ms))));
                    return Some(Out::Send(command));
                }
                // Answered, and the list does not name it: no caravan goes there.
                let Some(out) = exchange.tick(tick, false) else {
                    return Some(Out::GiveUp {
                        ban: true,
                        wrong: false,
                    });
                };
                self.job = Some(Job::Inquiring(exchange, named));
                Some(out)
            }
        }
    }

    fn moving(&mut self, mut mover: Mover, tick: &Tick<'_>) -> Option<Out> {
        match mover.tick(tick) {
            Moved::Not(out) => {
                self.owes.hands |= mover.hands_emptied;
                self.job = Some(Job::Moving(mover));
                Some(out)
            }
            Moved::Landed { believed } => {
                self.moved = true;
                if believed {
                    return Some(Out::Believed(self.expected));
                }
                if mover.hands_emptied {
                    // Lich: `fill_hands if need_full_hands`, once it lands.
                    self.owes.hands = false;
                    self.job = Some(Job::Deeded);
                    return Some(Out::Deed(Deed::FillHands));
                }
                None
            }
        }
    }

    /// One turn of a loop: is it over, and if not, what is sent next.
    fn turn(&mut self, mut looped: Looping, tick: &Tick<'_>, walker: &Walker) -> Option<Out> {
        let mut heard = false;
        if let Some(mut exchange) = looped.exchange.take() {
            if let Until::Said(lines) = &looped.until {
                heard = said(tick.lines, lines);
            }
            if !heard && let Some(out) = exchange.tick(tick, tick.here != looped.began_in) {
                looped.exchange = Some(exchange);
                self.job = Some(Job::Loop(looped));
                return Some(out);
            }
        }
        let over = match &looped.until {
            Until::RoomChanges => tick.here != looped.began_in,
            Until::There => tick.here == self.expected,
            Until::NoLonger(cond) => !cond.holds(walker),
            Until::Said(_) => heard,
        };
        if over {
            return None;
        }
        if looped.turns >= looped.tries.unwrap_or(MAX_TURNS).min(MAX_TURNS) {
            // A bounded try that ran out is upstream's to shrug at: it goes
            // on to the next step. An unbounded one that hit the walker's own
            // stop has met something it cannot do.
            return looped.tries.is_none().then_some(Out::GiveUp {
                ban: true,
                wrong: false,
            });
        }
        let command = choose(&looped, tick, walker)?;
        if looped.pick == Pick::AnyExit {
            looped.last_way = Some(command.clone());
        }
        looped.turns += 1;
        looped.exchange = Some(exchange_of(&command, tick.ms));
        self.job = Some(Job::Loop(looped));
        Some(Out::Send(command))
    }

    /// Keep waiting, unless it has gone on too long.
    fn wait(&mut self, job: Job, since: u64, ms: u64) -> Out {
        if ms.saturating_sub(since) >= MAX_WAIT_MS {
            return Out::GiveUp {
                ban: true,
                wrong: false,
            };
        }
        self.job = Some(job);
        Out::Hold
    }

    fn begin(&mut self, action: Action, tick: &Tick<'_>, walker: &Walker) -> Begun {
        let ms = tick.ms;
        if let Some(deed) = self.deed_for(&action) {
            self.job = Some(Job::Deeded);
            return Begun::Out(Out::Deed(deed));
        }
        let job = if let Some((pick, until, tries)) = loop_for(&action) {
            Job::Loop(Looping {
                pick,
                until,
                began_in: tick.here,
                turns: 0,
                tries,
                exchange: None,
                last_way: None,
            })
        } else {
            match action {
                Action::Move(command) => {
                    let Some(command) = filled(&command, walker, self.told.as_deref()) else {
                        return Begun::cannot();
                    };
                    self.job = Some(Job::Moving(Mover::new(tick.here, &command, ms)));
                    return Begun::Out(Out::Send(command));
                }
                Action::TryMove(command) | Action::Put(command) => {
                    let Some(command) = filled(&command, walker, self.told.as_deref()) else {
                        return Begun::cannot();
                    };
                    self.job = Some(Job::Trying(exchange_of(&command, ms)));
                    return Begun::Out(Out::Send(command));
                }
                Action::MoveByAnyExitBut(not_this) => {
                    let way = walker
                        .exits
                        .as_ref()
                        .and_then(|exits| exits.iter().find(|exit| **exit != not_this));
                    let Some(way) = way.cloned() else {
                        return Begun::cannot();
                    };
                    self.job = Some(Job::Moving(Mover::new(tick.here, &way, ms)));
                    return Begun::Out(Out::Send(way));
                }
                Action::Moves(commands) => Job::Moves(commands, None),
                Action::Round(commands) => Job::Round(commands, None),
                Action::MovesFromSetting(name) => {
                    let Some(listed) = walker.settings.get(&name) else {
                        return Begun::cannot();
                    };
                    let commands = listed.split(',').map(|c| c.trim().to_owned()).collect();
                    Job::Round(commands, None)
                }
                Action::Ask(command, after) => {
                    self.job = Some(Job::Asking(exchange_of(&command, ms), after));
                    return Begun::Out(Out::Send(command));
                }
                Action::OrderByName(named) => {
                    self.job = Some(Job::Inquiring(exchange_of("inquire", ms), named));
                    return Begun::Out(Out::Send("inquire".to_owned()));
                }
                Action::Pause(length) => Job::Pause(ms + u64::from(length)),
                Action::Await(line) => Job::Line(vec![line], ms),
                Action::AwaitAny(lines) => Job::Line(lines, ms),
                Action::AwaitArrival => Job::Arrival(tick.here, ms),
                Action::WaitUntil(cond) => Job::Fact(cond, ms),
                Action::Replan => {
                    return if tick.here == self.expected {
                        Begun::Skip
                    } else {
                        Begun::Out(Out::Replan)
                    };
                }
                Action::CastAt(spell, target) => {
                    // It carries the walker, so what follows the deed is the
                    // wait to land -- a short one, since the spell has gone
                    // off by the time the deed is done.
                    self.job = Some(Job::Carried(tick.here, None));
                    return Begun::Out(Out::Deed(Deed::CastAt(spell, target)));
                }
                // The loops and the deeds were taken above, so nothing is left.
                _ => return Begun::cannot(),
            }
        };
        // The job starts on this same tick.
        match self.work(job, tick, walker) {
            Some(out) => Begun::Out(out),
            None => Begun::Skip,
        }
    }

    /// The deed a step is, if it is one -- noting what it will owe back.
    fn deed_for(&mut self, action: &Action) -> Option<Deed> {
        Some(match action {
            Action::EmptyHands => {
                self.owes.hands = true;
                Deed::EmptyHands
            }
            Action::FillHands => {
                self.owes.hands = false;
                Deed::FillHands
            }
            Action::Stance(stance) => {
                self.owes.stance = true;
                Deed::Stance(stance.clone())
            }
            Action::RestoreStance => {
                self.owes.stance = false;
                Deed::RestoreStance
            }
            Action::Cast(spell) => Deed::Cast(spell.clone()),
            Action::Remember(name, value) => Deed::Remember(name.clone(), value.clone()),
            Action::Forget(name) => Deed::Forget(name.clone()),
            Action::AwaitFollowers => Deed::AwaitFollowers,
            Action::Speak(language) => {
                self.owes.speech = true;
                Deed::Speak(language.clone())
            }
            Action::RestoreSpeech => {
                self.owes.speech = false;
                Deed::RestoreSpeech
            }
            Action::TakeOut(thing) => {
                self.owes.taken = true;
                Deed::TakeOut(thing.clone())
            }
            Action::PutBack => {
                self.owes.taken = false;
                Deed::PutBack
            }
            _ => return None,
        })
    }
}

/// The loop a step is, if it is one: what is sent each turn, when it stops,
/// and upstream's bound on it.
fn loop_for(action: &Action) -> Option<(Pick, Until, Option<u32>)> {
    let one = |command: &String| Pick::InTurn(vec![command.clone()]);
    Some(match action {
        Action::KeepMoving(command) => (one(command), Until::RoomChanges, None),
        Action::KeepMovingAny(commands) => {
            (Pick::InTurn(commands.clone()), Until::RoomChanges, None)
        }
        Action::MoveUntilThere(command) => (one(command), Until::There, None),
        Action::MoveWhile(command, cond) | Action::PutWhile(command, cond) => {
            (one(command), Until::NoLonger(cond.clone()), None)
        }
        Action::RoundWhile(commands, cond) => (
            Pick::InTurn(commands.clone()),
            Until::NoLonger(cond.clone()),
            None,
        ),
        Action::MoveAnyWhile(commands, cond) => (
            Pick::AnyOf(commands.clone()),
            Until::NoLonger(cond.clone()),
            None,
        ),
        Action::WanderWhile(cond) => (Pick::AnyExit, Until::NoLonger(cond.clone()), None),
        Action::PutUntil {
            command,
            until,
            tries,
        } => (one(command), Until::Said(until.clone()), *tries),
        Action::RoundUntil {
            commands,
            until,
            tries,
        } => (
            Pick::InTurn(commands.clone()),
            Until::Said(until.clone()),
            *tries,
        ),
        _ => return None,
    })
}

enum Begun {
    Out(Out),
    /// The step had nothing to do, or did it at once: on to the next.
    Skip,
}

impl Begun {
    fn cannot() -> Begun {
        Begun::Out(Out::GiveUp {
            ban: true,
            wrong: false,
        })
    }
}

fn said(lines: &[String], any_of: &[String]) -> bool {
    lines
        .iter()
        .any(|line| any_of.iter().any(|wanted| line.contains(wanted.as_str())))
}

fn choose(looped: &Looping, tick: &Tick<'_>, walker: &Walker) -> Option<String> {
    let at = |len: usize, n: u64| usize::try_from(n % u64::try_from(len).ok()?).ok();
    match &looped.pick {
        Pick::InTurn(commands) => {
            let command = commands.get(at(commands.len(), u64::from(looped.turns))?)?;
            filled(command, walker, None)
        }
        Pick::AnyOf(commands) => commands.get(at(commands.len(), tick.random)?).cloned(),
        Pick::AnyExit => {
            let exits = walker.exits.as_ref()?;
            // Upstream's `walk` does not turn straight back when it can help it.
            let back = looped.last_way.as_deref().and_then(opposite);
            let ways: Vec<&String> = exits
                .iter()
                .filter(|exit| exits.len() < 2 || Some(exit.as_str()) != back)
                .collect();
            ways.get(at(ways.len(), tick.random)?)
                .map(|way| (*way).clone())
        }
    }
}

fn opposite(way: &str) -> Option<&'static str> {
    Some(match way {
        "n" => "s",
        "s" => "n",
        "e" => "w",
        "w" => "e",
        "ne" => "sw",
        "sw" => "ne",
        "nw" => "se",
        "se" => "nw",
        "up" => "down",
        "down" => "up",
        "out" => "out",
        _ => return None,
    })
}

/// A command with its placeholders filled in (`cena_map::Action`). `None`
/// when one cannot be: a setting the profile does not carry, or a `{told}`
/// nothing was told. **`{item:…}` is left as it is**: the game's id for a
/// thing is the driver's to look up when it sends (`drive`), since the trip
/// holds no ids.
pub(super) fn filled(command: &str, walker: &Walker, told: Option<&str>) -> Option<String> {
    let mut out = String::new();
    let mut rest = command;
    while let Some((before, after)) = rest.split_once('{') {
        let (name, tail) = after.split_once('}')?;
        out.push_str(before);
        if name == "told" {
            out.push_str(told?);
        } else if name.starts_with("item:") {
            out.push('{');
            out.push_str(name);
            out.push('}');
        } else {
            let value = walker
                .settings
                .get(name.strip_prefix("setting:")?)
                .filter(|value| !value.is_empty())?;
            out.push_str(value);
        }
        rest = tail;
    }
    out.push_str(rest);
    Some(out)
}
