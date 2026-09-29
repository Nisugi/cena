//! The files the actor writes, written off its task.
//!
//! A save is a file read, a write, an `fsync` of the file and a rename
//! ([`crate::store::save_text`]): milliseconds on a quiet disk, far more on a
//! busy one, and every millisecond of it held the actor, so the game's bytes,
//! the player's typing and every behavior waited on the disk (the crate
//! review of 2026-09-28; the database recorders were moved off the task for the
//! same reason). Each is now a blocking job on tokio's blocking pool.
//!
//! **One after another, in the order asked.** Two saves of one file must not
//! race: the five-minute save still running when the session ends would
//! otherwise land after the final one and put older facts over newer. So
//! each job waits for the one before it. What a job has to say comes back to
//! the actor to log ([`Saves::heard`]), since the log is the actor's.

use std::sync::mpsc;

use tokio::task::JoinHandle;

/// The actor's saves, queued in order and made off its task.
#[derive(Debug)]
pub(crate) struct Saves {
    /// The last save asked for; each waits for the one before it.
    last: Option<JoinHandle<()>>,
    tell: mpsc::Sender<String>,
    told: mpsc::Receiver<String>,
}

impl Default for Saves {
    fn default() -> Self {
        let (tell, told) = mpsc::channel();
        Self {
            last: None,
            tell,
            told,
        }
    }
}

impl Saves {
    /// Make `write` once every save asked for before it is made. What it
    /// returns is a line for the log.
    pub(crate) fn queue(&mut self, write: impl FnOnce() -> Option<String> + Send + 'static) {
        let earlier = self.last.take();
        let tell = self.tell.clone();
        self.last = Some(tokio::spawn(async move {
            if let Some(earlier) = earlier {
                let _ = earlier.await;
            }
            if let Ok(Some(line)) = tokio::task::spawn_blocking(write).await {
                let _ = tell.send(line);
            }
        }));
    }

    /// What the saves made so far have to say, for the log.
    pub(crate) fn heard(&self) -> Vec<String> {
        self.told.try_iter().collect()
    }

    /// Wait until every save asked for is made: the session is ending, and
    /// what it learned is on disk before it says so.
    pub(crate) async fn finish(&mut self) {
        if let Some(last) = self.last.take() {
            let _ = last.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Saves;
    use std::sync::{Arc, Mutex};

    /// A slow save asked for first is still made first: the order is the
    /// order asked, whatever each one costs.
    #[tokio::test(flavor = "current_thread")]
    async fn saves_are_made_in_the_order_asked_and_waited_for() {
        let mut saves = Saves::default();
        let made = Arc::new(Mutex::new(Vec::new()));
        let first = Arc::clone(&made);
        saves.queue(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            first.lock().unwrap().push(1);
            Some("first".to_owned())
        });
        let second = Arc::clone(&made);
        saves.queue(move || {
            second.lock().unwrap().push(2);
            None
        });
        saves.finish().await;
        assert_eq!(*made.lock().unwrap(), [1, 2]);
        assert_eq!(saves.heard(), ["first"], "only what a save said is heard");
    }
}
