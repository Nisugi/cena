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

use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::commands::{Commands, Took};
use cena_behavior::hunt::{self, Command, Desk, LoadError, parse_command};
use cena_behavior::loot;
use cena_behavior::spellcaster::{self, CasterProfile};
use cena_behavior::travel::Map;
use cena_session::{AuthorityToken, GameState, Notice, NoticeKind, SessionHandle, SessionObserver};

/// Register hunt's words. The character's instance and name, when the login
/// has said them, choose the character level of the chain; `map` is the one
/// travel loaded, and `None` when travel has none.
pub(crate) fn open(
    handle: &SessionHandle,
    observer: SessionObserver,
    state: &GameState,
    commands: &Commands,
    map: Option<Arc<Map>>,
) {
    let who = state
        .character
        .instance
        .clone()
        .zip(state.character.name.clone());
    let dir = cena_session::character_store::data_dir();
    let desk = map.map(|map| Desk::new(map, dir.clone(), AuthorityToken(3)));
    // The spellcaster profile, held so a typed line is judged without a
    // file read, and read again after `;sc` changes it.
    let caster = Arc::new(Mutex::new(read_caster(&dir, who.as_ref())));
    if let Some(desk) = desk.clone() {
        let (handler, observer, caster) = (handle.clone(), observer.clone(), Arc::clone(&caster));
        let took = handle.set_bare(Arc::new(move |line: &str| {
            let words = caster
                .lock()
                .ok()
                .and_then(|profile| spellcaster::typed(&profile, line));
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
    let handler = handle.clone();
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
            Command::Import { .. }
            | Command::ImportLoot { .. }
            | Command::Check(_)
            | Command::List
            | Command::KeepEdit(_)
            | Command::ScEdit(_) => {
                let (handle, who, dir) = (handler.clone(), who.clone(), dir.clone());
                let caster = Arc::clone(&caster);
                // Files are read and written, so not on the session's own thread.
                Took::Started(tokio::task::spawn_blocking(move || {
                    let sc = matches!(command, Command::ScEdit(_));
                    run(&handle, &dir, who.as_ref(), command);
                    if sc && let Ok(mut held) = caster.lock() {
                        *held = read_caster(&dir, who.as_ref());
                    }
                }))
            }
            Command::Nothing => Took::Done,
        };
        Some(took)
    }));
    eprintln!(
        "[hunt] ready: hunt <name>, hunt stop, hunt import <bigshot yaml>, hunt check <name>, hunt list"
    );
}

/// Run `command` on the hunt desk, once the session can be read. The task
/// is over when what the desk started is: a heal, a cast, a hunt.
fn start(
    desk: &Arc<Desk>,
    handle: &SessionHandle,
    observer: &SessionObserver,
    command: Command,
) -> tokio::task::JoinHandle<()> {
    let (desk, handle, observer) = (desk.clone(), handle.clone(), observer.clone());
    tokio::spawn(async move {
        match observer.subscribe().await {
            Ok(joined) => {
                if let Some(run) = desk.run(&handle, joined, command) {
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

/// The character's spellcaster profile, or the default when there is no
/// file or it does not read.
fn read_caster(dir: &Path, who: Option<&(String, String)>) -> CasterProfile {
    who.and_then(|(i, n)| spellcaster::path(dir, i, n))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| CasterProfile::parse(&text).ok())
        .unwrap_or_default()
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
        Command::Run(_)
        | Command::Quick(_)
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
            format!("Hunt: sequence {sequence} has no steps yet; the routine skips it."),
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
    let mut names: Vec<String> = match std::fs::read_dir(&profiles) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|x| x == "toml"))
            .filter_map(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            say(
                NoticeKind::Error,
                format!("Hunt: cannot read {}: {e}", profiles.display()),
            );
            return;
        }
    };
    names.sort();
    if names.is_empty() {
        say(
            NoticeKind::Info,
            format!(
                "Hunt: no profiles yet under {}. `hunt import <bigshot yaml>` brings one in.",
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
    let mut profile = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| cena_behavior::keep::KeepProfile::parse(&text).ok())
        .unwrap_or_default();
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
                .and_then(|text| {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, text)
                });
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
    let mut profile = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| cena_behavior::spellcaster::CasterProfile::parse(&text).ok())
        .unwrap_or_default();
    let words: Vec<String> = words.iter().map(|w| w.to_ascii_lowercase()).collect();
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match cena_behavior::spellcaster::edit(&mut profile, &words) {
        Ok(done) => {
            let written = profile
                .to_toml()
                .map_err(io::Error::other)
                .and_then(|text| {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, text)
                });
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
