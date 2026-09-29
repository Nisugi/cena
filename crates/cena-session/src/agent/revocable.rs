//! An agent's line that can still be taken back: until the session writes
//! it, a stop or a lowered level takes it back, and once written nothing
//! can (the integrated crate review of 2026-09-28, I1).
//!
//! **The check is at the write**, where the queue's other fences are (`plan/12`
//! §4.3's preempted holder, §5.2's generation): the session marks the line
//! written as its bytes go out, and a stop marks it taken back, and whichever
//! comes first is what happened. The operation only reported a line "already
//! the game's" the moment it was admitted, so a stop found nothing to stop
//! while the line still waited to be written, and wrote it after.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use tokio_util::sync::CancellationToken;

const WAITING: u8 = 0;
const WRITTEN: u8 = 1;
const TAKEN_BACK: u8 = 2;

/// One agent line's fate: waiting to be written, written, or taken back.
/// Cloned between the operation, the session's writer and the agent's list
/// of lines still waiting.
#[derive(Clone, Debug, Default)]
pub struct Revocable(Arc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    state: AtomicU8,
    taken_back: CancellationToken,
    why: OnceLock<&'static str>,
}

impl Revocable {
    /// The writer's side, as the bytes go out: `true` when the line may be
    /// written, and is now marked written; `false` when it was taken back
    /// first.
    pub(crate) fn write(&self) -> bool {
        let state = &self.0.state;
        match state.compare_exchange(WAITING, WRITTEN, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => true,
            Err(now) => now == WRITTEN,
        }
    }

    /// Take the line back, for `why`: `true` when it was still waiting, and
    /// now never will be written; `false` when it was written already, or
    /// taken back before.
    pub(crate) fn take_back(&self, why: &'static str) -> bool {
        let state = &self.0.state;
        let taken = state
            .compare_exchange(WAITING, TAKEN_BACK, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        if taken {
            let _ = self.0.why.set(why);
            self.0.taken_back.cancel();
        }
        taken
    }

    /// Whether the session wrote it.
    pub(crate) fn written(&self) -> bool {
        self.0.state.load(Ordering::Acquire) == WRITTEN
    }

    /// Whether it still waits to be written.
    pub(crate) fn waiting(&self) -> bool {
        self.0.state.load(Ordering::Acquire) == WAITING
    }

    /// Resolves once it is taken back, with why.
    pub(crate) async fn taken_back(&self) -> &'static str {
        self.0.taken_back.cancelled().await;
        self.0.why.get().copied().unwrap_or("stopped")
    }
}

#[cfg(test)]
mod tests {
    use super::Revocable;

    /// Whichever comes first is what happened: a line taken back is never
    /// written, and one written is never taken back.
    #[test]
    fn the_write_and_the_taking_back_exclude_each_other() {
        let line = Revocable::default();
        assert!(line.waiting());
        assert!(line.take_back("stopped"));
        assert!(!line.write(), "taken back: never written");
        assert!(!line.take_back("stopped"), "once");

        let line = Revocable::default();
        assert!(line.write());
        assert!(line.write(), "written stays written");
        assert!(!line.take_back("stopped"), "written: too late");
        assert!(line.written());
    }
}
