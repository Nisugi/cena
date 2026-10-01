//! How a run on the desk ends, whichever way it ends: returned, cut off,
//! or **panicked** (the crate review of 2026-10-01, BI-C-1).
//!
//! The release and the desk's tidying were plain statements after the run's
//! await, so a panic anywhere in a hunt, heal, keep, waggle or `;sc` run
//! skipped them: the hunt token kept the session's authority for the rest of
//! the session (every later behavior refused, `;go2` included), the next
//! desk command waited for ever on the dead run's `over`, and the player was
//! told nothing. The workspace unwinds on a panic (`Cargo.toml`'s
//! `panic = "unwind"`) so that one task dies and not the process; a guard's
//! `Drop` runs on that unwinding, so the ending lives in one.

use std::sync::{Arc, PoisonError};

use cena_session::{Notice, NoticeKind, SessionHandle};
use tokio_util::sync::CancellationToken;

use super::Desk;

/// A run's ending, done when it is dropped: the authority let go, the hunt
/// panel cleared, the desk's slot freed and `over` cancelled, so the next
/// run starts. A run that panicked is said to the player.
pub(super) struct Finish {
    desk: Arc<Desk>,
    handle: SessionHandle,
    number: u64,
    over: CancellationToken,
    what: String,
}

impl Finish {
    /// The ending of the desk's run `number`, named `what` to the player.
    pub(super) fn new(
        desk: &Arc<Desk>,
        handle: &SessionHandle,
        number: u64,
        over: CancellationToken,
        what: &str,
    ) -> Self {
        Self {
            desk: Arc::clone(desk),
            handle: handle.clone(),
            number,
            over,
            what: what.to_owned(),
        }
    }
}

impl Drop for Finish {
    fn drop(&mut self) {
        // A token that does not hold the authority (the claim was refused)
        // is ignored by the session (`SessionHandle::release`).
        self.handle.release(self.desk.token);
        if std::thread::panicking() {
            self.handle.say(
                Notice::line(
                    NoticeKind::Error,
                    format!(
                        "Hunt: {} failed: a fault in Hydra stopped it, and the character is free again.",
                        self.what
                    ),
                )
                .answering(),
            );
        }
        // Before `over`: the next run, waiting on it, reports after this.
        self.desk.reports.tell(None);
        let mut slot = self
            .desk
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if slot.as_ref().is_some_and(|run| run.number == self.number) {
            *slot = None;
        }
        drop(slot);
        self.over.cancel();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use cena_map::Map;
    use cena_platform::AnsweringSource;
    use cena_session::{AuthorityToken, Event, Session, State};

    use super::super::Running;
    use super::*;

    const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
    const TOKEN: AuthorityToken = AuthorityToken(7);

    /// A run that panics while it holds the authority lets it go, frees the
    /// desk for the next run, and says so: before, the token held the
    /// session for good and the next desk command waited for ever.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_run_that_panics_lets_go_and_frees_the_desk() {
        let (source, _transcript) = AnsweringSource::logged_in(PROMPT);
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        tokio::spawn(session.into_actor().run());
        while !matches!(events.recv().await, Ok(Event::StateChanged(State::Ready))) {}
        assert!(handle.claim(TOKEN).await.is_ok(), "free");

        let map = Map::from_rooms(Vec::new()).expect("an empty map");
        let desk = Desk::new(Arc::new(map), PathBuf::new(), TOKEN);
        let running = Running {
            number: 0,
            stop: CancellationToken::new(),
            over: CancellationToken::new(),
        };
        *desk.running.lock().unwrap_or_else(PoisonError::into_inner) = Some(running.clone());
        let task = {
            let (desk, handle, over) = (Arc::clone(&desk), handle.clone(), running.over.clone());
            tokio::spawn(async move {
                let _finish = Finish::new(&desk, &handle, 0, over, "hunt");
                panic!("a fault in the run");
            })
        };
        let joined = task.await;
        assert!(joined.is_err_and(|e| e.is_panic()), "the run panicked");

        assert!(
            running.over.is_cancelled(),
            "the next run is not kept waiting"
        );
        assert!(!desk.stop(), "nothing is left on the desk");
        // The release goes through the session's inbox: give it a turn.
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(handle.holder(), None, "the authority was let go");
        let mut told = false;
        while let Ok(event) = events.try_recv() {
            if let Event::Notice(notice) = event {
                told |= notice.lines().iter().any(|l| l.contains("hunt failed"));
            }
        }
        assert!(told, "the player is told the run failed");
    }
}
