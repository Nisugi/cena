//! Hunt, wired to this binary: `;hunt <name>`, `;hunt stop`, `;hunt import`,
//! `;hunt import-loot`, `;hunt check` and `;hunt list` on Hydra's command
//! line (`crate::commands`).
//!
//! The join only, as `travel.rs` is for travel: what a command means and
//! what it does are `cena_behavior::hunt`'s; where the data directory is,
//! who the character is and which map is loaded are known here. The map is
//! travel's, loaded once and shared: a hunt walks with travel's own driver,
//! so without a map there is no hunt, and `;hunt <name>` says so.
//!
//! # BUILT, NOT RUN
//!
//! Like everything in this binary that reaches the game (`main.rs`'s module
//! docs), this has been compiled and never executed: only the author runs
//! the binary (`CLAUDE.md`, Credentials). What it calls is tested in
//! `cena-behavior`; what is untested is the wiring here.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

mod caster;
mod panel;
pub(crate) mod settings;

use crate::commands::{Commands, Took};
use cena_behavior::group::{Boards, Place};
use cena_behavior::hunt::{self, Command, Desk, LoadError, parse_command};
use cena_behavior::loot;
use cena_behavior::spellcaster;
use cena_session::{AuthorityToken, GameState, Notice, NoticeKind, SessionHandle, SessionObserver};

/// Register hunt's words. The character's instance and name, when the login
/// has said them, choose the character level of the chain; `map` is the one
/// travel loaded, and `None` when travel has none; `party` is what every
/// character's hunt shares.
pub(crate) fn open(
    handle: &SessionHandle,
    observer: SessionObserver,
    state: &GameState,
    commands: &Commands,
    map: Option<Arc<crate::map_context::MapContext>>,
    party: &Party,
) {
    let who = state
        .character
        .instance
        .clone()
        .zip(state.character.name.clone());
    let dir = cena_session::character_store::data_dir();
    let desk = map.map(|context| {
        let desk = Desk::with_map_sha256(
            Arc::clone(&context.map),
            dir.clone(),
            AuthorityToken(3),
            context.sha256.clone(),
        );
        desk.group_on(Arc::clone(&party.boards));
        desk
    });
    if let Some(desk) = desk.clone() {
        commands.stops("hunt", Arc::new(move || desk.stop()));
    }
    if let (Some(desk), Some((_, name))) = (&desk, &who) {
        take_seat(party, name, desk, handle, &observer);
    }
    if let Some(desk) = &desk {
        panel::show(desk, handle.session(), party.window.as_ref());
    }
    let leader = who.as_ref().map(|(_, name)| name.clone());
    // The spellcaster profile, held so a typed line is judged without
    // reading the file, and read again when the file has changed: by `;sc`,
    // the settings menu, or a hand (`plan/50` §2 item 3).
    let caster = Arc::new(Mutex::new(caster::Caster::read(&dir, who.as_ref())));
    if let Some(desk) = desk.clone() {
        let (handler, observer, caster) = (handle.clone(), observer.clone(), Arc::clone(&caster));
        let (dir, who) = (dir.clone(), who.clone());
        let took = handle.set_bare(Arc::new(move |line: &str| {
            let words = caster
                .lock()
                .ok()
                .and_then(|mut held| spellcaster::typed(held.current(&dir, who.as_ref()), line));
            words.is_some_and(|words| {
                // Typed at the prompt: nobody waits for it.
                drop(start(&desk, &handler, &observer, Command::Sc(words)));
                true
            })
        }));
        if !took {
            eprintln!(
                "  !! [hunt] something already takes typed lines; a bare spell number goes to the game"
            );
        }
    }
    let (handler, seats) = (handle.clone(), Arc::clone(&party.seats));
    commands.hunt(Arc::new(move |line: &str| {
        let command = match parse_command(line)? {
            Ok(command) => command,
            Err(why) => {
                handler.say(Notice::line(NoticeKind::Error, format!("Hunt: {why}")));
                return Some(Took::Done);
            }
        };
        // What was started is handed back, so `;multi` can wait for it
        // (`crate::commands`).
        let took = match command {
            Command::Run(_)
            | Command::Quick(_)
            | Command::Bounty(_)
            | Command::Stop
            | Command::Heal { .. }
            | Command::Stock { .. }
            | Command::Keep
            | Command::Waggle(_)
            | Command::Sc(_) => {
                let Some(desk) = desk.clone() else {
                    handler.say(Notice::line(
                        NoticeKind::Error,
                        "Hunt: there is no map, so there is no hunting. Set the map and start Hydra again.",
                    ));
                    return Some(Took::Done);
                };
                Took::Started(start(&desk, &handler, &observer, command))
            }
            Command::Group { name, with } => {
                let (Some(desk), Some(leader)) = (desk.clone(), leader.clone()) else {
                    handler.say(Notice::line(
                        NoticeKind::Error,
                        "Hunt: a group needs a map and a character who has logged in.",
                    ));
                    return Some(Took::Done);
                };
                Took::Started(form(&seats, &desk, &handler, &observer, &leader, name, &with))
            }
            Command::Nothing => Took::Done,
            // The rest read and write files -- import, check, the settings
            // -- so not on the session's own thread; `run` names each.
            command => {
                let (handle, who, dir) = (handler.clone(), who.clone(), dir.clone());
                Took::Started(tokio::task::spawn_blocking(move || {
                    run(&handle, &dir, who.as_ref(), command);
                }))
            }
        };
        Some(took)
    }));
    eprintln!("[hunt] ready: `hunt help` lists the commands and how to change a setting");
}

/// What every character's hunt in this Hydra shares: each group's board, so
/// a leader's hunt and its followers' meet (`plan/39` §5), and each
/// character's seat, so a leader can start its followers' hunts.
///
/// **Owned by the session table** (`play.rs`) and handed to each character's
/// hunt as it opens. It was two process globals, which `plan/05` Rule 5.2
/// forbids ("No process globals. None.") and `every_static_is_allowlisted`
/// caught: shared state belongs to what owns the sessions, not to the process.
#[derive(Clone)]
pub(crate) struct Party {
    boards: Arc<Boards>,
    seats: Arc<Mutex<BTreeMap<String, Seat>>>,
    /// The window, when there is one: each hunt's reports go to its Hunt
    /// pane (`plan/47` step 8, `hunt/panel.rs`).
    window: Option<cena_gui::Sessions>,
}

impl Party {
    /// No groups and no seats yet; hunts shown in `window`, when there is one.
    pub(crate) fn new(window: Option<cena_gui::Sessions>) -> Self {
        Self {
            boards: Boards::new(),
            seats: Arc::default(),
            window,
        }
    }

    /// A character left the table: a leader can no longer start its hunt.
    /// Without this, a stopped character's seat stayed, and `hunt ... with`
    /// named it would have started a hunt on a session that was gone.
    pub(crate) fn unseat(&self, name: &str) {
        self.seats
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(name);
    }
}

/// A character's hunt desk and session: what a leader's `hunt <name> with`
/// starts a follower's hunt on.
#[derive(Clone)]
struct Seat {
    desk: Arc<Desk>,
    handle: SessionHandle,
    observer: SessionObserver,
}

/// This character's seat, for a leader to start its hunt from.
fn take_seat(
    party: &Party,
    name: &str,
    desk: &Arc<Desk>,
    handle: &SessionHandle,
    observer: &SessionObserver,
) {
    let seat = Seat {
        desk: Arc::clone(desk),
        handle: handle.clone(),
        observer: observer.clone(),
    };
    party
        .seats
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(name.to_owned(), seat);
}

/// `hunt <name> with A B`: each named character's own hunt on the profile
/// of that name, following `leader`; then the leader's, waiting for them
/// (`plan/39` §8, question 2). A name this Hydra is not running is said,
/// and hunted without.
fn form(
    seats: &Mutex<BTreeMap<String, Seat>>,
    desk: &Arc<Desk>,
    handle: &SessionHandle,
    observer: &SessionObserver,
    leader: &str,
    name: String,
    with: &[String],
) -> tokio::task::JoinHandle<()> {
    let seats = seats.lock().unwrap_or_else(PoisonError::into_inner).clone();
    let mut followers = Vec::new();
    for member in with {
        match seats.get(member).filter(|_| member != leader) {
            Some(seat) => {
                drop(start_placed(
                    &seat.desk,
                    &seat.handle,
                    &seat.observer,
                    Command::Run(name.clone()),
                    Place::Follow(leader.to_owned()),
                ));
                followers.push(member.clone());
            }
            None => handle.say(Notice::line(
                NoticeKind::Warn,
                format!(
                    "Hunt: {member} is not a character this Hydra is running; hunting without them."
                ),
            )),
        }
    }
    start_placed(
        desk,
        handle,
        observer,
        Command::Run(name),
        Place::Lead(followers),
    )
}

/// Run `command` on the hunt desk, once the session can be read. The task
/// is over when what the desk started is: a heal, a cast, a hunt.
fn start(
    desk: &Arc<Desk>,
    handle: &SessionHandle,
    observer: &SessionObserver,
    command: Command,
) -> tokio::task::JoinHandle<()> {
    start_placed(desk, handle, observer, command, Place::Read)
}

/// [`start`], a hunt taking `place` in its group.
fn start_placed(
    desk: &Arc<Desk>,
    handle: &SessionHandle,
    observer: &SessionObserver,
    command: Command,
    place: Place,
) -> tokio::task::JoinHandle<()> {
    let (desk, handle, observer) = (desk.clone(), handle.clone(), observer.clone());
    tokio::spawn(async move {
        match observer.subscribe().await {
            Ok((snapshot, events)) => {
                // Rejoinable, so a lag is recovered from, not decided
                // through (the crate review of 2026-09-28, R1).
                let joined = (
                    snapshot,
                    cena_behavior::travel::Heard::rejoinable(observer.clone(), events),
                );
                if let Some(run) = desk.run_placed(&handle, joined, command, place) {
                    let _ = run.await;
                }
            }
            Err(e) => handle.say(Notice::line(
                NoticeKind::Error,
                format!("Hunt: I could not read the session -- {e:?}."),
            )),
        }
    })
}

/// What is said to the player.
type Say<'a> = &'a dyn Fn(NoticeKind, String);

fn run(handle: &SessionHandle, dir: &Path, who: Option<&(String, String)>, command: Command) {
    let say = |kind: NoticeKind, text: String| handle.say(Notice::line(kind, text));
    match command {
        Command::Import { path, name } => import(dir, &path, name.as_deref(), &say),
        Command::ImportLoot { path } => import_loot(dir, who, &path, &say),
        Command::Check(name) => check(dir, who, &name, &say),
        Command::List => list(dir, &say),
        Command::KeepEdit(words) => keep_edit(dir, who, &words, &say),
        Command::ScEdit(words) => sc_edit(dir, who, &words, &say),
        Command::Set {
            profile,
            key,
            value,
        } => settings::set(dir, who, &profile, &key, &value, &say),
        Command::Unset { profile, key } => settings::unset(dir, who, &profile, &key, &say),
        Command::Show { profile, key } => settings::show(dir, who, &profile, key.as_deref(), &say),
        Command::Settings(of, setting) => settings::profile(dir, who, of, &setting, &say),
        Command::Help(topic) => settings::help(topic, &say),
        Command::Setup => settings::setup(&say),
        Command::Run(_)
        | Command::Quick(_)
        | Command::Bounty(_)
        | Command::Group { .. }
        | Command::Stop
        | Command::Heal { .. }
        | Command::Stock { .. }
        | Command::Keep
        | Command::Waggle(_)
        | Command::Sc(_)
        | Command::Nothing => {}
    }
}

/// Read a bigshot profile and write it as a Hydra one, never over a profile
/// that is already there.
fn import(dir: &Path, path: &str, name: Option<&str>, say: Say<'_>) {
    let own = || {
        Path::new(path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
    };
    let Some(name) = name
        .map(str::to_owned)
        .or_else(own)
        .and_then(|name| hunt::chain::file_name(&name))
    else {
        say(
            NoticeKind::Error,
            format!("Hunt: {path} gives no name a profile can have; say `as <name>`."),
        );
        return;
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            say(NoticeKind::Error, format!("Hunt: cannot read {path}: {e}"));
            return;
        }
    };
    let brought = match hunt::import(&name, &text) {
        Ok(brought) => brought,
        Err(why) => {
            say(
                NoticeKind::Error,
                format!("Hunt: {path} is not a bigshot profile: {why}"),
            );
            return;
        }
    };
    let rendered = match brought.render() {
        Ok(rendered) => rendered,
        Err(why) => {
            say(
                NoticeKind::Error,
                format!("Hunt: could not write the profile: {why}"),
            );
            return;
        }
    };
    let Some(target) = hunt::chain::profile_path(dir, &name) else {
        say(
            NoticeKind::Error,
            format!("Hunt: {name} is not a name a profile can have."),
        );
        return;
    };
    match hunt::chain::write_new(&target, &rendered) {
        Ok(()) => say(
            NoticeKind::Info,
            format!("Hunt: imported {name} to {}", target.display()),
        ),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            say(
                NoticeKind::Error,
                format!(
                    "Hunt: {} is already there. Import `as` another name, or delete it first.",
                    target.display()
                ),
            );
            return;
        }
        Err(e) => {
            say(
                NoticeKind::Error,
                format!("Hunt: cannot write {}: {e}", target.display()),
            );
            return;
        }
    }
    if brought.notes.is_empty() {
        say(
            NoticeKind::Info,
            "Hunt: everything in the bigshot profile was carried over.".to_owned(),
        );
    }
    for note in &brought.notes {
        say(NoticeKind::Warn, format!("Hunt: {note}"));
    }
}

/// Read eloot's settings and write them as this character's loot profile
/// (`plan/31` §6), never over one that is already there.
fn import_loot(dir: &Path, who: Option<&(String, String)>, path: &str, say: Say<'_>) {
    let Some((instance, character)) = who else {
        say(
            NoticeKind::Error,
            "Hunt: the game has not said who this is yet; the loot profile is per character, so wait for the login."
                .to_owned(),
        );
        return;
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            say(NoticeKind::Error, format!("Hunt: cannot read {path}: {e}"));
            return;
        }
    };
    let brought = match loot::import(&text) {
        Ok(brought) => brought,
        Err(why) => {
            say(
                NoticeKind::Error,
                format!("Hunt: {path} is not an eloot settings file: {why}"),
            );
            return;
        }
    };
    let rendered = match brought.render() {
        Ok(rendered) => rendered,
        Err(why) => {
            say(
                NoticeKind::Error,
                format!("Hunt: could not write the loot profile: {why}"),
            );
            return;
        }
    };
    let Some(target) = loot::path(dir, instance, character) else {
        say(
            NoticeKind::Error,
            format!("Hunt: {instance} {character} is not a name a file can have."),
        );
        return;
    };
    match hunt::chain::write_new(&target, &rendered) {
        Ok(()) => say(
            NoticeKind::Info,
            format!("Hunt: loot profile written to {}", target.display()),
        ),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            say(
                NoticeKind::Error,
                format!(
                    "Hunt: {} is already there. Delete it first to import again.",
                    target.display()
                ),
            );
            return;
        }
        Err(e) => {
            say(
                NoticeKind::Error,
                format!("Hunt: cannot write {}: {e}", target.display()),
            );
            return;
        }
    }
    for note in &brought.notes {
        say(NoticeKind::Warn, format!("Hunt: {note}"));
    }
}

/// Read a profile as this character would run it, and say what is held.
fn check(dir: &Path, who: Option<&(String, String)>, name: &str, say: Say<'_>) {
    let (instance, character) = who.map_or((None, None), |(instance, character)| {
        (Some(instance.as_str()), Some(character.as_str()))
    });
    if who.is_none() {
        say(
            NoticeKind::Warn,
            "Hunt: the game has not said who this is yet, so the character's own file is not read."
                .to_owned(),
        );
    }
    let loaded = match hunt::load(dir, instance, character, name) {
        Ok(loaded) => loaded,
        Err(LoadError::Invalid(problems)) => {
            for problem in problems {
                say(NoticeKind::Error, format!("Hunt: {name}: {problem}"));
            }
            return;
        }
        Err(e) => {
            say(NoticeKind::Error, format!("Hunt: {e}"));
            return;
        }
    };
    let sources: Vec<String> = loaded
        .sources
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    say(
        NoticeKind::Info,
        format!(
            "Hunt: {name} reads cleanly from {}",
            sources.join(", then ")
        ),
    );
    let profile = &loaded.profile;
    say(
        NoticeKind::Info,
        format!(
            "Hunt: {} target(s), {} routine(s), {} sequence(s); hunting room {}, resting room {}",
            profile.targets.len(),
            profile.routines.len(),
            profile.sequences.len(),
            room(profile.rooms.hunting),
            room(profile.rooms.resting),
        ),
    );
    for (place, step) in profile.held_steps() {
        say(
            NoticeKind::Warn,
            format!(
                "Hunt: {place} is held: `{}`: {}",
                step.send,
                step.held.as_deref().unwrap_or("")
            ),
        );
    }
    for sequence in profile.unwritten_sequences() {
        say(
            NoticeKind::Warn,
            format!(
                "Hunt: sequence {sequence} has no steps yet; the routine skips it. `hunt set {name} sequences.{sequence}.steps [\"...\", \"...\"]` writes them."
            ),
        );
    }
    let loot_file = instance
        .zip(character)
        .and_then(|(instance, character)| loot::path(dir, instance, character));
    match loot_file {
        Some(file) if file.is_file() => say(
            NoticeKind::Info,
            format!("Hunt: corpses are looted by {}", file.display()),
        ),
        _ => say(
            NoticeKind::Info,
            "Hunt: no loot profile for this character: corpses get `loot #id`. `hunt import-loot <eloot yaml>` brings one in."
                .to_owned(),
        ),
    }
}

fn room(id: Option<u32>) -> String {
    id.map_or_else(|| "unset".to_owned(), |id| id.to_string())
}

/// The profiles there are.
fn list(dir: &Path, say: Say<'_>) {
    let profiles = hunt::chain::profiles_dir(dir);
    let names = match hunt::chain::profile_names(dir) {
        Ok(names) => names,
        Err(e) => {
            say(
                NoticeKind::Error,
                format!("Hunt: cannot read {}: {e}", profiles.display()),
            );
            return;
        }
    };
    if names.is_empty() {
        say(
            NoticeKind::Info,
            format!(
                "Hunt: no profiles yet under {}. `hunt import <bigshot yaml>` brings one in; `hunt setup` says how to make one on the map.",
                profiles.display()
            ),
        );
    } else {
        say(NoticeKind::Info, format!("Hunt: {}", names.join(", ")));
    }
}

/// `;keep <words>`: the keep profile changed and written back, or listed.
fn keep_edit(dir: &Path, who: Option<&(String, String)>, words: &[String], say: Say<'_>) {
    let Some(path) = who.and_then(|(i, n)| cena_behavior::keep::path(dir, i, n)) else {
        say(
            NoticeKind::Error,
            "Keep: who is this? Log in first.".to_owned(),
        );
        return;
    };
    // A file that is there and does not read is said, never written over
    // with the defaults (`plan/44` Q05).
    let mut profile = match settings::stored(&path, cena_behavior::keep::KeepProfile::parse) {
        Ok(profile) => profile,
        Err(why) => {
            return say(
                NoticeKind::Error,
                format!("Keep: nothing was changed: {why}"),
            );
        }
    };
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    if words == ["list"] {
        say(
            NoticeKind::Info,
            format!(
                "Keep: {:?}; no-cast rooms {:?}; Sigil of Power {}.",
                profile.spells,
                profile.nocast,
                if profile.power { "on" } else { "off" }
            ),
        );
        return;
    }
    match cena_behavior::keep::edit(&mut profile, &words) {
        Ok(done) => {
            let written = profile
                .to_toml()
                .map_err(io::Error::other)
                .and_then(|text| cena_behavior::settings::save(&path, &text));
            match written {
                Ok(()) => say(NoticeKind::Info, format!("Keep: {done}.")),
                Err(e) => say(
                    NoticeKind::Error,
                    format!("Keep: {done}, but not saved -- {e}."),
                ),
            }
        }
        Err(usage) => say(NoticeKind::Error, format!("Keep: {usage}")),
    }
}

/// `;sc alias|verb|stance|set ...`: the spellcaster profile changed and
/// written back.
fn sc_edit(dir: &Path, who: Option<&(String, String)>, words: &[String], say: Say<'_>) {
    let Some(path) = who.and_then(|(i, n)| cena_behavior::spellcaster::path(dir, i, n)) else {
        say(
            NoticeKind::Error,
            "Sc: who is this? Log in first.".to_owned(),
        );
        return;
    };
    let mut profile =
        match settings::stored(&path, cena_behavior::spellcaster::CasterProfile::parse) {
            Ok(profile) => profile,
            Err(why) => return say(NoticeKind::Error, format!("Sc: nothing was changed: {why}")),
        };
    let words: Vec<String> = words.iter().map(|w| w.to_ascii_lowercase()).collect();
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match cena_behavior::spellcaster::edit(&mut profile, &words) {
        Ok(done) => {
            let written = profile
                .to_toml()
                .map_err(io::Error::other)
                .and_then(|text| cena_behavior::settings::save(&path, &text));
            match written {
                Ok(()) => say(NoticeKind::Info, format!("Sc: {done}.")),
                Err(e) => say(
                    NoticeKind::Error,
                    format!("Sc: {done}, but not saved -- {e}."),
                ),
            }
        }
        Err(usage) => say(NoticeKind::Error, format!("Sc: {usage}")),
    }
}
