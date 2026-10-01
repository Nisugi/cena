//! [`EndReason`]: why one connection ended, and whether to reconnect.
//!
//! Split out of `actor.rs` under Rule 4.1 (`plan/05:352-353`) -- **move code
//! down, do not raise the cap.** That file reached 402 lines against the 400
//! default when Milestone 2's end-reason landed, and its header had already
//! named this split in advance. Taken as written rather than argued about.
//!
//! The loop and the session's shape stay in [`super`]; what is here is the
//! vocabulary for *how a connection stopped*, which a supervisor reads and the
//! actor merely reports -- and, since Milestone 2's supervisor took `actor.rs`
//! to 437 lines, the **stopping itself**: [`SessionActor::shutdown`] and
//! [`SessionActor::transition`] came here in the second half of the split this
//! file's own header named.
//!
//! # What was moved back, and why
//!
//! The logging helpers (`log`, `log_wire`) came here too when `actor.rs` was
//! 5 lines over -- and that was **arithmetic, not a seam**. They are used by
//! `ingest` and `pump` on every turn, not only at the end, so they belonged
//! with the loop all along. When the author raised the default cap from 400 to
//! 800 they went back.
//!
//! Recorded because it is the cost the cap change was meant to stop paying: a
//! seam chosen to shed five lines is a seam nobody can explain later.

use super::{Event, SessionActor, SessionEnd, State};
use cena_platform::ByteSource;

/// Why one connection ended.
///
/// # What Milestone 1 deferred, arriving
///
/// The loop's read arm collapsed `Ok(0)` and `Err(_)` into one `break`, with a
/// comment saying distinguishing them "would only matter to reconnect, which
/// `plan/12` §9c puts in Milestone 2". This is Milestone 2 -- **but that
/// comment framed the question slightly wrong, and it is worth saying how.**
///
/// A supervisor does *not* care whether the peer hung up or the socket errored:
/// both are a lost transport and both reconnect. What it cares about is telling
/// either of them from a **cancellation**, which the M1 loop also collapsed and
/// which that comment never mentions. The distinction that matters is
/// deliberate-stop versus lost-transport, and it cuts across the one M1 named.
///
/// # Why `PeerClosed` and `ReadFailed` are still separate
///
/// [`Self::warrants_reconnect`] treats them identically, so they could be one
/// variant. They are two because **a log has to be able to tell them apart**:
/// "the server hung up" and "the socket errored" are different facts about a
/// session, and a transcript that says only "disconnected" is unreadable at
/// exactly the moment someone is working out why a session flapped.
///
/// That is the same argument [`Origin::Script`](crate::Origin::Script) makes --
/// a separate variant "not because it queues differently but because a log has
/// to be able to tell them apart."
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The cancel token fired: a deliberate stop.
    ///
    /// The player quit, a test ended, or `plan/16` §5b's orderly shutdown ran.
    /// **Never reconnects** -- this is the only variant that does not.
    Cancelled,
    /// `read` returned `Ok(0)`: the peer hung up, or a recording ran out.
    PeerClosed,
    /// A read failed. The socket is gone.
    ReadFailed,
    /// A write failed mid-`pump`. The command it failed on has
    /// already been answered [`Outcome::Dead`](crate::Outcome::Dead).
    WriteFailed,
}

impl EndReason {
    /// Whether a supervisor should open a new connection.
    ///
    /// **[`Self::Cancelled`] is the only `false`, and that is the whole rule.**
    /// A clean quit must not reconnect; everything else is a lost transport.
    ///
    /// A method rather than a `match` at the call site, because adding a
    /// variant must be a compile error *somewhere that decides this*. A
    /// wildcard arm in a supervisor would silently default a new reason to
    /// "reconnect", which is the wrong direction to be wrong in.
    #[must_use]
    pub const fn warrants_reconnect(self) -> bool {
        match self {
            Self::Cancelled => false,
            Self::PeerClosed | Self::ReadFailed | Self::WriteFailed => true,
        }
    }
}

impl<S: ByteSource> SessionActor<S> {
    /// Hand the closed connection's resources back to its owner. A plain
    /// session publishes its terminal snapshot here; a supervisor carries
    /// observation requests forward until the whole session ends.
    pub(super) fn into_end(mut self, reason: EndReason) -> SessionEnd<S> {
        if self.on_disconnect == crate::Outcome::Dead {
            self.observations
                .finish(self.events.snapshot(&self.state, self.lifecycle));
        }
        // The player's Lich outlives the connection: what it shows waits for
        // the next one's actor.
        if let Some(text) = self.lich_text.take() {
            self.events.lich_text().put_back(text);
        }
        SessionEnd {
            recorder: self.recorder,
            state: self.state,
            lifecycle: self.lifecycle,
            source: self.source,
            reason,
            commands: self.commands,
            sink: self.sink,
            observations: self.observations,
        }
    }

    /// Close the source and answer everyone still waiting.
    ///
    /// Everyone is answered [`Self::on_disconnect`] -- `Dead` for a plain
    /// session, `Disconnected` for a supervised one -- **except after a
    /// cancellation**, which is always `Dead`: a deliberate stop is not
    /// followed by a reconnect whoever owns the actor
    /// ([`EndReason::warrants_reconnect`]).
    pub(super) async fn shutdown(&mut self, reason: EndReason) {
        // A quit still pending here was not resolved by the EOF or the
        // deadline, which leaves the read/write-failure paths: the transport
        // died while we were waiting for a polite goodbye. `Unsent` is the
        // honest answer -- the command went out but nothing acknowledged it,
        // and the socket is gone rather than merely slow.
        //
        // This runs on EVERY exit path, so no caller of `quit()` is left
        // waiting on a reply that never comes.
        self.finish_quit(crate::command::Farewell::Unsent);
        // What waited for a script's display hooks is shown: the connection
        // that held it is over.
        self.show_held(true);
        // Idempotent by the trait's contract, which is why this is safe on
        // every one of the three exit paths.
        let _ = self.source.shutdown().await;
        let outcome = if reason.warrants_reconnect() {
            self.on_disconnect.clone()
        } else {
            crate::command::Outcome::Dead
        };
        self.queue.answer_all_waiters(&outcome);
        // Flush the character store unconditionally. This is what makes the
        // five-minute window safe: an ordinary logout never waits on it, so
        // the only way to lose facts is a crash, and the facts a crash could
        // lose are re-taught by a sync.
        self.save_character();
        // Made off this task, and waited for here: the facts are on disk
        // before anyone is told the session ended.
        self.saves.finish().await;
        self.log_saves();
        // **`Closed` means the session is over, so it is published only when
        // it is** (review finding 2). A supervised connection that was LOST is
        // not the end of anything: the supervisor is about to publish
        // `Reconnecting` with the next generation (`supervisor.rs`,
        // `reconnect`). Publishing `Closed` first told every observer the
        // session had ended -- `State::Closed` is documented as "the task has
        // ended", and `cena-behavior`'s travel maps it to `Dead` -- and then
        // contradicted it one event later. VERIFIED: with this guard removed,
        // `observation.rs`'s `a_lost_connection_goes_to_reconnecting_without_closing`
        // fails on a `Closed` before `Reconnecting`.
        //
        // The lifecycle this actor hands back is then its last live state,
        // and the supervisor owns what comes next: `Reconnecting`, or
        // `Closed` from `finish` if it decides to stop. A plain `Session`, and
        // any cancellation, has nothing after it and closes here as before.
        if !self.connection_loss_is_supervised(reason) {
            self.transition(State::Closed);
        }
        // Flush LAST, after the Closed transition has been logged, so the file
        // records its own end. Buffered writers otherwise lose the final lines
        // -- which are the ones that say why a session stopped.
        if let Some(sink) = self.sink.as_mut() {
            let _ = sink.flush();
        }
    }

    /// Whether this ending is a lost connection that a supervisor will act on,
    /// rather than the end of the session.
    ///
    /// `on_disconnect` is the fact the owner gave the actor about whether
    /// anything will open another connection -- the same fact `into_end`
    /// reads -- and `warrants_reconnect` is the actor's own verdict on the
    /// reason. Both must hold: a cancelled supervised session is over.
    fn connection_loss_is_supervised(&self, reason: EndReason) -> bool {
        reason.warrants_reconnect() && self.on_disconnect == crate::Outcome::Disconnected
    }

    /// Answer a pending quit, if there is one. Returns whether there was.
    ///
    /// The return value is what tells [`EndReason::PeerClosed`] from
    /// [`EndReason::Cancelled`] at the one place they are genuinely
    /// ambiguous -- an `Ok(0)` that is either the server hanging up or the
    /// server doing exactly what it was asked.
    ///
    /// Idempotent: the callers are taken, so a quit answered by the EOF is
    /// not answered again by the deadline. Every caller waiting is told the
    /// one verdict (the crate review of 2026-09-28, R7).
    pub(super) fn finish_quit(&mut self, farewell: crate::command::Farewell) -> bool {
        let Some(pending) = self.quitting.as_mut() else {
            return false;
        };
        let replies = std::mem::take(&mut pending.replies);
        if !replies.is_empty() {
            self.log(&format!("quit: {farewell:?}"));
        }
        for reply in replies {
            let _ = reply.send(farewell);
        }
        true
    }

    /// When a pending quit gives up on the server.
    ///
    /// Returns a far-future instant when nothing is quitting, so the loop's
    /// `sleep_until` arm is always well-formed -- it is disabled by its guard
    /// rather than by the value. An hour is arbitrary and unreachable: the arm
    /// is never polled without the guard being true.
    pub(super) fn quit_deadline(&self) -> tokio::time::Instant {
        self.quitting.as_ref().map_or_else(
            || tokio::time::Instant::now() + std::time::Duration::from_hours(1),
            |pending| pending.deadline,
        )
    }

    /// When the dirty groups are due to be written.
    ///
    /// A far-future instant when nothing is waiting, so the `select!` arm is
    /// always well-formed; its guard is what stops it firing. Same shape as
    /// [`Self::quit_deadline`], for the same reason.
    /// Read this character's stored facts into the model, once.
    ///
    /// # It reports rather than syncs
    ///
    /// Publishes [`Event::SyncNeeded`] with whatever the store says is stale.
    /// Running the sync means sending up to fifteen commands, and `plan/12`
    /// §4.2 gives the authority to one claimant at a time -- an actor that
    /// issued them on its own would be a claimant nobody claimed. So the
    /// session says what it found and whoever owns the character decides
    /// whether to spend that traffic; `cena_behavior::sync` is what runs it.
    ///
    /// # Why this is not in the constructor
    ///
    /// The file is named after the character, and the character's name arrives
    /// with `<app>` partway into the login burst -- so there is nothing to
    /// load from until the game says who logged in. This runs on the frame
    /// that teaches it.
    ///
    /// # Why it is refused after the first time
    ///
    /// A second load would overwrite what the session has learned since with
    /// the older values on disk. `<app>` is re-sent on reconnect, which is
    /// exactly when that would happen.
    ///
    /// A missing file is the ordinary case for a character nobody has synced,
    /// and a stale one is refused by `restore_into` rather than migrated. Both
    /// leave the model empty, which `stale_groups` then reports as everything
    /// needing a sync -- the right answer in both cases.
    pub(super) fn load_character(&mut self) {
        if self.persistence.loaded {
            return;
        }
        let (Some(dir), Some(name), Some(instance)) = (
            self.persistence.dir.clone(),
            self.state.character.name.clone(),
            self.state.character.instance.clone(),
        ) else {
            return;
        };
        self.persistence.loaded = true;
        // What the store says needs re-reading. Computed from whatever was
        // loaded -- including nothing, where every group is stale and the
        // answer is a full sync.
        let mut stale = cena_model::state::character::snapshot::Group::ALL.to_vec();
        match crate::character_store::load(&dir, &instance, &name) {
            Ok(snapshot) => {
                let restored = snapshot.restore_into(&mut self.state.character);
                // `restore_into` re-checks the schema version, so a file the
                // store accepted can still be refused here. Logged either way:
                // "loaded nothing" and "loaded a character" are different
                // facts and a session log that showed neither would make a
                // blank character look like a fresh one.
                self.log(&format!(
                    "character store: {} for {instance}_{name}",
                    if restored {
                        "restored stored facts"
                    } else {
                        "refused a snapshot from another schema version"
                    }
                ));
                // Only from a snapshot we actually believed. A refused one has
                // timestamps written by a different parser, so trusting them
                // would skip a sync the version check exists to force.
                if restored {
                    stale = snapshot.stale_groups(
                        std::time::SystemTime::now(),
                        crate::character_store::MAX_STALE,
                    );
                }
            }
            Err(crate::character_store::LoadError::Missing) => {
                self.log("character store: nothing stored for this character yet");
            }
            Err(err) => {
                self.log(&format!("character store: could not load: {err}"));
            }
        }
        if !stale.is_empty() {
            self.log(&format!("character store: {} group(s) stale", stale.len()));
        }
        let _ = self.events.send(Event::SyncNeeded(stale));
    }

    /// The game has named the character, so the player log can read which
    /// feeds they chose (`settings_store`, `player_log/tap.rs`).
    pub(super) fn choose_log_feeds(&mut self) {
        let (Some(log), Some(name), Some(instance)) = (
            &self.player_log,
            self.state.character.name.as_deref(),
            self.state.character.instance.as_deref(),
        ) else {
            return;
        };
        if let Err(why) = log.choose(instance, name) {
            self.log(&format!("settings: player log using defaults -- {why}"));
        }
    }

    /// # Its own `select!` arm, not folded into the quit timer
    ///
    /// Merging them saves one `Sleep` in `run`'s future. That was tried and
    /// REVERTED: it did not clear the `large_futures` warnings this feature
    /// introduces -- MEASURED 16776 bytes merged against a 16384 limit, versus
    /// 16784 unmerged -- and it put an unrelated five-minute timer in the path
    /// of the quit deadline, which criterion 6 ("no leaked sockets") makes
    /// safety-critical. A cosmetic lint is not a reason to touch that path.
    ///
    /// Those warnings are the feature's total footprint against an actor that
    /// was already just under the limit, not one isolable cause; three guesses
    /// at one were wrong. They are `pedantic`, the build is green, and
    /// `tokio::spawn` heap-allocates the future in production anyway -- only
    /// tests that `timeout` it see the lint.
    pub(super) fn save_deadline(&self) -> tokio::time::Instant {
        self.persistence
            .deadline
            .unwrap_or_else(|| tokio::time::Instant::now() + std::time::Duration::from_hours(1))
    }

    /// Push the save out to five minutes from now.
    ///
    /// Called whenever a group is marked, which is what makes the window
    /// "five minutes after it stops changing" rather than five minutes after
    /// it starts.
    pub(super) fn defer_save(&mut self) {
        self.persistence.deadline =
            Some(tokio::time::Instant::now() + crate::dirty_groups::IDLE_WINDOW);
    }

    /// Write the dirty groups, if there are any and a store is configured.
    ///
    /// Merges into whatever is on disk rather than replacing it: a group this
    /// session never learned must keep the timestamp and values an earlier
    /// session stored, or every save would report the rest of the character as
    /// never-synced and trigger a full re-sync on the next login.
    ///
    /// A failure is LOGGED AND SWALLOWED. The facts are already in the model,
    /// so the session is correct either way, and refusing to keep playing
    /// because a file could not be written would be the wrong trade.
    pub(super) fn save_character(&mut self) {
        self.persistence.deadline = None;
        if self.persistence.groups.is_empty() {
            return;
        }
        let Some(dir) = self.persistence.dir.clone() else {
            // Nothing to write to: drop the marks rather than accumulating
            // them forever in a session that will never save.
            let _ = self.persistence.groups.drain();
            return;
        };
        let (Some(name), Some(instance)) = (
            self.state.character.name.clone(),
            self.state.character.instance.clone(),
        ) else {
            // Before `<playerID>` and `<settingsInfo>` arrive there is no
            // filename to write under. The marks STAY: the login burst sends
            // both within the first few lines, and the next deadline writes
            // everything that was learned in the meantime.
            self.log("character store: waiting for the character's name");
            return;
        };

        let groups = self.persistence.groups.drain();
        let now = std::time::SystemTime::now();
        // Taken now, as the character is; read, merged and written off this
        // task, in order with every other save (`saves.rs`).
        let mut snapshot = cena_model::state::character::snapshot::CharacterSnapshot::of(
            &name,
            &instance,
            &self.state.character,
            std::collections::BTreeMap::new(),
        );
        self.saves.queue(move || {
            // Start from what is stored so untouched groups keep their stamps.
            let mut updated_at = crate::character_store::load(&dir, &instance, &name)
                .map(|stored| stored.updated_at)
                .unwrap_or_default();
            for group in &groups {
                updated_at.insert(*group, now);
            }
            snapshot.updated_at = updated_at;
            Some(match crate::character_store::save(&dir, &snapshot) {
                Ok(path) => format!(
                    "character store: wrote {} group(s) to {}",
                    groups.len(),
                    path.display()
                ),
                Err(err) => format!("character store: write failed: {err}"),
            })
        });
        self.log_saves();
    }

    /// Log what the saves made so far had to say.
    pub(super) fn log_saves(&mut self) {
        for line in self.saves.heard() {
            self.log(&line);
        }
    }

    pub(super) fn transition(&mut self, next: State) {
        self.lifecycle = next;
        self.log(&format!("lifecycle {next:?}"));
        let _ = self.events.send(Event::StateChanged(next));
    }
}

// ---------------------------------------------------------------------------
// Leaving at the player's word: `quit`, who may send it, and sending it.
// Moved down from `io.rs` when checking who may quit took it past its cap
// (the review of 2026-09-29): this file is the session stopping itself.
// ---------------------------------------------------------------------------

/// What Cena sends to log out (`plan/16` §5b).
///
/// Lich recognises either `exit` or `quit`, optionally wrapped in `<c>`
/// (`reference/lich-5/lib/common/shutdown_intent.rb:7`:
/// `/\A\s*(?:<c>)?\s*(?:exit|quit)\s*\z/i`). `quit` is chosen because it is
/// the word the author used and the one a player types; `exit` is the same
/// thing to both Lich and the game.
pub(super) const EXIT_COMMAND: &str = "quit";

/// How long a typed exit waits for the server's EOF.
///
/// The same bound `SessionHandle::quit` uses by default. A typed `quit` has no
/// caller holding a deadline -- the player is not awaiting a `Farewell` -- so
/// the actor supplies one rather than waiting unbounded, which `plan/12` §5.5
/// forbids.
pub(super) const QUIT_EOF_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// Is this line the player asking to log out?
///
/// Lich's own test (`reference/lich-5/lib/common/shutdown_intent.rb:7`):
///
/// ```text
/// /\A\s*(?:<c>)?\s*(?:exit|quit)\s*\z/i
/// ```
///
/// Reproduced without a regex crate -- `cena-session` has no `regex`
/// dependency and one line of trimming is not worth acquiring one (Rule -1).
/// The `<c>` prefix is Lich's client-command wrapper, kept because a frontend
/// porting Lich's input path may pass it through.
///
/// **And the game's abbreviations**, which Lich's test misses: the game
/// takes `qui` as `quit` (the author, 2026-10-01), so a `qui` that was not
/// recognised went to the game as a plain line, past [`may_quit`], and its
/// close was read as a drop and reconnected (the crate review of
/// 2026-10-01, L-2). `exi` is taken too: no other verb begins `exi`
/// (`reference/wiki_clean/Verb_VERB_Verb list.txt`: `exchange`, `exhale`,
/// `expel`, `experience`, `express`, `extinguish`, `extract`). Shorter ones
/// are not quits: the game's parser needs three letters (the author,
/// 2026-10-01), so `q`, `qu` and `ex` go as the ordinary lines they are.
///
/// [`may_quit`]: SessionActor::may_quit
pub(crate) fn is_exit_intent(line: &str) -> bool {
    let word = bare(line);
    ["quit", "qui", "exit", "exi"]
        .iter()
        .any(|exit| word.eq_ignore_ascii_case(exit))
}

/// The line without its spacing or Lich's `<c>`.
fn bare(line: &str) -> &str {
    let line = line.trim();
    line.strip_prefix("<c>").unwrap_or(line).trim()
}

impl<S: ByteSource> SessionActor<S> {
    /// Whether a `quit` from `origin` may log the character out.
    ///
    /// The author, 2026-09-29, of players who idle out on a timer: *"I think
    /// we allow it at higher permission levels."* So the player, their own
    /// scripts and triggers (an imported trigger's send is already held until
    /// approved, `plan/45` §1 row 1) and their own Lich may; an agent only
    /// while it holds the authority, which its `takeover` level gives it; and
    /// Hydra's own behaviors never, whatever they hold, nor Hydra's own asking.
    pub(super) fn may_quit(&self, origin: crate::command::Origin) -> bool {
        use crate::command::Origin;
        match origin {
            Origin::Manual | Origin::Script | Origin::Trigger | Origin::Lich => true,
            Origin::Agent(Some(token)) => self.queue.authority() == Some(token),
            Origin::Agent(None) | Origin::Behavior(_) | Origin::Hydra => false,
        }
    }

    /// Send the exit command and start waiting for the server's EOF
    /// (`plan/16` §5b.3).
    ///
    /// **Does not end the loop.** The loop keeps reading, because the read is
    /// what observes the close -- see [`SessionActor::quitting`]. What this
    /// does is put the command on the wire and arm the deadline.
    ///
    /// A second quit while one is pending is not re-sent: the server has
    /// already been asked, and sending `quit` twice against a type-ahead
    /// buffer of 2 would spend a slot for nothing (`plan/16` §5.2b). The newer
    /// caller waits for the first one's verdict and is told the same, so
    /// `Acknowledged` always means the server closed. It was told
    /// `Acknowledged` at once, while the server had said nothing
    /// (the crate review of 2026-09-28, R7).
    /// Returns `false` if the write failed, which means **the connection is
    /// gone** and the caller must end it rather than keep reading.
    pub(super) async fn begin_quit(
        &mut self,
        timeout: std::time::Duration,
        reply: tokio::sync::oneshot::Sender<crate::command::Farewell>,
    ) -> bool {
        if let Some(pending) = self.quitting.as_mut() {
            // Already asked: this caller waits for the same verdict.
            pending.replies.push(reply);
            return true;
        }

        // The write goes through the same one-write path every command uses:
        // two writes can emit two TLS records and the server drops the command
        // (`cena_platform::bytes::ByteSource::write_all`).
        let mut message = Vec::with_capacity(EXIT_COMMAND.len() + 1);
        message.extend_from_slice(EXIT_COMMAND.as_bytes());
        message.push(b'\n');
        if self.write_bounded(&message).await.is_err() {
            // Nothing to say goodbye to -- or a cancel overtook the write,
            // which ends the session as a quit would. Lich raises `IOError` here
            // (`orderly_shutdown.rb:181`); Cena reports it and lets the caller
            // cancel, because a transport that cannot be written to is already
            // the state a shutdown was trying to reach.
            self.log("quit: could not send, transport gone");
            let _ = reply.send(crate::command::Farewell::Unsent);
            return false;
        }
        self.recorder.outbound(&message);
        self.log_wire(false, &message);
        // Published like any other send, so an observer sees the session's
        // last act rather than it vanishing.
        let _ = self.events.send(super::Event::Sent {
            line: EXIT_COMMAND.to_owned(),
            origin: crate::command::Origin::Manual,
            by: None,
        });
        self.log(&format!("quit: sent, awaiting EOF within {timeout:?}"));
        self.quitting = Some(super::Quitting {
            // `checked_add`, because `Instant + Duration` PANICS on overflow
            // and `timeout` is caller-supplied. `SessionHandle::quit` doubles
            // its own backstop before passing it, so a caller near the top of
            // the range gets there in one multiply. A panic here takes down
            // the actor on the one path whose entire job is an orderly exit
            // (review SE-11).
            //
            // Saturating means "no deadline in any practical sense", which is
            // the honest reading of a caller asking to wait ~584 years.
            deadline: tokio::time::Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(|| {
                    tokio::time::Instant::now() + std::time::Duration::from_hours(24)
                }),
            replies: vec![reply],
        });
        true
    }
}
