//! A creature seen to leave: **read off the markup, not matched against prose.**
//!
//! # The author corrected the design, not a detail
//!
//! This began as a matcher over `creature_messages.tsv`'s 1,067 flee lines,
//! because `plan/20` said the third condition of the hiding rule needed one and
//! that 802 of them carry `{direction}`/`{pronoun}` placeholders. That work is
//! real and is kept (`creature_message.rs`) -- but it is the wrong tool for
//! *this* question:
//!
//! > **AUTHOR, 2026-09-21:** *"the room would be giving a link with the id"*.
//!
//! MEASURED on the wire, `GST-Nisugi/2026-08-21`:
//!
//! ```text
//! <pushBold/>A <a exist="8365650" noun="ghast">cadaverous tatterdemalion ghast</a><popBold/>
//! leans back on <pushBold/><a exist="8365650" noun="ghast">his</a><popBold/> haunches
//! and bounds <d>southwest</d>.
//! ```
//!
//! Both facts a departure carries are **in the markup**:
//!
//! | fact | where | not |
//! |---|---|---|
//! | which creature | the `<a exist=>` id, twice -- the pronoun is a link too | a name looked up in a table |
//! | which direction | a bare `<d>` command link | a word matched from a list |
//!
//! So this reads the frame. No bestiary lookup, no placeholder expansion, no
//! ambiguity about which of two creatures with the same name left, and a
//! creature whose flee line nobody has recorded is handled exactly as well as
//! one whose line is in the table.
//!
//! # What still needs the prose, and why the matcher is kept
//!
//! A `<d>` in a line naming a creature says *a direction was clickable there*.
//! It does not say the creature used it: a bare `<d>` also appears in room
//! descriptions and in `<compass>`. So [`classify`] requires **both** a bolded
//! creature link and a direction link in the same line, which is the shape a
//! departure has and a room description does not.
//!
//! What this cannot do is tell a departure from an *arrival*: `bounds
//! <d>southwest</d>` and `charges in` are the same shape once the words are
//! ignored. The words are what separate them, so the arrival/flee distinction
//! stays with `creature_message.rs` -- and the two are complementary rather
//! than duplicated. This one answers *"who left, and which way"*; that one
//! answers *"was this an arrival or a departure"*.

use cena_protocol::frame::LinkKind;

use super::chunks::ChunkLine;

/// A creature the feed showed leaving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Departure {
    /// The creature's `exist` id, from its link.
    pub id: i64,
    /// The direction it took, from the `<d>` link, lowercased.
    pub direction: String,
}

/// The departure a line shows, if it shows one.
///
/// Requires a **bolded** creature link and a direction link in one line. Bold
/// is the wire's own mark for a creature (`state/room.rs:207`), and requiring
/// it is what keeps a clickable direction in a room description from reading as
/// something leaving.
#[must_use]
pub fn classify(line: &ChunkLine) -> Option<Departure> {
    let mut creature = None;
    let mut direction = None;

    for run in &line.runs.runs {
        let Some(link) = &run.link else { continue };
        match &link.kind {
            // The creature. Bolded, and the FIRST such link: the line repeats
            // the id on the pronoun, and both are the same creature anyway.
            LinkKind::Exist { id, .. } if run.style.bold_depth > 0 && creature.is_none() => {
                creature = id.parse::<i64>().ok();
            }
            // The direction. A bare `<d>` carries its command as its text,
            // which for a movement link IS the direction.
            LinkKind::DirectText if direction.is_none() => {
                let word = link.text.trim().to_ascii_lowercase();
                if is_direction(&word) {
                    direction = Some(word);
                }
            }
            _ => {}
        }
    }

    Some(Departure {
        id: creature?,
        direction: direction?,
    })
}

/// Whether a `<d>` link's text is a direction a creature can leave by.
///
/// The list is Lich's (`creature.rb:858`), which includes `out` -- not a
/// compass point, and its absence there once made every such line unmatchable.
/// Checked rather than accepted, because a bare `<d>` also carries `go bridge`,
/// `climb wall` and worse.
#[must_use]
pub fn is_direction(word: &str) -> bool {
    matches!(
        word,
        "north"
            | "south"
            | "east"
            | "west"
            | "up"
            | "down"
            | "out"
            | "northeast"
            | "northwest"
            | "southeast"
            | "southwest"
    )
}
