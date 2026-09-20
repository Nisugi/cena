//! What the wire says about society membership: joining, advancing, leaving.
//!
//! A stateless classifier over one line, the shape `plan/12` §3a asks for and
//! the shape `skills.rs`, `psm.rs` and `bounty.rs` already take. The abilities
//! a member can then use are in [`super`]; this is only the membership itself.
//!
//! # Two bugs from `inventory/10` §8, fixed rather than ported
//!
//! **1. Joining the Council of Light records nothing.**
//!
//! `parser.rb:413-425` scans the matched line for `/Order|Council|Guardians/`
//! and branches on the result, but one of its three branches tests `'Lodge'`:
//!
//! ```ruby
//! case match[/Order|Council|Guardians/]
//! when 'Order'     then ... 'Order of Voln'
//! when 'Guardians' then ... "Guardians of Sunfist"
//! when 'Lodge'     then ... 'Council of Light'   # unreachable
//! end
//! ```
//!
//! The word `Lodge` really is in the wire text -- the Council's join line is
//! *`The Grand Poohbah smiles broadly.  "Welcome to the Lodge," he cries`*
//! (`parser.rb:41`) -- but the scan cannot produce it, because the scan looks
//! for `Council` and the line never says `Council`. So the `case` falls through
//! and a new Council member's status and rank are never written.
//!
//! Here each society owns its own join pattern, so there is no scan to
//! disagree with: matching the line *is* identifying the society.
//!
//! **2. Advancing a rank adds one to a possibly-absent rank.**
//!
//! `parser.rb:428` is `Infomon.set('society.rank', Infomon.get('society.rank') + 1)`,
//! which raises on `nil` -- the state a character is in before anything has
//! taught Lich their rank. [`MembershipLine::Advanced`] carries no number for
//! that reason: it says *a rank was gained*, and a consumer that does not know
//! the current rank cannot compute the new one and should ask rather than
//! guess.
//!
//! # Advancement does not name the society
//!
//! `SocietyStep` (`parser.rb:40`) matches nine different NPCs across all three
//! societies, and none of the phrasings says which society is advancing. That
//! is sound -- a character belongs to at most one -- but it means
//! [`MembershipLine::Advanced`] carries no society either, and a consumer
//! applies it to whichever membership it already holds.

use std::sync::OnceLock;

use regex::Regex;

use crate::state::character::vocabulary::Society;

/// What one line says about society membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipLine {
    /// `You are a member in the Order of Voln at step 15.`
    ///
    /// The `rank` is `None` for a Master, whose line carries no number; the
    /// rank is then [`Society::max_rank`]. That substitution is deliberately
    /// left to the consumer, so "the wire said 26" and "the wire said Master
    /// and we know Masters are 26" stay distinguishable.
    Standing {
        /// Which society.
        society: Society,
        /// The stated rank, absent for a Master.
        rank: Option<u8>,
        /// Whether the line said Master.
        master: bool,
    },
    /// `You are not a member of any society at this time.`
    NoSociety,
    /// A join, which starts a member at a known rank.
    ///
    /// Voln and the Council start at 1; Sunfist starts at 0, which Lich also
    /// records (`parser.rb:421`) and which is right -- a new Guardian has no
    /// sigils until they earn the first rank.
    Joined {
        /// Which society.
        society: Society,
        /// The rank a new member holds.
        rank: u8,
    },
    /// A rank was gained. **Carries no number**; see the module docs.
    Advanced,
    /// Membership ended.
    Resigned,
}

/// `parser.rb:41`, one prefix per society rather than one alternation.
///
/// Lich keeps these as a single pattern and then re-scans the matched text to
/// work out which society it was, which is the step that goes wrong; see the
/// module docs. Holding them separately means the match *is* the answer.
///
/// These are plain prefixes with no regex syntax in them, so they are compared
/// with `starts_with`. The join line for the Council contains a literal `"` and
/// two spaces after the full stop, both of which are reproduced exactly.
const JOIN_VOLN: &str = r#"The Grandmaster says, "Welcome to the Order"#;
const JOIN_SUNFIST: &str =
    r#"The Grandmaster says, "You are now a member of the Guardians of Sunfist"#;
const JOIN_COUNCIL: &str = r#"The Grand Poohbah smiles broadly.  "Welcome to the Lodge," he cries"#;

/// `parser.rb:39`.
const NO_SOCIETY: &str = "You are not a member of any society at this time.";

/// `parser.rb:40`, the nine NPCs who announce an advancement.
///
/// None of the three phrasings names a society; see the module docs.
const ADVANCE_NPCS: [&str; 9] = [
    "Zarak", "Faylanna", "Draelox", "Marl", "Vindar", "Taryn", "Meaha", "Oxanna", "Cyndelle",
];

/// The rest of an NPC advancement line, after the name.
const ADVANCE_NPC_TAIL: &str = " traces the outline of a sigil into the air before you and says";

/// The two advancement phrasings that name no NPC.
const ADVANCE_IMPERSONAL: [&str; 3] = [
    "The High Taskmaster looks at you, consults her notes, and then announces in a loud voice",
    "The High Taskmaster looks at you, consults his notes, and then announces in a loud voice",
    "The monk concludes ceremoniously,",
];

/// `parser.rb:42`. Three phrasings, all plain prefixes.
///
/// The third is an alternation in Lich with a `.+` in the middle; split here
/// into the two literal halves it spans, which [`MembershipLine::classify`]
/// tests together.
const RESIGN_PREFIXES: [&str; 2] = [
    r#"The Grandmaster says, "I'm sorry to hear that.  You are no longer in our service."#,
    r#"The Poohbah looks at you sternly.  "I had high hopes for you," he says, "but if this be your decision, so be it.  I hereby strip you of membership"#,
];

/// The one resignation line whose middle varies.
const RESIGN_SPANNING: (&str, &str) = (
    r#"The Grandmaster says, "I'm sorry to hear that,"#,
    "I wish you well with any of your future endeavors.",
);

/// The standing pattern, the only line here that needs captures.
///
/// `parser.rb:38`. The rank group is optional because a Master's line carries
/// no number.
fn standing_pattern() -> Option<&'static Regex> {
    static STANDING: OnceLock<Option<Regex>> = OnceLock::new();
    STANDING
        .get_or_init(|| {
            Regex::new(
                r"^\s*You are a (?P<standing>Master|member) (?:in|of) the (?P<society>Order of Voln|Council of Light|Guardians of Sunfist)(?: at (?:rank|step) (?P<rank>[0-9]+))?\.$",
            )
            .ok()
        })
        .as_ref()
}

impl MembershipLine {
    /// Classify one line.
    ///
    /// ```
    /// use cena_model::state::societies::membership::MembershipLine;
    /// use cena_model::Society;
    ///
    /// let line = "   You are a member in the Order of Voln at step 15.";
    /// assert_eq!(
    ///     MembershipLine::classify(line),
    ///     Some(MembershipLine::Standing {
    ///         society: Society::OrderOfVoln,
    ///         rank: Some(15),
    ///         master: false,
    ///     })
    /// );
    /// ```
    #[must_use]
    pub fn classify(line: &str) -> Option<Self> {
        if let Some(caps) = standing_pattern().and_then(|re| re.captures(line)) {
            let society = Society::parse(caps.name("society")?.as_str())?;
            let rank = caps
                .name("rank")
                .and_then(|m| m.as_str().parse::<u8>().ok());
            let master = caps
                .name("standing")
                .is_some_and(|m| m.as_str() == "Master");
            return Some(Self::Standing {
                society,
                rank,
                master,
            });
        }

        // Lich anchors this one with leading whitespace too: it arrives inside
        // an `info` block rather than as its own event.
        if line.trim_start().starts_with(NO_SOCIETY) {
            return Some(Self::NoSociety);
        }

        // **One prefix per society**, so the match identifies the society and
        // there is no second scan to disagree with it. This is the fix for
        // `inventory/10` §8; see the module docs.
        if line.starts_with(JOIN_VOLN) {
            return Some(Self::Joined {
                society: Society::OrderOfVoln,
                rank: 1,
            });
        }
        if line.starts_with(JOIN_COUNCIL) {
            return Some(Self::Joined {
                society: Society::CouncilOfLight,
                rank: 1,
            });
        }
        if line.starts_with(JOIN_SUNFIST) {
            return Some(Self::Joined {
                society: Society::GuardiansOfSunfist,
                rank: 0,
            });
        }

        if Self::is_advancement(line) {
            return Some(Self::Advanced);
        }
        if Self::is_resignation(line) {
            return Some(Self::Resigned);
        }

        None
    }

    /// Does this line announce a rank gain?
    fn is_advancement(line: &str) -> bool {
        if ADVANCE_IMPERSONAL
            .iter()
            .any(|prefix| line.starts_with(prefix))
        {
            return true;
        }
        ADVANCE_NPCS.iter().any(|npc| {
            line.strip_prefix(npc)
                .is_some_and(|rest| rest.starts_with(ADVANCE_NPC_TAIL))
        })
    }

    /// Does this line end a membership?
    fn is_resignation(line: &str) -> bool {
        if RESIGN_PREFIXES
            .iter()
            .any(|prefix| line.starts_with(prefix))
        {
            return true;
        }
        // The third phrasing varies in the middle, so both halves must be
        // present and in order -- the `.+` in `parser.rb:42`.
        let (head, tail) = RESIGN_SPANNING;
        line.strip_prefix(head)
            .is_some_and(|rest| rest.contains(tail))
    }

    /// The rank this line establishes, where it establishes one.
    ///
    /// A Master's line carries no number, so the society's maximum is
    /// substituted here -- the one place that substitution happens, rather than
    /// at each of Lich's two call sites (`parser.rb:400-407`).
    ///
    /// Returns `None` for [`Self::Advanced`], which states a change and not a
    /// value: a consumer must add one to a rank it already knows, and cannot if
    /// it does not. That is the second bug the module docs record.
    #[must_use]
    pub fn rank(&self) -> Option<u8> {
        match self {
            Self::Standing {
                society,
                rank,
                master,
            } => {
                if *master {
                    Some(society.max_rank())
                } else {
                    *rank
                }
            }
            Self::Joined { rank, .. } => Some(*rank),
            Self::NoSociety | Self::Resigned => Some(0),
            Self::Advanced => None,
        }
    }

    /// Which society this line is about, where it says.
    ///
    /// `None` for [`Self::Advanced`], whose nine phrasings name no society, and
    /// for the two lines that end membership.
    #[must_use]
    pub const fn society(&self) -> Option<Society> {
        match self {
            Self::Standing { society, .. } | Self::Joined { society, .. } => Some(*society),
            Self::NoSociety | Self::Advanced | Self::Resigned => None,
        }
    }
}
