//! The table itself. See the crate docs for why it is shaped this way.

use std::collections::BTreeMap;
use std::time::Duration;

use cena_session::{
    Connector, SessionHandle, SessionId, SessionObserver, SupervisedEnd, SupervisedSession,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// How long a removed session's `quit` waits for the game to close.
///
/// The binary's own shutdown uses the same bound: the game normally closes
/// within a second, and a session that does not is ended anyway (`plan/16`
/// §5b) -- the bound only decides how long it gets to do it cleanly.
pub const QUIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Who a session is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Who {
    /// The account the character lives on. **One character per account can
    /// be online at a time, across every instance** (author, 2026-09-23).
    pub account: String,
    /// The character, as the player names it.
    pub character: String,
}

/// Why a session was not added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddError {
    /// The account already has a session running. Logging in a second
    /// character would end the first -- the game allows one per account -- so
    /// the table refuses rather than let the game knock it off.
    AccountInUse {
        /// The account asked for.
        account: String,
        /// The character already online on it.
        by: String,
    },
}

impl std::fmt::Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccountInUse { account, by } => write!(
                f,
                "{by} is already logged in on account {account}; the game allows one \
                 character per account at a time"
            ),
        }
    }
}

impl std::error::Error for AddError {}

/// One session in the table, running.
#[derive(Debug)]
pub struct Hosted {
    /// Who it is.
    pub who: Who,
    /// Commands to it. Clones are cheap and all reach the same session.
    pub handle: SessionHandle,
    /// Observation of it, for any number of frontends.
    pub observer: SessionObserver,
    cancel: CancellationToken,
    task: JoinHandle<SupervisedEnd>,
}

impl Hosted {
    /// Whether the session is still running. A session that stopped on its
    /// own -- a refused login, the retry ladder's cap -- stays in the table
    /// until removed, so a frontend can show why.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    /// End it in order: `quit`, then cancel, then wait for the task.
    ///
    /// `None` when the task did not end within [`QUIT_TIMEOUT`] after being
    /// cancelled, or had panicked: it is aborted, and there is no ending to
    /// report.
    pub async fn stop(self) -> Option<SupervisedEnd> {
        if self.is_running() {
            let _ = self.handle.quit(QUIT_TIMEOUT).await;
        }
        self.cancel.cancel();
        let abort = self.task.abort_handle();
        match tokio::time::timeout(QUIT_TIMEOUT, self.task).await {
            Ok(Ok(end)) => Some(end),
            Ok(Err(_)) => None,
            Err(_) => {
                abort.abort();
                None
            }
        }
    }
}

/// The sessions one Hydra runs.
#[derive(Debug, Default)]
pub struct Host {
    /// The next id to hand out. Owned here, never a global: `plan/12` §7.1.
    /// Seeded at 0 and only counts up, so an id is never reused within a run
    /// -- a frontend holding a removed session's id cannot reach a new one.
    next: u32,
    sessions: BTreeMap<SessionId, Hosted>,
}

impl Host {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a session for `who` over `connector`, and return its id.
    ///
    /// `prepare` is given the session before it runs, for what only the
    /// caller knows: its log, its stores, its combat recorder.
    ///
    /// # Errors
    ///
    /// [`AddError::AccountInUse`] when a running session already holds the
    /// account. Accounts compare without regard to case, as the game's do.
    pub fn add<C>(
        &mut self,
        who: Who,
        connector: C,
        prepare: impl FnOnce(SupervisedSession<C>) -> SupervisedSession<C>,
    ) -> Result<SessionId, AddError>
    where
        C: Connector + 'static,
        C::Source: 'static,
    {
        if let Some(holder) = self
            .sessions
            .values()
            .find(|s| s.is_running() && s.who.account.eq_ignore_ascii_case(&who.account))
        {
            return Err(AddError::AccountInUse {
                account: who.account,
                by: holder.who.character.clone(),
            });
        }
        let id = SessionId(self.next);
        self.next += 1;
        let (session, handle) = SupervisedSession::numbered(id, connector);
        let session = prepare(session);
        let observer = session.observer();
        let cancel = session.cancel_token();
        let task = tokio::spawn(session.run());
        self.sessions.insert(
            id,
            Hosted {
                who,
                handle,
                observer,
                cancel,
                task,
            },
        );
        Ok(id)
    }

    /// Every session, in the order they were added.
    pub fn sessions(&self) -> impl Iterator<Item = (SessionId, &Hosted)> {
        self.sessions.iter().map(|(id, hosted)| (*id, hosted))
    }

    /// One session.
    #[must_use]
    pub fn get(&self, id: SessionId) -> Option<&Hosted> {
        self.sessions.get(&id)
    }

    /// Remove a session from the table, to be [`Hosted::stop`]ped by the
    /// caller. Immediate: its account is free for a new session at once.
    pub fn take(&mut self, id: SessionId) -> Option<Hosted> {
        self.sessions.remove(&id)
    }

    /// Remove every session, in the order they were added.
    pub fn take_all(&mut self) -> Vec<Hosted> {
        std::mem::take(&mut self.sessions).into_values().collect()
    }
}

/// Stop every session in `hosted` at once, and wait for all of them.
///
/// Concurrently, because each can take up to [`QUIT_TIMEOUT`]: stopping 25
/// one after another could take minutes.
pub async fn stop_all(hosted: Vec<Hosted>) -> Vec<Option<SupervisedEnd>> {
    let mut stopping = tokio::task::JoinSet::new();
    for (order, one) in hosted.into_iter().enumerate() {
        stopping.spawn(async move { (order, one.stop().await) });
    }
    let mut ended = Vec::new();
    while let Some(done) = stopping.join_next().await {
        if let Ok(done) = done {
            ended.push(done);
        }
    }
    ended.sort_by_key(|(order, _)| *order);
    ended.into_iter().map(|(_, end)| end).collect()
}
