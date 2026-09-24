//! Optional embedded presentation; neither opening nor closing a viewer owns
//! the native session's lifetime. Pairing tokens exist only for this process.

use cena_session::{
    Event, Generation, ObserveError, SessionHandle, SessionObserver, State, SupervisedEnd,
};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Server lifetime owned by the executable, independently of browser tabs.
pub(crate) struct Frontend {
    stop: CancellationToken,
    task: JoinHandle<std::io::Result<()>>,
}

impl Frontend {
    /// Bind only when explicitly selected. Failure leaves the CLI/session path
    /// available and is reported, rather than bypassing native shutdown.
    pub(crate) async fn start(observer: SessionObserver, handle: SessionHandle) -> Option<Self> {
        if !requested() {
            return None;
        }
        match cena_web::WebServer::bind(observer.clone(), handle).await {
            Ok(server) => {
                let stop = CancellationToken::new();
                // Explicit pairing handoff to the local operator, printed when
                // the session is Ready rather than at bind: printed at bind it
                // scrolled away under the login burst before anyone could use
                // it (author, 2026-09-23). Never into the game recorder or
                // normal application logs.
                tokio::spawn(announce(observer, server.pairing_url(), stop.clone()));
                let shutdown = stop.clone();
                let task =
                    tokio::spawn(async move { server.run(shutdown.cancelled_owned()).await });
                Some(Self { stop, task })
            }
            Err(error) => {
                eprintln!(
                    "[web] Frontend unavailable: {error}. Native session remains controlled by CLI."
                );
                None
            }
        }
    }

    /// Stop serving without changing the game's command authority.
    pub(crate) async fn shutdown(mut self) {
        self.stop.cancel();
        match tokio::time::timeout(Duration::from_secs(3), &mut self.task).await {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => eprintln!("[web] Frontend stopped with an error: {error}"),
            Ok(Err(error)) => eprintln!("[web] Frontend task failed: {error}"),
            Err(_) => {
                self.task.abort();
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
async fn announce(observer: SessionObserver, url: String, stop: CancellationToken) {
    let mut announced: Option<Generation> = None;
    let mut tell = |generation: Generation| {
        if announced != Some(generation) {
            announced = Some(generation);
            eprintln!(
                "[web] Ready. Play in the browser; the terminal shows only Hydra's own messages."
            );
            eprintln!("[web] Open this private pairing URL: {url}");
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

/// Keep a selected web session open until explicit shutdown, native session
/// completion, or an explicitly selected hold deadline. Non-web callers keep
/// the demonstration binary's existing ten-second default.
///
/// Ctrl-C arrives as `interrupt`, not as a `ctrl_c()` of its own. This was the
/// ONE place that listened for it, so the phases before the hold had no
/// handler at all; `crate::interrupt` now owns the signal for the whole run
/// and this is one of the waits it ends.
pub(crate) async fn wait_for_stop(
    holding: Option<Duration>,
    supervisor: &JoinHandle<SupervisedEnd>,
    interrupt: &CancellationToken,
) {
    if interrupt.is_cancelled() {
        // An earlier phase was interrupted; announcing a hold now would be
        // announcing something that is not going to happen.
        return;
    }
    match holding {
        Some(duration) => eprintln!("[session] holding for {duration:?} (Ctrl-C to stop early)"),
        None => eprintln!(
            "[session] Web frontend active; Ctrl-C stops the session. Closing a browser does not."
        ),
    }
    let deadline = async {
        match holding {
            Some(duration) => tokio::time::sleep(duration).await,
            None => std::future::pending::<()>().await,
        }
    };
    let session_ended = async {
        while !supervisor.is_finished() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    tokio::select! {
        () = deadline => {}
        () = session_ended => eprintln!("[session] Native session ended; finishing shutdown."),
        () = interrupt.cancelled() => {}
    }
}
