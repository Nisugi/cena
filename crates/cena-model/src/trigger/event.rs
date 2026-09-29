//! What the model reads a finished line as: a trigger's `event`
//! (`plan/45` §3a, Stage 2). A closed vocabulary Hydra defines, as the guard
//! words are, and each word reads a classifier the model already runs.
//!
//! | Word | A line that | Reads |
//! |---|---|---|
//! | `speech` | someone says aloud in the room | `state/message.rs`, the `speech` preset |
//! | `whisper` | someone whispers to you | `state/message.rs`, the `whisper` preset |
//! | `my_attack` | starts an attack of mine | `state/combat/attack.rs`: not aimed at me, and naming no attacker |
//! | `attacked` | starts an attack aimed at me: a creature's, the weather's, my own gear's | the same, `inbound` |
//! | `their_attack` | starts a nearby player's attack | the same, `foreign_caster` |
//! | `departure` | shows a creature leaving | `state/departure.rs` |
//! | `affliction`, `affliction <name>` | gives me one of the six statuses the game states only in prose | `state/afflictions.rs`; the names are its ids, `silenced` and the rest |
//! | `incident`, `incident <name>` | states one of Lich's 21 incidents | `state/incident.rs`; the names are Lich's keys, `weapon_reaction` and the rest |
//! | `idle_warning` | is the game's idle warning | `state/idle.rs` |
//!
//! An `event` on its own covers the whole line; beside a `text` or `regex`
//! it narrows them to the lines the model reads that way: `event = "speech"`
//! with `text = "Nisugi"` is Nisugi named in speech.
//!
//! **Not words, and why.** A death is the game's own `death` stream, which
//! `stream = "death"` already reads. A failed move is not a line's own fact:
//! `state/movement.rs` reads a line only as the answer to a move, and fed any
//! line its table matches ordinary English. A creature attacking someone else,
//! and an attack by my group, are left for when someone asks.

use std::fmt;

use crate::state::afflictions::{self, Affliction};
use crate::state::chunks::ChunkLine;
use crate::state::combat::attack::AttackLine;
use crate::state::departure;
use crate::state::idle::IDLE_WARNING;
use crate::state::incident::{self, Incident};
use crate::state::message::{self, Channel};

/// What a trigger's `event` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEvent {
    /// `speech`.
    Speech,
    /// `whisper`.
    Whisper,
    /// `my_attack`.
    MyAttack,
    /// `attacked`.
    Attacked,
    /// `their_attack`.
    TheirAttack,
    /// `departure`.
    Departure,
    /// `affliction`, or `affliction <name>`: gained, not recovered from.
    Affliction(Option<Affliction>),
    /// `incident`, or `incident <name>`, the name one of [`Incident::NAMES`].
    Incident(Option<&'static str>),
    /// `idle_warning`.
    IdleWarning,
}

/// The words that take nothing after them.
const WORDS: &[(&str, LineEvent)] = &[
    ("speech", LineEvent::Speech),
    ("whisper", LineEvent::Whisper),
    ("my_attack", LineEvent::MyAttack),
    ("attacked", LineEvent::Attacked),
    ("their_attack", LineEvent::TheirAttack),
    ("departure", LineEvent::Departure),
    ("idle_warning", LineEvent::IdleWarning),
];

impl LineEvent {
    /// Every event a trigger may name, each named one spelled out:
    /// `speech`, ..., `affliction`, `affliction stunned`, `incident`, ...
    /// For an editor to offer (`plan/54`).
    #[must_use]
    pub fn every() -> Vec<String> {
        let mut all: Vec<String> = WORDS.iter().map(|(word, _)| (*word).to_owned()).collect();
        all.push("affliction".to_owned());
        all.extend(
            Affliction::ALL
                .iter()
                .map(|a| format!("affliction {}", a.id())),
        );
        all.push("incident".to_owned());
        all.extend(
            Incident::NAMES
                .iter()
                .map(|name| format!("incident {name}")),
        );
        all
    }

    /// Read an `event` as the file writes it.
    ///
    /// # Errors
    ///
    /// A word Hydra does not know, or a name its word does not have. The
    /// message lists what it does.
    pub fn parse(written: &str) -> Result<Self, String> {
        let mut words = written.split_whitespace();
        let first = words.next().unwrap_or_default();
        let name = words.next();
        if words.next().is_some() {
            return Err(format!("`{written}` is more than an event and its name"));
        }
        match (first, name) {
            ("affliction", None) => Ok(Self::Affliction(None)),
            ("affliction", Some(name)) => Affliction::ALL
                .into_iter()
                .find(|affliction| affliction.id() == name)
                .map(|affliction| Self::Affliction(Some(affliction)))
                .ok_or_else(|| {
                    let all: Vec<&str> = Affliction::ALL.iter().map(|a| a.id()).collect();
                    format!(
                        "`{name}` is no affliction Hydra reads. They are: {}",
                        all.join(", ")
                    )
                }),
            ("incident", None) => Ok(Self::Incident(None)),
            ("incident", Some(name)) => Incident::NAMES
                .into_iter()
                .find(|known| *known == name)
                .map(|known| Self::Incident(Some(known)))
                .ok_or_else(|| {
                    format!(
                        "`{name}` is no incident Lich names. They are: {}",
                        Incident::NAMES.join(", ")
                    )
                }),
            (word, None) => WORDS
                .iter()
                .find(|(known, _)| *known == word)
                .map(|(_, event)| *event)
                .ok_or_else(|| {
                    format!(
                        "`{word}` is not an event Hydra knows. The events are: {}",
                        all()
                    )
                }),
            (word, Some(_)) if WORDS.iter().any(|(known, _)| *known == word) => {
                Err(format!("`{word}` takes no name after it"))
            }
            (word, Some(_)) => Err(format!(
                "`{word}` is not an event Hydra knows. The events are: {}",
                all()
            )),
        }
    }

    /// Whether `line`, whose text is `text`, is this event.
    #[must_use]
    pub fn reads(self, line: &ChunkLine, text: &str) -> bool {
        match self {
            Self::Speech => said_on(line, Channel::Speech),
            Self::Whisper => said_on(line, Channel::Whisper),
            Self::MyAttack => {
                AttackLine::classify(line).is_some_and(|a| !a.inbound && a.attacker.is_none())
            }
            Self::Attacked => AttackLine::classify(line).is_some_and(|a| a.inbound),
            Self::TheirAttack => AttackLine::classify(line).is_some_and(|a| a.foreign_caster),
            Self::Departure => departure::classify(line).is_some(),
            Self::Affliction(wanted) => afflictions::classify(text)
                .is_some_and(|(got, on)| on && wanted.is_none_or(|wanted| wanted == got)),
            Self::Incident(wanted) => incident::read(line)
                .is_some_and(|got| wanted.is_none_or(|wanted| wanted == got.name())),
            Self::IdleWarning => text.trim() == IDLE_WARNING,
        }
    }
}

impl fmt::Display for LineEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Affliction(None) => f.write_str("affliction"),
            Self::Affliction(Some(affliction)) => write!(f, "affliction {}", affliction.id()),
            Self::Incident(None) => f.write_str("incident"),
            Self::Incident(Some(name)) => write!(f, "incident {name}"),
            event => f.write_str(
                WORDS
                    .iter()
                    .find(|(_, known)| known == event)
                    .map_or("?", |(word, _)| word),
            ),
        }
    }
}

/// Whether `line` is a message on `channel`.
fn said_on(line: &ChunkLine, channel: Channel) -> bool {
    message::classify(line).is_some_and(|message| message.channel == channel)
}

/// Every event, as the file writes it: what an error lists.
fn all() -> String {
    let mut all: Vec<&str> = WORDS.iter().map(|(word, _)| *word).collect();
    all.extend(["affliction [<name>]", "incident [<name>]"]);
    all.join(", ")
}
