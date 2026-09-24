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
//! supports, because [`Creatures`](super::creatures::Creatures) keeps the
//! previous roster (`creatures.rs:81`) and flags the dead
//! ([`Classification::Dead`](crate::state::creature::status::Classification::Dead)).
//!
//! So that inference is a second entry alongside the prose, and an earlier
//! draft of this module had only the prose — which would miss every creature
//! that hides quietly.
//!
//! **It does NOT live here.** This doc named an `Overwatch::vanished` that was
//! never written; the inference is
//! [`Creatures::vanished_unaccounted`](super::creatures::Creatures::vanished_unaccounted),
//! on the roster that owns the facts it reasons over (Rule 2.2a, one home per
//! fact). The dead link was the only thing saying otherwise.
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

use cena_protocol::frame::LinkKind;
use cena_protocol::runs::Run;

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

/// Prose that means something hid, from `HIDING` (`overwatch.rb:131-148`),
/// split the way Lich's own patterns split.
///
/// # Anchored, as Lich anchors them
///
/// > **CORRECTED 2026-09-23 (review).** This was one flat list matched with
/// > `text.contains`, and no creature link required -- so `Bob says,
/// > "Something stirs in the shadows."` recorded a hider. Lich's fifteen
/// > patterns are not flat: nine put the phrase **straight after a bolded
/// > creature link** (`<pushBold/>\w+ <a exist="\d+" ...>...</a><popBold/>
/// > slips into hiding\.`), and only the rest are bare prose.
///
/// [`AFTER_CREATURE`] is the first kind: the phrase must be the text that
/// immediately follows a bolded, positive-id link. [`PROSE`] is the second,
/// matched anywhere as Lich matches it -- **except in speech**, which Lich does
/// not exclude and should: its regexes are unanchored, so a player quoting the
/// line trips them too. The `<preset id='speech'>` markup says who is talking,
/// and a line someone SAID is not the game narrating.
const AFTER_CREATURE: [&str; 7] = [
    " slips into hiding.",
    // `With a barely audible hiss, <creature> fades...` (`:137`).
    " fades into the surroundings.",
    // `With a sibilant exhalation, <creature> slips...` (`:138`).
    " slips into the shadows.",
    // `:141` ends in `.`, and `:142`'s pronoun link ends in `!`; both follow
    // a bolded positive-id link.
    " darts into the shadows",
    " disperses into roiling shadows!",
    " blends with the shadows, moving too swiftly for the eye to follow.",
    // `You notice the hiding place of <creature>, but do not ...` (`:147`).
    ", but do not reveal your hidden position.",
];

/// `HIDING`'s bare-prose patterns, which name no creature
/// (`overwatch.rb:134-136`, `:144-145`) and are unanchored in Lich.
const PROSE: [&str; 5] = [
    "flies out of the shadows toward you!",
    "A faint silvery light flickers from the shadows.",
    "Suddenly, a tiny shard of jet black crystal flies from the shadows toward you!",
    "Something stirs in the shadows.",
    "The figure quickly disappears from view.",
];

/// What precedes the target in `:139`/`:140`: `... flies out of the shadows
/// toward <someone>!` -- a player's link directly, or a creature's after its
/// article (`\w+ `). The one `HIDING` shape with text on BOTH sides of the
/// link, so it is checked apart from [`AFTER_CREATURE`].
const FLIES_TOWARD: &str = "flies out of the shadows toward";

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
    classify_text(line, &line.text())
}

/// [`classify`], given the line's text already rendered.
///
/// The chunk consumers render each line once and share it (review: a main
/// line was being rebuilt about six times on its way through).
pub(crate) fn classify_text(line: &ChunkLine, text: &str) -> Option<Sighting> {
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
    hid(line, text).then_some(Sighting::Hid)
}

/// Whether the line is one of `HIDING`'s fifteen, anchored as Lich anchors
/// them (see [`AFTER_CREATURE`]).
fn hid(line: &ChunkLine, text: &str) -> bool {
    if line.is_spoken() {
        return false;
    }
    if PROSE.iter().any(|phrase| text.contains(phrase)) {
        return true;
    }
    after_links(line, text).any(|(run, before, after)| {
        let creature = bolded_positive(run);
        if flies_toward(before) {
            // `:139` is toward a PLAYER -- a negative id, no bold -- and
            // `:140` toward a creature. Both end `!` straight after the link.
            let player = matches!(run.object().map(|l| &l.kind),
                Some(LinkKind::Exist { id, .. }) if id.starts_with('-'));
            return (creature || player) && after.starts_with('!');
        }
        creature
            && AFTER_CREATURE
                .iter()
                .any(|phrase| after.starts_with(phrase))
    })
}

/// Whether the text before a link ends `flies out of the shadows toward `,
/// with or without one word (a creature's article) between.
fn flies_toward(before: &str) -> bool {
    let before = before.trim_end();
    before.ends_with(FLIES_TOWARD)
        || before
            .rsplit_once(' ')
            .is_some_and(|(head, _article)| head.ends_with(FLIES_TOWARD))
}

/// Whether a run is a bolded link to a positive `exist` id -- Lich's
/// `<pushBold/>... <a exist="\d+" ...>` creature shape.
fn bolded_positive(run: &Run) -> bool {
    run.style.bold_depth > 0
        && matches!(run.object().map(|l| &l.kind),
            Some(LinkKind::Exist { id, .. }) if !id.starts_with('-'))
}

/// Every object link on the line, with the text before and after it.
///
/// A link can span several runs (a style change inside it), so the "after"
/// is taken from the LAST run of each span.
fn after_links<'a>(
    line: &'a ChunkLine,
    text: &'a str,
) -> impl Iterator<Item = (&'a Run, &'a str, &'a str)> + 'a {
    let runs = &line.runs.runs;
    let mut at = 0;
    let mut span_start = 0;
    runs.iter().enumerate().filter_map(move |(i, run)| {
        let object = run.object();
        // A run continuing the previous run's link keeps the span's start.
        if i == 0 || object.is_none() || runs[i - 1].object() != object {
            span_start = at;
        }
        at += run.text.len();
        let object = object?;
        if runs.get(i + 1).and_then(Run::object) == Some(object) {
            return None;
        }
        // `text` is these runs concatenated, so the offsets line up; `get`
        // rather than indexing so a caller passing other text degrades to
        // "no match" instead of a panic.
        Some((run, text.get(..span_start)?, text.get(at..)?))
    })
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
            Some(LinkKind::Exist { id, noun }) => {
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
