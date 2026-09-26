//! The profile's `[group]` table: bigshot's "MA Grouping" keys and the
//! author's, read on the leader's code paths, so **the leader's profile
//! decides them** (`plan/39` §0b). The group's rules take them as a
//! [`Settings`] ([`GroupTable::settings`]).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::group::{FriedTrigger, LOST_WAIT, Settings, looter_pattern};

/// `[group]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
