//! The window: what the binary opens with no `--headless` or `--web`
//! (`plan/47` step 2). The hub, over the sessions the binary attached, and a
//! play window for each character (step 4), each its own native window.
//!
//! eframe owns the main thread for as long as the window is open, so the
//! binary builds its runtime by hand and runs the sessions there; this only
//! draws what their feeds left on the seats, and hands what the player asks
//! for to the runtime.
//!
//! A character's play window opens when it starts. Closing it leaves the
//! character running headless (the author: *"when their running gui play
//! window is closed, if the connection isn't closed then they remain
//! headless"*); its hub card opens it again.
//!
//! Keybinds (step 7, `crate::keys`) send on the character whose play window
//! has the keyboard: a bound key is taken from that window's input, and the
//! numpad, which the fork hands over apart from any window, goes to the one
//! focused. Each character's own keys and the macro set it uses go over
//! every character's (`plan/52` step 2), read from its keys file when its
//! window first needs them.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cena_ui::LifecycleView;

use crate::hub::{HubAction, HubView};
use crate::keys::{self, Chord, KeyFile, Keybinds, Macro};
use crate::layout::Library;
use crate::own::Own;
use crate::placement::{self, Placements};
use crate::play::{Asked, Play, PlayView};
use crate::sessions::{Seat, lock};
use crate::widget::Character;
use crate::{Hub, Menu, Sessions};

/// The window's title: the product's name (`CLAUDE.md`: anything
/// user-facing is Hydra, not the working name).
pub const TITLE: &str = "Hydra";

/// The hub's key among the windows' places (`placement.rs`).
const HUB: &str = "hub";

/// The app eframe drives: the hub over [`Sessions`], and the play windows.
pub struct App {
    hub: Hub,
    sessions: Sessions,
    /// Each attached character's play window, open or not, by session.
    plays: BTreeMap<u32, Window>,
    /// Where play windows keep their layouts; `None`, and they keep none.
    layouts: Option<PathBuf>,
    /// Where play windows keep what was sent, per character; `None`, and
    /// each window keeps it until closed.
    histories: Option<PathBuf>,
    /// The presets a player saved, which every play window adds from
    /// (`plan/49` Stage A step 7), kept beside the layouts.
    presets: Library,
    /// The keybinds, and the file they are read from, when there is one.
    keys: Keybinds,
    keys_file: Option<PathBuf>,
    /// What a play window says of the keybinds: how many, and what is wrong.
    keys_said: Vec<String>,
    /// The fork is to be told again which keys to catch (`keyed.rs`).
    catch_again: bool,
    /// Each character's own keys, by its keys file, with what is wrong in
    /// it: read when first needed, and again once a change is written.
    mine: HashMap<PathBuf, (KeyFile, Vec<String>)>,
    /// The play window that has the keyboard, by session, and its
    /// character's keys file: whose keys the fork catches. `None` while no
    /// play window has it, and the fork catches none.
    focused: Option<(u32, Option<PathBuf>)>,
    /// The play window that had the keyboard this frame, by session.
    keyboard_now: Option<u32>,
    /// The keys the fork caught this frame -- the numpad's, and those egui
    /// has no name for -- bound for the play window with the keyboard.
    caught: Vec<Chord>,
    /// Commands a key's macro sends once a wait in it is over
    /// (`keyed.rs`).
    later: Vec<keyed::Later>,
    /// `NumLock`, as the last numpad press showed it.
    numlock: Option<bool>,
    /// The settings menu, the one every way in opens (`plan/50` §7).
    menu: Menu,
    /// Hydra's own settings, kept in the window's own file (step 2).
    own: Own,
    /// The fork was last told to hand every key it can catch to the Keys
    /// page.
    menu_waits: bool,
    /// A key the fork caught this frame while the Keys page waits for one.
    caught_for_page: Option<String>,
    /// On macOS, the numpad sends its keys rather than typing: switched by
    /// the Clear key, starting as the keybinds file's `numpad` says.
    clear_sends: bool,
    /// Each character's log window, once opened, by session (`logs.rs`).
    logs: BTreeMap<u32, crate::logs::Logs>,
    /// Where each window was, kept (`placement.rs`).
    placements: Placements,
    /// The trigger editor (`plan/54`).
    triggers: crate::triggers::Editor,
    /// The trigger import's question, while one is asked (`import.rs`).
    asking: trigger_import::Asking,
}

/// One character's play window.
#[derive(Debug)]
struct Window {
    play: Play,
    open: bool,
    /// Its session was closed, when last drawn.
    ended: bool,
}

impl App {
    /// The hub over `sessions`, on its first tab, its play windows keeping
    /// no layout.
    #[must_use]
    pub fn new(sessions: Sessions) -> Self {
        Self {
            hub: Hub::default(),
            sessions,
            plays: BTreeMap::new(),
            layouts: None,
            histories: None,
            presets: Library::default(),
            keys: Keybinds::default(),
            keys_file: None,
            keys_said: Vec::new(),
            catch_again: true,
            mine: HashMap::new(),
            focused: None,
            keyboard_now: None,
            caught: Vec::new(),
            later: Vec::new(),
            numlock: None,
            menu: Menu::default(),
            own: Own::default(),
            menu_waits: false,
            caught_for_page: None,
            clear_sends: false,
            logs: BTreeMap::new(),
            placements: Placements::default(),
            triggers: crate::triggers::Editor::default(),
            asking: trigger_import::Asking::default(),
        }
    }

    /// The same, keeping what it keeps in `data`, the data folder: play
    /// windows' layouts by character name (`plan/47` step 6), the keybinds
    /// read from it (step 7), and Hydra's own settings (`plan/50` §7 step 2).
    #[must_use]
    pub fn keeping(sessions: Sessions, data: &std::path::Path) -> Self {
        let own = Own::load(data);
        let mut app = Self {
            layouts: Some(data.join("layouts")),
            histories: Some(data.join("history")),
            presets: Library::load(Some(data.join("layouts"))),
            keys_file: Some(keys::path(data)),
            placements: Placements::load(data),
            ..Self::new(sessions)
        };
        app.hub.card_width = own.card_width();
        app.own = own;
        app.read_keys();
        app
    }

    /// Read the keybinds file again, and say what it bound; each
    /// character's own is read again when next needed.
    fn read_keys(&mut self) {
        self.mine.clear();
        let Some(file) = &self.keys_file else {
            return;
        };
        let (keys, problems) = Keybinds::load(file);
        self.keys_said = std::iter::once(match keys.changed() {
            0 => format!(
                "{} keys bound, all Hydra's: change them here, and the changes are kept in {}.",
                keys.len(),
                file.display()
            ),
            changed => format!(
                "{} keys bound, {changed} of them yours, from {}.",
                keys.len(),
                file.display()
            ),
        })
        .chain(problems)
        .collect();
        self.clear_sends = keys.numpad_always();
        self.keys = keys;
        self.catch_again = true;
    }

    /// Draw one frame into `ui` -- the hub, then each open play window --
    /// and act on what the player asked. What eframe calls each frame, and
    /// what a test drives directly.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        crate::carry::set_key(ui.ctx(), self.own.drag_with());
        let glance = self.sessions.glance();
        let seats = self.sessions.seated();
        self.seat(&seats);
        let named = |seat: &Seat| {
            glance
                .roster
                .iter()
                .find(|card| card.character.eq_ignore_ascii_case(&seat.name))
                .map_or_else(
                    || format!("{}:{}", seat.game, seat.name),
                    crate::menu::roster_name,
                )
        };
        if let Some((_, left)) = &glance.said {
            // A frame when the answer is due to go, or it stays until the
            // next input.
            ui.ctx().request_repaint_after(*left);
        }
        let windowed: Vec<u32> = self
            .plays
            .iter()
            .filter(|(_, window)| window.open)
            .map(|(session, _)| *session)
            .collect();
        let lich: Vec<u32> = seats
            .iter()
            .filter(|seat| seat.handle.lich_running())
            .map(|seat| seat.id.0)
            .collect();
        let view = HubView {
            cards: &glance.cards,
            offered: &glance.offered,
            roster: &glance.roster,
            listing: glance.listing.as_ref(),
            merged: &glance.merged,
            said: glance.said.as_ref().map(|(said, _)| said.as_str()),
            windowed: &windowed,
            lich: &lich,
        };
        match self.hub.show(ui, &view) {
            Some(HubAction::Ask(request)) => self.sessions.ask(request),
            Some(HubAction::Open(session)) => {
                if let Some(window) = self.plays.get_mut(&session) {
                    window.open = true;
                }
            }
            Some(HubAction::Settings) => self.menu.open_for(None),
            Some(HubAction::Triggers) => self.open_triggers(),
            Some(HubAction::Lich(session, on)) => {
                if let Some(seat) = seats.iter().find(|seat| seat.id.0 == session) {
                    self.hydras(seat, lich_word(on));
                }
            }
            Some(HubAction::Log(session)) => {
                if let Some(seat) = seats.iter().find(|seat| seat.id.0 == session) {
                    self.open_log(seat);
                }
            }
            None => {}
        }
        // A width a drag set is kept once the drag lets go.
        if ui.ctx().dragged_id().is_none()
            && let Err(why) = self.own.keep_width(self.hub.card_width)
        {
            self.menu.tell(why);
        }
        if let Some(next) = self.send_due(&seats, Instant::now()) {
            ui.ctx().request_repaint_after(next);
        }
        self.keyboard_now = None;
        for seat in &seats {
            // The character's own settings, by its roster name.
            match self.play(ui.ctx(), seat, &seats) {
                Some(Asked::Settings(page)) => {
                    self.menu.open_at(Some(named(seat)), page.as_deref());
                }
                Some(Asked::Keys) => self.menu.open_at(Some(named(seat)), Some("keys")),
                Some(Asked::Character(n)) => self.bring(ui.ctx(), &seats, n),
                Some(Asked::UseSet(set)) => {
                    let asked = crate::MenuAsked::Key {
                        character: Some(named(seat)),
                        change: crate::KeyChange::Choose(set),
                    };
                    self.menu_asked(asked);
                }
                _ => {}
            }
        }
        self.left_keyboard();
        self.settings(ui.ctx(), &glance);
        self.trigger_window(ui.ctx(), &glance);
        self.import_question(ui.ctx(), glance.import.as_ref());
        self.log_windows(ui.ctx(), &seats);
        if let Some(after) = self.placements.save_due(Instant::now()) {
            ui.ctx().request_repaint_after(after);
        }
    }

    /// A window for each seat new since the last frame, open; none for a
    /// seat gone.
    fn seat(&mut self, seats: &[Arc<Seat>]) {
        self.plays
            .retain(|session, _| seats.iter().any(|seat| seat.id.0 == *session));
        for seat in seats {
            let layouts = self.layouts.clone();
            let histories = self.histories.as_deref();
            self.plays.entry(seat.id.0).or_insert_with(|| {
                let play = Play::new(
                    seat.id.0,
                    &seat.name,
                    cena_session::instance(&seat.game),
                    layouts,
                );
                Window {
                    play: match histories {
                        Some(dir) => play.keeping_history(dir),
                        None => play,
                    },
                    open: true,
                    ended: false,
                }
            });
        }
    }

    /// Show `seat`'s play window, if open, and act on what it asked. The
    /// other `seats` are the characters its widgets may follow. The settings
    /// menu, when it asked for it, which the app opens.
    fn play(
        &mut self,
        context: &egui::Context,
        seat: &Arc<Seat>,
        seats: &[Arc<Seat>],
    ) -> Option<Asked> {
        let file = self.mine_path(&seat.game, &seat.name);
        self.load_mine(file.as_deref());
        let mine = file.as_ref().and_then(|file| self.mine.get(file));
        let keys = self.keys.of(mine.map(|(mine, _)| mine));
        let chosen = mine.map_or(0, |(mine, _)| mine.chosen);
        let keys_said = self.keys_said(mine, &seat.name);
        let caught = &mut self.caught;
        let numlock = self.numlock;
        let mut focused = false;
        let close_with_session = self.own.close_with_session();
        let window = self.plays.get_mut(&seat.id.0)?;
        let place = placement::key("play", &seat.game, &seat.name);
        let lifecycle = lock(&seat.card).lifecycle.clone();
        // Closed with its session when the player asked for that (`plan/50`
        // §6 item 11), once: reopened from its card, it stays open.
        let ended = matches!(lifecycle, LifecycleView::Closed { .. });
        if ended && !window.ended && close_with_session {
            window.open = false;
            self.placements.closed(&place);
        }
        window.ended = ended;
        if !window.open {
            return None;
        }
        let snapshot = lock(&seat.snapshot).clone();
        let hunt = lock(&seat.hunt).clone();
        let minimap = lock(&seat.where_now).clone();
        let others: Vec<Character> = seats
            .iter()
            .filter(|other| other.id != seat.id)
            .map(|other| other.seen_from(seat))
            .collect();
        let builder = self
            .placements
            .builder(&place, [980.0, 680.0])
            .with_title(format!("{} — {TITLE}", seat.name));
        // What every pass asks, kept. egui may draw a window's frame more
        // than once and keep only the last pass (`Context::run_ui`), as when
        // a layout settles -- a new window's first frame -- and a click or a
        // key is in the first pass alone. The settings menu's first opening
        // asked for a character's pages in a pass thrown away, noted them as
        // asked for, and waited for pages that never came (the author,
        // 2026-09-28: "when clicking on settings for the first time it only
        // shows widget settings").
        let (mut asked, mut sends, mut closed, mut seen) = (None, Vec::new(), false, None);
        context.show_viewport_immediate(
            egui::ViewportId::from_hash_of(("play", seat.id.0)),
            builder,
            |ui, _class| {
                closed |= ui.input(|input| input.viewport().close_requested());
                seen = Some(ui.input(|input| input.viewport().clone()));
                let (pressed, has) = keyed::pressed(ui, keys, &window.play, caught);
                focused |= has;
                for made in pressed {
                    match made {
                        Macro::Fill(text) => window.play.fill(&text),
                        Macro::Act(action) => {
                            asked =
                                asked
                                    .take()
                                    .or(keyed::asked(ui.ctx(), &mut window.play, action));
                        }
                        send @ Macro::Send(_) => sends.push(send),
                    }
                }
                let story = lock(&seat.story);
                let view = PlayView {
                    name: &seat.name,
                    lifecycle: &lifecycle,
                    snapshot: snapshot.as_deref(),
                    story: &story,
                    now: Instant::now(),
                    hunt: hunt.as_ref(),
                    minimap: minimap.as_ref(),
                    numlock,
                    keys: &keys_said,
                    set: chosen,
                    others: &others,
                    presets: &self.presets,
                    lich: seat.handle.lich_running(),
                };
                asked = asked.take().or(window.play.show(ui, &view));
                drop(story);
            },
        );
        if let Some(seen) = &seen {
            self.placements.note(&place, seen, Instant::now());
        }
        if closed {
            window.open = false;
            self.placements.closed(&place);
        }
        if focused {
            self.took_keyboard(seat.id.0, file);
        }
        if let Some(after) = clocks_run(snapshot.as_deref(), &seat.story) {
            context.request_repaint_after(after);
        }
        self.send_macros(context, seat, sends);
        self.asked(seat, asked?)
    }

    /// The window was asked to close. With a character still playing, it
    /// asks first, as the hub's Shut down does, and stays open; otherwise it
    /// closes, and the binary quits whatever is left. Returns whether it
    /// closes.
    pub fn close_asked(&mut self) -> bool {
        let playing = self
            .sessions
            .cards()
            .iter()
            .any(|card| !matches!(card.lifecycle, LifecycleView::Closed { .. }));
        if playing && !self.sessions.closing() {
            self.hub.confirm_shutdown();
            return false;
        }
        self.placements.save();
        true
    }
}

/// When something in a play window counts down by itself -- roundtime,
/// cast time, a banner, an effect's time left -- how soon it must be drawn
/// again without an event to prompt it: the clocks a quarter of a second,
/// an effect, which counts whole seconds, a second.
fn clocks_run(
    snapshot: Option<&cena_session::Snapshot>,
    story: &std::sync::Mutex<crate::story::Story>,
) -> Option<Duration> {
    let state = snapshot.map(|snapshot| &snapshot.state);
    let clocks = state.is_some_and(|state| {
        state.roundtime_remaining().is_some_and(|s| s > 0)
            || state.casttime_remaining().is_some_and(|s| s > 0)
    });
    if clocks || lock(story).alerts_at(Instant::now()).next().is_some() {
        return Some(Duration::from_millis(250));
    }
    let effects = state.is_some_and(|state| {
        state.game_time_now().is_some_and(|now| {
            state
                .effects
                .iter()
                .any(|(id, _)| state.effects.remaining(id, now).is_some_and(|s| s > 0))
                || state
                    .world
                    .pulse
                    .as_ref()
                    .and_then(|pulse| pulse.due(now))
                    .is_some_and(|(_, most)| most > 0)
        })
    });
    effects.then_some(Duration::from_secs(1))
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.catch(frame);
        self.caught_pressed(
            frame.numpad_keys(),
            frame.captured_keys(),
            cfg!(target_os = "macos"),
        );
        if ui.ctx().input(|input| input.viewport().close_requested()) && !self.close_asked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        // The hub's own place: eframe's root window, placed by `run`.
        let seen = ui.ctx().input(|input| input.viewport().clone());
        self.placements.note(HUB, &seen, Instant::now());
        egui::CentralPanel::default().show(ui, |ui| self.draw(ui));
    }
}

/// Open the window over `sessions` and run it until it closes: by the
/// player, or by [`Sessions::close`] once the run is over. Blocks the
/// calling thread, which must be the main one.
///
/// # Errors
///
/// The window could not be opened: no display, or no graphics adapter.
pub fn run(sessions: Sessions) -> eframe::Result {
    let data = cena_session::character_store::data_dir();
    let options = eframe::NativeOptions {
        viewport: Placements::load(&data)
            .builder(HUB, [560.0, 720.0])
            .with_title(TITLE),
        ..Default::default()
    };
    eframe::run_native(
        TITLE,
        options,
        Box::new(move |creation| {
            sessions.opened(&creation.egui_ctx);
            Ok(Box::new(App::keeping(sessions, &data)))
        }),
    )
}

mod asked;
use asked::lich_word;
mod import;
mod keyed;
mod logs;
mod settings;
#[cfg(test)]
mod tests;
mod trigger_import;
mod triggers;
