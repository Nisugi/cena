//! The character sync, run once a login is `Ready` (`plan/30` §2).
//!
//! The session reads the character store as the login names the character,
//! and reports the stale groups in `Event::SyncNeeded` -- inside the login
//! burst, before `Ready`, when a behavior's command would be refused. So this
//! listens from before the session runs, keeps what that event said, and
//! syncs after `Ready`: quietly, with a line per stage
//! (`cena_behavior::sync`).
//!
//! Once per session, not per connection: the store is read on the first
//! login only (`load_character`), so a reconnect reports nothing new.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cena_session::{AuthorityToken, CommandId, Event, Group, SessionHandle, State};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// The sync's claim on the command authority. Travel's desk is `2`
/// (`travel.rs`); the two only have to differ.
const SYNC_TOKEN: AuthorityToken = AuthorityToken(3);

/// Wait for `Ready`, keeping what the store said was stale on the way.
///
/// `None` if the session ended first. Taken from a subscription made before
/// the session ran, so `SyncNeeded` -- which fires mid-burst -- cannot be
/// missed by subscribing late.
pub(crate) async fn stale_at_ready(events: &mut broadcast::Receiver<Event>) -> Option<Vec<Group>> {
    let mut stale = Vec::new();
    loop {
        match events.recv().await {
            Ok(Event::SyncNeeded(groups)) => stale = groups,
            Ok(Event::StateChanged(State::Ready)) => return Some(stale),
            Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return None,
        }
    }
}

/// Sync `stale`, if there is anything to sync. What it said is on the
/// session's own notices; only a failure is added here.
pub(crate) async fn sync(handle: &SessionHandle, stale: &[Group], who: &str) {
    let commands = cena_behavior::commands_for(stale);
    if commands.is_empty() {
        return;
    }
    let next = Arc::new(AtomicU64::new(0));
    let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
    // Nobody stops a sync but the session ending, which answers every
    // command on its own; the token is the behavior's contract, not a switch.
    let stop = CancellationToken::new();
    if let Err(why) = cena_behavior::sync(handle, &stop, ids, SYNC_TOKEN, &commands).await {
        eprintln!("{who} !! the character sync stopped: {why:?}");
    }
}
