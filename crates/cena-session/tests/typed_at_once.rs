//! What a person types is written at once, not one command a round trip.
//!
//! The author, 2026-09-30, holding the `look` key: *"on hydra I can hold it
//! down, let up and it takes it 3-4 seconds to catch up or more. That is
//! entirely unacceptable."* MEASURED in that session's player log: 92 `look`s
//! went out over 12.7 s, each stamped the millisecond the reply before it
//! arrived, because a typed line queued behind the open window of the last
//! one (`CommandQueue::take_next` yields nothing while a window is open).
//! `VellumFE` and Lich write a typed line when it is typed.

use cena_platform::AnsweringSource;
use cena_session::{Outcome, Session};
use std::time::Duration;

/// A reply with no prompt: a window opened against it never closes.
const SILENT: &[u8] = b"You see nothing unusual.\n";

/// Ten typed lines reach the wire though the game has answered none of them
/// with a prompt. Queued behind a window, the second would never go.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn typed_lines_do_not_wait_for_each_others_prompts() {
    let (source, transcript) = AnsweringSource::new(SILENT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let driver = tokio::spawn(session.into_actor().run());

    let generation = handle.generation();
    for _ in 0..10 {
        let outcome = handle
            .send_typed_at(generation, "look", Duration::from_secs(5))
            .await;
        assert_eq!(outcome, Outcome::Sent);
    }
    let sent = transcript.lines();
    assert_eq!(sent.len(), 10, "{sent:?}");

    cancel.cancel();
    let _ = driver.await;
}
