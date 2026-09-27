//! The characters the window shows, as the binary puts them on the session
//! table: the GUI's side of what `cena_web::Sessions` is for the browser.
//!
//! The binary attaches a session when it starts one and detaches it when it
//! takes one off the table. Each attached session gets a [`Seat`] and a feed
//! (`feed.rs`) on the binary's runtime, which follows the session and wakes
//! the window when something changed. The window only reads: what it draws
//! is whatever the seats hold at that frame.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cena_session::{SessionHandle, SessionId, SessionObserver};
use cena_ui::SessionCard;
use tokio_util::sync::CancellationToken;

use crate::feed;

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
    /// Where the feeds run.
    runtime: tokio::runtime::Handle,
    /// Every attached session, in the order it was attached.
    seats: Mutex<Vec<Arc<Seat>>>,
    /// The window, once it is open.
    window: Wake,
    /// Asked to close before the window had opened.
    closing: std::sync::atomic::AtomicBool,
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
    /// Cancelled when it is detached, which ends its feed.
    pub(crate) stop: CancellationToken,
    /// Which session it is.
    id: SessionId,
}

impl Seat {
    /// A seat for session `id`, named `name`, with nothing seen yet.
    pub(crate) fn new(id: SessionId, name: &str) -> Self {
        Self {
            card: Mutex::new(SessionCard::of(id.0.to_string(), name.to_owned(), None)),
            stop: CancellationToken::new(),
            id,
        }
    }
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
            }),
        }
    }

    /// Show `handle`'s session as `name`, the character, and start following
    /// it. Replaces an earlier attachment of the same session.
    pub fn attach(&self, name: &str, observer: SessionObserver, handle: &SessionHandle) {
        let seat = Arc::new(Seat::new(handle.session(), name));
        {
            let mut seats = self.seats();
            if let Some(old) = seats.iter().position(|old| old.id == seat.id) {
                seats.remove(old).stop.cancel();
            }
            seats.push(Arc::clone(&seat));
        }
        self.shared
            .runtime
            .spawn(feed::feed(observer, seat, self.shared.window.clone()));
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

    /// Every attached session's card, in the order they were attached.
    #[must_use]
    pub fn cards(&self) -> Vec<SessionCard> {
        self.seats()
            .iter()
            .map(|seat| {
                seat.card
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone()
            })
            .collect()
    }

    fn seats(&self) -> std::sync::MutexGuard<'_, Vec<Arc<Seat>>> {
        self.shared
            .seats
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
