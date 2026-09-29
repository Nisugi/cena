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
use std::time::{Duration, Instant};

use cena_session::{
    Notice, NoticeKind, Outcome, SessionHandle, SessionId, SessionObserver, Snapshot,
};
use cena_ui::settings::Page;
use cena_ui::{
    HubControl, HubRequest, HuntView, Listing, MergedHistory, MergedLine, RosterCard, SessionCard,
};
use tokio_util::sync::CancellationToken;

use crate::feed;

/// Where a character is on the map, from its game state: the binary's
/// answer (`plan/53` §7 steps 4-5), which alone holds the map and its
/// layout. Asked each time the character's feed takes a snapshot.
pub type Minimap = Arc<dyn Fn(&cena_session::GameState) -> cena_ui::MinimapView + Send + Sync>;
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
    /// Every character on the roster, as the binary last said.
    roster: Mutex<Vec<RosterCard>>,
    /// The last account whose characters the binary listed.
    listing: Mutex<Option<Listing>>,
    /// The settings menu's pages the binary last gave, and for whom.
    settings: Mutex<Option<(String, Vec<Page>)>>,
    /// The triggers file as the binary last gave it (`plan/54`).
    triggers: Mutex<Option<cena_ui::triggers::Book>>,
    /// An import asking before it writes (`plan/54` step 5).
    import: Mutex<Option<cena_ui::triggers::ImportQuestion>>,
    /// Every session's shared streams, merged (`plan/29` step 5d).
    merged: Arc<Mutex<MergedHistory>>,
    /// The binary's answer to the last request, and when it came.
    said: Mutex<Option<(String, Instant)>>,
}

/// How long the binary's answer stays on the hub: long enough to read, and
/// gone before it goes stale. Live, 2026-09-27: *"Starting Nisugi."* still
/// showed after Nisugi had quit.
const SAID_FOR: Duration = Duration::from_secs(10);

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
    /// Where it is on the map, from the binary (`plan/53` §7 step 5);
    /// `None` without a map.
    pub(crate) minimap: Option<Minimap>,
    /// That, as its feed last worked it out from a snapshot.
    pub(crate) where_now: Mutex<Option<cena_ui::MinimapView>>,
    /// Its card on the hub, as its feed last saw it.
    pub(crate) card: Mutex<SessionCard>,
    /// The character as its feed last saw it, for its play window.
    pub(crate) snapshot: Mutex<Option<Arc<Snapshot>>>,
    /// Its story, messages and banners, kept by its feed.
    pub(crate) story: Mutex<Story>,
    /// What its hunt is doing, as the binary last said; `None` while none
    /// runs (`plan/47` step 8).
    pub(crate) hunt: Mutex<Option<HuntView>>,
    /// What its play window's commands go through.
    pub(crate) handle: SessionHandle,
    /// Cancelled when it is detached, which ends its feed.
    pub(crate) stop: CancellationToken,
    /// Which session it is.
    pub(crate) id: SessionId,
    /// The character, as the table named it.
    pub(crate) name: String,
    /// The game it is on, by its code: its layout is kept by game and name.
    pub(crate) game: String,
}

impl Seat {
    /// A seat for `handle`'s session, named `name`, on the game `game`
    /// (its code), with nothing seen yet.
    pub(crate) fn new(handle: SessionHandle, name: &str, game: &str) -> Self {
        let id = handle.session();
        Self {
            card: Mutex::new(SessionCard {
                game: game.to_owned(),
                ..SessionCard::of(id.0.to_string(), name.to_owned(), None)
            }),
            minimap: None,
            where_now: Mutex::default(),
            snapshot: Mutex::default(),
            story: Mutex::default(),
            hunt: Mutex::default(),
            handle,
            stop: CancellationToken::new(),
            id,
            name: name.to_owned(),
            game: game.to_owned(),
        }
    }

    /// It, as `from`'s window offers it to follow: by its name on the same
    /// game, and as `GAME:Name` on another, so one name on two games is two
    /// characters to follow, and a widget following a name follows the one
    /// on its own window's game (the crate review of 2026-09-28, R6).
    pub(crate) fn seen_from(&self, from: &Seat) -> crate::widget::Character {
        let name = if self.game.eq_ignore_ascii_case(&from.game) {
            self.name.clone()
        } else {
            format!("{}:{}", self.game, self.name)
        };
        crate::widget::Character {
            name,
            snapshot: lock(&self.snapshot).clone(),
            hunt: lock(&self.hunt).clone(),
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
    pub(crate) roster: Vec<RosterCard>,
    pub(crate) listing: Option<Listing>,
    pub(crate) settings: Option<(String, Vec<Page>)>,
    /// The triggers file, for the trigger editor.
    pub(crate) triggers: Option<cena_ui::triggers::Book>,
    /// An import asking before it writes.
    pub(crate) import: Option<cena_ui::triggers::ImportQuestion>,
    pub(crate) merged: Vec<MergedLine>,
    /// The binary's last answer, while it is fresh, and how long it has left.
    pub(crate) said: Option<(String, Duration)>,
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
                roster: Mutex::default(),
                listing: Mutex::default(),
                settings: Mutex::default(),
                triggers: Mutex::default(),
                import: Mutex::default(),
                merged: Arc::default(),
                said: Mutex::default(),
            }),
        }
    }

    /// Show `handle`'s session as `name`, the character, and start following
    /// it. Replaces an earlier attachment of the same session.
    ///
    /// `minimap` says where the character is on the map, from its game
    /// state; `None` without a map.
    pub fn attach(
        &self,
        name: &str,
        game: &str,
        observer: SessionObserver,
        handle: &SessionHandle,
        minimap: Option<Minimap>,
    ) {
        let seat = Arc::new(Seat {
            minimap,
            ..Seat::new(handle.clone(), name, game)
        });
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

    /// What session `id`'s hunt is doing now: `None` when none runs. Its
    /// play window's Hunt pane shows it (`plan/47` step 8).
    pub fn hunt(&self, id: SessionId, hunt: Option<HuntView>) {
        if let Some(seat) = self.seats().iter().find(|seat| seat.id == id) {
            *lock(&seat.hunt) = hunt;
        }
        self.shared.window.wake();
    }

    /// The characters the hub may start now.
    pub fn offer(&self, offered: Vec<String>) {
        *lock(&self.shared.offered) = offered;
        self.shared.window.wake();
    }

    /// Every character on the roster, for the Not launched and New login
    /// tabs (`plan/49` Stage C): never a password, only whether one is kept.
    pub fn roster(&self, roster: Vec<RosterCard>) {
        *lock(&self.shared.roster) = roster;
        self.shared.window.wake();
    }

    /// The characters an account has on one game, as the login service
    /// listed them, for the New login tab (`plan/49` Stage C step 7).
    pub fn characters(&self, listing: Listing) {
        *lock(&self.shared.listing) = Some(listing);
        self.shared.window.wake();
    }

    /// The settings menu's pages for `character` (`GAME:Name`), as the
    /// binary built them from its files (`plan/50` §7 step 1).
    pub fn settings(&self, character: String, pages: Vec<Page>) {
        *lock(&self.shared.settings) = Some((character, pages));
        self.shared.window.wake();
    }

    /// The triggers file, as the binary read it for the trigger editor
    /// (`plan/54`).
    pub fn triggers(&self, book: cena_ui::triggers::Book) {
        *lock(&self.shared.triggers) = Some(book);
        self.shared.window.wake();
    }

    /// Ask the player about an import that brings commands, before it
    /// writes anything (`plan/54` step 5).
    pub fn ask_import(&self, question: cena_ui::triggers::ImportQuestion) {
        *lock(&self.shared.import) = Some(question);
        self.shared.window.wake();
    }

    /// The import `id` was answered: its question goes.
    pub(crate) fn answered_import(&self, id: u64) {
        let mut import = lock(&self.shared.import);
        if import.as_ref().is_some_and(|question| question.id == id) {
            *import = None;
        }
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
            *lock(&self.shared.said) = Some((
                "Nothing here can start or stop characters.".to_owned(),
                Instant::now(),
            ));
            return;
        };
        let shared = Arc::clone(&self.shared);
        self.shared.runtime.spawn(async move {
            let said = control(request).await;
            // An empty answer says nothing: what was asked for is shown.
            if !said.is_empty() {
                *lock(&shared.said) = Some((said, Instant::now()));
            }
            shared.window.wake();
        });
    }

    /// Read `seat`'s player log for its log window, off the window's thread
    /// (`plan/25` §5): the writer asked to flush first, then `ask` run on a
    /// blocking task, its answer put in `inbox` and the window woken.
    pub(crate) fn read_log(
        &self,
        seat: &Arc<Seat>,
        ask: crate::logs::Ask,
        inbox: crate::logs::Inbox,
    ) {
        let (shared, handle, name) = (
            Arc::clone(&self.shared),
            seat.handle.clone(),
            seat.name.clone(),
        );
        self.shared.runtime.spawn(async move {
            handle.flush_player_log().await;
            let root = cena_session::player_log::writer::root();
            let read =
                tokio::task::spawn_blocking(move || crate::logs::run(&root, &name, ask)).await;
            let reply = read.unwrap_or_else(|_| {
                crate::logs::Reply::Days(Err("the read stopped before it finished".to_owned()))
            });
            lock(&inbox).push(reply);
            shared.window.wake();
        });
    }

    /// Run the trigger editor's *What would it have caught?* off the
    /// window's thread (`plan/54` step 4): the character's player log
    /// flushed first when it is playing, the answer in `inbox`, the window
    /// woken.
    pub(crate) fn check_log(
        &self,
        ask: crate::triggers::catch::Ask,
        inbox: crate::triggers::catch::Inbox,
    ) {
        let shared = Arc::clone(&self.shared);
        let handle = lock(&self.shared.seats)
            .iter()
            .find(|seat| seat.name.eq_ignore_ascii_case(&ask.character))
            .map(|seat| seat.handle.clone());
        self.shared.runtime.spawn(async move {
            if let Some(handle) = handle {
                handle.flush_player_log().await;
            }
            let root = cena_session::player_log::writer::root();
            let caught =
                tokio::task::spawn_blocking(move || crate::triggers::catch::run(&root, &ask))
                    .await
                    .unwrap_or_else(|_| crate::triggers::catch::Caught {
                        why: Some("the check stopped before it finished".to_owned()),
                        ..crate::triggers::catch::Caught::default()
                    });
            *lock(&inbox) = Some(caught);
            shared.window.wake();
        });
    }

    /// What the hub draws this frame.
    pub(crate) fn glance(&self) -> Glance {
        self.glance_at(Instant::now())
    }

    /// What the hub draws at `now`: the answer only while it is fresh.
    fn glance_at(&self, now: Instant) -> Glance {
        let said = lock(&self.shared.said).as_ref().and_then(|(said, at)| {
            SAID_FOR
                .checked_sub(now.saturating_duration_since(*at))
                .filter(|left| !left.is_zero())
                .map(|left| (said.clone(), left))
        });
        Glance {
            cards: self.cards(),
            offered: lock(&self.shared.offered).clone(),
            roster: lock(&self.shared.roster).clone(),
            listing: lock(&self.shared.listing).clone(),
            settings: lock(&self.shared.settings).clone(),
            triggers: lock(&self.shared.triggers).clone(),
            import: lock(&self.shared.import).clone(),
            merged: lock(&self.shared.merged).lines().cloned().collect(),
            said,
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
        let seat = Arc::new(Seat::new(handle, name, cena_session::DEFAULT_GAME_CODE));
        self.seats().push(Arc::clone(&seat));
        seat
    }

    /// Send `line` on `seat`'s character as the player typed it, on the
    /// connection its window last saw: a script's input hooks first, then
    /// Hydra's command line, then the game (`SessionHandle::send_typed_at`).
    /// It is echoed in the story at
    /// once; a line that may not have gone is said in Hydra's pane.
    pub(crate) fn send(&self, seat: &Arc<Seat>, line: String) {
        if refused(seat, &line) {
            return;
        }
        lock(&seat.story).typed(&line);
        self.deliver(seat, line);
    }

    /// Send `line` as [`Self::send`] does, but not echoed in the story: a
    /// menu asked for on a click, which the player did not type.
    pub(crate) fn send_quietly(&self, seat: &Arc<Seat>, line: String) {
        if refused(seat, &line) {
            return;
        }
        self.deliver(seat, line);
    }

    /// `line`, one command, sent on the connection `seat`'s window last saw.
    fn deliver(&self, seat: &Arc<Seat>, line: String) {
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
                .send_typed_at(generation, &line, SEND_DEADLINE)
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

/// Whether `line` is refused, not one command -- a key bound to two lines,
/// say -- and said so in Hydra's pane: everything the window sends passes
/// here (`cena_ui::validate_line`; the crate review of 2026-09-28, R10).
fn refused(seat: &Seat, line: &str) -> bool {
    let Err(why) = cena_ui::validate_line(line) else {
        return false;
    };
    lock(&seat.story).tell(Notice::line(NoticeKind::Warn, format!("Not sent: {why}.")));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_shows_for_a_while_then_goes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        let sessions = Sessions::new(runtime.handle().clone());
        let before = Instant::now();
        // With no control, the answer is said at once.
        sessions.ask(HubRequest::Shutdown);
        let after = Instant::now();

        let almost = before + SAID_FOR.saturating_sub(Duration::from_millis(1));
        let (said, left) = sessions.glance_at(almost).said.expect("still fresh");
        assert_eq!(said, "Nothing here can start or stop characters.");
        // It goes when it said it would: the window's next frame is then.
        assert!(sessions.glance_at(almost + left).said.is_none());
        assert!(sessions.glance_at(after + SAID_FOR).said.is_none());
    }

    /// A handle with no session behind it.
    fn handle() -> SessionHandle {
        SessionHandle::new(
            tokio::sync::mpsc::channel(1).0,
            cena_session::GenerationCell::default(),
            tokio::sync::broadcast::channel(1).0,
        )
    }

    /// Another window's character is offered to follow by its name on the
    /// same game, and with its game on another: one name on two games is
    /// two characters, and a name alone follows the one on the window's own
    /// game (the crate review of 2026-09-28, R6).
    #[test]
    fn one_name_on_another_game_is_followed_by_its_game() {
        let seat = |name: &str, game: &str| Seat::new(handle(), name, game);
        let (mine, same, other) = (
            seat("Ashryn", "GS3"),
            seat("Baelor", "GS3"),
            seat("Baelor", "GSF"),
        );
        assert_eq!(same.seen_from(&mine).name, "Baelor");
        assert_eq!(other.seen_from(&mine).name, "GSF:Baelor");
        assert_eq!(
            lock(&other.card).game,
            "GSF",
            "its card says its game, for the launcher"
        );
    }
}
