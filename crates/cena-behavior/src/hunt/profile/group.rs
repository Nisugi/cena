//! The profile's `[group]` table: bigshot's "MA Grouping" keys and the
//! author's. Most are read on the leader's code paths, so **the leader's
//! profile decides them** (`plan/39` §0b); the group's rules take those as
//! a [`Settings`] ([`GroupTable::settings`]). Two are read where each member
//! attacks, so **each member's own profile decides them**:
//! `disable_commands` and `troubadours_rally`.
//!
//! bigshot's `group_deader` is not here: the author replaced its pause
//! with the dead member's recovery (`plan/39` §8, question 10), and the
//! importer says so.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::Step;

use crate::group::{FriedTrigger, LOST_WAIT, Settings, looter_pattern};

/// `[group]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "bigshot's switches, each its own key"
)]
pub struct GroupTable {
    /// Who loots, as a pattern over the members' names (`ma_looter`,
    /// `bigshot.lic:3543`), anchored to the whole name ([`looter_pattern`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ma_looter: Option<String>,
    /// Names that never loot (`never_loot`, `bigshot.lic:3544`).
    pub never_loot: Vec<String>,
    /// The member with the most encumbrance headroom loots (`random_loot`,
    /// `bigshot.lic:3545`).
    pub random_loot: bool,
    /// When being fried rests the group: `"all"` (bigshot, `:9060`),
    /// `"any"`, or a list of names (eohunter's `fried_trigger`).
    pub fried_trigger: Fried,
    /// Who leads when the leader is lost, first present first (`plan/39`
    /// §8, question 4).
    pub successors: Vec<String>,
    /// Seconds each wait for a member lasts (`plan/39` §8, question 5).
    pub lost_wait: u64,
    /// At the rest, the leader's prep first (`quiet_followers`,
    /// `bigshot.lic:3551`).
    pub quiet_followers: bool,
    /// To the hunting ground not as a group (`independent_travel`,
    /// `bigshot.lic:3539`, `:7238-7263`): the leader disbands, and each
    /// follower walks the leader's rally rooms and to its hunting room on
    /// its own; they join again there.
    pub independent_travel: bool,
    /// From the hunting ground not as a group (`independent_return`,
    /// `bigshot.lic:3540`, `:7478-7496`): the leader disbands, and each
    /// follower fogs by its own profile and walks the leader's waypoints
    /// to its resting room on its own; they join again there.
    pub independent_return: bool,
    /// The leader loots the room once more before leaving it, solo too
    /// (`final_loot`, `bigshot.lic:3545`, `:9435-9438`).
    pub final_loot: bool,
    /// **Each member's own:** fried and in a group, this routine instead of
    /// the target's (`disable_commands`, `find_routine`,
    /// `bigshot.lic:7166-7168`).
    pub disable_commands: Vec<Step>,
    /// **Each member's own:** with 1040 known, cast it before a routine
    /// step when stuck, or when a member of the group here is
    /// (`troubadours_rally`, `group_status_ailments`,
    /// `bigshot.lic:6698-6712`).
    pub troubadours_rally: bool,
}

impl Default for GroupTable {
    fn default() -> Self {
        Self {
            ma_looter: None,
            never_loot: Vec::new(),
            random_loot: false,
            fried_trigger: Fried::Word("all".to_owned()),
            successors: Vec::new(),
            lost_wait: LOST_WAIT.as_secs(),
            quiet_followers: true,
            independent_travel: false,
            independent_return: false,
            final_loot: false,
            disable_commands: Vec::new(),
            troubadours_rally: false,
        }
    }
}

/// `fried_trigger`: a word, or the names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Fried {
    /// `"all"` or `"any"`.
    Word(String),
    /// Any of these members.
    Names(Vec<String>),
}

impl GroupTable {
    /// The settings the group's rules read. A `ma_looter` that is not a
    /// pattern and a `fried_trigger` word that is neither `all` nor `any`
    /// are [`Self::problems`], and read here as unset.
    #[must_use]
    pub fn settings(&self) -> Settings {
        Settings {
            ma_looter: self
                .ma_looter
                .as_deref()
                .and_then(|text| looter_pattern(text).ok()),
            never_loot: self.never_loot.clone(),
            random_loot: self.random_loot,
            fried_trigger: match &self.fried_trigger {
                Fried::Word(word) if word.eq_ignore_ascii_case("any") => FriedTrigger::Any,
                Fried::Word(_) => FriedTrigger::All,
                Fried::Names(names) => FriedTrigger::Names(names.clone()),
            },
            successors: self.successors.clone(),
            lost_wait: Duration::from_secs(self.lost_wait),
            quiet_followers: self.quiet_followers,
        }
    }

    /// What in the table cannot be used as written.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(text) = &self.ma_looter
            && let Err(why) = looter_pattern(text)
        {
            out.push(format!("group.ma_looter is not a pattern: {why}"));
        }
        if let Fried::Word(word) = &self.fried_trigger
            && !["all", "any"].iter().any(|w| word.eq_ignore_ascii_case(w))
        {
            out.push(format!(
                "group.fried_trigger is `{word}`: it takes \"all\", \"any\" or a list of names"
            ));
        }
        out
    }
}
