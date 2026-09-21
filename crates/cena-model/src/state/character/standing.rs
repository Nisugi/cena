//! Society, citizenship, warcries and resources: the facts one line teaches.
//!
//! # Why these are together and separate from `blocks.rs`
//!
//! `blocks.rs` reads a command's *report* out of a completed chunk -- `info`
//! is fifteen lines and means nothing until they are read together. Everything
//! here is a **single line that stands alone**, and most of them arrive during
//! ordinary play rather than during a sync:
//!
//! ```text
//! The Grandmaster says, "Welcome to the Order of Voln."
//! You have now achieved rank 5 of Shield Bash, costing 20 Combat points.
//! ```
//!
//! That is the difference that matters for the store. A sync BOOTSTRAPS a
//! character the client has never seen, because a gameplay message only tells
//! you about a *change* and a character who trained before the client existed
//! never emits those lines again. After that, these keep the store current
//! incrementally, and a resync is a repair tool rather than the source.
//!
//! > **AUTHOR, 2026-09-20:** *"training updates, and things do get emitted and
//! > they get picked up by the parser automatically."*
//!
//! # Classifiers, per `plan/12` §3a
//!
//! Each is a free function, `&str` in, typed `Option` out, remembering
//! nothing. A line that is not one of these returns `None` and the caller is
//! unchanged -- so a player quoting "Welcome to the Order" in a channel
//! changes no state, because the quote arrives inside a speech frame and
//! never reaches this as a bare line.

use std::collections::BTreeSet;

use super::vocabulary::{ResourceType, Society, Warcry};

/// Society membership, citizenship, warcries and resources.
///
/// Everything [`society_line`] and its siblings teach, stored. Grouped in one
/// struct rather than five fields on [`Character`](super::Character) because
/// they share a refresh: `Group::Standing` is one line of a sync and one entry
/// in the dirty set.
///
/// **Every field is `Option` or empty-by-default**, per `plan/12` §5.2: a
/// character the client has never synced has not "no society", it has an
/// unknown one, and the two must not read the same. `;infomon show` blurs
/// this -- it stores the string `'None'` for no society -- which is the
/// sentinel `vocabulary.rs` records as the reason `Option<Society>` is better.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Standing {
    /// The society, once stated. `Some(None)` is "stated: none".
    pub society: Option<Option<Society>>,
    /// The rank within it.
    ///
    /// For a Master the wire sends no number and this holds
    /// [`Society::max_rank`], applied by [`Self::apply_society`] -- VERIFIED
    /// against the author's character, whose `society` output says only
    /// "Master of the Guardians of Sunfist" while the store holds rank 20.
    pub society_rank: Option<u8>,
    /// The citizenship town. `Some(None)` is "stated: none".
    pub citizenship: Option<Option<String>>,
    /// The warcries known, in [`Warcry::ALL`] order.
    ///
    /// **A set, not a count.** Lich writes presence under one key spelling and
    /// absence under another (`parser.rb:379` vs `:369-374`), so a warcry
    /// learned is never un-learned -- `plan/dazzling` bug 2. A set cannot have
    /// that failure: [`Self::set_warcries`] replaces it wholesale.
    pub warcries: BTreeSet<Warcry>,
    /// Which resource this profession earns, from a `Suffused` line.
    pub resource_type: Option<ResourceType>,
    /// Weekly and total earned.
    pub resources: Option<ResourceAmounts>,
    /// The amount currently suffused.
    pub suffused: Option<u32>,
    /// Covert Arts charges, out of 200. Signed: the wire can send a negative.
    pub covert_arts_charges: Option<i32>,
}

impl Standing {
    /// Apply one society line's meaning.
    ///
    /// Returns whether anything changed, so a caller can mark a group dirty
    /// only when there is something to write.
    ///
    /// # The arithmetic the line does not carry
    ///
    /// A Master's line has no rank, so [`Society::max_rank`] fills it. A step
    /// line says only that one happened, so the rank rises by one -- **and an
    /// unknown rank stays unknown**, rather than becoming 1. Lich does
    /// `get + 1` on a possibly-nil value (`parser.rb:428`), which turns "we
    /// never asked" into "rank 1" and is `plan/dazzling` bug 3.
    pub fn apply_society(&mut self, event: SocietyEvent) -> bool {
        let before = (self.society, self.society_rank);
        match event {
            SocietyEvent::Report {
                society,
                rank,
                master,
            } => {
                self.society = Some(society);
                self.society_rank = match (society, rank, master) {
                    (Some(s), None, true) => Some(s.max_rank()),
                    (Some(_), rank, _) => rank,
                    (None, _, _) => Some(0),
                };
            }
            SocietyEvent::Joined(society) => {
                self.society = Some(Some(society));
                // Voln and the Council start at step 1; Sunfist at rank 0,
                // which is Lich's reading at `parser.rb:417-424`.
                self.society_rank = Some(u8::from(society != Society::GuardiansOfSunfist));
            }
            SocietyEvent::Resigned => {
                self.society = Some(None);
                self.society_rank = Some(0);
            }
            SocietyEvent::Stepped => {
                // Unknown stays unknown. Saturating because a step past a
                // society's maximum is not a thing the game does, and
                // wrapping to 0 would be worse than standing still.
                self.society_rank = self.society_rank.map(|r| r.saturating_add(1));
            }
        }
        before != (self.society, self.society_rank)
    }

    /// Replace the known warcries wholesale.
    ///
    /// Wholesale because a `warcry` report states the complete set, and
    /// merging would keep a warcry the character no longer has.
    pub fn set_warcries(&mut self, warcries: BTreeSet<Warcry>) -> bool {
        let changed = self.warcries != warcries;
        self.warcries = warcries;
        changed
    }
}

/// What a society line stated.
///
/// **Four shapes, not one.** Lich has four separate regexes here
/// (`parser.rb:38-42`) and they answer different questions: a report states
/// the standing, a join/resign/step announces a change. Collapsing them would
/// lose the difference between "you are rank 5" and "you just became rank 5",
/// and only the second is a reason to write anything down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocietyEvent {
    /// The `society` command's report: membership and rank as they stand.
    Report {
        /// Which society, or `None` for "not a member of any society".
        society: Option<Society>,
        /// The rank, or `None` when the line is a Master's.
        ///
        /// **Absent is not zero.** `parser.rb:38`'s rank group is optional and
        /// a Master's line carries no number; the rank comes from
        /// [`Society::max_rank`] instead, which the caller applies because it
        /// is knowledge about the game rather than about the line.
        rank: Option<u8>,
        /// Whether the line said "Master" rather than "member".
        master: bool,
    },
    /// Just joined. The rank that follows is the society's first step.
    Joined(Society),
    /// Just resigned. Every society fact is now false.
    Resigned,
    /// Advanced one step. The line does not say to what.
    ///
    /// **The new rank is the old one plus one**, which is the caller's
    /// arithmetic and not this line's. Lich does `get + 1` on a possibly-nil
    /// value (`parser.rb:428`); `plan/dazzling` records that as a bug to fix
    /// rather than port, and the fix is that an unknown rank stays unknown
    /// instead of becoming 1.
    Stepped,
}

/// Read a society line.
///
/// Ported from `parser.rb:38-42`, with one fix. Lich's `SocietyJoin` scans
/// `match[/Order|Council|Guardians/]` and then tests a `'Lodge'` branch that
/// the scan cannot produce, so **joining Council of Light records nothing**
/// (`plan/dazzling`, bug 1). Here the three welcomes are matched directly.
#[must_use]
pub fn society_line(line: &str) -> Option<SocietyEvent> {
    let trimmed = line.trim_start();

    // The REPORT lines are indented; the announcements are not. Lich anchors
    // both report patterns `^\s+` (`parser.rb:38-39`) and that is load-bearing
    // rather than cosmetic -- VERIFIED against the author's live `society`
    // output, whose report line is indented under a `<popBold/>`:
    //
    // ```text
    // <popBold/>   You are a Master of the Guardians of Sunfist.
    // ```
    //
    // Without the indent requirement, a player typing the same sentence into a
    // channel would restate the character's society. With it, they cannot: a
    // speech frame carries no leading run of spaces.
    let indented = line.len() != trimmed.len();

    if indented && trimmed.starts_with("You are not a member of any society at this time") {
        return Some(SocietyEvent::Report {
            society: None,
            rank: None,
            master: false,
        });
    }

    if indented
        && let Some((rest, master)) = trimmed
            .strip_prefix("You are a Master ")
            .map(|rest| (rest, true))
            .or_else(|| {
                trimmed
                    .strip_prefix("You are a member ")
                    .map(|r| (r, false))
            })
    {
        return society_report(rest, master);
    }

    // A join. Lich matches three welcomes; the Lodge one belongs to the
    // Council of Light, whose Grand Poohbah is its own greeter.
    for (prefix, society) in [
        (
            "The Grandmaster says, \"Welcome to the Order",
            Society::OrderOfVoln,
        ),
        (
            "The Grandmaster says, \"You are now a member of the Guardians of Sunfist",
            Society::GuardiansOfSunfist,
        ),
        (
            "The Grand Poohbah smiles broadly.  \"Welcome to the Lodge,\" he cries",
            Society::CouncilOfLight,
        ),
    ] {
        if trimmed.starts_with(prefix) {
            return Some(SocietyEvent::Joined(society));
        }
    }

    if is_resignation(trimmed) {
        return Some(SocietyEvent::Resigned);
    }

    if is_step(trimmed) {
        return Some(SocietyEvent::Stepped);
    }

    None
}

/// The tail of "You are a {Master,member} ..." -- `in the X at rank N.`
fn society_report(rest: &str, master: bool) -> Option<SocietyEvent> {
    let rest = rest
        .strip_prefix("in the ")
        .or_else(|| rest.strip_prefix("of the "))?;
    let (name, tail) = match rest.find(" at ") {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest.trim_end_matches('.'), ""),
    };
    let society = Society::parse(name)?;
    // "at rank 5." or "at step 5." -- both spellings are in Lich's regex.
    let rank = tail
        .strip_prefix(" at rank ")
        .or_else(|| tail.strip_prefix(" at step "))
        .and_then(|n| n.trim_end_matches('.').parse().ok());
    Some(SocietyEvent::Report {
        society: Some(society),
        rank,
        master,
    })
}

/// `parser.rb:42`'s three resignation lines.
fn is_resignation(line: &str) -> bool {
    line.starts_with(
        "The Grandmaster says, \"I'm sorry to hear that.  You are no longer in our service.",
    ) || line.starts_with("The Poohbah looks at you sternly.  \"I had high hopes for you,\"")
        || (line.starts_with("The Grandmaster says, \"I'm sorry to hear that,")
            && line.contains("I wish you well with any of your future endeavors."))
}

/// `parser.rb:40`'s three advancement lines.
fn is_step(line: &str) -> bool {
    const SIGIL_TRACERS: [&str; 9] = [
        "Zarak", "Faylanna", "Draelox", "Marl", "Vindar", "Taryn", "Meaha", "Oxanna", "Cyndelle",
    ];
    if line.starts_with("The High Taskmaster looks at you, consults ")
        || line.starts_with("The monk concludes ceremoniously,")
    {
        return true;
    }
    SIGIL_TRACERS.iter().any(|who| {
        line.strip_prefix(who).is_some_and(|rest| {
            rest.starts_with(" traces the outline of a sigil into the air before you and says")
        })
    })
}

/// Read a citizenship line: the town, or `None` for no citizenship.
///
/// `parser.rb:36-37`. Returns `Some(None)` for "you don't seem to have
/// citizenship" and `Some(Some(town))` for a town -- the outer `Option` is
/// "was this a citizenship line at all".
///
/// The town stays a `String`: Lich enumerates no towns, and inventing a list
/// would refuse a town the game adds tomorrow.
#[must_use]
pub fn citizenship_line(line: &str) -> Option<Option<String>> {
    if line.starts_with("You don't seem to have citizenship.") {
        return Some(None);
    }
    let rest = line.strip_prefix("You currently have ")?;
    let town = rest.split_once(" citizenship in ")?.1.strip_suffix('.')?;
    Some(Some(town.to_owned()))
}

/// Read one `AFFILIATIONS` line from `profile`.
///
/// **`profile` words these differently from the reports**, which is why
/// [`society_line`] and [`citizenship_line`] cannot read them and this exists:
///
/// | report | `profile` |
/// |---|---|
/// | `   You are a Master of the Guardians of Sunfist.` | `Master of the Guardians of Sunfist` |
/// | `You currently have full citizenship in Kraken's Fall.` | `Full citizen of Kraken's Fall` |
///
/// MEASURED against `fixtures/character_profile.xml`. Found by a test that
/// asserted the profile's society reached `Standing` and got `None`: the first
/// version of the profile reader assumed the report readers would take these,
/// on no evidence.
///
/// Returns nothing for the lines `Standing` has no field for -- `Follower of
/// Zelia`, `Attuned to the Element of Earth`, `Member of House of Paupers`.
/// Those stay in [`Profile::affiliations`](super::profile::Profile).
#[must_use]
pub fn profile_affiliation(line: &str) -> Option<Affiliation> {
    let text = line.trim();
    // A society, `Master of` or `Member of`. The rank is never on this line,
    // so a Master's is `max_rank` and a member's is unknown -- NOT 1, which is
    // the mistake `apply_society` already records as `plan/dazzling` bug 3.
    for (prefix, master) in [("Master of the ", true), ("Member of the ", false)] {
        if let Some(name) = text.strip_prefix(prefix)
            && let Some(society) = Society::parse(name)
        {
            return Some(Affiliation::Society(SocietyEvent::Report {
                society: Some(society),
                rank: master.then(|| society.max_rank()),
                master,
            }));
        }
    }
    // Citizenship. `Full citizen of X`, and the game's other wordings for a
    // partial one are unmeasured -- so only the form in the capture is read,
    // and an unfamiliar one falls through to the kept lines rather than being
    // guessed at.
    if let Some(town) = text.strip_prefix("Full citizen of ") {
        return Some(Affiliation::Citizenship(town.to_owned()));
    }
    None
}

/// What one `profile` affiliation line states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Affiliation {
    Society(SocietyEvent),
    Citizenship(String),
}

/// Read a warcry line from the `warcry` report.
///
/// `parser.rb:43-44`. `Some(None)` is the "you are not a Warrior Guild member"
/// line, which states that the character has **none** -- distinct from having
/// simply not been asked.
#[must_use]
pub fn warcry_line(line: &str) -> Option<Option<Warcry>> {
    if line.starts_with("You must be an active member of the Warrior Guild to use this skill.") {
        return Some(None);
    }
    // The report indents each name and carries nothing else on the line.
    let name = line.trim();
    if name.len() == line.trim_end().len() && !line.starts_with(char::is_whitespace) {
        return None;
    }
    Warcry::parse(name).map(Some)
}

/// One PSM rank change, from ordinary play rather than from a table.
///
/// `parser.rb:45-50`. Five regexes, one shape: a PSM, a category and the rank
/// it now holds. Lich's `UnlearnTechnique` and `LostTechnique` differ only in
/// what the new rank is, so that arithmetic is done here and the caller stores
/// a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsmChange {
    /// The PSM's display name, e.g. `"Shield Bash"`.
    pub name: String,
    /// The category word the line used, e.g. `"Combat"` or `"Shield"`.
    ///
    /// **Not parsed into [`PsmCategory`](super::psm::PsmCategory) here.** The
    /// line's word is the game's, and mapping it is the caller's business --
    /// a classifier that refused an unrecognised category would silently drop
    /// a PSM the game added, which is the Rule 2.2 failure `plan/dazzling`
    /// records as bug 5.
    pub category: String,
    /// The rank the PSM now holds. Zero means no longer trained.
    pub rank: u16,
}

/// Read a PSM rank change.
#[must_use]
pub fn psm_change(line: &str) -> Option<PsmChange> {
    // "You have now achieved rank 5 of Shield Bash, costing 20 Combat points."
    if let Some(rest) = line.strip_prefix("You have now achieved rank ") {
        let (rank, rest) = rest.split_once(" of ")?;
        let (name, rest) = rest.split_once(", costing ")?;
        let category = rest.split_whitespace().nth(1)?;
        return Some(PsmChange {
            name: name.to_owned(),
            category: category.to_owned(),
            rank: rank.parse().ok()?,
        });
    }

    // "You decide to unlearn rank 5 of Shield Bash, regaining 20 Combat points."
    //
    // The rank named is the one being REMOVED, so what remains is one less --
    // the same reading Lich gives it at `parser.rb:447`'s `- 1`.
    if let Some(rest) = line.strip_prefix("You decide to unlearn rank ") {
        let (rank, rest) = rest.split_once(" of ")?;
        let (name, rest) = rest.split_once(", regaining ")?;
        let category = rest.split_whitespace().nth(1)?;
        return Some(PsmChange {
            name: name.to_owned(),
            category: category.to_owned(),
            rank: rank.parse::<u16>().ok()?.saturating_sub(1),
        });
    }

    bracketed_technique(line)
}

/// The `[...]` technique lines, which name the category before the PSM.
fn bracketed_technique(line: &str) -> Option<PsmChange> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;

    // "[You are no longer trained in Shield Specialization: Block Mastery.]"
    if let Some(rest) = inner.strip_prefix("You are no longer trained in ") {
        let (category, name) = split_technique(rest)?;
        return Some(PsmChange {
            name,
            category,
            rank: 0,
        });
    }

    // "[You have gained rank 2 of Shield Specialization: Block Mastery.]"
    // "[You have increased to rank 2 of ...]" / "decreased to rank 1 of ..."
    let rest = inner
        .strip_prefix("You have gained rank ")
        .or_else(|| inner.strip_prefix("You have increased to rank "))
        .or_else(|| inner.strip_prefix("You have decreased to rank "))?;
    let (rank, rest) = rest.split_once(" of ")?;
    let (category, name) = split_technique(rest)?;
    Some(PsmChange {
        name,
        category,
        rank: rank.parse().ok()?,
    })
}

/// `Shield Specialization: Block Mastery.` -> `("Shield", "Block Mastery")`.
fn split_technique(rest: &str) -> Option<(String, String)> {
    let (head, name) = rest.split_once(": ")?;
    let category = head.split_whitespace().next()?;
    Some((category.to_owned(), name.trim_end_matches('.').to_owned()))
}

/// The per-profession resource amounts, from the `resource` report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ResourceAmounts {
    /// Earned this week, against a 50,000 cap.
    pub weekly: u32,
    /// Earned in total, against a 200,000 cap.
    pub total: u32,
}

/// Read the `resource` report's amounts line.
///
/// `parser.rb:51`. **The line does not say which resource it is** -- Lich's
/// alternation is non-capturing -- so only the numbers come back. The type is
/// learned from [`suffused_line`], which is why `vocabulary.rs` records that
/// the two are stored independently.
#[must_use]
pub fn resource_line(line: &str) -> Option<ResourceAmounts> {
    let (name, rest) = line.split_once(": ")?;
    ResourceType::parse(name)?;
    let (weekly, rest) = rest.split_once("/50,000 (Weekly)")?;
    let (total, _) = rest.trim_start().split_once("/200,000 (Total)")?;
    Some(ResourceAmounts {
        weekly: parse_comma_number(weekly)?,
        total: parse_comma_number(total)?,
    })
}

/// Read a `Suffused <type>: <amount>` line, which names the resource.
#[must_use]
pub fn suffused_line(line: &str) -> Option<(ResourceType, u32)> {
    let rest = line.strip_prefix("Suffused ")?;
    let (name, amount) = rest.split_once(": ")?;
    Some((ResourceType::parse(name)?, parse_comma_number(amount)?))
}

/// Read the `Covert Arts Charges: N/200` line.
///
/// `parser.rb:54`. VERIFIED in the author's live `resource` output, where it
/// follows the suffused line:
///
/// ```text
/// Covert Arts Charges: 164/200
/// ```
///
/// The denominator is fixed at 200 in Lich's pattern, so it is required here
/// too -- a different denominator would mean the game changed the cap, and
/// silently reading the numerator anyway would store a number against the
/// wrong scale.
///
/// **Signed on the wire.** Lich captures `[-\d,]+`, so the charges can be
/// negative; `i32` carries that rather than failing to parse it the way a
/// `u32` silently would.
#[must_use]
pub fn covert_arts_line(line: &str) -> Option<i32> {
    let rest = line.strip_prefix("Covert Arts Charges: ")?;
    let charges = rest.strip_suffix("/200")?;
    let digits: String = charges.chars().filter(|c| *c != ',').collect();
    digits.parse().ok()
}

/// `"12,345"` -> `12345`. The wire groups thousands.
fn parse_comma_number(text: &str) -> Option<u32> {
    let digits: String = text.trim().chars().filter(|c| *c != ',').collect();
    digits.parse().ok()
}
