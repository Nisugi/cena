//! The characters the window shows, as the binary puts them on the session
//! table: the GUI's side of what `cena_web::Sessions` is for the browser.
//!
//! The binary attaches a session when it starts one and detaches it when it
//! takes one off the table. Each attached session gets a [`Seat`] and a feed
//! (`feed.rs`) on the binary's runtime, which follows the session and wakes
//! the window when something changed. The window only reads what the seats
//! hold at that frame, and asks the binary to act through the [`HubControl`]
//! it was given, on the runtime, never on its own thread.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use cena_session::{
    Notice, NoticeKind, Outcome, SessionHandle, SessionId, SessionObserver, Snapshot,
};
use cena_ui::{HubControl, HubRequest, MergedHistory, MergedLine, SessionCard};
use tokio_util::sync::CancellationToken;

use crate::feed;
use crate::story::Story;

/// The sessions the window shows. Cloneable, and usable from the binary's
/// runtime while the window runs on the main thread.
#[derive(Clone)]
pub struct Sessions {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Sessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sessions").finish_non_exhaustive()
    }
}

struct Shared {
    /// Where the feeds, and the answers to the hub's requests, run.
    runtime: tokio::runtime::Handle,
    /// Every attached session, in the order it was attached.
    seats: Mutex<Vec<Arc<Seat>>>,
    /// The window, once it is open.
    window: Wake,
    /// Asked to close before the window had opened.
    closing: std::sync::atomic::AtomicBool,
    /// Who answers the hub's requests; `None`, and it can ask nothing.
    control: Mutex<Option<HubControl>>,
    /// Characters the hub may start, as the binary last said.
    offered: Mutex<Vec<String>>,
    /// Every session's shared streams, merged (`plan/29` step 5d).
    merged: Arc<Mutex<MergedHistory>>,
    /// The binary's answer to the last request.
    said: Mutex<Option<String>>,
}

/// How a feed wakes the window: the window's context, once it exists.
#[derive(Clone, Default)]
pub(crate) struct Wake(Arc<OnceLock<egui::Context>>);

impl Wake {
    /// Ask the window for a frame, if it is open.
    pub(crate) fn wake(&self) {
        if let Some(context) = self.0.get() {
            context.request_repaint();
        }
    }
}

/// One character as the window holds it.
pub(crate) struct Seat {
    /// Its card on the hub, as its feed last saw it.
    pub(crate) card: Mutex<SessionCard>,
    /// The character as its feed last saw it, for its play window.
    pub(crate) snapshot: Mutex<Option<Arc<Snapshot>>>,
    /// Its story, messages and banners, kept by its feed.
    pub(crate) story: Mutex<Story>,
    /// What its play window's commands go through.
    pub(crate) handle: SessionHandle,
    /// Cancelled when it is detached, which ends its feed.
    pub(crate) stop: CancellationToken,
    /// Which session it is.
    pub(crate) id: SessionId,
    /// The character, as the table named it.
    pub(crate) name: String,
}

impl Seat {
    /// A seat for `handle`'s session, named `name`, with nothing seen yet.
    pub(crate) fn new(handle: SessionHandle, name: &str) -> Self {
        let id = handle.session();
        Self {
            card: Mutex::new(SessionCard::of(id.0.to_string(), name.to_owned(), None)),
            snapshot: Mutex::default(),
            story: Mutex::default(),
            handle,
            stop: CancellationToken::new(),
            id,
            name: name.to_owned(),
        }
    }

    /// Its tag on a merged line: the character's name, or its session when it
    /// was given none, as Despana tags it.
    pub(crate) fn tag(&self) -> String {
        if self.name.is_empty() {
            format!("Session {}", self.id.0)
        } else {
            self.name.clone()
        }
    }
}

/// What the hub draws this frame, copied out of [`Sessions`] so no lock is
/// held while it draws.
#[derive(Debug, Default)]
pub(crate) struct Glance {
    pub(crate) cards: Vec<SessionCard>,
    pub(crate) offered: Vec<String>,
    pub(crate) merged: Vec<MergedLine>,
    pub(crate) said: Option<String>,
}

impl Sessions {
    /// No session yet. Feeds will run on `runtime`.
    #[must_use]
    pub fn new(runtime: tokio::runtime::Handle) -> Self {
        Self {
            shared: Arc::new(Shared {
                runtime,
                seats: Mutex::default(),
                window: Wake::default(),
                closing: std::sync::atomic::AtomicBool::new(false),
                control: Mutex::default(),
                offered: Mutex::default(),
                merged: Arc::default(),
                said: Mutex::default(),
            }),
        }
    }

    /// Show `handle`'s session as `name`, the character, and start following
    /// it. Replaces an earlier attachment of the same session.
    pub fn attach(&self, name: &str, observer: SessionObserver, handle: &SessionHandle) {
        let seat = Arc::new(Seat::new(handle.clone(), name));
        {
            let mut seats = self.seats();
            if let Some(old) = seats.iter().position(|old| old.id == seat.id) {
                seats.remove(old).stop.cancel();
            }
            seats.push(Arc::clone(&seat));
        }
        self.shared.runtime.spawn(feed::feed(
            observer,
            seat,
            Arc::clone(&self.shared.merged),
            self.shared.window.clone(),
        ));
        self.shared.window.wake();
    }

    /// Stop showing session `id`: its card goes; the session is not touched.
    pub fn detach(&self, id: SessionId) {
        let gone = {
            let mut seats = self.seats();
            seats
                .iter()
                .position(|seat| seat.id == id)
                .map(|at| seats.remove(at))
        };
        if let Some(seat) = gone {
            seat.stop.cancel();
        }
        self.shared.window.wake();
    }

    /// Answer the hub's requests with `control` (`plan/29` step 5c). Until
    /// this is called the hub can ask nothing.
    pub fn control(&self, control: HubControl) {
        *lock(&self.shared.control) = Some(control);
    }

    /// The characters the hub may start now.
    pub fn offer(&self, offered: Vec<String>) {
        *lock(&self.shared.offered) = offered;
        self.shared.window.wake();
    }

    /// Close the window: the run is over. Called by the binary once every
    /// character has quit, however the ending began.
    pub fn close(&self) {
        self.shared
            .closing
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(context) = self.shared.window.0.get() {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
            context.request_repaint();
        }
    }

    /// Whether [`Self::close`] was called.
    pub(crate) fn closing(&self) -> bool {
        self.shared
            .closing
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The window is open: feeds wake it from now on.
    pub(crate) fn opened(&self, context: &egui::Context) {
        let _ = self.shared.window.0.set(context.clone());
        if self.closing() {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Ask the binary for `request`, on the runtime; its answer is shown when
    /// it comes. With nobody to answer, says so at once.
    pub(crate) fn ask(&self, request: HubRequest) {
        let Some(control) = lock(&self.shared.control).clone() else {
            *lock(&self.shared.said) =
                Some("Nothing here can start or stop characters.".to_owned());
            return;
        };
        let shared = Arc::clone(&self.shared);
        self.shared.runtime.spawn(async move {
            let said = control(request).await;
            *lock(&shared.said) = Some(said);
            shared.window.wake();
        });
    }

    /// What the hub draws this frame.
    pub(crate) fn glance(&self) -> Glance {
        Glance {
            cards: self.cards(),
            offered: lock(&self.shared.offered).clone(),
            merged: lock(&self.shared.merged).lines().cloned().collect(),
            said: lock(&self.shared.said).clone(),
        }
    }

    /// Every attached session's seat, in the order they were attached.
    pub(crate) fn seated(&self) -> Vec<Arc<Seat>> {
        self.seats().clone()
    }

    /// A seat for `handle`'s session with no feed behind it, as a test
    /// needs: nothing here can make a live session.
    #[cfg(test)]
    pub(crate) fn seat_for_test(&self, handle: SessionHandle, name: &str) -> Arc<Seat> {
        let seat = Arc::new(Seat::new(handle, name));
        self.seats().push(Arc::clone(&seat));
        seat
    }

    /// Send `line` on `seat`'s character as the player typed it, on the
    /// connection its window last saw: Hydra's command line first, then the
    /// game (`SessionHandle::send_manual_at`). It is echoed in the story at
    /// once; a line that may not have gone is said in Hydra's pane.
    pub(crate) fn send(&self, seat: &Arc<Seat>, line: String) {
        lock(&seat.story).typed(&line);
        let Some(generation) = lock(&seat.snapshot).as_ref().map(|shot| shot.generation) else {
            lock(&seat.story).tell(Notice::line(
                NoticeKind::Warn,
                "Not connected yet; nothing was sent.",
            ));
            return;
        };
        let (seat, window) = (Arc::clone(seat), self.shared.window.clone());
        self.shared.runtime.spawn(async move {
            let outcome = seat
                .handle
                .send_manual_at(generation, &line, SEND_DEADLINE)
                .await;
            if let Some(why) = unsent(&outcome) {
                lock(&seat.story).tell(Notice::line(NoticeKind::Warn, why));
                window.wake();
            }
        });
    }

    /// Every attached session's card, in the order they were attached.
    #[must_use]
    pub fn cards(&self) -> Vec<SessionCard> {
        self.seats()
            .iter()
            .map(|seat| lock(&seat.card).clone())
            .collect()
    }

    fn seats(&self) -> MutexGuard<'_, Vec<Arc<Seat>>> {
        lock(&self.shared.seats)
    }
}

/// How long a typed line waits for its answer: Despana's `COMMAND_TIMEOUT`
/// is the same order.
const SEND_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// What the player is told about a line that may not have gone, in
/// Despana's receipt words (`cena-web/src/socket.rs`, `outcome_receipt`);
/// `None` when it went, or Hydra ran it.
fn unsent(outcome: &Outcome) -> Option<String> {
    match outcome {
        Outcome::Handled | Outcome::Confirmed(_) => None,
        Outcome::Refused(why) => Some(format!("Not sent: the session refused it ({why:?}).")),
        Outcome::Timeout => Some(
            "No answer yet; the command may have reached the game. Do not send it again blindly."
                .to_owned(),
        ),
        Outcome::Interrupted | Outcome::Dead | Outcome::Disconnected => Some(
            "The connection was interrupted; that command may not have reached the game."
                .to_owned(),
        ),
    }
}

/// A lock that a panic elsewhere does not poison for the window.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
