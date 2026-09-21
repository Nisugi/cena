//! The login sync: what it asks for, and what it leaves alone.
//!
//! `plan` is a pure function of a snapshot and a clock, so most of this needs
//! no session at all. The running half is exercised against a real actor at
//! the bottom.

use std::time::{Duration, SystemTime};

use cena_behavior::sync::{MAX_AGE, plan};
use cena_session::{CharacterSnapshot, Group};

fn snapshot() -> CharacterSnapshot {
    CharacterSnapshot::new("GS", "Nisugi")
}

#[test]
fn a_character_nobody_has_synced_is_asked_everything() {
    // What makes the first login work. A group with no timestamp is always
    // stale, so an empty snapshot plans every command.
    let commands = plan(&snapshot(), SystemTime::now(), MAX_AGE);
    let groups: Vec<Group> = commands.iter().map(|(g, _)| *g).collect();
    for group in Group::ALL {
        assert!(groups.contains(&group), "{group:?} was not planned");
    }
}

#[test]
fn a_full_sync_is_sixteen_commands() {
    // Pinned so a group gaining a command is a visible change to the cost of
    // a login rather than a silent one.
    //
    // MEASURED, because the number was wrong twice before this test existed.
    // Lich sends **15**, not the 16 stated in conversation all session:
    //
    // ```text
    // $ awk "/request = \{/,/\}$/" reference/lich-5/lib/gemstone/infomon/cli.rb     //     | grep -coE "'[a-z ]+'\s+=>"
    // 15
    // ```
    //
    // Sixteen here, and every difference is accounted for rather than assumed:
    //
    //   15  Lich's list
    //   -3  groups this model does not read yet: `spell`, `experience` (the
    //       report is read but not persisted -- see `Group`, which has no
    //       `Experience`), `profile full`
    //   +2  `info full` is planned twice, because `Stats` and `Identity` are
    //       separately staleable and either alone can be the stale one
    //   +2  `wealth` and `tickets`, which Lich does not sync at all: its
    //       currency keys are filled only from ordinary play, so a character
    //       who never ran them reads as unknown
    //   ==
    //   16
    assert_eq!(plan(&snapshot(), SystemTime::now(), MAX_AGE).len(), 16);
}

#[test]
fn a_freshly_stamped_character_is_asked_nothing() {
    // The point of persisting at all: a login that re-ran sixteen commands
    // regardless of the store would make the store pointless.
    let mut snapshot = snapshot();
    let now = SystemTime::now();
    for group in Group::ALL {
        snapshot.touch(group, now);
    }
    assert!(plan(&snapshot, now, MAX_AGE).is_empty());
}

#[test]
fn only_the_stale_groups_are_asked_for() {
    // Per-group staleness, which is where this differs from Lich: its store is
    // key/value with no notion of groups, so `db_refresh_needed?` re-syncs
    // everything or nothing.
    let mut snapshot = snapshot();
    let now = SystemTime::now();
    for group in Group::ALL {
        snapshot.touch(group, now);
    }
    // Standing went stale; the rest did not.
    snapshot.touch(Group::Standing, now - MAX_AGE - Duration::from_secs(1));

    let commands = plan(&snapshot, now, MAX_AGE);
    assert!(
        commands.iter().all(|(g, _)| *g == Group::Standing),
        "only Standing: {commands:?}"
    );
    assert_eq!(commands.len(), 4, "society, citizenship, warcry, resource");
}

#[test]
fn psms_need_all_six_tables() {
    // `refresh_command` answers with ONE command per group, which is right for
    // a menu and wrong for a sync: five of the six PSM tables would stay empty
    // while the group was stamped fresh.
    let mut snapshot = snapshot();
    let now = SystemTime::now();
    for group in Group::ALL {
        snapshot.touch(group, now);
    }
    snapshot.touch(Group::Psms, now - MAX_AGE - Duration::from_secs(1));

    let commands: Vec<&str> = plan(&snapshot, now, MAX_AGE)
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    assert_eq!(commands.len(), 6);
    for category in ["armor", "cman", "feat", "shield", "weapon", "ascension"] {
        assert!(
            commands.iter().any(|c| c.starts_with(category)),
            "{category} missing from {commands:?}"
        );
    }
}

#[test]
fn stats_and_identity_share_one_command() {
    // Both come from `info full`, and sending it twice would double the
    // traffic to learn nothing.
    let mut snapshot = snapshot();
    let now = SystemTime::now();
    for group in Group::ALL {
        snapshot.touch(group, now);
    }
    let stale = now - MAX_AGE - Duration::from_secs(1);
    snapshot.touch(Group::Stats, stale);
    snapshot.touch(Group::Identity, stale);

    let commands = plan(&snapshot, now, MAX_AGE);
    assert_eq!(
        commands.iter().filter(|(_, c)| *c == "info full").count(),
        2,
        "once per stale group -- the CALLER dedupes, and this records that it \
         has to: sending `info full` twice is the cost of not doing so"
    );
}

#[test]
fn a_clock_that_went_backwards_does_not_make_data_fresh() {
    // A corrected system clock, or a file copied from another machine.
    // `stale_groups` treats an un-subtractable stamp as stale rather than
    // trusting a timestamp it has just proved wrong.
    let mut snapshot = snapshot();
    let now = SystemTime::now();
    let future = now + Duration::from_hours(1);
    for group in Group::ALL {
        snapshot.touch(group, future);
    }
    assert_eq!(plan(&snapshot, now, MAX_AGE).len(), 16, "all of it");
}

#[test]
fn every_group_names_at_least_one_command() {
    // A group with no commands could never be taught and would be reported
    // stale forever.
    for group in Group::ALL {
        assert!(
            !group.sync_commands().is_empty(),
            "{group:?} has no sync command"
        );
    }
}
