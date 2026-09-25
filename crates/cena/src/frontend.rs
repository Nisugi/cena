//! Optional embedded presentation; neither opening nor closing a viewer owns
//! the native session's lifetime. Pairing tokens exist only for this process.

use cena_session::{Event, Generation, ObserveError, SessionHandle, SessionObserver, State};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Server lifetime owned by the executable, independently of browser tabs.
pub(crate) struct Frontend {
    stop: CancellationToken,
    /// Taken by [`Self::shutdown`]; behind a lock so the frontend can be
    /// shared with the hub's control while it runs.
    task: std::sync::Mutex<Option<JoinHandle<std::io::Result<()>>>>,
    sessions: cena_web::Sessions,
    pairing: String,
    map_projection: Option<cena_web::MapProjection>,
}

impl Frontend {
    /// Bind, serving no session yet, when `--web` asked for it; see
    /// [`Self::attach`]. One listener serves every character (`plan/23` §D1a).
    pub(crate) async fn open(map: &crate::map_context::ConfiguredMap) -> Option<Self> {
        if !requested() {
            return None;
        }
        let opened = async {
            let server = cena_web::WebServer::open().await?;
            match std::env::var_os("CENA_HUNTING_CORRECTIONS_DIR") {
                Some(directory) => server.with_hunting_corrections(directory.into()),
                None => Ok(server),
            }
        }
        .await;
        match opened {
            Ok(server) => {
                let stop = CancellationToken::new();
                let sessions = server.sessions();
                let pairing = server.pairing_url();
                let shutdown = stop.clone();
                let task =
                    tokio::spawn(async move { server.run(shutdown.cancelled_owned()).await });
                Some(Self {
                    stop,
                    task: std::sync::Mutex::new(Some(task)),
                    sessions,
                    pairing,
                    map_projection: crate::map_context::projection(map),
                })
            }
            Err(error) => {
                eprintln!(
                    "[web] Frontend unavailable: {error}. Native session remains controlled by CLI."
                );
                None
            }
        }
    }

    /// Serve one more session. `character` names it when several run: its
    /// page's link then carries `&session=N`, and the announcement says whose
    /// it is.
    ///
    /// The link is the pairing handoff to the local operator, printed when the
    /// session is `Ready` rather than at bind: printed at bind it scrolled
    /// away under the login burst before anyone could use it (author,
    /// 2026-09-23). Never into the game recorder or normal application logs.
    pub(crate) fn attach(
        &self,
        character: Option<&str>,
        observer: SessionObserver,
        handle: SessionHandle,
    ) {
        let url = match character {
            Some(_) => format!("{}&session={}", self.pairing, handle.session().0),
            None => self.pairing.clone(),
        };
        self.sessions.attach_with_map(
            character.unwrap_or_default(),
            observer.clone(),
            handle,
            self.map_projection.clone(),
        );
        tokio::spawn(announce(
            observer,
            url,
            character.map(str::to_owned),
            self.stop.clone(),
        ));
    }

    /// Print the hub page's link, labelled: every character on one page
    /// (`plan/29` step 5b). Printed once, when several characters run --
    /// each character's own link is printed, labelled with its name, when it
    /// is `Ready`. The author asked for both to be printed and labelled
    /// rather than the hub being a link the operator had to edit by hand.
    pub(crate) fn announce_hub(&self) {
        eprintln!("[web] Hub, every character: {}", self.pairing);
    }

    /// The sessions it serves, for the hub's control and its offer.
    pub(crate) fn sessions(&self) -> &cena_web::Sessions {
        &self.sessions
    }

    /// Stop serving `id`: its page closes; the session is not touched.
    pub(crate) fn detach(&self, id: cena_session::SessionId) {
        self.sessions.detach(id);
    }

    /// Stop serving without changing the game's command authority.
    pub(crate) async fn shutdown(&self) {
        self.stop.cancel();
        let task = self
            .task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some(mut task) = task else { return };
        match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => eprintln!("[web] Frontend stopped with an error: {error}"),
            Ok(Err(error)) => eprintln!("[web] Frontend task failed: {error}"),
            Err(_) => {
                task.abort();
                eprintln!("[web] Frontend shutdown exceeded its deadline; task aborted.");
            }
        }
    }
}

/// Print the pairing URL each time a connection becomes Ready.
///
/// Once per generation: a reconnect earns a fresh reminder, a lagged
/// resubscription does not. Reads the observer's snapshot first, so a session
/// that was already Ready when this started is announced too.
async fn announce(
    observer: SessionObserver,
    url: String,
    character: Option<String>,
    stop: CancellationToken,
) {
    let mut announced: Option<Generation> = None;
    let mut tell = |generation: Generation| {
        if announced != Some(generation) {
            announced = Some(generation);
            if let Some(name) = &character {
                eprintln!("[{name}] [web] Ready. {name}'s page: {url}");
            } else {
                eprintln!(
                    "[web] Ready. Play in the browser; the terminal shows only Hydra's own messages."
                );
                eprintln!("[web] Open this private pairing URL: {url}");
            }
        }
    };
    loop {
        let (snapshot, mut events) = match observer.subscribe().await {
            Ok(subscription) => subscription,
            Err(ObserveError::Closed) => return,
            // `Busy` and `Timeout` are retryable (`SessionObserver::subscribe`).
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }
        };
        if snapshot.lifecycle == State::Ready {
            tell(snapshot.generation);
        }
        loop {
            tokio::select! {
                () = stop.cancelled() => return,
                next = events.recv() => match next {
                    Ok(o) if o.event == Event::StateChanged(State::Ready) => tell(o.generation),
                    Ok(_) => {}
                    // Resubscribe: the fresh snapshot says whether it is Ready.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                },
            }
        }
    }
}

/// No implicit frontend for existing command-line users.
pub(crate) fn requested() -> bool {
    std::env::args().skip(1).any(|arg| arg == "--web")
}
