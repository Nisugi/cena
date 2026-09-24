//! The hunt desk: `;hunt <name>` and `;hunt stop` for one session, while it
//! is being played.
//!
//! One desk per session, the map shared between them all. It loads the
//! profile the way this character would run it ([`chain::load`]), claims
//! the authority, runs the [`hunt`] with a [`watch`] beside it, and releases
//! on every exit. **One hunt at a time**: a second `;hunt <name>` stops the
//! first and starts when it has let go, as travel's desk does for walks.
//!
//! Importing, checking and listing profiles need no session state and stay
//! with whoever installs the desk.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use cena_map::Map;
use cena_session::{AuthorityToken, CommandId, Notice, NoticeKind, SessionHandle, Snapshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::chain;
use super::command::Command;
use super::drive::{HuntEnd, hunt};
use super::engine::Hunt;
use crate::error::BehaviorError;
use crate::travel::Heard;
use crate::watchdog::{BEHAVIOR_WATCHDOG, Heartbeat, Watched, watch};

/// One session's hunt desk.
pub struct Desk {
    map: Arc<Map>,
    /// The data directory the profiles are under.
    dir: PathBuf,
    token: AuthorityToken,
    running: Mutex<Option<Running>>,
    ids: Arc<AtomicU64>,
    hunts: AtomicU64,
}

/// A hunt under way: how to stop it, and how to know it is over.
#[derive(Clone)]
struct Running {
    number: u64,
    stop: CancellationToken,
    over: CancellationToken,
}

impl Desk {
    /// A desk with no hunt under way. `dir` is the data directory; `token`
    /// is the authority its hunts claim.
    #[must_use]
    pub fn new(map: Arc<Map>, dir: PathBuf, token: AuthorityToken) -> Arc<Desk> {
        Arc::new(Desk {
            map,
            dir,
            token,
            running: Mutex::new(None),
            ids: Arc::new(AtomicU64::new(1)),
            hunts: AtomicU64::new(0),
        })
    }

    /// Stop the hunt under way. `false` when there is none.
    pub fn stop(&self) -> bool {
        let running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        running
            .as_ref()
            .filter(|hunt| !hunt.stop.is_cancelled())
            .is_some_and(|hunt| {
                hunt.stop.cancel();
                true
            })
    }

    /// Do `command`, for the session behind `handle`, from a fresh
    /// subscription. `Some` when a hunt was started.
    pub fn run(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        command: Command,
    ) -> Option<JoinHandle<HuntEnd>> {
        let say = |kind, text: String| handle.say(Notice::line(kind, format!("Hunt: {text}")));
        match command {
            Command::Stop => {
                if !self.stop() {
                    say(NoticeKind::Info, "I am not hunting.".to_owned());
                }
                None
            }
            Command::Run(name) => {
                let character = &joined.0.state.character;
                let loaded = chain::load(
                    &self.dir,
                    character.instance.as_deref(),
                    character.name.as_deref(),
                    &name,
                );
                let loaded = match loaded {
                    Ok(loaded) => loaded,
                    Err(chain::LoadError::Invalid(problems)) => {
                        for problem in problems {
                            say(NoticeKind::Error, format!("{name}: {problem}"));
                        }
                        return None;
                    }
                    Err(why) => {
                        say(NoticeKind::Error, why.to_string());
                        return None;
                    }
                };
                for (place, step) in loaded.profile.held_steps() {
                    say(
                        NoticeKind::Warn,
                        format!("{place} is held and will be skipped: `{}`", step.send),
                    );
                }
                for sequence in loaded.profile.unwritten_sequences() {
                    say(
                        NoticeKind::Warn,
                        format!("sequence {sequence} has no steps and will be skipped."),
                    );
                }
                say(NoticeKind::Info, format!("hunting on {name}."));
                let seed = joined.0.state.game_time_now().map_or(1, u64::from);
                let machine = Hunt::new(loaded.profile, seed);
                Some(self.start(handle.clone(), (joined.0, joined.1.into()), machine))
            }
            _ => None,
        }
    }

    /// Start a hunt, stopping the one under way first.
    fn start(
        self: &Arc<Self>,
        handle: SessionHandle,
        joined: (Snapshot, Heard),
        machine: Hunt,
    ) -> JoinHandle<HuntEnd> {
        let running = Running {
            number: self.hunts.fetch_add(1, Ordering::Relaxed),
            stop: CancellationToken::new(),
            over: CancellationToken::new(),
        };
        let before = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(running.clone());
        if let Some(before) = &before {
            before.stop.cancel();
        }
        let desk = Arc::clone(self);
        tokio::spawn(async move {
            let Running { number, stop, over } = running;
            if let Some(before) = before {
                before.over.cancelled().await;
            }
            let end = desk.hunt_once(&handle, &stop, joined, machine).await;
            let mut slot = desk.running.lock().unwrap_or_else(PoisonError::into_inner);
            if slot.as_ref().is_some_and(|hunt| hunt.number == number) {
                *slot = None;
            }
            drop(slot);
            over.cancel();
            end
        })
    }

    /// Claim, hunt with the watchdog beside it, release.
    async fn hunt_once(
        &self,
        handle: &SessionHandle,
        stop: &CancellationToken,
        joined: (Snapshot, Heard),
        machine: Hunt,
    ) -> HuntEnd {
        if handle.claim(self.token).await.is_err() {
            handle.say(Notice::line(
                NoticeKind::Error,
                "Hunt: something else holds the session; stop it first.",
            ));
            return HuntEnd::Stopped(BehaviorError::AuthorityHeld);
        }
        let heartbeat = Heartbeat::default();
        let next = Arc::clone(&self.ids);
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let end = {
            let run = Box::pin(hunt(
                handle, stop, ids, self.token, joined, &self.map, machine, &heartbeat,
            ));
            tokio::select! {
                end = run => end,
                // Stopped by the player, or preempted as wedged: either way
                // the hunt was cancelled from outside.
                watched = watch(handle, stop, &heartbeat, BEHAVIOR_WATCHDOG, "Hunt") => match watched {
                    Watched::Stopped | Watched::Wedged(_) => HuntEnd::Stopped(BehaviorError::Cancelled),
                },
            }
        };
        handle.release(self.token);
        end
    }
}
