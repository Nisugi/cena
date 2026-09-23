//! The wait itself; `mod.rs` says why.

use cena_session::{Event, State};
use tokio::sync::broadcast::Receiver;

/// Resolves once `events` has carried [`State::Ready`].
///
/// # Errors
///
/// A message, if the session ended first. A `Result` rather than a panic
/// because this is not a `#[test]` function, so the workspace's `panic` and
/// `expect_used` denials reach it.
pub async fn until_ready(mut events: Receiver<Event>) -> Result<(), String> {
    loop {
        match events.recv().await {
            Ok(Event::StateChanged(State::Ready)) => return Ok(()),
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(closed) => return Err(format!("the session ended before it was Ready: {closed}")),
        }
    }
}
