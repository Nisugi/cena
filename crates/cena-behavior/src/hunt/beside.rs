//! A cast sent beside what the desk is running (the author, 2026-09-29:
//! *";sc should run all the time if it's enabled"*).
//!
//! Every run on the desk replaces the one before it. That is right for a
//! hunt started over a hunt, and wrong for `;sc`: a bare `401` typed mid-hunt
//! (`;sc set typed` on, the default) or a trigger's `send = "401"` cancelled
//! the hunt to make room (the review of 2026-09-29). Lich's spellcaster ran
//! alongside bigshot. So while something runs, a cast's lines are sent here,
//! on the desk's own authority, and what runs is left alone.
//!
//! Each line goes through the hunt's own gate ([`Gate::Act`]: no roundtime,
//! not stunned, webbed or dead), so it cannot cut into the hunt's action. A
//! line refused for now is tried again, for up to [`WAIT`], because a cast
//! the player asked for should go when it can rather than be dropped the way
//! a hunt's own step is (the hunt decides again next turn; the player does
//! not).

use std::time::Duration;

use cena_session::command::{CommandId, Gate, Origin, Outcome};
use cena_session::{AuthorityToken, Frame, Notice, NoticeKind, SessionHandle};
use tokio_util::sync::CancellationToken;

use super::said::Ending;
use crate::hunt::drive::HuntEnd;
use crate::operation::{Steering, Underway};

/// How long a refused line is tried again before it is given up.
const WAIT: Duration = Duration::from_secs(10);

/// How long between tries.
const AGAIN: Duration = Duration::from_millis(250);

/// How long one line may take to be answered with a prompt.
const ANSWERED: Duration = Duration::from_secs(10);

/// Send `lines` beside the run under way, on `token`, each id from `ids`.
pub(super) fn cast(
    handle: SessionHandle,
    token: AuthorityToken,
    lines: Vec<String>,
    mut ids: impl FnMut() -> CommandId + Send + 'static,
) -> Underway<HuntEnd> {
    let stop = CancellationToken::new();
    let steering = Steering::new(stop.clone());
    let task = tokio::spawn(async move {
        for line in lines {
            if !send(&handle, token, &line, &mut ids, &stop).await {
                break;
            }
        }
        HuntEnd::Finished(Ending::Sent)
    });
    Underway { task, steering }
}

/// Send one line, trying again while it is refused; whether to go on.
async fn send(
    handle: &SessionHandle,
    token: AuthorityToken,
    line: &str,
    ids: &mut impl FnMut() -> CommandId,
    stop: &CancellationToken,
) -> bool {
    let until = tokio::time::Instant::now() + WAIT;
    loop {
        let outcome = tokio::select! {
            biased;
            () = stop.cancelled() => return false,
            outcome = handle.send_gated(
                ids(),
                line,
                Origin::Behavior(token),
                ANSWERED,
                |frame| matches!(frame, Frame::Prompt { .. }),
                Gate::Act { target: None },
            ) => outcome,
        };
        match outcome {
            Outcome::Refused(_) if tokio::time::Instant::now() < until => {
                tokio::time::sleep(AGAIN).await;
            }
            Outcome::Refused(why) => {
                handle.say(Notice::line(
                    NoticeKind::Warn,
                    format!("Sc: `{line}` was not sent: {why:?}."),
                ));
                return false;
            }
            _ => return true,
        }
    }
}
