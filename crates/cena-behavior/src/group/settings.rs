//! The leader's `[group]` settings that these rules read.
//!
//! bigshot's "MA Grouping" keys are read only on the leader's code paths,
//! so **the leader's profile decides them** (`plan/39` §0b; the `ma_looter`
//! tooltip says so, `bigshot.lic:2199`). The table they will be read from,
//! and the importer's rows for them, are Stage 5's; here they are a plain
//! struct the rules take as an argument.
//!
//! `independent_travel`, `independent_return`, `final_loot`,
//! `group_deader`, `disable_commands` and Troubadour's Rally are not here:
//! no Stage 2 rule reads them.

use std::time::Duration;

use regex::{Regex, RegexBuilder};

/// How long the group waits for a member before it stops waiting: 90
/// seconds. The author: *"60-120 seconds? While still keeping the room
/// safe. Configurable ..."* (`plan/39` §8, question 5).
pub const LOST_WAIT: Duration = Duration::from_secs(90);

/// When the members' being fried rests the group.
///
/// bigshot hard-codes [`Self::All`] (`bigshot.lic:9060`). eohunter made it
/// a setting, `fried_trigger` (`Group::Policy#fried_rest?`,
/// `scripts/eohunter/group.rb:72-83`), and defaults it to `any`; Hydra
/// defaults to bigshot's ([`Settings::default`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FriedTrigger {
    /// Only when every connected member is fried (bigshot).
    All,
    /// When any member is fried.
    Any,
    /// When any of these members is fried, by name.
    Names(Vec<String>),
}

/// The group settings, as the leader's profile gives them.
#[derive(Clone, Debug)]
pub struct Settings {
    /// Who loots, as a pattern over the members' names (`ma_looter`,
    /// `bigshot.lic:3543`): build it with [`looter_pattern`].
    pub ma_looter: Option<Regex>,
    /// Names that never loot (`never_loot`, `bigshot.lic:3544`), exactly as
    /// bigshot compares them (`include?`, `:7118`, `:7130`).
    pub never_loot: Vec<String>,
    /// The member with the most encumbrance headroom loots (`random_loot`,
    /// `bigshot.lic:3545`).
    pub random_loot: bool,
    /// When being fried rests the group.
    pub fried_trigger: FriedTrigger,
    /// Who leads when the leader is lost, first present first (`plan/39`
    /// §8, question 4: *"a priority list or random if one not set up (the
    /// healthiest?)"*).
    pub successors: Vec<String>,
    /// How long each wait lasts ([`LOST_WAIT`]).
    pub lost_wait: Duration,
    /// At the rest, the leader's prep first and the followers' after
    /// (`quiet_followers`, `bigshot.lic:3551`; on by default, the author:
    /// *"sure, and toggleable"*, `plan/39` §8, question 11).
    pub quiet_followers: bool,
}

impl Default for Settings {
    /// bigshot's defaults (`bigshot.lic:3543-3551`), and the author's for
    /// the keys bigshot does not have.
    fn default() -> Self {
        Self {
            ma_looter: None,
            never_loot: Vec::new(),
            random_loot: false,
            fried_trigger: FriedTrigger::All,
            successors: Vec::new(),
            lost_wait: LOST_WAIT,
            quiet_followers: true,
        }
    }
}

/// `ma_looter`'s text as the pattern [`Settings::ma_looter`] holds:
/// case-insensitive, as bigshot's `/#{@MA_LOOTER}/i` is
/// (`bigshot.lic:7110`), and **anchored to the whole name**, as bigshot's
/// is not.
///
/// eohunter anchored it after the unanchored match *"handed every corpse to
/// Bobby when the profile named Bo"* (`scripts/eohunter/group.rb:739-741`).
/// Anchoring keeps a pattern's alternatives (`Bo|Dicate`) and gives up only
/// a bare fragment of a name matching the whole of it. A pattern that does
/// not compile is refused here, so a profile that names one is refused
/// when it loads rather than looting with nobody.
///
/// # Errors
///
/// The text is not a pattern the `regex` crate reads.
pub fn looter_pattern(text: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(&format!("^(?:{text})$"))
        .case_insensitive(true)
        .build()
}
