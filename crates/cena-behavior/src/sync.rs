//! Teaching the client a character it has never seen.
//!
//! # Why a sync exists at all
//!
//! The parser keeps a character's facts current from ordinary play -- you
//! train, the game says so, the store follows. But a gameplay message only
//! reports a **change**:
//!
//! > **AUTHOR, 2026-09-20:** *"training updates, and things do get emitted and
//! > they get picked up by the parser automatically."*
//!
//! A character who trained before this client existed never emits those lines
//! again. So the sync is a **bootstrap**, not the source of truth: it runs the
//! commands that state everything at once, and after that the incremental
//! classifiers keep it right.
//!
//! # What it asks for, and what it does not
//!
//! Only the groups the store says are stale. A character whose stats were
//! taught last week and whose society changed today re-reads one group, not
//! six. `CharacterSnapshot::stale_groups` answers that, and a group with no
//! timestamp is always stale -- which is what makes the first login sync
//! everything.
//!
//! This is Lich's own shape (`infomon/cli.rb`), with one difference worth
//! naming: Lich decides with `db_refresh_needed?` (`cli.rb:73-80`) and then
//! syncs **everything**, because its store is a key/value table with no notion
//! of groups. Per-group staleness is cheaper and is already in the snapshot.
//!
//! # Quiet
//!
//! > **AUTHOR, 2026-09-20:** *"if not, then we run through a set of commands
//! > quietly capturing all the data."*
//!
//! Lich says the same thing in a comment -- *"since none of this information
//! is 3rd party displayed, silence is golden"* (`cli.rb:8`). Nothing here
//! suppresses output, and that is deliberate: this crate sends commands and
//! does not render, so "quiet" is a frontend concern. What this owes the
//! frontend is a way to know the traffic was ours, which [`Origin::Behavior`]
//! already provides on every command.

use std::time::Duration;

use cena_session::command::{CommandId, Origin, Outcome};
use cena_session::queue::AuthorityToken;
use cena_session::{CharacterSnapshot, Frame, Group, SessionHandle};
use tokio_util::sync::CancellationToken;

use crate::BehaviorError;

/// How long a sync command may take to answer.
///
/// Lich uses `timeout: 5` per command (`infomon/cli.rb:37`). This is longer
/// because a miss here is not free: an unanswered `skills full` leaves the
/// group unstamped and the next login asks again, so it is better to wait than
/// to give up early on a slow server.
pub const SYNC_DEADLINE: Duration = Duration::from_secs(10);

/// How stale a group may be before a sync re-reads it.
///
/// Re-exported from `cena-session` rather than defined twice: the session
/// decides staleness when it loads the store and publishes
/// `Event::SyncNeeded`, so a second copy here could disagree with the answer
/// a caller was already given.
pub use cena_session::MAX_STALE as MAX_AGE;

/// What a sync would send, in order, for one stored snapshot: each command
/// **once**.
///
/// Separate from running it so a caller can ask **what this would cost** --
/// fifteen commands at up to ten seconds each is a visible amount of game
/// traffic, and a frontend that wants to say so should not have to run it to
/// find out. `stale_groups` is a pure function of the snapshot and the clock.
///
/// # One command, several groups
///
/// `info full` teaches both [`Group::Stats`] and [`Group::Identity`]
/// (`Group::sync_commands`), and either can be stale alone. This planned it
/// once per stale group and left the duplicate for "the caller" to drop --
/// and no caller did, so a full sync sent `info full` twice, the exact
/// doubling `sync_commands`' own comment says it exists to prevent (review,
/// 2026-09-23). The plan is the one place that sees every group at once, so
/// it drops it here.
///
/// The [`Group`] beside a command is the **first** stale group that asks for
/// it. A caller stamping groups fresh after a sync should stamp every stale
/// group whose `sync_commands` were all sent, not read this column as the
/// only group a command taught.
#[must_use]
pub fn plan(
    snapshot: &CharacterSnapshot,
    now: std::time::SystemTime,
    max_age: Duration,
) -> Vec<(Group, &'static str)> {
    let mut planned: Vec<(Group, &'static str)> = Vec::new();
    for group in snapshot.stale_groups(now, max_age) {
        for command in group.sync_commands() {
            if !planned.iter().any(|(_, already)| already == command) {
                planned.push((group, command));
            }
        }
    }
    planned
}

/// Matches the prompt that ends a command's output.
///
/// Every sync command is a report terminated by a prompt, which is what Lich
/// captures to (`issue_command(cmd, start_capture, /<prompt/)`). The model
/// reads the report out of the chunk the prompt closes, so this behavior only
/// has to wait for it -- it never looks at the content, which is the whole
/// point of `plan/12` §3a's split.
fn ends_a_report(frame: &Frame) -> bool {
    matches!(frame, Frame::Prompt { .. })
}

/// Run the commands that teach whatever the store says is stale.
///
/// Returns how many commands were sent. Zero is the ordinary case for a
/// character synced recently, and it is **not** a failure.
///
/// # Errors
///
/// [`BehaviorError::AuthorityHeld`] if a behavior is already running -- a sync
/// is a sequence and holds the authority for all of it, per `plan/12` §4.2.
/// [`BehaviorError::Cancelled`] on `stop`, [`BehaviorError::Dead`] or
/// [`BehaviorError::Disconnected`] if the session goes away mid-sync.
pub async fn sync(
    handle: &SessionHandle,
    cancel: &CancellationToken,
    mut next_id: impl FnMut() -> CommandId,
    token: AuthorityToken,
    commands: &[(Group, &'static str)],
) -> Result<usize, BehaviorError> {
    if commands.is_empty() {
        return Ok(0);
    }
    handle
        .claim(token)
        .await
        .map_err(|_held| BehaviorError::AuthorityHeld)?;
    let result = sync_holding_authority(handle, cancel, &mut next_id, token, commands).await;
    // Released on EVERY exit, including cancellation and a dead session.
    handle.release(token);
    result
}

/// The loop, with the authority already held.
///
/// Split out so `sync` releases on every path without a `defer`-shaped guard,
/// exactly as `look` does.
async fn sync_holding_authority(
    handle: &SessionHandle,
    cancel: &CancellationToken,
    next_id: &mut impl FnMut() -> CommandId,
    token: AuthorityToken,
    commands: &[(Group, &'static str)],
) -> Result<usize, BehaviorError> {
    let mut sent = 0;
    for (_group, command) in commands {
        // Checked before the send, so a sync started with an already-cancelled
        // token sends nothing (`plan/12` §4.3).
        if cancel.is_cancelled() {
            return Err(BehaviorError::Cancelled);
        }

        // Cancellation wins over the await, for the reason `look` documents at
        // length: awaiting `send_and_await` bare makes `stop` wait for the
        // game, and criterion 4's budget is 250ms against a 10s deadline.
        let outcome = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(BehaviorError::Cancelled),
            outcome = handle.send_and_await(
                next_id(),
                command,
                Origin::Behavior(token),
                SYNC_DEADLINE,
                ends_a_report,
            ) => outcome,
        };

        // §5.1: no automation runs while a session has no transport.
        if let Some(gone) = BehaviorError::from_outcome(&outcome) {
            return Err(gone);
        }
        match outcome {
            // `Timeout` is counted as sent, and that is deliberate: §4.4 says
            // it means "no match within the window", never "the command did
            // not happen". The report may have arrived without a prompt we
            // recognised, and the model reads reports for itself -- so what
            // the group learned is the model's answer, not this loop's.
            Outcome::Confirmed(_) | Outcome::Timeout => sent += 1,
            // A refusal is the queue being full, which a sync should not push
            // against: the rest of the commands would refuse too. (What is
            // left was answered above.)
            _ => return Ok(sent),
        }
    }
    Ok(sent)
}
