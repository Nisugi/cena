//! `--character Nisugi --character Nerten`: several characters in one Hydra
//! (`plan/29`, the binary on the session table).
//!
//! Every login is settled **before** anything connects: a character the
//! roster knows (`roster.rs`) takes its account and game from there and its
//! password from the ladder (`secrets.rs`); one it does not is asked for, at
//! the terminal. So the prompts happen once, up front, one after another --
//! and a run whose characters are all in the roster and the keyring needs no
//! terminal at all, which is the headless run the author wants.
//!
//! Then each goes on the table (`cena_host::Host`) with everything a session
//! is given (`setup.rs`), Hydra's command line and travel. The terminal shows
//! Hydra's own lines, each tagged with its character, and **no game text**:
//! two characters' story in one window is what `plan/29` §5a R2 rules out.
//! Ctrl-C quits every session together.
//!
//! **The only run path** (`plan/30` §2): `--headless` or `--web` with no
//! `--character` asks for one at the terminal; the window (`gui.rs`) starts
//! with none and runs the same table.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cena_host::{Host, Who, stop_all};
use cena_session::{Event, SessionHandle, SessionId, SessionObserver, State, StoppedBecause};
use cena_ui::{HubControl, HubRequest};

use crate::ask::{self, Typed};
use crate::commands::Commands;
use crate::connector::LiveConnector;
use crate::{
    batch, connector, frontend, interrupt, launcher, learn, loot, pages, roster, secrets, setup,
    sorter, travel, triggers, watch,
};

/// The characters named with `--character`, in order. Empty means none was
/// named, and `main` asks for one at the prompt.
pub(crate) fn characters() -> Vec<String> {
    characters_in(std::env::args().skip(1))
}

/// Whether this run has no window: `--headless`, or `--web`, whose browser
/// pages are the frontend (the author: *"keep despana like it is"*,
/// `plan/47` §1). Anything else opens the window.
pub(crate) fn headless() -> bool {
    headless_in(std::env::args().skip(1))
}

/// [`headless`], over any argument list, so it can be tested.
fn headless_in(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter()
        .any(|arg| arg == "--headless" || arg == "--web")
}

/// [`characters`], over any argument list, so it can be tested.
fn characters_in(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut named = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if let Some(name) = arg.strip_prefix("--character=") {
            named.push(name.to_owned());
        } else if arg == "--character"
            && let Some(name) = args.next()
        {
            named.push(name);
        }
    }
    named
}

/// One started session's loose ends, for its stop.
struct Started {
    character: String,
    /// The roster name it logs in by, `GAME:Name`, for a reconnect from the
    /// hub.
    login: String,
    watcher: tokio::task::JoinHandle<()>,
    /// The combat recorder's and the loot ledger's flushes, when recording.
    records: Vec<std::thread::JoinHandle<()>>,
    player: tokio::task::JoinHandle<u64>,
}

/// Everything the running characters share -- the table, their loose ends,
/// the web frontend -- behind locks, so the hub page can add and remove
/// characters while Hydra runs (`plan/29` step 5c).
struct Table {
    map: crate::map_context::ConfiguredMap,
    /// Behind an `Arc` so each character's `;to` and `;all` can look it up
    /// (`relay.rs`), holding it weakly.
    host: Arc<tokio::sync::Mutex<Host>>,
    started: std::sync::Mutex<BTreeMap<SessionId, Started>>,
    web: Option<frontend::Frontend>,
    /// The window's sessions, when this run has a window (`gui.rs`).
    gui: Option<cena_gui::Sessions>,
    /// What every character's hunt shares: the groups' boards and the
    /// characters' seats (`crate::hunt::Party`).
    party: crate::hunt::Party,
    dir: PathBuf,
    pin: PathBuf,
    turn: Arc<std::sync::Mutex<()>>,
    /// The Ctrl-C token: the hub's Shut down cancels it, and the run ends by
    /// the one orderly path either way.
    interrupt: tokio_util::sync::CancellationToken,
    /// The desk every character's sounds and notifications go to
    /// (`attention.rs`): one, so a thing several see sounds once.
    attention: std::sync::mpsc::Sender<crate::attention::Called>,
    /// Word of each change to the one triggers file, so every character
    /// reads it again (`triggers::Changes`).
    changes: triggers::Changes,
    /// Word that the roster or a kept password changed with no request to
    /// answer -- a login proven `Ready` -- so the hubs are offered it again.
    roster: Arc<tokio::sync::Notify>,
}

/// Run every named character, with no window, until Ctrl-C or until all
/// have stopped.
///
/// # Errors
///
/// A login could not be settled -- a character unknown to the roster with
/// nobody at the terminal, or no password -- before anything connected; or
/// every session's login was refused.
pub(crate) async fn play(names: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let logins = settle(&names)?;
    serve(logins, None, interrupt::on_ctrl_c()).await?;
    Ok(())
}

/// Settle every named character's login, in order, before anything
/// connects: from the roster and the ladder, or asked for at the terminal.
///
/// # Errors
///
/// A login could not be settled: a character unknown to the roster with
/// nobody at the terminal, or no password.
pub(crate) fn settle(names: &[String]) -> std::io::Result<Vec<Typed>> {
    let dir = cena_session::character_store::data_dir();
    names.iter().map(|name| login_for(&dir, name)).collect()
}

/// Put `logins` on the session table and run it until `interrupt` is
/// cancelled -- or, with no frontend to start another, until every
/// character has stopped -- then quit them all. The window, when there is
/// one, is `gui`, and is closed at the end.
///
/// # Errors
///
/// Every session's login was refused.
pub(crate) async fn serve(
    logins: Vec<Typed>,
    gui: Option<cena_gui::Sessions>,
    interrupt: tokio_util::sync::CancellationToken,
) -> Result<(), String> {
    let dir = cena_session::character_store::data_dir();
    eprintln!();
    let map = crate::map_context::load();
    let table = Arc::new(Table {
        host: Arc::new(tokio::sync::Mutex::new(Host::new())),
        started: std::sync::Mutex::default(),
        // One listener for every character, each with its own page; each
        // page's link is printed when its character is `Ready`.
        web: frontend::Frontend::open(&map).await,
        party: crate::hunt::Party::new(gui.clone()),
        gui,
        map,
        pin: dir.join(cena_platform::PIN_FILENAME),
        attention: crate::attention::start(&dir),
        changes: triggers::Changes::new(),
        roster: Arc::default(),
        dir,
        turn: Arc::default(),
        interrupt: interrupt.clone(),
    });
    for typed in logins {
        if let Err(e) = table.start(typed).await {
            eprintln!("{e}");
        }
    }
    // Both hubs are answered the same way, by the table.
    let answering = Arc::clone(&table);
    let control: HubControl = Arc::new(move |request| {
        let table = Arc::clone(&answering);
        Box::pin(async move { table.answer(request).await })
    });
    if let Some(web) = &table.web {
        web.announce_hub();
        web.sessions().control(Arc::clone(&control));
    }
    if let Some(gui) = &table.gui {
        gui.control(control);
    }
    table.offer().await;
    let offering = Arc::clone(&table);
    tokio::spawn(async move {
        loop {
            offering.roster.notified().await;
            offering.offer().await;
        }
    });

    eprintln!("[play] running; Ctrl-C quits every character");
    // With the hub up, no character left running is not the end: the hub can
    // start one again, and quitting the last from it shut Hydra down under
    // the page (author's live run, 2026-09-24). Only Ctrl-C ends a --web run,
    // and a windowed one ends when its window closes too (`gui.rs`).
    // Without either, nothing could start another, so all stopped is the end.
    if table.web.is_some() || table.gui.is_some() {
        interrupt.cancelled().await;
    } else {
        tokio::select! {
            () = interrupt.cancelled() => {}
            () = table.all_stopped() => eprintln!("[play] every session has stopped"),
        }
    }
    if let Some(web) = &table.web {
        web.shutdown().await;
    }
    eprintln!("\n[disconnect] quitting every session");
    let (stopped, refused) = Box::pin(table.stop_everything()).await;
    if let Some(gui) = &table.gui {
        gui.close();
    }
    if refused > 0 && refused == stopped {
        return Err("every login was refused".to_owned());
    }
    Ok(())
}

impl Table {
    /// Put one character on the table, and start what hangs off it.
    async fn start(&self, typed: Typed) -> Result<SessionId, String> {
        let character = typed.character.clone();
        let (account, game) = (typed.account.clone(), typed.game_code.clone());
        let proven = Proven::of(&typed);
        let login = format!("{game}:{character}");
        let who = Who {
            account: account.clone(),
            character: character.clone(),
        };
        let connector = LiveConnector::new(typed, connector::login_provider(), self.pin.clone());
        let mut host = self.host.lock().await;
        let mut attached = None;
        let id = host
            .add(who, connector, |session| {
                // Subscribed before it runs, so the watcher sees the whole
                // login, and the sync hears the store's report mid-burst.
                let (_, events) = session.subscribe();
                let (_, learning) = session.subscribe();
                let (_, calls) = session.subscribe();
                let (_, acts) = session.subscribe();
                let (session, records, player) =
                    setup::attach(session, &character, &game, &account);
                attached = Some((events, learning, calls, acts, records, player));
                session
            })
            .map_err(|e| format!("[{character}] not started: {e}"))?;
        let (Some((events, learning, calls, acts, records, player)), Some(hosted)) =
            (attached, host.get(id))
        else {
            return Err(format!(
                "[{character}] not started: it left the table at once"
            ));
        };
        let commands = Commands::install(&hosted.handle);
        crate::relay::open(&hosted.handle, &commands, self.characters());
        let kept = crate::general::Kept::of(&self.dir, &login);
        sorter::open(&hosted.handle, &commands, kept);
        // Following before the first read, so no change falls between.
        let following = self.changes.follow();
        triggers::open(&hosted.handle, &self.dir, &character);
        triggers::command(
            &hosted.handle,
            &commands,
            self.dir.clone(),
            character.clone(),
            self.changes.clone(),
        );
        batch::open(&hosted.handle, &hosted.observer, &commands);
        loot::reports(&hosted.handle, &commands, &self.dir, &game, &character);
        if let Some(web) = &self.web {
            web.attach(
                Some(&character),
                hosted.observer.clone(),
                hosted.handle.clone(),
            );
        }
        if let Some(gui) = &self.gui {
            gui.attach(&character, hosted.observer.clone(), &hosted.handle);
        }
        let watcher = tokio::spawn(watch::watch_events(events, format!("[{character}]")));
        tokio::spawn(crate::attention::forward(
            calls,
            character.clone(),
            self.attention.clone(),
        ));
        // A trigger's send goes as if typed, through this character's
        // command table (`plan/45` Stage 5), and another character's change
        // to the triggers file is read again here.
        tokio::spawn(triggers::run(
            acts,
            following,
            hosted.handle.clone(),
            self.dir.clone(),
            character.clone(),
        ));
        proven.on_ready(&hosted.observer, &self.turn, &self.roster);
        tokio::spawn(after_ready(
            hosted.handle.clone(),
            hosted.observer.clone(),
            commands,
            learning,
            format!("[{character}]"),
            self.map.clone(),
            self.party.clone(),
        ));
        self.started
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                id,
                Started {
                    character,
                    login,
                    watcher,
                    records,
                    player,
                },
            );
        Ok(id)
    }

    /// Answer a hub (`cena_ui::HubRequest`), Despana's or the window's,
    /// with one line for it.
    async fn answer(&self, request: HubRequest) -> String {
        let said = match request {
            HubRequest::Add(name) => match hub_login(&self.dir, &name) {
                Ok(typed) => match self.start(typed).await {
                    Ok(_) => format!("Starting {name}."),
                    Err(e) => e,
                },
                Err(e) => e,
            },
            HubRequest::Remove(id) => self.remove(SessionId(id)).await,
            HubRequest::Login(login) => {
                let name = login.character.trim().to_owned();
                match self.start(crate::ask::from_window(&login)).await {
                    Ok(_) => format!("Logging {name} in."),
                    Err(e) => e,
                }
            }
            HubRequest::Forget(name) => launcher::forget(&self.dir, &name),
            HubRequest::ForgetPassword(account) => launcher::forget_password(&account),
            HubRequest::Favourite(name, star) => launcher::favourite(&self.dir, &name, star),
            HubRequest::Remember(saved) => launcher::remember(&self.dir, &saved),
            HubRequest::Characters(account) => {
                launcher::characters(&self.pin, account, self.gui.as_ref()).await
            }
            HubRequest::Settings(character) => {
                pages::send(&self.dir, &character, self.gui.as_ref())
            }
            HubRequest::Change(change) => {
                let said = pages::apply(&self.dir, &change);
                // The symbol and the sorter reach a running character at once.
                if crate::general::owns(&change.page)
                    && let Some(kept) = crate::general::Kept::of(&self.dir, &change.character)
                    && let Some(handle) = self.handle_of(&change.character).await
                {
                    kept.take(&handle);
                }
                let problem = pages::send(&self.dir, &change.character, self.gui.as_ref());
                if problem.is_empty() { said } else { problem }
            }
            HubRequest::Reconnect(id) => self.reconnect(SessionId(id)).await,
            HubRequest::Shutdown => {
                eprintln!("[play] shut down from the hub");
                self.interrupt.cancel();
                return "Hydra is shutting down: every character is quitting.".to_owned();
            }
        };
        self.offer().await;
        said
    }

    /// Quit session `id` and take it off the table: its page is detached and
    /// its `quit` sent. Its logs are flushed in the background, so the answer
    /// does not wait on them.
    async fn remove(&self, id: SessionId) -> String {
        match self.take_off(id).await {
            Some((character, _)) => format!("{character} has quit."),
            None => "That character is no longer on the table.".to_owned(),
        }
    }

    /// The handle of the character the roster names `login`, while it is
    /// on the table.
    async fn handle_of(&self, login: &str) -> Option<cena_session::SessionHandle> {
        let id = self
            .started
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|(_, one)| one.login.eq_ignore_ascii_case(login))
            .map(|(id, _)| *id)?;
        let host = self.host.lock().await;
        host.get(id).map(|hosted| hosted.handle.clone())
    }

    /// Log a stopped character back in: off the table, then started again
    /// from the roster and the ladder, as the hub's Add does.
    async fn reconnect(&self, id: SessionId) -> String {
        let running = self
            .host
            .lock()
            .await
            .get(id)
            .map(cena_host::Hosted::is_running);
        match running {
            None => "That character is no longer on the table.".to_owned(),
            Some(true) => "That character is still connected.".to_owned(),
            Some(false) => {
                let Some((character, login)) = self.take_off(id).await else {
                    return "That character is no longer on the table.".to_owned();
                };
                match hub_login(&self.dir, &login) {
                    Ok(typed) => match self.start(typed).await {
                        Ok(_) => format!("Reconnecting {character}."),
                        Err(e) => e,
                    },
                    Err(e) => e,
                }
            }
        }
    }

    /// Take session `id` off the table and stop it; its loose ends are closed
    /// in the background. Its character and roster name, when it was there.
    async fn take_off(&self, id: SessionId) -> Option<(String, String)> {
        let hosted = self.host.lock().await.take(id)?;
        if let Some(web) = &self.web {
            web.detach(id);
        }
        if let Some(gui) = &self.gui {
            gui.detach(id);
        }
        let end = hosted.stop().await;
        let one = self
            .started
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id)?;
        self.party.unseat(&one.character);
        let named = (one.character.clone(), one.login.clone());
        // A character still in the process holds its logs open (`setup`'s
        // FLUSH_WAIT has why); the hub is not made to wait for that.
        tokio::spawn(finish(one, end));
        Some(named)
    }

    /// Tell the hubs which characters they can add: in the roster, with a saved
    /// password, and not running. A name on two games is offered as
    /// `GAME:Name`.
    async fn offer(&self) {
        if self.web.is_none() && self.gui.is_none() {
            return;
        }
        let running: Vec<String> = self
            .host
            .lock()
            .await
            .sessions()
            .filter(|(_, hosted)| hosted.is_running())
            .map(|(_, hosted)| hosted.who.character.to_lowercase())
            .collect();
        let roster = roster::all(&self.dir).unwrap_or_default();
        let available = roster
            .iter()
            .filter(|e| !running.contains(&e.character.to_lowercase()))
            .filter(|e| secrets::saved(&e.account))
            .map(|e| {
                let twice = roster
                    .iter()
                    .filter(|other| other.character.eq_ignore_ascii_case(&e.character))
                    .count()
                    > 1;
                if twice {
                    format!("{}:{}", e.game_code, e.character)
                } else {
                    e.character.clone()
                }
            })
            .collect::<Vec<String>>();
        if let Some(web) = &self.web {
            web.sessions().offer(available.clone());
        }
        if let Some(gui) = &self.gui {
            gui.offer(available);
            gui.roster(
                roster
                    .iter()
                    .map(|entry| entry.card(secrets::saved(&entry.account)))
                    .collect(),
            );
        }
    }

    /// The characters running now, for a relay (`relay.rs`): looked up when
    /// the relay is typed, through a weak hold, so a character's command
    /// line never keeps the table alive.
    fn characters(&self) -> crate::relay::Characters {
        let host = Arc::downgrade(&self.host);
        Arc::new(move || {
            let host = host.clone();
            Box::pin(async move {
                let Some(host) = host.upgrade() else {
                    return Vec::new();
                };
                let host = host.lock().await;
                host.sessions()
                    .filter(|(_, hosted)| hosted.is_running())
                    .map(|(_, hosted)| (hosted.who.character.clone(), hosted.handle.clone()))
                    .collect()
            })
        })
    }

    /// Resolves when no session on the table is still running.
    async fn all_stopped(&self) {
        loop {
            let running = self
                .host
                .lock()
                .await
                .sessions()
                .any(|(_, hosted)| hosted.is_running());
            if !running {
                return;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Stop every session together, and report each. Returns how many were
    /// stopped and how many of those had their login refused.
    async fn stop_everything(&self) -> (usize, usize) {
        let hosted = self.host.lock().await.take_all();
        let ids: Vec<SessionId> = hosted.iter().map(|one| one.handle.session()).collect();
        let ended = stop_all(hosted).await;
        let mut refused = 0;
        let mut stopped = 0;
        for (id, end) in ids.into_iter().zip(ended) {
            let one = self
                .started
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&id);
            if let Some(one) = one {
                stopped += 1;
                if matches!(
                    end.as_ref().map(|e| &e.stopped_because),
                    Some(StoppedBecause::Fatal(_))
                ) {
                    refused += 1;
                }
                finish(one, end).await;
            }
        }
        (stopped, refused)
    }
}

/// Close one stopped session's loose ends, and say how it ended.
async fn finish(one: Started, end: Option<cena_session::SupervisedEnd>) {
    one.watcher.abort();
    setup::flush_records(one.records).await;
    setup::flush_player_log(one.player).await;
    let character = &one.character;
    match end.map(|end| end.stopped_because) {
        Some(StoppedBecause::Fatal(error)) => eprintln!("[{character}] never started: {error}"),
        Some(because) => eprintln!("[{character}] stopped: {because:?}"),
        None => eprintln!("[{character}] did not end cleanly; its task was aborted"),
    }
}

/// Settle one character's login: from the roster, or asked for.
fn login_for(dir: &Path, name: &str) -> std::io::Result<Typed> {
    match roster::find(dir, name)? {
        Some(entry) => ask::from_roster(&entry, std::io::stdin().is_terminal()),
        // A `GAME:` prefix names a game for a known character; for a new
        // one, the game is asked for with the account.
        None => ask::ask_for(name.split_once(':').map_or(name, |(_, n)| n)),
    }
}

/// A login for the hub page: the roster and the ladder, **never a prompt** --
/// nobody is at the terminal for a request made in a browser, and no
/// password crosses the browser (`plan/29` step 5c, author's choice).
fn hub_login(dir: &Path, name: &str) -> Result<Typed, String> {
    match roster::find(dir, name) {
        Ok(Some(entry)) => ask::from_roster(&entry, false).map_err(|e| e.to_string()),
        Ok(None) => Err(format!(
            "{name} has not logged in through Hydra yet; log in once from the command line."
        )),
        Err(e) => Err(e.to_string()),
    }
}

/// Once the login is proven, open travel, then sync the character if the
/// store said anything is stale (`learn.rs`).
async fn after_ready(
    handle: SessionHandle,
    observer: SessionObserver,
    commands: Commands,
    mut learning: tokio::sync::broadcast::Receiver<Event>,
    who: String,
    map: crate::map_context::ConfiguredMap,
    party: crate::hunt::Party,
) {
    let Some(stale) = learn::stale_at_ready(&mut learning).await else {
        return;
    };
    drop(learning);
    travel::after_login(&handle, observer, &commands, &map, &party).await;
    learn::sync(&handle, &stale, &who).await;
}

/// What a login leaves behind once it is proven: its roster entry, and --
/// for a typed password -- the offer to keep it in the keyring. Taken from
/// the login before it is handed to the connector, started once the session
/// exists.
pub(crate) struct Proven {
    entry: roster::Entry,
    typed_password: Option<(String, String)>,
    /// A password typed in the window's launcher with its box ticked: kept
    /// in the keyring once the login is proven, without a question.
    kept: Option<(String, String)>,
}

impl Proven {
    pub(crate) fn of(typed: &Typed) -> Self {
        Self {
            entry: roster::Entry::of(typed),
            typed_password: (typed.password_from == secrets::Source::Prompt)
                .then(|| (typed.account.clone(), typed.password.clone())),
            kept: (typed.password_from == secrets::Source::Window { keep: true })
                .then(|| (typed.account.clone(), typed.password.clone())),
        }
    }

    /// When `observer`'s login reaches `Ready`: record the roster entry, keep
    /// a password the window's box asked to keep, and offer one typed at the
    /// terminal to the keyring, one question at a time (`turn`). Each change
    /// is word to `changed`.
    pub(crate) fn on_ready(
        self,
        observer: &SessionObserver,
        turn: &Arc<std::sync::Mutex<()>>,
        changed: &Arc<tokio::sync::Notify>,
    ) {
        if let Some((account, password)) = self.typed_password {
            tokio::spawn(secrets::offer_to_remember(
                account,
                password,
                observer.clone(),
                Arc::clone(turn),
                Arc::clone(changed),
            ));
        }
        let observer = observer.clone();
        let (entry, kept, changed) = (self.entry, self.kept, Arc::clone(changed));
        tokio::spawn(async move {
            if until_ready(&observer).await {
                remember(&cena_session::character_store::data_dir(), entry);
                if let Some((account, password)) = kept {
                    match secrets::keep(&account, &password) {
                        Ok(()) => eprintln!("[login] saved to the OS keyring for {account}"),
                        Err(e) => eprintln!("[login] the OS keyring would not save it ({e})"),
                    }
                }
                changed.notify_one();
            }
        });
    }
}

/// Record `entry` in the roster, saying so if it cannot be.
fn remember(dir: &Path, entry: roster::Entry) {
    let character = entry.character.clone();
    if let Err(e) = roster::record(dir, entry) {
        eprintln!(
            "[{character}] could not be added to the roster ({e}); `--character {character}` will ask again"
        );
    }
}

/// Resolves `true` once `observer`'s session is `Ready`; `false` if it ended
/// first.
async fn until_ready(observer: &SessionObserver) -> bool {
    let Ok((snapshot, mut events)) = observer.subscribe().await else {
        return false;
    };
    if snapshot.lifecycle == State::Ready {
        return true;
    }
    loop {
        match events.recv().await {
            Ok(o) if o.event == Event::StateChanged(State::Ready) => return true,
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Proven, characters_in, headless_in, hub_login};

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn every_character_is_taken_in_order_in_either_spelling() {
        assert_eq!(
            characters_in(args("--web --character Nisugi --character=Nerten --hold 5")),
            ["Nisugi", "Nerten"]
        );
        assert_eq!(
            characters_in(args("--character GS3:Nisugi")),
            ["GS3:Nisugi"]
        );
    }

    /// The hub never prompts: a character that has not logged in before is
    /// told how to, rather than asked for a password nobody is there to type.
    #[test]
    fn the_hub_cannot_start_a_character_that_never_logged_in() {
        let dir = std::env::temp_dir().join(format!("cena-hub-login-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let Err(said) = hub_login(&dir, "Stranger") else {
            panic!("a character with no roster entry was started");
        };
        assert!(said.contains("command line"), "{said}");
    }

    /// Only `--headless` and `--web` keep the window shut; a character named
    /// alone opens it with that character started (`plan/47` §6 item 2).
    #[test]
    fn the_window_opens_unless_headless_or_web() {
        assert!(!headless_in(args("")));
        assert!(!headless_in(args("--character Nisugi --record")));
        assert!(headless_in(args("--character Nisugi --headless")));
        assert!(headless_in(args("--web --character Nisugi")));
    }

    /// A password typed in the window is kept once proven only when its box
    /// was ticked, and is never asked about at the terminal.
    #[test]
    fn a_window_password_is_kept_only_when_its_box_was_ticked() {
        let login = |remember| cena_ui::Login {
            account: "acct".to_owned(),
            password: cena_ui::Password::new("pw".to_owned()),
            game: "gst".to_owned(),
            character: "Ashryn".to_owned(),
            remember,
        };
        let ticked = Proven::of(&crate::ask::from_window(&login(true)));
        assert_eq!(ticked.kept, Some(("acct".to_owned(), "pw".to_owned())));
        assert_eq!(ticked.typed_password, None, "never asked at the terminal");
        let unticked = Proven::of(&crate::ask::from_window(&login(false)));
        assert_eq!((unticked.kept, unticked.typed_password), (None, None));
    }

    #[test]
    fn no_character_named_is_none() {
        assert!(characters_in(args("--web")).is_empty());
        // A trailing flag with no name names nobody, rather than the next run.
        assert!(characters_in(args("--character")).is_empty());
    }
}
