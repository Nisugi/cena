//! Who said what, and on which channel.
//!
//! **Not a port.** MEASURED: Lich has no speech classifier — nothing in
//! `lib/` matches `says` or `whispers` outside `move.rb`'s movement-blocked
//! messages and the combat tables. `messaging.rb` (148 lines) only *emits*
//! markup to a client and parses nothing, which `plan/20` §1 records and
//! `plan/12` §2 puts in `cena-ui`.
//!
//! So the wire is the only authority here, and it turns out to say more than
//! enough.
//!
//! # The markup already carries the channel and the speaker
//!
//! ```text
//! <preset id='speech'><a exist="-10007833" noun="Pukk">Pukk</a> says</preset>, "Have fun!"
//! <preset id="whisper"><a exist="-10807620" noun="Calvix">Calvix</a> whispers,</preset> "Vellum is cool"
//! ```
//!
//! Three facts, all structural:
//!
//! | Fact | Where |
//! |---|---|
//! | the channel | the `<preset id=>` |
//! | the speaker | the `exist` link inside it |
//! | the body | the run **after** the preset closes |
//!
//! So this classifier re-tokenizes nothing — §3a's bargain, and the reason it
//! is twenty lines rather than a pile of regexes. A text-matching version
//! would have to decide what `says` means in prose, and the corpus shows why
//! that fails: **`<X> whispers a quiet invocation`** and **`<X> whispers
//! arcane incantations`** are *spell-casting emotes*, not speech. They carry
//! no preset, so nothing here mistakes them for messages.
//!
//! MEASURED over the 208 live Lich XML logs: **203 `speech` and 3 `whisper`
//! presets**, and those are the only two preset ids in the corpus.
//!
//! > A first pass counted "192, all `speech`" — wrong, because the pattern
//! > only accepted single quotes and the whisper lines use double. The wire
//! > uses **both quoting styles**, which `text::attribute` already handles and
//! > a hand-rolled scan would not.
//!
//! # Verbs are kept, not enumerated
//!
//! The verb phrase is whatever sits between the speaker and the close:
//! `says`, `asks`, `exclaims`, and also `softly`, `squeakily`, `monotonously`,
//! `darkly`. MEASURED: **14 distinct openers over 206 lines**, and the tail is
//! plainly adverbial rather than a closed set. So [`Message::verb`] is the
//! text, and the *channel* — the part that is closed — is the preset.

use cena_protocol::frame::LinkKind;

use crate::state::chunks::ChunkLine;

/// Which channel a message arrived on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Channel {
    /// Spoken aloud in the room: `<preset id='speech'>`.
    Speech,
    /// Whispered to you: `<preset id='whisper'>`.
    Whisper,
}

impl Channel {
    /// Read a `<preset id=>` value.
    ///
    /// `None` for any other preset: this is **not** a catch-all for styled
    /// text, and treating an unknown preset as speech would invent a message
    /// the game did not send.
    #[must_use]
    pub fn parse(preset: &str) -> Option<Self> {
        Some(match preset.trim() {
            "speech" => Self::Speech,
            "whisper" => Self::Whisper,
            _ => return None,
        })
    }
}

/// Something a character said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// Which channel it came on.
    pub channel: Channel,
    /// The speaker's `exist` id, when the line named one.
    ///
    /// **`None` for your own speech**, which the game writes without a link:
    /// `<preset id='speech'>You say</preset>, "..."`. MEASURED: 16 of the 206
    /// carry no speaker link.
    pub speaker_id: Option<String>,
    /// The speaker's name as displayed, when the line named one.
    pub speaker: Option<String>,
    /// The verb phrase, e.g. `"says"`, `"asks softly"`, `"exclaims"`.
    ///
    /// **Text, not an enum.** MEASURED: 14 distinct openers over 206 lines,
    /// with an adverbial tail that is plainly open. The part that IS closed is
    /// [`Self::channel`].
    pub verb: String,
    /// What was said, without the surrounding quotes.
    pub body: String,
}

/// Read a line as a message, or `None` if it is not one.
///
/// The preset decides. A line with no `speech` or `whisper` preset is not a
/// message however much it looks like one — see the module doc on spell
/// emotes.
#[must_use]
pub fn classify(line: &ChunkLine) -> Option<Message> {
    let channel = line
        .runs
        .runs
        .iter()
        .find_map(|run| run.style.preset.as_deref().and_then(Channel::parse))?;

    // The speaker is the first object link inside the preset. Outside it,
    // a link belongs to the BODY -- `says, "Here are your winnings,
    // <a exist=...>Ryeka</a>"` names its listener, not its speaker.
    let speaker = line
        .runs
        .runs
        .iter()
        .filter(|run| run.style.preset.is_some())
        .find_map(|run| match run.object().map(|link| &link.kind) {
            Some(LinkKind::Exist { id, .. }) => Some((id.clone(), run.text.clone())),
            _ => None,
        });

    // The verb phrase is what the preset holds after the speaker; the body is
    // everything after the preset closes.
    let mut verb = String::new();
    let mut body = String::new();
    let mut speaker_seen = speaker.is_none();
    for run in &line.runs.runs {
        if run.style.preset.is_some() {
            if speaker_seen {
                verb.push_str(&run.text);
            } else if run.object().is_some() {
                speaker_seen = true;
            }
        } else {
            body.push_str(&run.text);
        }
    }

    let (speaker_id, speaker_name) = match speaker {
        Some((id, name)) => (Some(id), Some(name)),
        None => (None, None),
    };
    Some(Message {
        channel,
        speaker_id,
        speaker: speaker_name,
        verb: verb.trim().trim_end_matches(',').trim().to_owned(),
        body: unquote(&body),
    })
}

/// The quoted part of a body, without its quotes.
///
/// The wire writes `, "Have fun!"` after a `speech` preset and `"Vellum is
/// cool"` after a `whisper` one — MEASURED, 193 of 206 begin `, ` and the
/// rest go straight into the quote. Both forms give the same body.
fn unquote(body: &str) -> String {
    let trimmed = body.trim().trim_start_matches(',').trim();
    trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(trimmed)
        .to_owned()
}

/// The most recent messages, oldest first.
///
/// **Bounded**, for the reason `Chunk` is: a session that runs for hours in a
/// busy town would otherwise grow without limit, and the oldest message is
/// the one least worth keeping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Messages {
    recent: Vec<Message>,
}

/// How many messages are retained.
///
/// The same order as `MAX_CHUNK_LINES`, and chosen for the same reason: enough
/// that a consumer reacting a few lines later still sees it, few enough that
/// nothing accumulates.
pub const MAX_MESSAGES: usize = 200;

impl Messages {
    /// Record one, dropping the oldest at the cap.
    pub fn push(&mut self, message: Message) {
        if self.recent.len() >= MAX_MESSAGES {
            self.recent.remove(0);
        }
        self.recent.push(message);
    }

    /// Every message held, oldest first.
    #[must_use]
    pub fn all(&self) -> &[Message] {
        &self.recent
    }

    /// The most recent message on a channel.
    #[must_use]
    pub fn last_on(&self, channel: Channel) -> Option<&Message> {
        self.recent.iter().rev().find(|m| m.channel == channel)
    }

    /// Everything one speaker has said, oldest first.
    pub fn from_speaker(&self, id: &str) -> impl Iterator<Item = &Message> {
        self.recent
            .iter()
            .filter(move |m| m.speaker_id.as_deref() == Some(id))
    }

    /// Whether anything has been heard.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.recent.is_empty()
    }

    /// Forget everything.
    pub fn clear(&mut self) {
        self.recent.clear();
    }
}
