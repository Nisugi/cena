//! Who leads when the leader is lost (`plan/39` §8, question 4). Neither
//! bigshot nor eohunter has a handover (`plan/39` §1): a follower that loses
//! its leader leaves the group and walks home (`bigshot.lic:10231-10240`).
//! The author's answer: *"a priority list or random if one not set up (the
//! healthiest?)"*.

use super::looter::pick;
use super::report::Report;
use super::settings::Settings;

/// The member who takes the lead: the first name on `successors` that is
/// present, else the present member with the most health, a tie settled by
/// `roll`. `None` when no member is present ([`Report::present`]).
///
/// `members` are the ones who could lead: every member but the lost
/// leader. Health the game has not stated ranks below any it has
/// (INFERRED: a member whose health is unknown is not known to be the
/// healthiest).
#[must_use]
pub fn successor<'a>(members: &'a [Report], settings: &Settings, roll: u64) -> Option<&'a str> {
    let present = || members.iter().filter(|member| member.present());
    let listed = settings
        .successors
        .iter()
        .find_map(|name| present().find(|member| member.name == *name));
    if let Some(member) = listed {
        return Some(&member.name);
    }
    let most = present().map(|member| member.health).max()?;
    let healthiest: Vec<&str> = present()
        .filter(|member| member.health == most)
        .map(|member| member.name.as_str())
        .collect();
    pick(&healthiest, roll)
}
