//! The batch desk: one `;multi`, or one `;foreach`, at a time per session.
//!
//! A desk per kind, so a `;multi` can run a `;foreach` as one of its
//! commands and the other way round -- different Lich scripts could run each
//! other -- while a second of the same kind is refused, as Lich refused to
//! start a script already running and `VellumFE` refuses a second
//! `.foreach` (`src/core/app_core/commands.rs:1659-1668`). The hunt and
//! travel desks replace what is running instead; a list of a hundred items
//! half done is not something a second command should end without being
//! asked.
//!
//! It claims the authority, runs the batch with a [`watch`] beside it, and
//! releases on every exit, as the hunt desk does (`hunt/desk.rs`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use cena_session::{AuthorityToken, Notice, NoticeKind, SessionHandle, Snapshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::build;
use super::command::Job;
use super::drive::{Driver, Halt};
use super::foreach::Foreach;
use super::line::Hydra;
use super::multi::Multi;
use super::pick::{filtered, listing, select};
use crate::error::BehaviorError;
use crate::travel::Heard;
use crate::watchdog::{BEHAVIOR_WATCHDOG, Heartbeat, watch};

/// How often a `;foreach` says how far it has got: every tenth item, from
/// the first (foreach.lic's `ITEM_UPDATE_INTERVAL`, `:872`).
pub const EVERY: usize = 10;

/// One kind of batch's desk, for one session.
pub struct Desk {
    /// `Multi` or `Foreach`.
    name: &'static str,
    token: AuthorityToken,
    running: Mutex<Option<Running>>,
    ids: AtomicU64,
    runs: AtomicU64,
}

/// A batch under way: how to stop it.
#[derive(Clone)]
struct Running {
    number: u64,
    stop: CancellationToken,
}

impl Desk {
    /// A desk with nothing running. `name` is how it is said (`Multi`);
    /// `token` is the authority its batches claim.
    #[must_use]
    pub fn new(name: &'static str, token: AuthorityToken) -> Arc<Desk> {
        Arc::new(Desk {
            name,
            token,
            running: Mutex::new(None),
            // Clear of the ids the other desks use.
            ids: AtomicU64::new(200_000),
            runs: AtomicU64::new(0),
        })
    }

    /// Stop the batch under way. `false` when there is none.
    pub fn stop(&self) -> bool {
        let running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        running
            .as_ref()
            .filter(|run| !run.stop.is_cancelled())
            .is_some_and(|run| {
                run.stop.cancel();
                true
            })
    }

    /// Run `job` for the session behind `handle`, from a fresh subscription,
    /// unless one is already running. `hydra` runs its Hydra commands.
    pub fn run(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        job: Job,
        hydra: Hydra,
    ) -> Option<JoinHandle<Result<(), Halt>>> {
        let running = {
            let mut slot = self.running.lock().unwrap_or_else(PoisonError::into_inner);
            if slot.is_some() {
                handle.say(Notice::line(
                    NoticeKind::Warn,
                    format!(
                        "{}: one is already running; `{} stop` stops it.",
                        self.name,
                        self.name.to_ascii_lowercase()
                    ),
                ));
                return None;
            }
            let running = Running {
                number: self.runs.fetch_add(1, Ordering::Relaxed),
                stop: CancellationToken::new(),
            };
            *slot = Some(running.clone());
            running
        };
        let desk = Arc::clone(self);
        let handle = handle.clone();
        let joined = (joined.0, joined.1.into());
        Some(tokio::spawn(async move {
            let ended = desk
                .run_once(&handle, &running.stop, joined, job, &hydra)
                .await;
            let mut slot = desk.running.lock().unwrap_or_else(PoisonError::into_inner);
            if slot
                .as_ref()
                .is_some_and(|run| run.number == running.number)
            {
                *slot = None;
            }
            ended
        }))
    }

    /// Claim, run with the watchdog beside, release, and say how it ended.
    async fn run_once(
        &self,
        handle: &SessionHandle,
        stop: &CancellationToken,
        joined: (Snapshot, Heard),
        job: Job,
        hydra: &Hydra,
    ) -> Result<(), Halt> {
        let say =
            |kind, text: String| handle.say(Notice::line(kind, format!("{}: {text}", self.name)));
        if handle.claim(self.token).await.is_err() {
            say(
                NoticeKind::Error,
                "something else holds the session; stop it first.".to_owned(),
            );
            return Err(BehaviorError::AuthorityHeld.into());
        }
        let heartbeat = Heartbeat::default();
        let ended = {
            let mut driver = Driver::new(
                handle,
                stop,
                self.token,
                &self.ids,
                (joined.0.state, joined.1),
                &heartbeat,
                hydra,
                self.name,
            );
            let symbol = handle
                .command_symbol()
                .unwrap_or(cena_session::command::claimant::DEFAULT_SYMBOL);
            let work = async {
                match job {
                    Job::Multi(multi) => rounds(&mut driver, &multi).await,
                    Job::Foreach(foreach) => each(&mut driver, &foreach, symbol).await,
                }
            };
            tokio::select! {
                ended = Box::pin(work) => ended,
                // Stopped by the player, or preempted as wedged.
                _ = watch(handle, stop, &heartbeat, BEHAVIOR_WATCHDOG, self.name) => {
                    Err(BehaviorError::Cancelled.into())
                }
            }
        };
        handle.release(self.token);
        match &ended {
            Ok(()) => {}
            Err(Halt::Stopped(BehaviorError::Cancelled)) => {
                say(NoticeKind::Info, "stopped.".to_owned());
            }
            Err(Halt::Stopped(BehaviorError::AuthorityHeld)) => say(
                NoticeKind::Error,
                "something else took the session while a command ran; the rest was not sent."
                    .to_owned(),
            ),
            Err(Halt::Stopped(why)) => say(NoticeKind::Warn, format!("ended: {why:?}.")),
            Err(Halt::Failed(why)) => say(NoticeKind::Error, why.clone()),
        }
        ended
    }
}

/// `;multi`: the list, so many times (`multi.lic:55-64`).
async fn rounds(driver: &mut Driver<'_>, multi: &Multi) -> Result<(), Halt> {
    for _ in 0..multi.times {
        for line in &multi.lines {
            driver.line(line).await?;
        }
    }
    let times = multi.times;
    driver.say(
        NoticeKind::Info,
        &format!("done, {times} time{}.", if times == 1 { "" } else { "s" }),
    );
    Ok(())
}

/// `;foreach`: look, pick, then each item's lines -- or, with no commands,
/// the list (`foreach.lic:1771-2150`). Every item's lines are made before
/// the first is sent, so one that cannot be made stops the run before it
/// starts.
async fn each(driver: &mut Driver<'_>, foreach: &Foreach, symbol: char) -> Result<(), Halt> {
    let found = driver.scan(foreach).await?;
    let groups = select(&foreach.options, filtered(&foreach.filter, found));
    let total: usize = groups.iter().map(|group| group.items.len()).sum();
    if total == 0 {
        driver.say(NoticeKind::Info, "no matching items found.");
        return Ok(());
    }
    if foreach.commands.is_empty() {
        driver
            .handle
            .say(Notice::table(NoticeKind::Info, listing(&groups)));
        return Ok(());
    }
    let mut work = Vec::with_capacity(total);
    for group in &groups {
        for item in &group.items {
            let lines = build::lines(&foreach.commands, &group.place, item, symbol)
                .map_err(Halt::Failed)?;
            work.push(lines);
        }
    }
    for (done, lines) in work.iter().enumerate() {
        if done % EVERY == 0 {
            driver.say(
                NoticeKind::Info,
                &format!(
                    "item {} of {total} ({}% complete).",
                    done + 1,
                    done * 100 / total
                ),
            );
        }
        for line in lines {
            driver.line(line).await?;
        }
    }
    driver.say(
        NoticeKind::Info,
        &format!("done, {total} item{}.", if total == 1 { "" } else { "s" }),
    );
    Ok(())
}
