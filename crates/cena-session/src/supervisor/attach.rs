//! What a supervised session can be given before it runs: its log, its
//! stores, its combat recorder.
//!
//! Moved down out of `supervisor.rs` under Rule 4.4 when the two stores took
//! it past its cap (680 of 650). Each of these only sets a field on the
//! durable core; what is done with it is the actor's, per connection.

use cena_platform::SessionSink;

use super::{Connector, SupervisedSession};

impl<C: Connector> SupervisedSession<C> {
    /// Attach a log sink. It spans every generation, so one file records the
    /// whole session including its reconnects.
    #[must_use]
    pub fn with_sink(mut self, sink: SessionSink) -> Self {
        self.core.sink = Some(sink);
        self
    }

    /// Give the combat tracker its crit tables. See
    /// [`Session::with_crit_tables`](crate::Session::with_crit_tables); the
    /// state is carried across connections, so once is enough.
    #[must_use]
    pub fn with_crit_tables(
        mut self,
        tables: std::sync::Arc<cena_model::crit::CritTables>,
    ) -> Self {
        self.core.state.combat_mut().set_crit_tables(tables);
        self
    }

    /// Offer every closed chunk's combat facts to this recorder, on every
    /// connection this session makes.
    #[must_use]
    pub fn with_combat_recorder(
        mut self,
        recorder: crate::combat_recorder::worker::RecorderHandle,
    ) -> Self {
        self.core.combat = Some(recorder);
        self
    }

    /// Keep what the character has learned in this directory, across logins
    /// (`character_store`, and `Session::with_character_store`).
    ///
    /// **Missing until 2026-09-21, and nothing noticed.** `Session` had this
    /// and `SupervisedSession` -- what the binary runs -- did not, so no live
    /// session ever read or wrote a snapshot: MEASURED, there was no data
    /// directory on the author's machine after several live runs. Every test
    /// of the store drives a plain `Session`, so every test passed.
    #[must_use]
    pub fn with_character_store(mut self, dir: std::path::PathBuf) -> Self {
        self.core.character_dir = Some(dir);
        self
    }

    /// Write `<cmdlist>` pushes to this directory the moment they arrive
    /// (`menu_store`, and `Session::with_menu_store`). Missing for the same
    /// reason as [`Self::with_character_store`].
    #[must_use]
    pub fn with_menu_store(mut self, dir: std::path::PathBuf) -> Self {
        self.core.menu_dir = Some(dir);
        self
    }

    /// Record what the player saw and sent, on every connection this session
    /// makes (`plan/25`). One log spans a reconnect; the generation on each
    /// line says where the boundary was.
    #[must_use]
    pub fn with_player_log(
        self,
        log: crate::PlayerLog,
        capture: crate::player_log::Capture,
        settings_dir: Option<std::path::PathBuf>,
    ) -> Self {
        let _ = self.core.player_log.set(crate::player_log::Tap::new(
            log,
            self.core.id,
            capture,
            settings_dir,
        ));
        self
    }
}
