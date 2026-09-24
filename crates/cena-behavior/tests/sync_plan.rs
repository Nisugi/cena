//! The login sync: what it asks for, and what it leaves alone.
//!
//! `plan` is a pure function of a snapshot and a clock, so most of this needs
//! no session at all. The running half, `sync`, is exercised against a real
//! actor at the bottom.
//!
//! > **CORRECTED 2026-09-23.** This header promised that running half, and
//! > the file ended without it: `sync` had no caller and no test. A claim of
//! > coverage that resolves to nothing is a false negative waiting to be
//! > believed.

mod ready;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use cena_behavior::BehaviorError;
use cena_behavior::sync::{MAX_AGE, commands_for, plan, sync};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{
    AuthorityToken, CharacterSnapshot, CommandId, Event, GenerationCell, Group, Session,
    SessionHandle,
};
use tokio_util::sync::CancellationToken;

fn snapshot() -> CharacterSnapshot {
    CharacterSnapshot::new("GS", "Nisugi")
}

#[test]
fn a_character_nobody_has_synced_is_asked_everything() {
    // What makes the first login work. A group with no timestamp is always
    // stale, so an empty snapshot plans every command.
    //
    // Asked of the COMMANDS, not the group column: `Identity` shares `info
    // full` with `Stats` and is planned under it, so a group can be taught
    // without ever being named there (`plan`'s docs).
    let commands = plan(&snapshot(), SystemTime::now(), MAX_AGE);
    let planned: Vec<&str> = commands.iter().map(|(_, c)| *c).collect();
    for group in Group::ALL {
        for command in group.sync_commands() {
            assert!(
                planned.contains(command),
                "{group:?}'s {command:?} was not planned"
            );
        }
    }
}

#[test]
fn a_full_sync_is_fifteen_commands() {
    // Pinned so a group gaining a command is a visible change to the cost of
    // a login rather than a silent one.
    //
    // MEASURED, because the number was wrong twice before this test existed.
    // Lich sends **15**:
    //
    // ```text
    // $ awk "/request = \{/,/\}$/" reference/lich-5/lib/gemstone/infomon/cli.rb     //     | grep -coE "'[a-z ]+'\s+=>"
    // 15
    // ```
    //
    // Fifteen here too, by a different sum -- every difference accounted for
    // rather than assumed:
    //
    //   15  Lich's list
    //   -3  groups this model does not read yet: `spell`, `experience` (the
    //       report is read but not persisted -- see `Group`, which has no
    //       `Experience`), `profile full`
    //   +1  `inventory enhancive totals`, for `Group::Enhancives`
    //   +2  `wealth` and `tickets`, which Lich does not sync at all: its
    //       currency keys are filled only from ordinary play, so a character
    //       who never ran them reads as unknown
    //   ==
    //   15
    //
    // > CORRECTED 2026-09-23. This was sixteen, and its sum said `info full`
    // > was planned twice "because Stats and Identity are separately
    // > staleable" -- counting as +2 a duplicate that is +1, and leaving the
    // > enhancive report out so the total came right. The duplicate was a
    // > defect (`stats_and_identity_share_one_command`), and `plan` now drops
    // > it.
    assert_eq!(plan(&snapshot(), SystemTime::now(), MAX_AGE).len(), 15);
}

#[test]
fn a_freshly_stamped_character_is_asked_nothing() {
    // The point of persisting at all: a login that re-ran fifteen commands
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
        commands,
        [(Group::Stats, "info full")],
        "one report teaches both groups, so it is sent once. This asserted TWO \
         and said the caller dedupes; no caller did (review, 2026-09-23)"
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
    assert_eq!(plan(&snapshot, now, MAX_AGE).len(), 15, "all of it");
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

// ---------------------------------------------------------------------------
// The running half: `sync`, against a real session.
// ---------------------------------------------------------------------------

/// Every sync command is a report closed by a prompt.
const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

fn ids() -> impl FnMut() -> CommandId {
    let next = Arc::new(AtomicU64::new(0));
    move || CommandId(next.fetch_add(1, Ordering::Relaxed))
}

/// A running session that answers every command with a prompt. The
/// generation cell comes back for the test that moves the connection under a
/// sync.
async fn a_session() -> Result<
    (
        SessionHandle,
        TranscriptHandle,
        CancellationToken,
        GenerationCell,
    ),
    String,
> {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, ready) = session.subscribe();
    let handle = session.handle();
    let cell = session.generation_cell();
    let session_cancel = session.cancel_token();
    tokio::spawn(session.into_actor().run());
    ready::until_ready(ready).await?;
    Ok((handle, transcript, session_cancel, cell))
}

/// A character whose stats and identity are stale: one command between them.
fn stats_and_identity_stale() -> Vec<(Group, &'static str)> {
    let mut snapshot = snapshot();
    let now = SystemTime::now();
    for group in Group::ALL {
        snapshot.touch(group, now);
    }
    let stale = now - MAX_AGE - Duration::from_secs(1);
    snapshot.touch(Group::Stats, stale);
    snapshot.touch(Group::Identity, stale);
    plan(&snapshot, now, MAX_AGE)
}

/// What is planned is what goes on the wire, once, and the authority is free
/// again afterwards -- the property the next behavior depends on.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_sync_sends_what_was_planned_once_and_lets_go() {
    let (handle, transcript, session, _) = a_session().await.expect("the session becomes Ready");
    let commands = stats_and_identity_stale();
    let stop = CancellationToken::new();

    let sent = sync(&handle, &stop, ids(), AuthorityToken(1), &commands).await;

    assert_eq!(sent, Ok(1));
    assert_eq!(
        transcript.lines(),
        ["info full"],
        "once, not once per group"
    );
    assert!(
        handle.claim(AuthorityToken(2)).await.is_ok(),
        "the authority was not released"
    );
    session.cancel();
}

/// A full sync, end to end: every planned command, in order, and nothing
/// else.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_full_sync_sends_every_planned_command_in_order() {
    let (handle, transcript, session, _) = a_session().await.expect("the session becomes Ready");
    let commands = plan(&snapshot(), SystemTime::now(), MAX_AGE);
    let stop = CancellationToken::new();

    let sent = sync(&handle, &stop, ids(), AuthorityToken(1), &commands).await;

    assert_eq!(sent, Ok(commands.len()));
    let planned: Vec<&str> = commands.iter().map(|(_, command)| *command).collect();
    assert_eq!(transcript.lines(), planned);
    session.cancel();
}

/// Stopped before it starts, it sends nothing (`plan/12` §4.3), and still
/// lets the authority go.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_sync_stopped_before_it_starts_sends_nothing() {
    let (handle, transcript, session, _) = a_session().await.expect("the session becomes Ready");
    let stop = CancellationToken::new();
    stop.cancel();
    let commands = stats_and_identity_stale();

    let sent = sync(&handle, &stop, ids(), AuthorityToken(1), &commands).await;

    assert_eq!(sent, Err(BehaviorError::Cancelled));
    assert_eq!(transcript.written_count(), 0);
    assert!(handle.claim(AuthorityToken(2)).await.is_ok());
    session.cancel();
}

/// The connection changes under the sync: the actor discards its command as
/// an older connection's and answers `Interrupted`. That is a disconnection,
/// not the player's stop.
///
/// Not the test that pins the `Interrupted` arm: `cena-session` now answers
/// a stale command `Disconnected` at admission (`actor/io.rs`), so this
/// reaches that first. MEASURED: it stayed green with the old
/// `Interrupted -> Cancelled` arm restored. `BehaviorError::from_outcome`'s
/// unit test pins the arm; this pins the path end to end.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_from_an_older_connection_is_a_disconnection() {
    let (handle, transcript, session, cell) = a_session().await.expect("the session becomes Ready");
    // What a supervisor does between connections. This actor keeps the old
    // generation, so everything the handle stamps from now on is stale.
    cell.advance();
    let commands = stats_and_identity_stale();
    let stop = CancellationToken::new();

    let sent = sync(&handle, &stop, ids(), AuthorityToken(1), &commands).await;

    assert_eq!(sent, Err(BehaviorError::Disconnected));
    assert_eq!(transcript.written_count(), 0, "discarded, never written");
    session.cancel();
}

/// `commands_for` is `plan` for groups a caller already has -- what
/// `Event::SyncNeeded` carries -- and plans the same commands.
#[test]
fn planning_from_groups_matches_planning_from_a_snapshot() {
    assert_eq!(
        commands_for(&Group::ALL),
        plan(&snapshot(), SystemTime::now(), MAX_AGE)
    );
}

/// Quiet, like infomon (author, 2026-09-24): every command goes out quietly,
/// so its report stays out of the story, and Hydra says one line per stage --
/// a start, one per command naming it, and an end -- so the player knows
/// what is running and that it is not stuck.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_sync_is_quiet_and_says_each_stage() {
    let (source, _transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, ready) = session.subscribe();
    let (_, mut events) = session.subscribe();
    let handle = session.handle();
    let session_cancel = session.cancel_token();
    tokio::spawn(session.into_actor().run());
    ready::until_ready(ready)
        .await
        .expect("the session becomes Ready");
    let commands = commands_for(&[Group::Stats, Group::Skills]);
    assert_eq!(commands.len(), 2);

    let sent = sync(
        &handle,
        &CancellationToken::new(),
        ids(),
        AuthorityToken(1),
        &commands,
    )
    .await;
    assert_eq!(sent, Ok(2));

    let (mut said, mut quiet_windows) = (Vec::new(), 0);
    while let Ok(event) = events.try_recv() {
        match event {
            Event::Notice(notice) => said.extend(notice.lines().iter().cloned()),
            Event::Quiet(true) => quiet_windows += 1,
            _ => {}
        }
    }
    assert_eq!(quiet_windows, 2, "every sync command goes out quietly");
    assert_eq!(said.len(), 4, "a start, one per command, an end: {said:?}");
    assert!(said[1].contains("(1 of 2): info full"), "{said:?}");
    assert!(said[2].contains("(2 of 2): skills full"), "{said:?}");
    assert!(said[3].contains("2 of 2"), "{said:?}");
    session_cancel.cancel();
}
