//! What the actor holds, and the rule for what is history: **`plan/25` step 2b**.
//!
//! Steps 1 and 2 built a channel and a writer and nothing fed them. This is the
//! feed, kept out of `actor/` so the actor's side is one call per event.
//!
//! # Readouts, and the rule this file used to enforce
//!
//! Some streams are **readouts**: the game clears them and writes the whole
//! state again. [`READOUTS`] is `cena-model/tests/stream_routing.rs`'s census:
//! MEASURED over 24 files across 6 characters, seven ids are ever pushed to,
//! and six are readouts. The seventh, `thoughts`, is never cleared.
//!
//! > **CORRECTED 2026-09-21, the same day.** This file first DROPPED the
//! > readouts outright, on the author's word that they did not belong in a
//! > log. Walking a line from the wire to the file changed that: the wire log
//! > churns, so in a year this is the only record, and *"what was I wearing
//! > when I died"* is answerable only if the readout was captured. The author:
//! > every feed is **available**, which ones are written is **configurable**
//! > because they cost disk, and the viewer filters by tag.
//!
//! So the list no longer drops anything. It names the feeds that default to
//! off and that are written only on change -- see [`super::feed`].

use cena_platform::line_time;

use super::feed::Capture;
use super::{LogLine, PlayerLog};
use crate::lifecycle::{Generation, SessionId};

/// Streams that carry state rather than history: off by default, and written
/// only when they change.
pub const READOUTS: [&str; 6] = ["room", "inv", "bounty", "society", "charprofile", "Spells"];

/// The tag sent commands are logged under.
pub const COMMANDS: &str = "cmd";

/// The tag Hydra's own notices are logged under.
pub const NOTICES: &str = "hydra";

/// The tag the main window's lines are logged under.
pub const MAIN: &str = "main";

/// The tag a stream's lines are logged under.
///
/// The main window's id on the wire is the empty string, which is no use to
/// someone grepping a file; it is written as [`MAIN`].
#[must_use]
pub const fn tag(stream: &str) -> &str {
    if stream.is_empty() { MAIN } else { stream }
}

/// Where a session's [`Tap`] is kept so that everything which speaks for the
/// session can reach it: the actor, and every clone of the `SessionHandle`.
///
/// **A slot rather than a field**, because the handle is built -- and may be
/// cloned -- before `with_player_log` is called. Set once, read by all.
pub type Slot = std::sync::Arc<std::sync::OnceLock<Tap>>;

/// A [`PlayerLog`] that knows whose it is and what it captures.
///
/// Shared, through a [`Slot`], by everything that speaks for the session. Game
/// text goes through a [`Feed`](super::feed::Feed), which holds a chunk until
/// it is classified; a notice comes straight here, because Hydra's own voice
/// belongs to no chunk.
#[derive(Clone, Debug)]
pub struct Tap {
    log: PlayerLog,
    session: SessionId,
    /// What the caller asked for.
    base: Capture,
    /// Where this character's settings file is looked for. `None`: the
    /// caller's capture is the whole story, which is what a test wants.
    settings_dir: Option<std::sync::Arc<std::path::Path>>,
    /// That, with the character's settings file laid over it. Set once, when
    /// the game has named the character; shared by every clone.
    chosen: std::sync::Arc<std::sync::OnceLock<Capture>>,
}

impl Tap {
    /// A tap for `session` writing to `log`, capturing `capture` until the
    /// character's settings file (looked for in `settings_dir`, if given) is
    /// laid over it.
    #[must_use]
    pub fn new(
        log: PlayerLog,
        session: SessionId,
        capture: Capture,
        settings_dir: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            log,
            session,
            settings_dir: settings_dir.map(Into::into),
            base: capture,
            chosen: std::sync::Arc::default(),
        }
    }

    /// Which feeds are written.
    #[must_use]
    pub fn capture(&self) -> Capture {
        self.chosen.get().unwrap_or(&self.base).clone()
    }

    /// The game has named the character: read their settings file
    /// (`settings_store`) and lay its choices over what the caller asked for.
    ///
    /// **The log carries its own directory** rather than borrowing the
    /// snapshot's. MEASURED 2026-09-21: `SupervisedSession` -- what the binary
    /// runs -- has no `with_character_store`, so a hook on the snapshot's load
    /// would have worked in every test and never once in a real session.
    ///
    /// Once: `<app>` is re-sent on every reconnect, and a session's capture
    /// should not change under it because a file was edited mid-hunt.
    ///
    /// # Errors
    ///
    /// Why the file or its section could not be used. The defaults stay in
    /// force; the caller says so, because a session must not stop over a
    /// preference and must not quietly ignore one either.
    pub fn choose(&self, instance: &str, character: &str) -> Result<(), String> {
        let Some(dir) = &self.settings_dir else {
            return Ok(());
        };
        if self.chosen.get().is_some() {
            return Ok(());
        }
        let settings = crate::settings_store::load(dir, instance, character)
            .map_err(|e| e.to_string())?
            .section::<super::feed::LogSettings>(super::feed::SECTION)
            .map_err(|e| format!("the player_log section is malformed: {e}"))?;
        let _ = self.chosen.set(self.base.clone().with(&settings));
        Ok(())
    }

    /// Something Hydra said to the player (`crate::notice`), as `[hydra]`.
    ///
    /// Its own tag, for the reason `notice.rs` gives for its own event: neither
    /// a reader nor a parser may mistake Hydra's voice for the game's.
    pub fn notice(&self, generation: Generation, notice: &crate::notice::Notice) {
        if !self.capture().wants(NOTICES) {
            return;
        }
        let (crate::notice::Body::Lines(lines) | crate::notice::Body::Mono(lines)) = &notice.body;
        for text in lines.iter().filter(|l| !l.trim().is_empty()) {
            self.record_at(generation, line_time(), NOTICES.to_owned(), text.clone());
        }
    }

    /// Record one line under a stamp taken earlier -- when the line arrived,
    /// not when its chunk closed.
    pub(super) fn record_at(&self, generation: Generation, at: String, tag: String, text: String) {
        self.log.record(LogLine {
            at,
            stream: tag,
            text,
            session: self.session,
            generation,
        });
    }
}
