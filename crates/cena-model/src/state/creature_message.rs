//! Matching a game line against the bestiary's messages: **arrival, flee,
//! death and decay**.
//!
//! # The gap this closes
//!
//! `creature_messages.tsv` holds **3,863** lines -- 1,094 death, 1,067 flee,
//! 991 arrival, 710 decay -- joined onto every creature by
//! [`Creature::messages_of`](super::creature::Creature::messages_of). MEASURED
//! 2026-09-21: **nothing called it.** The whole table was loaded and unread, so
//! `MessageKind::Flee` existed and no line was ever matched against it.
//!
//! # What this is for, and what it is NOT for
//!
//! This began as the third condition of the author's hiding rule -- "not seen
//! to leave" -- and that is **no longer its job**, twice over. The author:
//!
//! > **2026-09-21:** *"the room would be giving a link with the id"*. And:
//! > *"leaving a room indicates a direction in the `<d>`. arriving in the room
//! > does not. hiding in the room does not."*
//!
//! MEASURED over eight combat logs: of the lines carrying a creature link, all
//! **195** departures carry a direction `<d>`, and **0** of 142 arrivals and 0
//! of 38 hides do. So `departure.rs` reads all three apart from the markup
//! alone, and an earlier version of this file claiming the arrival/departure
//! distinction "stays here" was wrong.
//!
//! **What survives is death and decay prose, and NOT because it is early.**
//!
//! I claimed it was the earliest death signal, on the strength of 44 death
//! lines carrying no `<crtrStatus>` on the same LINE. The author corrected it:
//!
//! > **2026-09-21:** *"the earliest signal is crtrStatus which will have the
//! > death=1 tag at the start of the blob and the death line is usually towards
//! > the end."*
//!
//! The measurement was about lines and the claim was about blobs, which is not
//! the same question -- the status tag and the prose are in one blob, the tag
//! first. `<crtrStatus dead="1">` is the authority on death and this is not.
//! MEASURED for what it is worth: 184 `dead=` attributes across three logs,
//! every one inside a `room objs` component.
//!
//! So what this is for is narrower than I first wrote: it recognises a
//! creature's death and decay **prose** as prose -- which a log reader, a
//! transcript or a highlight rule wants, and which nothing else can do. It is
//! not how a behavior learns that something died.
//!
//! # NOT WIRED, and deliberately named as such
//!
//! Nothing in the library calls this yet. The consumer is a hunting behavior
//! (M6), and wiring death prose into `Creatures` now would mean choosing
//! between it and `<crtrStatus>` as the authority on one creature's death --
//! a decision with no caller to justify it. Rule -1: it stays a tested
//! classifier with a stated purpose until something needs it.
//!
//! # Placeholders, and why matching is a port rather than a compare
//!
//! MEASURED: **1,164 of the 3,863 lines carry a placeholder** -- 802 flee, 171
//! death, 162 arrival, 29 decay -- so two thirds of flee lines cannot be
//! compared as text:
//!
//! ```text
//! An Agresh bear lumbers {direction}.
//! An Agresh bear slowly backs away, {pronoun} teeth bared.
//! ```
//!
//! The vocabularies are Lich's (`creature.rb:854-861`), taken verbatim
//! including the comment that earns them:
//!
//! > *"a form that is missing here makes an otherwise-correct message
//! > unmatchable -- `its` and `their` (possessives) and `out` (a flee
//! > direction) were absent."*
//!
//! The algorithm is `PlaceholderTemplate#build_regex` (`creature.rb:1003`):
//! escape the literal text, replace each `{name}` with an alternation of its
//! forms. Here it is done without a regex engine -- the template is split on
//! its placeholders and the parts are matched in order, which is the same
//! decision `afflictions.rs` made for the same reason.
//!
//! # `{target}` and `{weapon}` are NOT matchable, and Lich's are not either
//!
//! MEASURED: 24 lines carry `{target}` and 8 carry `{weapon}`. `{target}` is
//! **absent from Lich's `PLACEHOLDER_MAP`**, so those lines are unmatchable
//! there too -- an empty option list makes an empty alternation group.
//! `{weapon}` is `RAW:.+?`, a wildcard.
//!
//! Both are treated here as **wildcards over a single segment**: any run of
//! text with no line break. That matches more than Lich does for `{target}`,
//! and the alternative is silently never matching 24 real lines. Recorded
//! because it is a deliberate divergence, not an accident.

use super::creature::MessageKind;

/// Pronoun forms, lowercase. `creature.rb:855`.
const PRONOUN: &[&str] = &[
    "he",
    "she",
    "it",
    "his",
    "her",
    "its",
    "their",
    "him",
    "them",
    "himself",
    "herself",
    "itself",
    "themselves",
];

/// Reflexive forms, lowercase. `creature.rb:857`.
const REFLEXIVE: &[&str] = &["himself", "herself", "itself", "themselves"];

/// Flee and arrival directions. `creature.rb:858`.
///
/// **`out` is in this list and is not a compass point.** Lich's comment says it
/// was once missing, which made every "rides {target} out" line unmatchable.
const DIRECTION: &[&str] = &[
    "north",
    "south",
    "east",
    "west",
    "up",
    "down",
    "out",
    "northeast",
    "northwest",
    "southeast",
    "southwest",
];

/// What one placeholder can be.
enum Slot {
    /// One of a closed list of forms, matched case-insensitively at the start
    /// of a line (Lich keeps a capitalised copy of each list for that).
    OneOf(&'static [&'static str]),
    /// Any run of text: Lich's `RAW:.+?`, and our reading of `{target}`.
    Wildcard,
}

fn slot_for(name: &str) -> Option<Slot> {
    match name {
        "pronoun" | "Pronoun" => Some(Slot::OneOf(PRONOUN)),
        "reflexive" | "Reflexive" => Some(Slot::OneOf(REFLEXIVE)),
        "direction" => Some(Slot::OneOf(DIRECTION)),
        "weapon" | "target" => Some(Slot::Wildcard),
        _ => None,
    }
}

/// What a matched message says happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    /// Which kind of message matched.
    pub kind: MessageKind,
    /// The direction a `{direction}` placeholder captured, lowercased.
    ///
    /// **`None` for a message with no direction in it**, which is a real case:
    /// `A bear slowly backs away, its teeth bared.` is a flee line that names
    /// no direction. A consumer wanting "which way did it go" must handle that.
    pub direction: Option<String>,
}

/// Whether `line` is this template, and what it captured.
///
/// The template may carry placeholders; a literal one is a plain comparison.
#[must_use]
pub fn match_template(template: &str, line: &str) -> Option<Option<String>> {
    let line = line.trim();
    let mut direction = None;
    let mut rest = line;
    let mut tail = template;

    while let Some(open) = tail.find('{') {
        let close = tail[open..].find('}')? + open;
        let name = &tail[open + 1..close];
        let literal = &tail[..open];

        // The literal run before the placeholder must be exactly next.
        let head = rest.get(..literal.len())?;
        if !head.eq_ignore_ascii_case(literal) {
            return None;
        }
        rest = &rest[literal.len()..];

        let slot = slot_for(name)?;
        let after = &tail[close + 1..];
        // What the literal text after this placeholder starts with, up to the
        // next placeholder: the anchor a wildcard stops at.
        let anchor = after.split('{').next().unwrap_or("");

        match slot {
            Slot::OneOf(options) => {
                // Longest first, so `northeast` is not matched as `north`.
                let mut sorted: Vec<&&str> = options.iter().collect();
                sorted.sort_by_key(|o| std::cmp::Reverse(o.len()));
                let found = sorted.into_iter().find(|option| {
                    rest.get(..option.len())
                        .is_some_and(|head| head.eq_ignore_ascii_case(option))
                })?;
                if name == "direction" {
                    direction = Some((*found).to_ascii_lowercase());
                }
                rest = &rest[found.len()..];
            }
            Slot::Wildcard => {
                // Shortest match, as `RAW:.+?` is lazy: stop at the next
                // literal run. An empty anchor means the wildcard runs to the
                // end of the line.
                if anchor.is_empty() {
                    rest = "";
                } else {
                    let at = rest.find(anchor)?;
                    if at == 0 {
                        // `.+?` requires at least one character.
                        return None;
                    }
                    rest = &rest[at..];
                }
            }
        }
        tail = after;
    }

    rest.eq_ignore_ascii_case(tail).then_some(direction)
}

/// Match a line against one creature's messages of the given kinds.
///
/// Kinds are tried in the order given, and the first whole match wins. The
/// caller supplies the order because it knows what it is asking: a hunting
/// behavior cares whether a creature died before whether it fled.
#[must_use]
pub fn classify(
    creature: &super::creature::Creature,
    line: &str,
    kinds: &[MessageKind],
) -> Option<Match> {
    for kind in kinds {
        for template in creature.messages_of(*kind) {
            if let Some(direction) = match_template(template, line) {
                return Some(Match {
                    kind: *kind,
                    direction,
                });
            }
        }
    }
    None
}
