//! The hunt desk: `;hunt <name>` and `;hunt stop` for one session, while it
//! is being played.
//!
//! One desk per session, the map shared between them all. It loads the
//! profile the way this character would run it ([`chain::load`]), claims
//! the authority, runs the [`hunt_in`] with a [`watch`] beside it, and releases
//! on every exit. **One hunt at a time**: a second `;hunt <name>` stops the
//! first and starts when it has let go, as travel's desk does for walks.
//!
//! Importing, checking and listing profiles need no session state and stay
//! with whoever installs the desk.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cena_map::Map;
use cena_session::travel_store::{self, TravelFile};
use cena_session::{
    AuthorityToken, CommandId, GameState, Notice, NoticeKind, SessionHandle, Snapshot,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::chain;
use super::command::Command;
use super::drive::{HuntEnd, hunt_in};
use super::engine::Hunt;
use super::report::{Reports, Status};
use crate::error::BehaviorError;
use crate::group::{Boards, Place};
use crate::heal::{self, HealProfile};
use crate::loot::{self, LootProfile};
use crate::operation::{Steering, Underway};
use crate::settings::Stored;
use crate::travel::{Heard, TravelNotes};
use crate::watchdog::{BEHAVIOR_WATCHDOG, Heartbeat, Watched, outlasting, watch};

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
    /// Every group's board in this Hydra, when hunts here may group
    /// (`plan/39` §5).
    boards: OnceLock<Arc<Boards>>,
    /// What its runs are doing, turn by turn, for a hunt panel (`plan/47`
    /// step 8).
    pub(super) reports: Reports,
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
            boards: OnceLock::new(),
            reports: Reports::default(),
        })
    }

    /// Hear what this desk's runs are doing, turn by turn: the latest, or
    /// `None` while nothing runs.
    #[must_use]
    pub fn reports(&self) -> tokio::sync::watch::Receiver<Option<Status>> {
        self.reports.follow()
    }

    /// Let this desk's hunts hunt in a group, on `boards`: one set for every
    /// session in the process, so a leader's and its followers' hunts meet.
    /// Set once; a second call is ignored.
    pub fn group_on(&self, boards: Arc<Boards>) {
        let _ = self.boards.set(boards);
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

    /// Whether something runs on the desk now: a hunt, a walk home, a
    /// waggle, a keep.
    fn running(&self) -> bool {
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|run| !run.stop.is_cancelled())
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
        self.run_placed(handle, joined, command, Place::Read)
    }

    /// [`Self::run`], a hunt taking `place` in its group: as the command
    /// that formed the group said (`hunt <name> with ...`, `plan/39` §8,
    /// question 2). Only a hunt groups; a heal or a cast does not.
    pub fn run_placed(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        command: Command,
        place: Place,
    ) -> Option<JoinHandle<HuntEnd>> {
        self.underway_placed(handle, joined, command, place)
            .map(|underway| underway.task)
    }

    /// [`Self::run`], with the controls for what it started, and for
    /// nothing started after it (`crate::operation`).
    pub fn underway(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        command: Command,
    ) -> Option<Underway<HuntEnd>> {
        self.underway_placed(handle, joined, command, Place::Read)
    }

    /// [`Self::run_placed`], with the controls for what it started.
    pub fn underway_placed(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
        command: Command,
        place: Place,
    ) -> Option<Underway<HuntEnd>> {
        let say = answer(handle);
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
            Command::Loot(errand) => self.loot_errand(handle, joined, errand),
            Command::LootLast => {
                self.say_last_round(handle);
                None
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
                said_skipped(&loaded.profile, say);
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
                let machine = match self.waggle_profile(&joined.0.state) {
                    Some(profile) => machine.with_waggle(profile),
                    None => machine,
                };
                Some(self.start_in(
                    handle.clone(),
                    (joined.0, joined.1.into()),
                    machine,
                    Some(place),
                    ("hunt", &name),
                ))
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
            "This hunt was saved against different or unverified map bytes. Review its setup; nothing started."
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

    /// Start a run that is not a hunt, named `what` for its reports and
    /// `word`, as the player starts it, for the echo of its commands.
    pub(super) fn start(
        self: &Arc<Self>,
        (word, what): (&'static str, &str),
        handle: SessionHandle,
        joined: (Snapshot, Heard),
        machine: Hunt,
    ) -> Underway<HuntEnd> {
        self.start_in(handle, joined, machine, None, (word, what))
    }

    /// A per-character settings file of this character's, as found: `path`
    /// names it (`keep::path` and its kin), `parse` reads it. Missing when
    /// the game has not said who this is.
    fn stored<T>(
        &self,
        state: &GameState,
        path: fn(&std::path::Path, &str, &str) -> Option<std::path::PathBuf>,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> Stored<T> {
        let character = &state.character;
        character
            .instance
            .as_deref()
            .zip(character.name.as_deref())
            .and_then(|(i, n)| path(&self.dir, i, n))
            .map_or(Stored::Missing, |path| crate::settings::read(&path, parse))
    }

    /// The character's waggle profile, for the waggle after a death when
    /// `react.depart_switch` asks for one (`hunt/death.rs`).
    fn waggle_profile(&self, state: &GameState) -> Option<crate::waggle::WaggleProfile> {
        let character = &state.character;
        character
            .instance
            .as_deref()
            .zip(character.name.as_deref())
            .and_then(|(i, n)| crate::waggle::path(&self.dir, i, n))
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| crate::waggle::WaggleProfile::parse(&text).ok())
            .filter(|p| !p.cast_list.is_empty())
    }

    /// Start `machine`, named `what` for its reports; `place` in a group
    /// when it is a hunt.
    fn start_in(
        self: &Arc<Self>,
        handle: SessionHandle,
        joined: (Snapshot, Heard),
        machine: Hunt,
        place: Option<Place>,
        (word, what): (&'static str, &str),
    ) -> Underway<HuntEnd> {
        let what = what.to_owned();
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
        let steering = Steering::new(running.stop.clone());
        let machine = machine.steered_by(steering.clone());
        let task = tokio::spawn(async move {
            let Running { number, stop, over } = running;
            if let Some(before) = before {
                before.over.cancelled().await;
            }
            desk.reports.running(&what);
            // Its commands echoed as the player starts it: `hunt>attack`.
            handle.name_behavior(desk.token, word);
            let end = Box::pin(desk.hunt_once(&handle, &stop, joined, machine, place)).await;
            // Before `over`: the next run, waiting on it, reports after this.
            desk.reports.tell(None);
            let mut slot = desk.running.lock().unwrap_or_else(PoisonError::into_inner);
            if slot.as_ref().is_some_and(|hunt| hunt.number == number) {
                *slot = None;
            }
            drop(slot);
            over.cancel();
            end
        });
        Underway { task, steering }
    }

    /// The character's loot profile (`plan/31` §6), when one has been
    /// imported and reads. Said either way, since it changes what a corpse
    /// gets.
    pub(super) fn loot_profile(
        &self,
        handle: &SessionHandle,
        instance: Option<&str>,
        name: Option<&str>,
    ) -> Option<LootProfile> {
        let say = answer(handle);
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
    ) -> Option<Underway<HuntEnd>> {
        let state = &joined.0.state;
        let profile = match self.stored(state, crate::spellcaster::path, |text| {
            crate::spellcaster::CasterProfile::parse(text)
        }) {
            Stored::Found(profile) => profile,
            Stored::Broken(why) => {
                handle.say(
                    Notice::line(NoticeKind::Error, format!("Sc: nothing was cast: {why}"))
                        .answering(),
                );
                return None;
            }
            Stored::Missing => crate::spellcaster::CasterProfile::default(),
        };
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        match crate::spellcaster::lines(&profile, state, &words) {
            // Beside what runs, never in its place (`beside.rs`).
            Ok(lines) if self.running() => {
                let next = Arc::clone(&self.ids);
                let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
                Some(super::beside::cast(handle.clone(), self.token, lines, ids))
            }
            Ok(lines) => {
                let machine = Hunt::send_only(lines);
                Some(self.start(
                    ("sc", "sc"),
                    handle.clone(),
                    (joined.0, joined.1.into()),
                    machine,
                ))
            }
            Err(why) => {
                handle.say(Notice::line(NoticeKind::Warn, format!("Sc: {why}")).answering());
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
    ) -> Option<Underway<HuntEnd>> {
        let profile = match self.stored(&joined.0.state, crate::waggle::path, |text| {
            crate::waggle::WaggleProfile::parse(text)
        }) {
            Stored::Found(profile) if !profile.cast_list.is_empty() => profile,
            Stored::Broken(why) => {
                handle.say(Notice::line(NoticeKind::Error, format!("Waggle: {why}")).answering());
                return None;
            }
            _ => {
                handle.say(Notice::line(
                    NoticeKind::Error,
                    "Waggle: no spells to cast yet. `waggle set cast_list [101, 107, 401]` names them; `waggle show` lists the rest.",
                ).answering());
                return None;
            }
        };
        let machine = Hunt::waggle_only(profile, targets);
        Some(self.start(
            ("waggle", "waggle"),
            handle.clone(),
            (joined.0, joined.1.into()),
            machine,
        ))
    }

    /// `;keep`: the keep profile's spells kept up until stopped.
    fn keep_spells(
        self: &Arc<Self>,
        handle: &SessionHandle,
        joined: (Snapshot, impl Into<Heard>),
    ) -> Option<Underway<HuntEnd>> {
        let profile = match self.stored(&joined.0.state, crate::keep::path, |text| {
            crate::keep::KeepProfile::parse(text)
        }) {
            Stored::Found(profile) if !profile.spells.is_empty() => profile,
            Stored::Broken(why) => {
                handle.say(Notice::line(NoticeKind::Error, format!("Keep: {why}")).answering());
                return None;
            }
            _ => {
                handle.say(
                    Notice::line(
                        NoticeKind::Error,
                        "Hunt: nothing to keep up: `keep add <spell>` first.",
                    )
                    .answering(),
                );
                return None;
            }
        };
        handle.say(
            Notice::line(
                NoticeKind::Info,
                format!(
                    "Hunt: keeping up {:?}; `hunt stop` ends it.",
                    profile.spells
                ),
            )
            .answering(),
        );
        Some(self.start(
            ("keep", "keep"),
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
    ) -> Option<Underway<HuntEnd>> {
        let character = &joined.0.state.character;
        let Some(profile) = self.heal_profile(
            handle,
            character.instance.as_deref(),
            character.name.as_deref(),
        ) else {
            handle.say(Notice::line(
                NoticeKind::Error,
                "Hunt: no heal profile yet. `heal set container <your herb container>` makes one; `heal show` lists the rest.",
            ).answering());
            return None;
        };
        Some(self.start(
            ("heal", "herbs"),
            handle.clone(),
            (joined.0, joined.1.into()),
            make(profile),
        ))
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
        match crate::settings::read(&path, HealProfile::parse) {
            Stored::Found(profile) => Some(profile),
            Stored::Missing => None,
            Stored::Broken(why) => {
                handle.say(Notice::line(NoticeKind::Error, format!("Heal: {why}.")));
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
        place: Option<Place>,
    ) -> HuntEnd {
        if handle.claim(self.token).await.is_err() {
            handle.say(
                Notice::line(
                    NoticeKind::Error,
                    "Hunt: something else holds the session; stop it first.",
                )
                .answering(),
            );
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
            let learned = |learned: &loot::Learned| {
                super::keep_learned::remember(handle, loot_file.as_deref(), learned);
            };
            let run = Box::pin(hunt_in(
                handle,
                stop,
                ids,
                self.token,
                joined,
                &self.map,
                machine,
                &heartbeat,
                notes,
                wrote,
                learned,
                self.boards.get().cloned().zip(place),
                &self.reports,
            ));
            // Stopped by the player, the hunt is given its own ending (a
            // follower's `leave group`, a leader's board closed); preempted as
            // wedged, it is cut off.
            outlasting(
                run,
                watch(handle, stop, &heartbeat, BEHAVIOR_WATCHDOG, "Hunt"),
                |watched| match watched {
                    Watched::Stopped => HuntEnd::Stopped(BehaviorError::Cancelled),
                    Watched::Wedged(_) => HuntEnd::Stopped(BehaviorError::Wedged),
                },
            )
            .await
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
        // Not the settings: the player's, which the menu may have changed
        // since the walk read them (`travel_store::save`).
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

/// Say what of `profile` the hunt will skip: each held step, and each
/// sequence with no steps written.
fn said_skipped(profile: &super::profile::Profile, say: impl Fn(NoticeKind, String)) {
    for (place, step) in profile.held_steps() {
        say(
            NoticeKind::Warn,
            format!("{place} is held and will be skipped: `{}`", step.send),
        );
    }
    for sequence in profile.unwritten_sequences() {
        say(
            NoticeKind::Warn,
            format!(
                "sequence {sequence} has no steps and will be skipped; `hunt set <profile> sequences.{sequence}.steps [...]` writes them."
            ),
        );
    }
}

/// What the hunt answers the player with, on `handle`: a line after its
/// name, in the story (`cena_session::Notice::answer`).
fn answer(handle: &SessionHandle) -> impl Fn(NoticeKind, String) + Copy + '_ {
    move |kind, text| {
        handle.say(Notice::line(kind, format!("Hunt: {text}")).answering());
    }
}
