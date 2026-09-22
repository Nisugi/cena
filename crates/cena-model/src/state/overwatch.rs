//! Creatures that hide, and the ones the game forgets to give back.
//!
//! Ports `gemstone/overwatch.rb` (256 lines) — and the **purpose** is the
//! author's, because the source alone does not explain it (2026-09-20):
//!
//! > *"So you're in a room with a creature, that creature hides and it gets
//! > removed from the room's creatures. If we see the creature hide, or we see
//! > the creature disappear without being dead and without seeing it leave the
//! > room, we know it hid, right? […] Where overwatch comes in: when a
//! > creature attacks from hiding, sometimes there's a bug in the game and
//! > that creature doesn't get added to the room creatures. So overwatch
//! > watches for their attack and forces them in, making sure they existed so
//! > our scripts could attack."*
//!
//! Two mechanisms, and the second is the one that earns the module:
//!
//! | | What it does | Why |
//! |---|---|---|
//! | **Hiding** | remembers *something is hidden here* | so a behavior can reveal it before leaving |
//! | **Reveal** | puts the creature **back on the roster** | **a game bug** drops it, and a script cannot target what is not there |
//!
//! # A creature can hide without saying so
//!
//! The prose is not the only signal, and it is not the reliable one. A
//! creature that **vanishes from `room creatures` without dying and without
//! being seen to leave has hidden** — that is an inference the roster already
//! supports, because [`Creatures`] keeps the previous roster
//! (`creatures.rs:81`) and flags the dead
//! ([`Classification::Dead`](crate::state::creature::status::Classification::Dead)).
//!
//! So [`Overwatch::vanished`] is a second entry alongside the prose, and an
//! earlier draft of this module had only the prose — which would miss every
//! creature that hides quietly.
//!
//! # The port is a tenth the size, and §3a is why
//!
//! MEASURED over `overwatch.rb`: **29 named pattern constants, 39 regex
//! literals**, and inside them **53 `<pushBold/>` and 57 `<a exist=`
//! occurrences**. Every pattern re-tokenizes the same fragment:
//!
//! ```text
//! <pushBold\/>\w+ <a exist="(?<id>\d+)" noun=" ?(?<noun>\w+)">(?<name>[^<]+)<\/a><popBold\/>
//! ```
//!
//! because `GameObj` hands Lich neither the link nor the bold depth. Cena's
//! parser produced both, so a rule here is the prose alone and the
//! participants are the links.
//!
//! # Monsterbold is the creature test
//!
//! Lich's patterns require `<pushBold/>` **and** a positive `exist` id, and
//! that is deliberate: overwatch re-targets hidden *creatures*, and a player
//! stepping out of hiding is not a hunt target.
//!
//! VERIFIED by running Lich's own `REVEALED_COMES_OUT` and
//! `SILENT_LEAP_ATTACK` against the corpus: **all 41 matching lines are
//! players** — negative ids, no bold — and **not one matches**:
//!
//! ```text
//! <a exist="-10319971" noun="Riend">Riend</a> comes out of hiding.
//! <a exist="-11182674" noun="Clairebie">Clairebie</a> leaps from hiding to attack!
//! ```
//!
//! Players hiding are tracked separately, by the `room players` status that
//! `RoomPlayer::is_hiding` reads, and this must not overwrite it.
//!
//! # What is NOT ported
//!
//! The per-creature message families — 20 `REVEALED_*` and 6 `SILENT_*` —
//! collapse into their prose. `@@debug` and its `respond` calls are output.

use crate::state::chunks::ChunkLine;

/// A creature revealed from hiding, or the fact that one hid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sighting {
    /// Something hid in this room. No creature is named — that is the point.
    Hid,
    /// A creature came out of hiding, or struck from it.
    Revealed {
        /// The creature's `exist` id.
        id: String,
        /// Its noun, for targeting.
        noun: String,
        /// Its display name.
        name: String,
        /// Whether this was an attack from hiding rather than a plain reveal.
        ///
        /// Lich's `silent_strike:` flag (`overwatch.rb:109`). A creature that
        /// strikes from hiding may re-hide immediately, so a behavior that
        /// wants to answer has to be quicker.
        struck: bool,
    },
}

/// Prose that means something hid, from `HIDING` (`overwatch.rb:131-147`).
///
/// The markup is stripped out: what is left is the phrase. Several of Lich's
/// fifteen name no creature at all (`Something stirs in the shadows.`), which
/// is why [`Sighting::Hid`] carries nobody.
const HID: [&str; 12] = [
    " slips into hiding.",
    " fades into the surroundings.",
    " slips into the shadows.",
    " darts into the shadows.",
    " disperses into roiling shadows!",
    " blends with the shadows, moving too swiftly for the eye to follow.",
    "flies out of the shadows toward",
    "A faint silvery light flickers from the shadows.",
    "Suddenly, a tiny shard of jet black crystal flies from the shadows toward you!",
    "Something stirs in the shadows.",
    "The figure quickly disappears from view.",
    ", but do not reveal your hidden position.",
];

/// Prose that means a creature was revealed, from the 20 `REVEALED_*`
/// constants (`overwatch.rb:150-178`).
const REVEALED: [&str; 14] = [
    " is revealed from hiding.",
    " is forced from hiding!",
    " comes out of hiding.",
    " leaps out of",
    " suddenly leaps from",
    "The shadows melt away to reveal",
    "You discover the hiding place of",
    "You reveal ",
    "The thorny barrier surrounding you blocks the attack from the",
    ", who is forced into view!",
    " explodes from the shadows!",
    ", who was hidden!",
    " glides from the shadows and",
    " twists fluidly to spear you with",
];

/// Prose that means a creature struck from hiding, from the 6 `SILENT_*`
/// constants (`overwatch.rb:180-190`).
///
/// Tested before [`REVEALED`], though **VERIFIED that the order cannot
/// matter**: no `STRUCK` phrase contains any `REVEALED` phrase, so no line
/// reaches both. A mutation swapping the two passed the whole suite, which is
/// how that was found.
///
/// The order is kept because it states the intent — a strike *is* a reveal,
/// and the specific answer is the useful one — and because the two lists are
/// prose that will grow. It is **not** claimed to be load-bearing today, and
/// no test pretends otherwise: an unreachable branch given a test to satisfy
/// a mutation is the failure `plan/05` §0 warns about from the other side.
const STRUCK: [&str; 6] = [
    " leaps from hiding to attack!",
    " springs upon you from behind",
    " leaps from the shadows and hurtles at you",
    " leaps from the shadows and throws",
    " glides past you in a single fluid motion, interposing",
    "Caught unaware, you can only stare in fascination as",
];

/// Read a line as a sighting, or `None` if it is not one.
///
/// A reveal needs a **bolded** creature link — see the module doc on why a
/// player coming out of hiding is not this.
#[must_use]
pub fn classify(line: &ChunkLine) -> Option<Sighting> {
    let text = line.text();

    // A strike is also a reveal, so the specific test comes first.
    for (phrases, struck) in [(STRUCK.as_slice(), true), (REVEALED.as_slice(), false)] {
        if phrases.iter().any(|phrase| text.contains(phrase))
            && let Some((id, noun, name)) = bolded_creature(line)
        {
            return Some(Sighting::Revealed {
                id,
                noun,
                name,
                struck,
            });
        }
    }
    HID.iter()
        .any(|phrase| text.contains(phrase))
        .then_some(Sighting::Hid)
}

/// The first bolded creature link on a line.
///
/// **Bold is the creature test.** `Run::style.bold_depth` is what Lich spells
/// `<pushBold/>` in every one of its patterns, and it is already typed here.
fn bolded_creature(line: &ChunkLine) -> Option<(String, String, String)> {
    line.runs
        .runs
        .iter()
        .filter(|run| run.style.bold_depth > 0)
        .find_map(|run| match run.object().map(|link| &link.kind) {
            Some(cena_protocol::frame::LinkKind::Exist { id, noun }) => {
                Some((id.clone(), noun.clone(), run.text.clone()))
            }
            _ => None,
        })
}

/// Where something was last seen hiding.
///
/// `@@hidden_targets` (`overwatch.rb:19`), which holds exactly one room id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overwatch {
    room: Option<String>,
}

impl Overwatch {
    /// Record that something hid in this room.
    pub fn hid_in(&mut self, room: Option<String>) {
        self.room = room;
    }

    /// The room something was last seen hiding in.
    #[must_use]
    pub fn room(&self) -> Option<&str> {
        self.room.as_deref()
    }

    /// Whether something is hiding in this room.
    ///
    /// `hiders?` (`overwatch.rb:38`). **False when nothing has been seen**,
    /// and false in any other room: the tracker holds one room, so leaving
    /// and returning answers false until something hides again.
    #[must_use]
    pub fn hiders_in(&self, room: Option<&str>) -> bool {
        self.room.is_some() && self.room.as_deref() == room
    }

    /// Forget it. `clear` (`overwatch.rb:24`).
    pub fn clear(&mut self) {
        self.room = None;
    }
}

// **`vanished_not_dead` is not here, and `Creatures::vanished_unaccounted` is
// -- BUILT 2026-09-21.**
//
// An earlier draft had a two-condition version: departed-and-not-dead, reported
// as hiding. The author corrected it (2026-09-20):
//
// > *"but gone just means not in the room, doesn't mean hid. We have creature
// > arrival and leaving messaging though, which would get tagged somewhere
// > along the way and get pushed to the creature."*
//
// The third condition -- **not seen to leave** -- now exists, and the author
// corrected its design too (2026-09-21): *"the room would be giving a link with
// the id"*. So a departure is read off the MARKUP (`departure.rs`), not matched
// against `creature_messages.tsv`'s 1,067 flee lines: the creature's `exist` id
// and the `<d>` direction are both in the line. No name lookup, no placeholder
// expansion, and a creature whose flee prose nobody recorded is handled as well
// as one in the table.
//
// The inference lives on `Creatures`, where the roster and the death state are,
// rather than here: this type knows about a ROOM something hid in and no ids at
// all. `Creatures::vanished_unaccounted` is the three-condition answer.
//
// The primitive it needed, [`Creatures::departed`], stays. It says only what
// left, which is true, and the flee classifier will join it to what that
// means.
