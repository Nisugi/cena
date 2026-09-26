//! The group's rules, pure (`plan/39` Stage 2, `crates/cena-behavior/src/group/`):
//! the role, the looter, the rest and hunt merges, and the rest's order.
//! One test per bigshot rule or author's answer, each cited; muster, the
//! recovery and the successor are in `group_muster.rs`.

use cena_behavior::group::{
    FriedTrigger, Hindrance, Leading, PrepOrder, Report, RestCall, Role, Settings, all_dropped,
    looter, looter_pattern, prep_order, role, should_rest, unready,
};
use cena_behavior::hunt::said::Why;
use cena_map::RoomId;
use cena_session::State;
use cena_session::group::{Group, GroupEvent, Member};

/// A member in room 1, connected and grouped, with nothing to report.
fn member(name: &str) -> Report {
    Report {
        name: name.to_owned(),
        link: State::Ready,
        room: Some(RoomId(1)),
        rest: None,
        unready: None,
        hindrance: None,
        grouped: true,
        health: None,
        headroom: None,
    }
}

/// `member`, resting for `why`.
fn resting(name: &str, why: Why) -> Report {
    Report {
        rest: Some(why),
        ..member(name)
    }
}

/// `member`, with this much encumbrance headroom.
fn carrying(name: &str, headroom: i32) -> Report {
    Report {
        headroom: Some(headroom),
        ..member(name)
    }
}

/// `member`, with its connection in `link`.
fn linked(name: &str, link: State) -> Report {
    Report {
        link,
        ..member(name)
    }
}

/// A link to someone, as the game sends it.
fn person(id: &str, noun: &str) -> Member {
    Member {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: noun.to_owned(),
    }
}

fn never(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

// --- the role -----------------------------------------------------------------

/// `plan/39` §8, questions 2 and 8: the role is read off the game's group.
/// Leading with members is lead; leading nobody is solo, as bigshot's
/// `solo?` is (`bigshot.lic:7199-7202`).
#[test]
fn leading_members_is_lead_and_leading_nobody_is_solo() {
    let mut group = Group::default();
    group.emptied();
    assert_eq!(role(&group), Some(Role::Solo), "you, leading nobody");
    group.apply(&GroupEvent::Joined(person("-1", "Dicate")), None);
    assert_eq!(role(&group), Some(Role::Lead), "Dicate joined you");
}

/// `plan/39` §8, question 2: in a group someone else leads is follow
/// (`following?`, `bigshot.lic:7210-7212`).
#[test]
fn in_someone_elses_group_is_follow() {
    let mut group = Group::default();
    group.apply(&GroupEvent::JoinedGroup(person("-2", "Kiyna")), Some("-9"));
    assert_eq!(role(&group), Some(Role::Follow));
}

/// `group/report.rs`, `role`: nobody has said who leads, so there is no
/// role, even with members held -- never lead, and never solo.
#[test]
fn an_unknown_leader_is_no_role() {
    let mut group = Group::default();
    assert_eq!(role(&group), None, "a new session");
    group.apply(&GroupEvent::Joined(person("-1", "Dicate")), None);
    assert_eq!(
        role(&group),
        None,
        "members, and still nobody said who leads"
    );
}

// --- the looter -----------------------------------------------------------------

/// `ma_looter`, `bigshot.lic:7106`: solo, the leader loots, whatever
/// `never_loot` says.
#[test]
fn solo_the_leader_loots_even_on_never_loot() {
    let settings = Settings {
        never_loot: never(&["Leader"]),
        ..Settings::default()
    };
    assert_eq!(looter(&member("Leader"), &[], &settings, 0), Some("Leader"));
}

/// `bigshot.lic:7109-7112`: `ma_looter` names the looter.
#[test]
fn ma_looter_names_the_looter() -> Result<(), regex::Error> {
    let settings = Settings {
        ma_looter: Some(looter_pattern("kiyna")?),
        ..Settings::default()
    };
    let followers = [member("Dicate"), member("Kiyna")];
    assert_eq!(
        looter(&member("Leader"), &followers, &settings, 0),
        Some("Kiyna")
    );
    Ok(())
}

/// `bigshot.lic:7109` before `:7118`: a named looter loots even when
/// `never_loot` names it too.
#[test]
fn ma_looter_wins_over_never_loot() -> Result<(), regex::Error> {
    let settings = Settings {
        ma_looter: Some(looter_pattern("Kiyna")?),
        never_loot: never(&["Kiyna"]),
        ..Settings::default()
    };
    let followers = [member("Kiyna")];
    assert_eq!(
        looter(&member("Leader"), &followers, &settings, 0),
        Some("Kiyna")
    );
    Ok(())
}

/// The `ma_looter` tooltip, `bigshot.lic:2199`: *"If nothing is entered or
/// that character isn't part of the group the lead will resume looting."*
#[test]
fn ma_looter_naming_nobody_here_leaves_it_to_the_leader() -> Result<(), regex::Error> {
    let settings = Settings {
        ma_looter: Some(looter_pattern("Somebody")?),
        ..Settings::default()
    };
    let followers = [member("Kiyna")];
    assert_eq!(
        looter(&member("Leader"), &followers, &settings, 0),
        Some("Leader")
    );
    Ok(())
}

/// `scripts/eohunter/group.rb:739-741`: anchored, so `Bo` is not `Bobby`.
#[test]
fn ma_looter_matches_the_whole_name() -> Result<(), regex::Error> {
    let settings = Settings {
        ma_looter: Some(looter_pattern("Bo")?),
        ..Settings::default()
    };
    let followers = [member("Bobby"), member("Bo")];
    assert_eq!(
        looter(&member("Leader"), &followers, &settings, 0),
        Some("Bo")
    );
    Ok(())
}

/// eohunter chooses among the fresh reports (`group.rb:735`), as bigshot's
/// `member_online` drops a silent follower (`bigshot.lic:954-966`): a
/// follower whose connection is lost is not named.
#[test]
fn a_follower_not_connected_is_not_the_looter() -> Result<(), regex::Error> {
    let settings = Settings {
        ma_looter: Some(looter_pattern("Kiyna")?),
        ..Settings::default()
    };
    let followers = [linked("Kiyna", State::Reconnecting)];
    assert_eq!(
        looter(&member("Leader"), &followers, &settings, 0),
        Some("Leader")
    );
    Ok(())
}

/// `random_loot`, `bigshot.lic:7115-7122`: the most headroom loots.
#[test]
fn random_loot_gives_it_to_the_most_headroom() {
    let settings = Settings {
        random_loot: true,
        ..Settings::default()
    };
    let followers = [carrying("Dicate", 10), carrying("Kiyna", 40)];
    assert_eq!(
        looter(&carrying("Leader", 20), &followers, &settings, 0),
        Some("Kiyna")
    );
}

/// `bigshot.lic:7118`: `never_loot` is left out of the weighing.
#[test]
fn random_loot_leaves_out_never_loot() {
    let settings = Settings {
        random_loot: true,
        never_loot: never(&["Kiyna"]),
        ..Settings::default()
    };
    let followers = [carrying("Dicate", 10), carrying("Kiyna", 40)];
    assert_eq!(
        looter(&carrying("Leader", 20), &followers, &settings, 0),
        Some("Leader")
    );
}

/// `bigshot.lic:7119`, `compact!`: a member whose headroom is unknown is
/// not weighed.
#[test]
fn random_loot_leaves_out_unknown_headroom() {
    let settings = Settings {
        random_loot: true,
        ..Settings::default()
    };
    let followers = [member("Dicate"), carrying("Kiyna", 5)];
    assert_eq!(
        looter(&carrying("Leader", 1), &followers, &settings, 0),
        Some("Kiyna")
    );
}

/// `bigshot.lic:7121-7125`: a tie is settled at random, among the tied
/// only.
#[test]
fn random_loot_settles_a_tie_by_the_roll() {
    let settings = Settings {
        random_loot: true,
        ..Settings::default()
    };
    let followers = [carrying("Dicate", 40), carrying("Kiyna", 40)];
    let leader = carrying("Leader", 10);
    assert_eq!(looter(&leader, &followers, &settings, 0), Some("Dicate"));
    assert_eq!(looter(&leader, &followers, &settings, 1), Some("Kiyna"));
    assert_eq!(looter(&leader, &followers, &settings, 2), Some("Dicate"));
}

/// `group/looter.rs`: with nobody to weigh, the default rule; bigshot
/// returns `nil` (`:7125`), eohunter falls back (`group.rb:753`).
#[test]
fn random_loot_with_nobody_to_weigh_falls_back_to_the_leader() {
    let settings = Settings {
        random_loot: true,
        ..Settings::default()
    };
    let followers = [member("Kiyna")];
    assert_eq!(
        looter(&member("Leader"), &followers, &settings, 0),
        Some("Leader")
    );
}

/// `bigshot.lic:7130-7131`: otherwise the leader, whatever the roll.
#[test]
fn otherwise_the_leader_loots() {
    let followers = [carrying("Kiyna", 90)];
    let leader = carrying("Leader", 1);
    for roll in [0, 1] {
        assert_eq!(
            looter(&leader, &followers, &Settings::default(), roll),
            Some("Leader")
        );
    }
}

/// `bigshot.lic:7131`: the leader on `never_loot`, a member it does not
/// name, at random.
#[test]
fn a_leader_on_never_loot_hands_it_to_a_follower_by_the_roll() {
    let settings = Settings {
        never_loot: never(&["Leader", "Dicate"]),
        ..Settings::default()
    };
    let followers = [member("Dicate"), member("Kiyna"), member("Zeta")];
    let leader = member("Leader");
    assert_eq!(looter(&leader, &followers, &settings, 0), Some("Kiyna"));
    assert_eq!(looter(&leader, &followers, &settings, 1), Some("Zeta"));
}

/// `bigshot.lic:7130-7131`: `never_loot` naming everyone leaves nobody.
#[test]
fn never_loot_naming_everyone_leaves_nobody() {
    let settings = Settings {
        never_loot: never(&["Leader", "Kiyna"]),
        ..Settings::default()
    };
    assert_eq!(
        looter(&member("Leader"), &[member("Kiyna")], &settings, 0),
        None
    );
}

// --- the rest merge ---------------------------------------------------------------

/// `should_rest?`, `bigshot.lic:9053`: nobody needs to rest, the group hunts.
#[test]
fn nobody_needing_a_rest_hunts_on() {
    let followers = [member("Kiyna")];
    assert_eq!(
        should_rest(&member("Leader"), &followers, false, &Settings::default()),
        None
    );
}

/// `bigshot.lic:9060`: fried rests the group only when every member is.
#[test]
fn fried_rests_the_group_only_when_every_member_is() {
    let settings = Settings::default();
    let some = [member("Kiyna")];
    assert_eq!(
        should_rest(&resting("Leader", Why::Fried), &some, false, &settings),
        None,
        "the leader alone"
    );
    let all = [resting("Kiyna", Why::Fried)];
    assert_eq!(
        should_rest(&resting("Leader", Why::Fried), &all, false, &settings),
        Some(RestCall::Rest {
            why: Why::Fried,
            loot_first: true
        }),
        "both"
    );
}

/// `scripts/eohunter/group.rb:571`: the quorum is the members still
/// connected, so a lost one does not hold a fried group out forever.
#[test]
fn a_follower_not_connected_does_not_count_against_fried() {
    let followers = [linked("Kiyna", State::Reconnecting)];
    assert!(
        should_rest(
            &resting("Leader", Why::Fried),
            &followers,
            false,
            &Settings::default()
        )
        .is_some()
    );
}

/// `bigshot.lic:9060`: the all-fried rule is only for a group whose every
/// reason is fried; another reason rests it.
#[test]
fn fried_beside_another_reason_rests() {
    let followers = [resting("Kiyna", Why::Encumbered), member("Dicate")];
    assert_eq!(
        should_rest(
            &resting("Leader", Why::Fried),
            &followers,
            false,
            &Settings::default()
        ),
        Some(RestCall::Rest {
            why: Why::Fried,
            loot_first: true
        })
    );
}

/// eohunter's `fried_trigger` `any` (`group.rb:79`).
#[test]
fn fried_trigger_any_rests_on_one() {
    let settings = Settings {
        fried_trigger: FriedTrigger::Any,
        ..Settings::default()
    };
    let followers = [resting("Kiyna", Why::Fried)];
    assert!(should_rest(&member("Leader"), &followers, false, &settings).is_some());
}

/// eohunter's `fried_trigger` by name (`group.rb:82`).
#[test]
fn fried_trigger_names_rests_on_a_named_member_only() {
    let settings = Settings {
        fried_trigger: FriedTrigger::Names(never(&["Kiyna"])),
        ..Settings::default()
    };
    let named = [resting("Kiyna", Why::Fried), member("Dicate")];
    assert!(should_rest(&member("Leader"), &named, false, &settings).is_some());
    let other = [member("Kiyna"), resting("Dicate", Why::Fried)];
    assert_eq!(
        should_rest(&member("Leader"), &other, false, &settings),
        None
    );
}

/// `bigshot.lic:9065-9067`: a wounded rest waits while a member here is
/// stunned.
#[test]
fn a_wounded_rest_waits_for_a_stunned_member_here() {
    let stunned = Report {
        hindrance: Some(Hindrance::Stuck),
        ..member("Dicate")
    };
    let followers = [resting("Kiyna", Why::Wounded), stunned];
    assert_eq!(
        should_rest(&member("Leader"), &followers, false, &Settings::default()),
        Some(RestCall::Stunned)
    );
}

/// `bigshot.lic:9065`: the stunned hold is for a wounded rest only; any
/// other leaves with the member (bigshot's own *"Fixme: wounded group
/// members need to be mobile before leaving"*, `:9074`, is Muster's hold).
#[test]
fn a_stunned_member_here_holds_only_a_wounded_rest() {
    let stunned = Report {
        hindrance: Some(Hindrance::Stuck),
        ..member("Dicate")
    };
    let followers = [resting("Kiyna", Why::Encumbered), stunned];
    assert_eq!(
        should_rest(&member("Leader"), &followers, false, &Settings::default()),
        Some(RestCall::Rest {
            why: Why::Encumbered,
            loot_first: true
        })
    );
}

/// `group_member_stunned?`, `bigshot.lic:6723-6724`: a stunned member in
/// another room is not here, and the wounded rest leaves.
#[test]
fn a_stunned_member_elsewhere_does_not_hold_the_rest() {
    let stunned = Report {
        hindrance: Some(Hindrance::Stuck),
        room: Some(RoomId(2)),
        ..member("Dicate")
    };
    let followers = [resting("Kiyna", Why::Wounded), stunned];
    assert_eq!(
        should_rest(&member("Leader"), &followers, false, &Settings::default()),
        Some(RestCall::Rest {
            why: Why::Wounded,
            loot_first: false
        })
    );
}

/// `bigshot.lic:6719`: the leader's own stun holds the wounded rest, in a
/// room the map cannot place too.
#[test]
fn the_leaders_own_stun_holds_the_wounded_rest() {
    let leader = Report {
        hindrance: Some(Hindrance::Stuck),
        room: None,
        ..member("Leader")
    };
    let followers = [resting("Kiyna", Why::Wounded)];
    assert_eq!(
        should_rest(&leader, &followers, false, &Settings::default()),
        Some(RestCall::Stunned)
    );
}

/// `bigshot.lic:9071-9073`: bounty, fried, mana and weight loot once more
/// first; a rest the game forced does not (`$bigshot_should_rest`, set on
/// `unable to hold the number of items`, `:2838-2839`, which is
/// `Why::Loaded` here).
#[test]
fn which_rests_loot_once_more_first() {
    let settings = Settings {
        fried_trigger: FriedTrigger::Any,
        ..Settings::default()
    };
    for why in [Why::Bounty, Why::Fried, Why::Mana, Why::Encumbered] {
        assert_eq!(
            should_rest(&resting("Leader", why), &[], false, &settings),
            Some(RestCall::Rest {
                why,
                loot_first: true
            }),
            "{why}"
        );
    }
    for why in [Why::Loaded, Why::BoxInHand, Why::Injured, Why::Wounded] {
        assert_eq!(
            should_rest(&resting("Leader", why), &[], false, &settings),
            Some(RestCall::Rest {
                why,
                loot_first: false
            }),
            "{why}"
        );
    }
}

/// `bigshot.lic:9071`, `!rest_reasons.include?('wounded.')`: any member
/// wounded, the group leaves at once, though another would loot.
#[test]
fn a_wounded_member_leaves_at_once() {
    let followers = [resting("Kiyna", Why::Wounded)];
    assert_eq!(
        should_rest(
            &resting("Leader", Why::Encumbered),
            &followers,
            false,
            &Settings::default()
        ),
        Some(RestCall::Rest {
            why: Why::Encumbered,
            loot_first: false
        })
    );
}

/// `scripts/eohunter/rest.rb:753-766`: the leader's reason names the rest,
/// else the first follower's.
#[test]
fn the_leaders_reason_names_the_rest_else_the_first_followers() {
    let followers = [
        member("Dicate"),
        resting("Kiyna", Why::Mana),
        resting("Zeta", Why::Encumbered),
    ];
    let settings = Settings::default();
    let why = |leader: &Report| match should_rest(leader, &followers, false, &settings) {
        Some(RestCall::Rest { why, .. }) => Some(why),
        _ => None,
    };
    assert_eq!(why(&member("Leader")), Some(Why::Mana));
    assert_eq!(why(&resting("Leader", Why::Bounty)), Some(Why::Bounty));
}

/// `plan/39` §8, question 9: *"rest first. someone probably died."* Back
/// from everyone dropping, the group rests whatever the thresholds say.
#[test]
fn everyone_having_dropped_rests_first() {
    assert_eq!(
        should_rest(
            &member("Leader"),
            &[member("Kiyna")],
            true,
            &Settings::default()
        ),
        Some(RestCall::Rest {
            why: Why::Dropped,
            loot_first: false
        })
    );
}

/// `plan/39` §8, question 9 and §3: everyone dropped is every member,
/// the leader too.
#[test]
fn all_dropped_is_every_member() {
    let lost = [linked("Kiyna", State::Reconnecting)];
    assert!(all_dropped(&linked("Leader", State::Reconnecting), &lost));
    assert!(!all_dropped(&member("Leader"), &lost), "the leader is here");
    let one_here = [linked("Kiyna", State::Reconnecting), member("Dicate")];
    assert!(!all_dropped(&linked("Leader", State::Closed), &one_here));
}

// --- the hunt merge ----------------------------------------------------------------

/// `should_hunt?`, `bigshot.lic:8991-8994`: every member ready, hunt again.
#[test]
fn every_member_ready_hunts_again() {
    assert!(unready(&member("Leader"), &[member("Kiyna")]).is_empty());
}

/// `group_should_hunt?`, `bigshot.lic:1185-1197`, shown leader first
/// (`:7584-7585`): each member not ready, and why.
#[test]
fn each_member_not_ready_is_named_with_its_reason_leader_first() {
    let leader = Report {
        unready: Some("mana still below threshold"),
        ..member("Leader")
    };
    let followers = [
        Report {
            unready: Some("wounded"),
            ..member("Kiyna")
        },
        member("Dicate"),
    ];
    assert_eq!(
        unready(&leader, &followers),
        vec![
            ("Leader", "mana still below threshold"),
            ("Kiyna", "wounded")
        ]
    );
}

/// `plan/39` §8, question 7: a member not connected is not ready.
#[test]
fn a_follower_not_connected_is_not_ready() {
    assert_eq!(
        unready(&member("Leader"), &[linked("Kiyna", State::Reconnecting)]),
        vec![("Kiyna", "its connection is lost")]
    );
}

// --- the rest's order ----------------------------------------------------------------

/// `quiet_followers`, `bigshot.lic:7525`, `:7538-7542`: on, the leader
/// preps first, and a follower begins once the leader has prepared for
/// this rest -- not the last one (`plan/39` §0e).
#[test]
fn quiet_followers_prep_after_the_leader_has_for_this_rest() {
    let followers = [resting("Kiyna", Why::Encumbered)];
    let order = prep_order(&member("Leader"), &followers, &Settings::default());
    assert_eq!(order, PrepOrder::LeaderFirst, "a rest, but not for wounds");
    let before = Leading {
        rest: 3,
        prepared: Some(2),
    };
    assert!(!order.follower_may_prep(&before), "the last rest's prep");
    let after = Leading {
        rest: 3,
        prepared: Some(3),
    };
    assert!(order.follower_may_prep(&after));
}

/// `bigshot.lic:7525`, `!any_wounded` (`:7428-7430`): a member resting
/// wounded, everyone preps at once.
#[test]
fn a_wounded_member_preps_everyone_at_once() {
    let order = prep_order(
        &member("Leader"),
        &[resting("Kiyna", Why::Wounded)],
        &Settings::default(),
    );
    assert_eq!(order, PrepOrder::Together);
    assert!(order.follower_may_prep(&Leading::default()));
}

/// `bigshot.lic:7543-7548`: `quiet_followers` off, everyone at once.
#[test]
fn quiet_followers_off_preps_everyone_at_once() {
    let settings = Settings {
        quiet_followers: false,
        ..Settings::default()
    };
    assert_eq!(
        prep_order(&member("Leader"), &[member("Kiyna")], &settings),
        PrepOrder::Together
    );
}
