//! Who may log the character out (the review of 2026-09-29, the author's
//! answer the same day: *"I think we allow it at higher permission levels"*).
//!
//! A `quit` took a shortcut past the authority gate, so a behavior without
//! the authority, or an agent at the `commands` level, could log the
//! character out; and a `quit` sent at once (Lich's `put`) was a plain write,
//! so the supervisor logged the character straight back in.

use std::time::Duration;

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::command::{Gate, Refusal, Sent};
use cena_session::{AuthorityToken, CommandId, Origin, Outcome, Session, SessionHandle};

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

/// A running session, and what reached its wire.
async fn started() -> (
    SessionHandle,
    TranscriptHandle,
    tokio_util::sync::CancellationToken,
) {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (handle, cancel) = (session.handle(), session.cancel_token());
    tokio::spawn(session.into_actor().run());
    tokio::time::sleep(Duration::from_millis(20)).await;
    (handle, transcript, cancel)
}

/// `quit` from `origin`, queued, and how it was answered and whether it
/// reached the wire; `None` if the authority could not be claimed first.
async fn quit_from(origin: Origin, claim: Option<AuthorityToken>) -> Option<(Outcome, bool)> {
    said_from("quit", origin, claim).await
}

/// [`quit_from`] for any `line`: whether it, or the session's own `quit`,
/// reached the wire.
async fn said_from(
    line: &str,
    origin: Origin,
    claim: Option<AuthorityToken>,
) -> Option<(Outcome, bool)> {
    let (handle, transcript, cancel) = started().await;
    if let Some(token) = claim {
        handle.claim(token).await.ok()?;
    }
    let outcome = handle
        .send_and_await(
            CommandId(1),
            line,
            origin,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    let sent = transcript
        .lines()
        .iter()
        .any(|written| written == "quit" || written == line);
    cancel.cancel();
    Some((outcome, sent))
}

/// The game takes `qui` as `quit` (the author, 2026-10-01): it does not get
/// past the gate from an origin that may not log the character out (the
/// crate review of 2026-10-01, L-2). `q`, `qu` and `ex` are not quits: the
/// game's parser needs three letters (the author, the same day).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_abbreviated_quit_is_checked_too() {
    let token = AuthorityToken(3);
    for line in ["qui", "QUI", "<c>qui", "exi"] {
        for (origin, claim) in [
            (Origin::Agent(None), None),
            (Origin::Behavior(token), Some(token)),
        ] {
            let (outcome, sent) = said_from(line, origin, claim).await.unwrap();
            assert_eq!(
                outcome,
                Outcome::Refused(Refusal::Permanent),
                "{line:?} from {origin:?}"
            );
            assert!(!sent, "{line:?} from {origin:?} reached the game");
        }
    }
    let (handle, transcript, cancel) = started().await;
    let refused = handle
        .send_now("qui", Origin::Behavior(token), Gate::None)
        .await;
    assert!(
        matches!(refused, Sent::Refused(Refusal::Permanent)),
        "{refused:?}"
    );
    assert!(
        !transcript
            .lines()
            .iter()
            .any(|line| line == "qui" || line == "quit"),
        "{:?}",
        transcript.lines()
    );
    cancel.cancel();
}

/// The player's `qui` is a quit the session asked for, so nothing
/// reconnects; a bare `q` from the player is the game's to read.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_players_qui_is_a_quit() {
    let (outcome, sent) = said_from("qui", Origin::Manual, None).await.unwrap();
    assert_eq!(outcome, Outcome::Disconnected);
    assert!(sent);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_player_their_scripts_and_triggers_may_quit() {
    for origin in [Origin::Manual, Origin::Script, Origin::Trigger] {
        let (outcome, sent) = quit_from(origin, None).await.unwrap();
        assert_eq!(outcome, Outcome::Disconnected, "{origin:?}");
        assert!(sent, "{origin:?}'s quit reached the game");
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_agent_may_quit_only_holding_the_authority() {
    let (outcome, sent) = quit_from(Origin::Agent(None), None).await.unwrap();
    assert_eq!(outcome, Outcome::Refused(Refusal::Permanent));
    assert!(
        !sent,
        "an agent at the commands level logged the character out"
    );

    let token = AuthorityToken(9);
    let (outcome, sent) = quit_from(Origin::Agent(Some(token)), Some(token))
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Disconnected, "a takeover may");
    assert!(sent);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_behavior_never_quits_even_holding_the_authority() {
    let token = AuthorityToken(3);
    for claim in [None, Some(token)] {
        let (outcome, sent) = quit_from(Origin::Behavior(token), claim).await.unwrap();
        assert_eq!(outcome, Outcome::Refused(Refusal::Permanent), "{claim:?}");
        assert!(!sent, "a behavior logged the character out: {claim:?}");
    }
}

/// Lich's `put quit` comes at once, not queued: it is a quit the session
/// knows it asked for, so nothing reconnects, and nothing more is sent.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_quit_sent_at_once_is_a_quit_and_is_checked_too() {
    let (handle, transcript, cancel) = started().await;
    let refused = handle
        .send_now("quit", Origin::Behavior(AuthorityToken(3)), Gate::None)
        .await;
    assert!(
        matches!(refused, Sent::Refused(Refusal::Permanent)),
        "{refused:?}"
    );
    assert!(!transcript.lines().iter().any(|line| line == "quit"));

    let sent = handle.send_now("quit", Origin::Lich, Gate::None).await;
    assert!(
        matches!(sent, Sent::Dead),
        "the connection is ending: {sent:?}"
    );
    assert!(transcript.lines().iter().any(|line| line == "quit"));
    let after = handle
        .send_and_await(
            CommandId(2),
            "look",
            Origin::Manual,
            Duration::from_millis(200),
            cena_session::queue::any_frame,
        )
        .await;
    assert_eq!(after, Outcome::Disconnected, "the session is leaving");
    cancel.cancel();
}
