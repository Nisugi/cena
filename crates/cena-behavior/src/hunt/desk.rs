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
use cena_session::travel_store::{self, TravelFile};
use cena_session::{
    AuthorityToken, CommandId, GameState, Notice, NoticeKind, SessionHandle, Snapshot,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::chain;
use super::command::Command;
use super::drive::{HuntEnd, hunt};
use super::engine::Hunt;
use crate::error::BehaviorError;
use crate::heal::{self, HealProfile};
use crate::loot::{self, LootProfile};
use crate::travel::{Heard, TravelNotes};
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
    map_sha256: Option<String>,
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
            map_sha256: None,
        })
    }

    /// Pin map-created profiles to the host's exact loaded bytes.
    #[must_use]
    pub fn with_map_sha256(
        map: Arc<Map>,
        dir: PathBuf,
        token: AuthorityToken,
        hash: String,
    ) -> Arc<Self> {
        let mut desk = Self::new(map, dir, token);
        if let Some(inner) = Arc::get_mut(&mut desk) {
            inner.map_sha256 = Some(hash);
        }
        desk
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
        let (command, quick, bounty) = match command {
            Command::Quick(name) => (Command::Run(name), true, false),
            Command::Bounty(name) => (Command::Run(name), false, true),
            other => (other, false, false),
        };
        match command {
            Command::Stop => {
                if !self.stop() {
                    say(NoticeKind::Info, "I am not hunting.".to_owned());
                }
                None
            }
            Command::Heal {
                spellcast,
                ranged,
                blood,
            } => self.herbs(handle, joined, |mut profile| {
                profile.blood_only |= blood;
                Hunt::heal_only(profile, spellcast, ranged)
            }),
            Command::Keep => self.keep_spells(handle, joined),
            Command::Sc(words) => self.sc(handle, joined, &words),
            Command::Waggle(targets) => self.waggle(handle, joined, targets),
            Command::Stock { fill } => {
                self.herbs(handle, joined, |profile| Hunt::stock_only(profile, fill))
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
                if self.refuses_map(&loaded.profile, say) {
                    return None;
                }
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
                let machine = if quick { machine.quick() } else { machine };
                let machine = if bounty { machine.bounty() } else { machine };
                let machine = match self.loot_profile(
                    handle,
                    character.instance.as_deref(),
                    character.name.as_deref(),
                ) {
                    Some(profile) => machine.with_loot(profile),
                    None => machine,
                };
                let machine = match self.heal_profile(
                    handle,
                    character.instance.as_deref(),
                    character.name.as_deref(),
                ) {
                    Some(profile) => machine.with_heal(profile),
                    None => machine,
                };
                // For the waggle after a death, when `react.depart_switch`
                // asks for one (`hunt/death.rs`).
                let waggle = character
                    .instance
                    .as_deref()
                    .zip(character.name.as_deref())
                    .and_then(|(i, n)| crate::waggle::path(&self.dir, i, n))
                    .and_then(|path| std::fs::read_to_string(path).ok())
                    .and_then(|text| crate::waggle::WaggleProfile::parse(&text).ok())
                    .filter(|p| !p.cast_list.is_empty());
                let machine = match waggle {
                    Some(profile) => machine.with_waggle(profile),
                    None => machine,
                };
                Some(self.start(handle.clone(), (joined.0, joined.1.into()), machine))
            }
            _ => None,
        }
    }

    /// Start a hunt, stopping the one under way first.
    /// Whether `profile` must not run on this map, having said why: pinned
    /// to other map bytes, or its allowed rooms not valid on it.
    fn refuses_map(
        &self,
        profile: &super::profile::Profile,
        say: impl Fn(NoticeKind, String),
    ) -> bool {
        let why = if profile
            .map_sha256
            .as_ref()
            .is_some_and(|hash| Some(hash) != self.map_sha256.as_ref())
        {
            "This hunt was saved against different or unverified map bytes.              Review its setup; nothing started."
                .to_owned()
        } else if profile.rooms.allowed.is_some()
            && let Err(why) = super::setup::validate_map(profile, &self.map)
        {
            why
        } else {
            return false;
        };
        say(NoticeKind::Error, why);
        true
    }

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

    /// The character's loot profile (`plan/31` §6), when one has been
    /// imported and reads. Said either way, since it changes what a corpse
    /// gets.
    fn loot_profile(
        &self,
        handle: &SessionHandle,
        instance: Option<&str>,
        name: Option<&str>,
    ) -> Option<LootProfile> {
        let say = |kind, text: String| handle.say(Notice::line(kind, format!("Hunt: {text}")));
        let path = loot::path(&self.dir, instance?, name?)?;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                say(
                    NoticeKind::Info,
                    "no loot profile, so corpses get `loot #id`; `hunt import-loot <eloot yaml>` brings one in.".to_owned(),
                );
                return None;
            }
            Err(e) => {
                say(
                    NoticeKind::Warn,
                    format!(
                        "cannot read {}: {e}; corpses get `loot #id`.",
                        path.display()
                    ),
                );
                return None;
            }
        };
        match LootProfile::parse(&text) {
            Ok(profile) => {
                for problem in profile.problems() {
                    say(NoticeKind::Warn, format!("loot profile: {problem}"));
                }
                say(NoticeKind::Info, format!("looting by {}.", path.display()));
                if profile.skin.enable {
                    let weapon = if profile.skin.weapon.is_empty() {
                        "whatever is in the right hand".to_owned()
                    } else {
                        format!("the {}", profile.skin.weapon)
                    };
                    say(NoticeKind::Info, format!("skinning with {weapon}."));
                }
                Some(profile)
            }
            Err(why) => {
                say(
                    NoticeKind::Error,
                    format!(
                        "{} does not read: {why}; corpses get `loot #id`.",
                        path.display()
                    ),
                );
                None
            }
        }
    }

    /// `;sc <spell|alias> [target] [count]`: the lines, sent once.
    fn sc(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        words: &[String],
    ) -> Option<JoinHandle<HuntEnd>> {
        let state = &joined.0.state;
        let profile = state
            .character
            .instance
            .as_deref()
            .zip(state.character.name.as_deref())
            .and_then(|(i, n)| crate::spellcaster::path(&self.dir, i, n))
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| crate::spellcaster::CasterProfile::parse(&text).ok())
            .unwrap_or_default();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        match crate::spellcaster::lines(&profile, state, &words) {
            Ok(lines) => {
                let machine = Hunt::send_only(lines);
                Some(self.start(handle.clone(), (joined.0, joined.1.into()), machine))
            }
            Err(why) => {
                handle.say(Notice::line(NoticeKind::Warn, format!("Sc: {why}")));
                None
            }
        }
    }

    /// `;waggle [names]`: the waggle profile's spells on these people.
    fn waggle(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        targets: Vec<String>,
    ) -> Option<JoinHandle<HuntEnd>> {
        let character = &joined.0.state.character;
        let profile = character
            .instance
            .as_deref()
            .zip(character.name.as_deref())
            .and_then(|(i, n)| crate::waggle::path(&self.dir, i, n))
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| crate::waggle::WaggleProfile::parse(&text).ok())
            .filter(|p| !p.cast_list.is_empty());
        let Some(profile) = profile else {
            handle.say(Notice::line(
                NoticeKind::Error,
                "Hunt: no waggle profile: write one with a `cast_list`.",
            ));
            return None;
        };
        let machine = Hunt::waggle_only(profile, targets);
        Some(self.start(handle.clone(), (joined.0, joined.1.into()), machine))
    }

    /// `;keep`: the keep profile's spells kept up until stopped.
    fn keep_spells(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
    ) -> Option<JoinHandle<HuntEnd>> {
        let character = &joined.0.state.character;
        let profile = character
            .instance
            .as_deref()
            .zip(character.name.as_deref())
            .and_then(|(i, n)| crate::keep::path(&self.dir, i, n))
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| crate::keep::KeepProfile::parse(&text).ok());
        let Some(profile) = profile.filter(|p| !p.spells.is_empty()) else {
            handle.say(Notice::line(
                NoticeKind::Error,
                "Hunt: nothing to keep up: `keep add <spell>` first.",
            ));
            return None;
        };
        handle.say(Notice::line(
            NoticeKind::Info,
            format!(
                "Hunt: keeping up {:?}; `hunt stop` ends it.",
                profile.spells
            ),
        ));
        Some(self.start(
            handle.clone(),
            (joined.0, joined.1.into()),
            Hunt::keep_only(profile),
        ))
    }

    /// `;heal` and its `stock` and `fill`: the machine `make` builds from the
    /// character's heal profile, started; said and refused when there is none.
    fn herbs(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        make: impl FnOnce(HealProfile) -> Hunt,
    ) -> Option<JoinHandle<HuntEnd>> {
        let character = &joined.0.state.character;
        let Some(profile) = self.heal_profile(
            handle,
            character.instance.as_deref(),
            character.name.as_deref(),
        ) else {
            handle.say(Notice::line(
                NoticeKind::Error,
                "Hunt: no heal profile: write one naming the herb `container`.",
            ));
            return None;
        };
        Some(self.start(handle.clone(), (joined.0, joined.1.into()), make(profile)))
    }

    /// The character's heal profile (`plan/36`), when there is one and it
    /// reads. Quiet when there is none: most characters rest without herbs.
    fn heal_profile(
        &self,
        handle: &SessionHandle,
        instance: Option<&str>,
        name: Option<&str>,
    ) -> Option<HealProfile> {
        let path = heal::path(&self.dir, instance?, name?)?;
        let text = std::fs::read_to_string(&path).ok()?;
        match HealProfile::parse(&text) {
            Ok(profile) => Some(profile),
            Err(why) => {
                handle.say(Notice::line(
                    NoticeKind::Error,
                    format!("Heal: {} does not read: {why}.", path.display()),
                ));
                None
            }
        }
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
        let (mut file, notes) = self.traveller(handle, &joined.0.state);
        let loot_file = joined
            .0
            .state
            .character
            .instance
            .as_deref()
            .zip(joined.0.state.character.name.as_deref())
            .and_then(|(instance, name)| loot::path(&self.dir, instance, name));
        let end = {
            let wrote = |notes: &TravelNotes| self.keep(handle, file.as_mut(), notes);
            let learned = |names: &[String]| unskinnable(handle, loot_file.as_deref(), names);
            let run = Box::pin(hunt(
                handle, stop, ids, self.token, joined, &self.map, machine, &heartbeat, notes,
                wrote, learned,
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

    /// The character's travel file and the notes a walk works from, as
    /// travel's desk reads them. Without a name, or a readable file, the
    /// walks run on empty notes and nothing is remembered; said once.
    fn traveller(
        &self,
        handle: &SessionHandle,
        state: &GameState,
    ) -> (Option<TravelFile>, TravelNotes) {
        let character = &state.character;
        let (Some(instance), Some(name)) =
            (character.instance.as_deref(), character.name.as_deref())
        else {
            handle.say(Notice::line(
                NoticeKind::Warn,
                "Hunt: the game has not said who this is, so the walks will remember nothing.",
            ));
            return (None, TravelNotes::default());
        };
        match travel_store::load(&self.dir, instance, name) {
            Ok(file) => {
                let notes = TravelNotes {
                    settings: file.settings.clone().into_iter().collect(),
                    memories: file.memories.clone().into_iter().collect(),
                    targets: file.targets.clone(),
                    last_room: file.last_room,
                };
                (Some(file), notes)
            }
            Err(why) => {
                handle.say(Notice::line(
                    NoticeKind::Warn,
                    format!("Hunt: {why} -- the travel file is left alone, and the walks will remember nothing."),
                ));
                (None, TravelNotes::default())
            }
        }
    }

    /// Write what a walk learned back to the character's spot in the travel
    /// file, as travel's desk does.
    fn keep(&self, handle: &SessionHandle, file: Option<&mut TravelFile>, notes: &TravelNotes) {
        let Some(file) = file else { return };
        file.settings = notes.settings.clone().into_iter().collect();
        file.memories = notes.memories.clone().into_iter().collect();
        file.last_room = notes.last_room;
        if let Err(why) = travel_store::save(&self.dir, file) {
            handle.say(Notice::line(
                NoticeKind::Error,
                format!("Hunt: what the walk learned could not be saved -- {why}."),
            ));
        }
    }
}

/// Write creatures learned unskinnable into the loot profile, as eloot saves
/// its profile when the game says *You cannot skin* (`eloot.lic:5846`), so
/// the next hunt does not try them. Said either way.
fn unskinnable(handle: &SessionHandle, file: Option<&std::path::Path>, names: &[String]) {
    let say = |kind, text: String| handle.say(Notice::line(kind, format!("Hunt: {text}")));
    let Some(file) = file else { return };
    let saved = std::fs::read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|text| loot::remember_unskinnable(&text, names))
        .and_then(|written| match written {
            Some(text) => std::fs::write(file, text).map_err(|e| e.to_string()),
            None => Ok(()),
        });
    match saved {
        Ok(()) => say(
            NoticeKind::Info,
            format!(
                "{} cannot be skinned; the loot profile remembers.",
                names.join(", ")
            ),
        ),
        Err(why) => say(
            NoticeKind::Warn,
            format!(
                "{} cannot be skinned, but the loot profile could not be updated -- {why}.",
                names.join(", ")
            ),
        ),
    }
}
