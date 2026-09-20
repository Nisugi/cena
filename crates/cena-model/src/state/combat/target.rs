//! Who a line is about: resolving captures to links, by position.
//!
//! Ports the link-reading half of `parser.rb` -- `extract_target_from_match`,
//! `extract_attacker_from_match`, `extract_creature_target`, `link_name`,
//! `self_target?` -- without the re-scanning. A [`ChunkLine`] is plain text
//! plus the runs it was reassembled from; a regex capture is a byte span of
//! that text; the runs the span overlaps are where the links are.
//!
//! # The rules, as Lich states them
//!
//! | Question | Lich | Here |
//! |---|---|---|
//! | target from a `(?<target>)` capture | the **first** link in the capture text; a negative id (a player) is no target | [`link_in`] with [`Pick::First`], `id >= 0` |
//! | attacker from an `(?<attacker>)` capture | the **last** link -- a flavour prefix can carry the attacker's own pronoun first (*"Froth bubbling on `<his>` lips, `<a berserker>` swings"*, hunt log 2026-09-07 recorded the attacker as `"his"`) | [`Pick::Last`], any id |
//! | target from the whole line | only a **bolded** link; unbolded links are equipment and bystanders | [`bolded_link`], `id > 0` |
//! | a creature's name | strip a possessive `'s` the game puts INSIDE the link, squeeze doubled spaces | [`link_name`] |
//!
//! The "possessive inside the link" case needed a second pattern in Lich
//! (`OPEN_LINK_TAIL_PATTERN`) because the capture ended at the apostrophe and
//! the closing tag fell outside it. A byte span that ends inside a run still
//! overlaps that run, so here it is the same lookup.

use std::ops::Range;

use cena_protocol::frame::LinkKind;

use crate::state::chunks::ChunkLine;

/// Someone a line names: a creature or a player.
///
/// `id` is the wire's `exist`, **negative for players**, and `None` when the
/// line named them in prose with no link at all (a foreign caster named
/// mid-sentence, or a pre-stripped log). `noun` is likewise the link's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// The `exist` id. Negative is a player.
    pub id: Option<i64>,
    /// The `noun`, when linked.
    pub noun: Option<String>,
    /// The display name, cleaned by [`link_name`].
    pub name: String,
}

impl Actor {
    /// Named in prose only: no link, no id.
    #[must_use]
    pub fn unlinked(name: &str) -> Self {
        Self {
            id: None,
            noun: None,
            name: name.trim().to_owned(),
        }
    }

    /// Is this a player rather than a creature?
    ///
    /// `parser.rb:112`: *"player attackers have NEGATIVE exist ids"*.
    #[must_use]
    pub fn is_player(&self) -> bool {
        self.id.is_some_and(|id| id < 0)
    }
}

/// Which link to take when a span overlaps several.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// The first: a target capture.
    First,
    /// The last: an attacker capture, whose prefix may carry a pronoun link.
    Last,
}

/// A creature's display name from its link text.
///
/// `parser.rb:224-226`: the game sometimes puts the possessive INSIDE the link
/// (`<a>grim gigas skald's</a> mastery of music`), which registered a second
/// creature named `"grim gigas skald's"`. Same id, same creature: strip it. A
/// hidden adjective leaves a doubled space (`halfling  cannibal`), squeezed
/// for the same reason.
#[must_use]
pub fn link_name(text: &str) -> String {
    let stripped = text.strip_suffix("'s").unwrap_or(text);
    let mut out = String::with_capacity(stripped.len());
    let mut last_space = false;
    for c in stripped.chars() {
        if c == ' ' {
            if !last_space {
                out.push(c);
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out.trim().to_owned()
}

/// Each run's byte span in the plain text, with its link if any.
fn spans(line: &ChunkLine) -> Vec<(Range<usize>, &cena_protocol::runs::Run)> {
    let mut at = 0;
    line.runs
        .runs
        .iter()
        .map(|run| {
            let start = at;
            at += run.text.len();
            (start..at, run)
        })
        .collect()
}

fn actor_of(run: &cena_protocol::runs::Run) -> Option<Actor> {
    let link = run.link.as_ref()?;
    let LinkKind::Exist { id, noun } = &link.kind else {
        return None;
    };
    Some(Actor {
        id: id.parse::<i64>().ok(),
        noun: Some(noun.clone()),
        name: link_name(&link.text),
    })
}

/// The `exist` link a capture span overlaps, first or last.
///
/// `span` is a byte range of `line.text()`, as `regex::Match::range` gives it.
/// Returns `None` when no run in the span carries an `exist` link.
#[must_use]
pub fn link_in(line: &ChunkLine, span: Range<usize>, pick: Pick) -> Option<Actor> {
    let mut found = spans(line)
        .into_iter()
        .filter(|(r, _)| r.start < span.end && r.end > span.start)
        .filter_map(|(_, run)| actor_of(run));
    match pick {
        Pick::First => found.next(),
        Pick::Last => found.next_back(),
    }
}

/// The first **bolded** `exist` link on the line, with a positive id.
///
/// `parser.rb:325-343`, `extract_creature_target`: *"ONLY accept bolded
/// creatures as targets. Non-bolded links are equipment, objects, or other
/// non-combatants."* The bold is the wire's own hostility marker
/// (`frame/payload.rs`, `bold_depth`).
#[must_use]
pub fn bolded_link(line: &ChunkLine) -> Option<Actor> {
    spans(line)
        .into_iter()
        .filter(|(_, run)| run.style.bold_depth > 0)
        .filter_map(|(_, run)| actor_of(run))
        .find(|a| a.id.is_some_and(|id| id > 0))
}

/// The first `exist` link anywhere on the line, any sign.
///
/// What `spell_loss` uses: a wear-off applies to players in view as well as
/// creatures, so a negative id is a valid answer (`parser.rb:283-286`).
#[must_use]
pub fn any_link(line: &ChunkLine) -> Option<Actor> {
    spans(line).into_iter().find_map(|(_, run)| actor_of(run))
}

/// Does a target capture refer to us?
///
/// `parser.rb:161`, `SELF_TARGET_PATTERN = /\A(?:you|your)\b/i`: anchored at
/// the start so *"a youngling"* is not us, and `\b` so *"you with its tusk"*
/// is.
#[must_use]
pub fn is_self(target_text: &str) -> bool {
    let t = target_text.trim();
    let lower = t.to_ascii_lowercase();
    for word in ["your", "you"] {
        if let Some(rest) = lower.strip_prefix(word) {
            // `\b`: end of text, or a non-word character follows.
            if rest
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_')
            {
                return true;
            }
        }
    }
    false
}

/// Does a pattern's own text address us?
///
/// Lives on [`super::defs::Defs`] beside the tables; this is the call-site
/// spelling the attack classifier reads.
#[must_use]
pub fn pattern_addresses_self(pattern: &str) -> bool {
    super::defs::defs().pattern_addresses_self(pattern)
}
