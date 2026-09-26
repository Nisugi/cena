//! Who loots: bigshot's `ma_looter` (`bigshot.lic:7104-7135`), in its
//! order.
//!
//! 1. **Solo, the leader** (`:7106`).
//! 2. **`ma_looter`**: the first name it matches, followers before the
//!    leader, as `get_names` lists them (`:1001-1002`, `:7109-7112`).
//!    Checked before `never_loot`, so a named looter loots even when
//!    `never_loot` names it too (`plan/39` §0b, INFERRED there from the
//!    order and kept).
//! 3. **`random_loot`**: the most encumbrance headroom, `never_loot` and
//!    the unknown left out, a tie settled at random (`:7115-7127`).
//! 4. **Otherwise the leader**, unless `never_loot` names it; then a member
//!    `never_loot` does not name, at random (`:7130-7131`).
//!
//! Two departures, each where bigshot's own branch cannot do what it says:
//!
//! - **The tie that goes to `ma_looter` is not built.** bigshot favours
//!   the named looter among the tied (`:7124-7125`), but step 2 has already
//!   returned whenever the pattern matches a member, so a name equal to it
//!   cannot be among the tied. eohunter found the same and kept the plain
//!   random pick (`scripts/eohunter/group.rb:749-753`).
//! - **`random_loot` with nobody to weigh falls back to step 4**, as
//!   eohunter's does (`group.rb:753`, `if candidates.any?`). bigshot returns
//!   `nil` there (`candidates.sample` of none, `:7125`), and its loot then
//!   hands the corpses to the member named `nil` (`:7842`, `:1034-1036`).
//!
//! The members are the leader and every follower still connected and
//! alive ([`Report::present`]): eohunter chooses among the members whose
//! reports are fresh (`names = online + [@name]`, `group.rb:735`), as
//! bigshot's `member_online` drops a follower that stopped answering
//! (`bigshot.lic:954-966`).

use std::iter;

use super::report::Report;
use super::settings::Settings;

/// Who loots, by name: the leader's or a follower's. `None` when
/// `never_loot` names every member.
///
/// `roll` is the random number for a random choice; the caller rolls it.
#[must_use]
pub fn looter<'a>(
    leader: &'a Report,
    followers: &'a [Report],
    settings: &Settings,
    roll: u64,
) -> Option<&'a str> {
    let members: Vec<&Report> = followers
        .iter()
        .filter(|member| member.present())
        .chain(iter::once(leader))
        .collect();
    if members.len() == 1 {
        return Some(&leader.name);
    }
    if let Some(pattern) = &settings.ma_looter
        && let Some(named) = members.iter().find(|member| pattern.is_match(&member.name))
    {
        return Some(&named.name);
    }
    let never = |name: &str| settings.never_loot.iter().any(|never| never == name);
    if settings.random_loot {
        let weighed: Vec<(&str, i32)> = members
            .iter()
            .filter(|member| !never(&member.name))
            .filter_map(|member| Some((member.name.as_str(), member.headroom?)))
            .collect();
        if let Some(most) = weighed.iter().map(|(_, headroom)| *headroom).max() {
            let tied: Vec<&str> = weighed
                .iter()
                .filter(|(_, headroom)| *headroom == most)
                .map(|(name, _)| *name)
                .collect();
            return pick(&tied, roll);
        }
    }
    if !never(&leader.name) {
        return Some(&leader.name);
    }
    let eligible: Vec<&str> = members
        .iter()
        .map(|member| member.name.as_str())
        .filter(|name| !never(name))
        .collect();
    pick(&eligible, roll)
}

/// One of `items`, chosen by `roll`: bigshot's `sample`, with the caller's
/// random number. `None` when there are none.
pub(super) fn pick<'a>(items: &[&'a str], roll: u64) -> Option<&'a str> {
    let count = u64::try_from(items.len()).ok().filter(|count| *count > 0)?;
    let index = usize::try_from(roll % count).ok()?;
    items.get(index).copied()
}
