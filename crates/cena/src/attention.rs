//! A trigger's sound and OS notification, played by the binary (`plan/45`
//! Stage 3): once for every character that saw the same thing, and whether
//! or not any page is open, because a session nobody watches still sounds.
//!
//! The session decides what calls for attention, cooldowns and all, and
//! publishes it ([`Event::Attention`]); [`forward`] carries each character's
//! calls here, and one desk, on a thread of its own, plays and notifies. The
//! banner is not the desk's: each character's page shows its own.
//!
//! **One occurrence, several characters.** Three characters in one room hear
//! one shout. A call that another character made within a second -- the same
//! trigger, the same sound, the same words -- is that occurrence again, and
//! passes silently: the merged streams' second (`cena_ui::Merger`). The same
//! character calling twice is two occurrences, which its cooldown has
//! already let through.
//!
//! **The sound** is found as `VellumFE` finds one
//! (`reference/VellumFE/src/sound.rs`): by its file name in the sounds folder,
//! `<data dir>/sounds`, as written or with `.wav`, `.mp3`, `.ogg` or `.flac`
//! after it. **A path is never followed**, not even one that is there
//! (`cena_behavior::triggers::sound`, the crate review of 2026-10-01,
//! BI-B-1): on Windows a network path in a file another player wrote would
//! make Hydra reach out to their host with the player's credentials. The audio
//! device is opened at the first sound, not before: `VellumFE` found opening
//! it can take ten seconds on a machine with none.

use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use cena_behavior::triggers::sound;
use cena_session::Event;
use cena_session::trigger::Attention;
use tokio::sync::broadcast::{self, error::RecvError};

/// How long one thing seen by several characters counts as one.
const SAME: Duration = Duration::from_secs(1);

/// The extensions a sound named without one is tried with, `VellumFE`'s.
const EXTENSIONS: [&str; 4] = ["wav", "mp3", "ogg", "flac"];

/// One call for attention, and whose.
#[derive(Debug, Clone)]
pub(crate) struct Called {
    /// The character whose trigger called.
    pub(crate) character: String,
    /// What it called for.
    pub(crate) attention: Arc<Attention>,
}

/// The sounds folder under the data directory `dir`.
pub(crate) fn sounds_dir(dir: &Path) -> PathBuf {
    dir.join("sounds")
}

/// Where the sound `named` is: the file of that name in `sounds`, as
/// written or with one of [`EXTENSIONS`]. `None` when it is nowhere, and,
/// before anything is looked for, when `named` is not a file name
/// ([`sound::refused`]): a path is never followed.
pub(crate) fn found(sounds: &Path, named: &str) -> Option<PathBuf> {
    if sound::refused(named).is_some() {
        return None;
    }
    let file = named.trim();
    let here = sounds.join(file);
    if here.is_file() {
        return Some(here);
    }
    if Path::new(file).extension().is_some() {
        return None;
    }
    EXTENSIONS
        .iter()
        .map(|extension| sounds.join(format!("{file}.{extension}")))
        .find(|path| path.is_file())
}

/// Start the desk on a thread of its own, for the sounds under `dir`: the
/// audio device is opened, and lives, on one thread. It stops when every
/// sender is gone.
pub(crate) fn start(dir: &Path) -> Sender<Called> {
    let (desk, calls) = channel();
    let sounds = sounds_dir(dir);
    // A notification on Linux goes over D-Bus through zbus on tokio, which
    // wants the runtime's context even from a plain thread.
    let runtime = tokio::runtime::Handle::try_current().ok();
    let started = std::thread::Builder::new()
        .name("attention".into())
        .spawn(move || {
            let _entered = runtime.as_ref().map(tokio::runtime::Handle::enter);
            run(&calls, &sounds);
        });
    if let Err(e) = started {
        eprintln!("[attention] no sounds or notifications: {e}");
    }
    desk
}

/// Carry `character`'s calls for attention from its session's `events` to
/// the `desk`, until the session or the desk ends.
pub(crate) async fn forward(
    mut events: broadcast::Receiver<Event>,
    character: String,
    desk: Sender<Called>,
) {
    loop {
        match events.recv().await {
            Ok(Event::Attention(attention)) => {
                let called = Called {
                    character: character.clone(),
                    attention,
                };
                if desk.send(called).is_err() {
                    return;
                }
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

/// The desk: each call that passes is played and notified.
fn run(calls: &Receiver<Called>, sounds: &Path) {
    let mut once = Once::default();
    let mut speaker = Speaker::default();
    for called in calls {
        let Some(pass) = once.admit(&called, Instant::now()) else {
            continue;
        };
        if let Some(sound) = &pass.sound {
            speaker.play(sounds, sound);
        }
        if let Some(words) = &pass.notify {
            notify(&called.character, words);
        }
    }
}

/// What passes the desk: a call's sound and notification, unless another
/// character made the same call within [`SAME`].
#[derive(Debug, Default)]
pub(crate) struct Once {
    /// What passed lately: when, whose, and the call.
    recent: VecDeque<(Instant, String, Arc<Attention>)>,
}

/// A call's sound and notification, to be played and shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pass {
    /// The sound, as the trigger names it.
    pub(crate) sound: Option<String>,
    /// What the notification says.
    pub(crate) notify: Option<String>,
}

impl Once {
    /// `called` at `now`, if it passes.
    pub(crate) fn admit(&mut self, called: &Called, now: Instant) -> Option<Pass> {
        let call = &called.attention;
        if call.sound.is_none() && call.notify.is_none() {
            return None;
        }
        while self
            .recent
            .front()
            .is_some_and(|(at, ..)| now.saturating_duration_since(*at) >= SAME)
        {
            self.recent.pop_front();
        }
        let seen = self.recent.iter().any(|(_, who, earlier)| {
            *who != called.character
                && (&earlier.trigger, &earlier.sound, &earlier.notify)
                    == (&call.trigger, &call.sound, &call.notify)
        });
        if seen {
            return None;
        }
        self.recent
            .push_back((now, called.character.clone(), Arc::clone(call)));
        Some(Pass {
            sound: call.sound.clone(),
            notify: call.notify.clone(),
        })
    }
}

/// The audio device, opened at the first sound, and what has been said
/// about it, so each trouble is said once.
#[derive(Default)]
struct Speaker {
    sink: Option<rodio::MixerDeviceSink>,
    /// The device would not open; no sound is tried again.
    broken: bool,
    /// Sounds found nowhere, already said.
    missing: Vec<String>,
}

impl Speaker {
    fn play(&mut self, sounds: &Path, named: &str) {
        let Some(path) = found(sounds, named) else {
            if !self.missing.iter().any(|said| said == named) {
                match sound::refused(named) {
                    Some(why) => eprintln!("[attention] sound `{named}` not played: it {why}"),
                    None => eprintln!(
                        "[attention] no sound `{named}`: put it in {}",
                        sounds.display()
                    ),
                }
                self.missing.push(named.to_owned());
            }
            return;
        };
        if self.broken {
            return;
        }
        if self.sink.is_none() {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut sink) => {
                    sink.log_on_drop(false);
                    self.sink = Some(sink);
                }
                Err(e) => {
                    eprintln!("[attention] no sound: the audio device would not open: {e}");
                    self.broken = true;
                    return;
                }
            }
        }
        let Some(sink) = &self.sink else {
            return;
        };
        let started = File::open(&path)
            .map_err(|e| e.to_string())
            .and_then(|file| {
                rodio::play(sink.mixer(), BufReader::new(file)).map_err(|e| e.to_string())
            });
        match started {
            Ok(sounding) => sounding.detach(),
            Err(e) => eprintln!("[attention] {}: {e}", path.display()),
        }
    }
}

/// Show `words` as an OS notification from Hydra, for `character`.
fn notify(character: &str, words: &str) {
    let shown = notify_rust::Notification::new()
        .appname("Hydra")
        .summary(&format!("Hydra: {character}"))
        .body(words)
        .show();
    if let Err(e) = shown {
        eprintln!("[attention] notification not shown: {e}");
    }
}

#[cfg(test)]
mod tests;
