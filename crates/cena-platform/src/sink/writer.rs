//! The writer itself: two files per session, and the redaction applied on the
//! way to them.
//!
//! Split from [`super`] under `plan/05` Rule 4.4 -- a `mod.rs` re-exports and
//! wires, it does not implement. The architecture test caught this code
//! sitting in the facade and named the rule, exactly as it did for
//! `eaccess/mod.rs` earlier the same day.
//!
//! Read [`super`] for what these files are for, why the recorder does not own
//! one, and why a session log is a private development artifact rather than
//! something shareable.

use super::config::{
    CLIENT_CLOSE, CLIENT_OPEN, bytes_timestamps_enabled, line_time, rotate_after_lines,
};
use super::redactions::Redactions;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Two files for one session: the raw wire, and what we did.
///
/// Holds its own handles rather than reaching a global. `plan/05` Rule 5.2 --
/// no process globals, none -- and 25 sessions must write 25 independent
/// transcripts without contending on one logger.
#[derive(Debug)]
pub struct SessionSink {
    bytes: BufWriter<File>,
    events: BufWriter<File>,
    redactions: Redactions,
    bytes_path: PathBuf,
    events_path: PathBuf,
    lines_written: u64,
    /// `<dir>/<char>-<stamp>`, shared by every part of this session.
    stem: PathBuf,
    /// Which part is open. 0 is the unnumbered first file.
    part: u32,
    /// Lines before rolling. Read once at creation, for the same reason
    /// `stamp_bytes` is: a threshold that changed mid-session would produce
    /// parts of inconsistent size.
    rotate_after: u64,
    /// Inbound bytes held back so a secret cannot hide in a chunk boundary.
    ///
    /// Bounded by `longest_secret - 1`, and empty whenever nothing is
    /// registered. See `redact_across_chunks`; `drain_pending` is what
    /// guarantees these bytes still reach the file.
    pending: Vec<u8>,
    /// Whether the last inbound byte seen left a line unfinished.
    line_open: bool,
    /// Framed commands waiting for that line to end. See [`Self::wire`].
    awaiting_line_end: Vec<Vec<u8>>,
    /// Whether the bytes file carries per-line times. Read once at creation
    /// rather than per line: an env var that changed mid-session would produce
    /// a file that is half one format.
    stamp_bytes: bool,
}

/// Flush on drop, so a sink that is simply dropped still leaves a complete
/// file.
///
/// `BufWriter` already does this for its own buffer, but the straddle tail
/// (`redact_across_chunks`) is OURS: bytes the wire really carried, held back
/// deliberately. Losing them would make the `.bytes` file silently short --
/// exactly the failure mode the file exists to prevent, since it is what
/// fixtures are cut from.
///
/// Errors are swallowed, as they must be in `Drop`. Callers that need to KNOW
/// the write succeeded call [`SessionSink::flush`], which returns the error.
impl Drop for SessionSink {
    fn drop(&mut self) {
        let _ = self.drain_pending();
        let _ = self.bytes.flush();
        let _ = self.events.flush();
    }
}

impl SessionSink {
    /// Create both files under `dir`, named for the character and a stamp.
    ///
    /// The stamp is the caller's, not read from a clock here: this crate's
    /// [`record`](crate::record) module explains why a wall clock inside the
    /// recording path is the easiest way to make a replay non-deterministic.
    /// The same reasoning applies to a filename a test might assert on.
    ///
    /// # Errors
    ///
    /// Any failure to create the directory or either file.
    pub fn create(
        dir: &Path,
        character: &str,
        stamp: &str,
        redactions: Redactions,
    ) -> io::Result<Self> {
        Self::create_with_rotation(dir, character, stamp, redactions, rotate_after_lines())
    }

    /// [`Self::create`], with the rotation threshold given rather than read
    /// from the environment.
    ///
    /// Exists because a test must be able to roll a file without setting a
    /// process-wide environment variable: `unsafe_code = "deny"` makes
    /// `set_var` unavailable (it is `unsafe` since Rust 2024), and a test that
    /// mutated global state would race every other test in the binary anyway.
    ///
    /// Taking the value is also simply better design -- the threshold is a
    /// property of this sink, and reading it from the environment at the point
    /// of use was a hidden input.
    ///
    /// # Errors
    ///
    /// Any failure to create the directory or either file.
    pub fn create_with_rotation(
        dir: &Path,
        character: &str,
        stamp: &str,
        redactions: Redactions,
        rotate_after: u64,
    ) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        // The character name is a filename component, so it must not be able
        // to escape the directory or name a device. Characters are
        // alphabetical in practice; anything else is dropped rather than
        // substituted, so two names cannot collide through substitution.
        let safe: String = character
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        let safe = if safe.is_empty() {
            "session".to_owned()
        } else {
            safe
        };

        // `<char>-<stamp>` is the stem every part of this session shares, and
        // **every part is numbered, including the first**.
        //
        // The first version left part 0 unnumbered -- `Tester-stamp.bytes`,
        // then `Tester-stamp-001.bytes` -- on the reasoning that a short
        // session should have no suffix to explain. That sorts WRONG:
        // lexicographically `-` (0x2D) precedes `.` (0x2E), so the unnumbered
        // first part sorts AFTER every numbered one, and reading a directory
        // in name order replays the session out of order.
        //
        // Caught by the reassembly assertion in `sink_redaction.rs`, not by
        // the part-count one -- every part existed and held the right bytes,
        // and only their order was wrong.
        let safe_stem = format!("{safe}-{stamp}");
        let bytes_path = dir.join(format!("{safe_stem}-000.bytes"));
        let events_path = dir.join(format!("{safe_stem}.log"));
        let bytes = BufWriter::new(File::create(&bytes_path)?);
        let mut events = BufWriter::new(File::create(&events_path)?);

        // Say what this file is and what it is not, in the file itself. A log
        // read six months from now must not have to guess whether it was
        // scrubbed.
        // The set GROWS after this line: the launch key is minted by the
        // connect and registered afterwards (`redact_key`), because it does not
        // exist when the file is created. So this header must describe what was
        // registered AT CREATION and point at the in-band marker for the rest.
        //
        // FIXED 2026-09-19, found by the first live web-login run. It used to
        // say "redacted: NO -- nothing was registered, treat this file as raw"
        // whenever the creation-time set was empty -- which is EVERY live
        // session, since the key arrives later. The body of that same file then
        // showed `redaction registered (session key)`, so the header
        // contradicted its own contents and did so in the dangerous direction:
        // it told a future reader to treat a redacted log as raw.
        writeln!(
            events,
            "# Hydra session log -- PRIVATE DEVELOPMENT LOG, not automatically shareable.\n\
             # Credentials registered BEFORE the first byte: {}.\n\
             # The session key is registered LATER, when the connect mints it --\n\
             # look for `redaction registered (session key)` below. Everything\n\
             # before that line predates its redaction.\n\
             # OTHER PLAYERS' NAMES ARE NOT REDACTED. Cut fixtures through\n\
             # cena_protocol::scrub with the names named, as CLAUDE.md requires.",
            if redactions.is_empty() {
                "none -- no account was registered".to_owned()
            } else {
                redactions.kinds()
            }
        )?;

        Ok(Self {
            bytes,
            events,
            redactions,
            bytes_path,
            events_path,
            lines_written: 0,
            pending: Vec::new(),
            line_open: false,
            awaiting_line_end: Vec::new(),
            stamp_bytes: bytes_timestamps_enabled(),
            stem: dir.join(safe_stem),
            part: 0,
            rotate_after,
        })
    }

    /// Append raw wire bytes, redacted, marking the direction.
    ///
    /// **The framing is `logxml.lic`'s, not a new one.** Inbound bytes are
    /// written verbatim; client input is wrapped in `<!-- CLIENT -->` /
    /// `<!-- ENDCLIENT -->`. No sequence number: that format carries none, and
    /// [`Recorder`](crate::record::Recorder) already owns ordering, so writing
    /// one here would be a second source of truth for the same fact.
    ///
    /// Chunk boundaries are preserved because
    /// [`Recorder`](crate::record::Recorder)'s own docs explain that replaying
    /// the split that happened live is what drives `Parser::push_bytes`'s
    /// partial-line path with a real boundary rather than an invented one.
    ///
    /// # Errors
    ///
    /// Any write failure.
    pub fn wire(&mut self, inbound: bool, bytes: &[u8]) -> io::Result<()> {
        if inbound {
            // `false` means every byte is still held back waiting for more:
            // nothing was written, so there is no line to count.
            if !self.inbound(bytes)? {
                return Ok(());
            }
        } else {
            let record = self.client_record(bytes);
            // **A command waits for the inbound line in progress to end.**
            //
            // The server's bytes before the command reached this file split
            // wherever a TCP read ended -- or wherever the redaction hold cut
            // them, which with any secret registered is every chunk. Writing
            // the command at once spliced it into the middle of a line, even
            // a tag. VERIFIED before this fix:
            //
            // ```text
            // <prompt ti<!-- CLIENT -->look<!-- ENDCLIENT -->
            // me="1758600000">&gt;</prompt>
            // ```
            //
            // That is not a parseable file, and this is the file fixtures are
            // cut from. `logxml.lic`, whose framing this copies, is written a
            // line at a time and so never splits one; interleaving at the next
            // line end is the same granularity. The command moves by at most
            // the rest of one line, and `Recorder` keeps the exact order for
            // replay.
            //
            // It also makes the flush below safe for redaction: the held-back
            // tail ends in a newline, and no secret spans one.
            if self.line_open {
                self.awaiting_line_end.push(record);
            } else {
                self.drain_pending()?;
                self.bytes.write_all(&record)?;
            }
        }
        // **NOTHING is appended to inbound bytes**, and that fidelity is this
        // file's whole purpose. This used to add a newline to any chunk that did
        // not end in one, which splits a tag straddling a read: a boundary
        // inside `<pushStream id='room'/>` was written with a newline in the
        // middle of the attribute, so a replay parsed something the live session
        // never saw.
        //
        // `wire`'s own doc says chunk boundaries are preserved because replaying
        // the real split drives `Parser::push_bytes`'s partial-line path with a
        // real boundary. The append contradicted the sentence above it.
        //
        // The `.bytes` file is what M2's golden corpus is cut from and what
        // criterion 7 replays, so a byte invented here is inherited by every
        // future fixture. Found by review.
        self.lines_written += 1;
        if self.lines_written >= self.rotate_after {
            self.roll()?;
        }
        Ok(())
    }

    /// Write inbound bytes, releasing any command that was waiting for the
    /// line they finish. Returns whether anything reached the file.
    fn inbound(&mut self, bytes: &[u8]) -> io::Result<bool> {
        let Some(last) = bytes.last() else {
            return Ok(false);
        };
        let mut wrote = false;
        let mut rest = bytes;
        if !self.awaiting_line_end.is_empty()
            && let Some(at) = rest.iter().position(|&b| b == b'\n')
        {
            let (line_end, after) = rest.split_at(at + 1);
            self.inbound_chunk(line_end)?;
            // The tail now ends in a newline, so draining it cannot cut a
            // secret in two -- and the drain releases the waiting commands.
            self.drain_pending()?;
            wrote = true;
            rest = after;
        }
        if !rest.is_empty() {
            wrote |= self.inbound_chunk(rest)?;
        }
        self.line_open = *last != b'\n';
        Ok(wrote)
    }

    /// One inbound chunk through the redaction hold.
    fn inbound_chunk(&mut self, bytes: &[u8]) -> io::Result<bool> {
        // `None` means every byte is still held back waiting for more: nothing
        // to write yet, and writing a stamp for it would invent a line.
        let Some(redacted) = self.redact_across_chunks(bytes) else {
            return Ok(false);
        };
        if self.stamp_bytes {
            write!(self.bytes, "{}: ", line_time())?;
        }
        self.bytes.write_all(&redacted)?;
        Ok(true)
    }

    /// One outbound command, redacted and framed, ready to write.
    fn client_record(&self, bytes: &[u8]) -> Vec<u8> {
        let redacted = self.redactions.apply_bytes(bytes);
        let mut record = Vec::with_capacity(redacted.len() + 64);
        if self.stamp_bytes {
            record.extend_from_slice(format!("{}: ", line_time()).as_bytes());
        }
        // Client input is WRAPPED, not prefixed -- the same markers
        // `logxml.lic` uses ("Messages from the client will be wrapped in
        // <!-- CLIENT -->...<!-- ENDCLIENT --> tags"). That script produced
        // the 49.55 GB corpus every fixture in this repo came from, so
        // matching it means a Cena capture can be read by anything that
        // already reads those, and vice versa. Inventing a private framing
        // here would have made our own logs the odd ones out.
        record.extend_from_slice(CLIENT_OPEN);
        record.extend_from_slice(redacted.trim_ascii_end());
        record.extend_from_slice(CLIENT_CLOSE);
        // **The wrapper gets its own terminator, unconditionally**, because
        // the command's own newline was just trimmed off. Without it the
        // next inbound line is glued to `ENDCLIENT -->` and a reader
        // splitting on lines sees one line where there were two.
        //
        // The old check tested `redacted` -- the UNTRIMMED buffer -- while
        // writing the trimmed one, so a command ending in a newline took the
        // "already terminated" branch and got nothing.
        record.push(b'\n');
        record
    }

    /// Redact `bytes`, carrying a tail forward so a secret cannot hide in a
    /// chunk boundary.
    ///
    /// # The bug this exists for
    ///
    /// `apply_bytes` sees one `read` at a time, and a TCP read boundary falls
    /// wherever the network puts it. A launch key split across two reads
    /// matched neither half and was **written in the clear** -- the live
    /// credential, in a file on disk. Review finding PL-5, which noted "No test
    /// covers it."
    ///
    /// # How the tail works, and why it is bounded
    ///
    /// Holding back `longest_secret - 1` bytes guarantees that any secret is
    /// wholly inside some chunk: a secret of length `n` cannot span more than
    /// `n - 1` bytes of held-back tail plus the new chunk. The tail is bounded
    /// by the longest registered secret, so it cannot grow with the session.
    ///
    /// # What this costs, stated plainly
    ///
    /// **Chunk boundaries shift** by up to `longest_secret - 1` bytes while
    /// secrets are registered, and this function's own doc says boundaries are
    /// preserved so a replay drives the parser's partial-line path with a real
    /// split. That guarantee is weakened here, deliberately:
    ///
    /// - the boundary is still a boundary the wire *could* have produced -- no
    ///   byte is invented, reordered or dropped, only deferred;
    /// - [`Recorder`](crate::record::Recorder) keeps its own in-memory copy and
    ///   is unaffected, so live replay fidelity is untouched;
    /// - a credential in a log is a worse outcome than a shifted split in a
    ///   fixture.
    ///
    /// Inbound only. Outbound commands are whole, so there is nothing to
    /// straddle; they are placed at line ends instead (see [`Self::wire`]).
    fn redact_across_chunks(&mut self, bytes: &[u8]) -> Option<Vec<u8>> {
        let hold = self.redactions.longest_secret().saturating_sub(1);
        if hold == 0 {
            // Nothing registered: redact in place.
            return Some(self.redactions.apply_bytes(bytes));
        }

        self.pending.extend_from_slice(bytes);
        // Keep back the last `hold` bytes: a secret could still be completed by
        // the next chunk.
        let mut safe = self.pending.len().saturating_sub(hold);
        if safe == 0 {
            return None;
        }

        // **And do not cut through a secret that is ALREADY whole.** Holding
        // back `hold` bytes stops an INCOMING boundary from splitting a secret,
        // but the cut made here is a second boundary, and a naive one lands
        // mid-secret just as easily.
        //
        // Found by the straddle test still failing after the hold was added:
        // the key was whole in `pending` and got sliced at byte 17 of 48, so
        // neither piece matched. Walk the cut backwards past any secret that
        // spans it.
        safe = self.cut_clear_of_secrets(safe);
        if safe == 0 {
            return None;
        }
        let head: Vec<u8> = self.pending.drain(..safe).collect();
        Some(self.redactions.apply_bytes(&head))
    }

    /// Move a proposed cut back until no registered secret spans it.
    ///
    /// A secret occupying `[start, start + len)` spans the cut when
    /// `start < cut < start + len`. Moving the cut to `start` puts the whole
    /// secret in the held-back tail, where the next chunk -- or
    /// `drain_pending` -- redacts it intact.
    ///
    /// Bounded: only secrets beginning within `longest_secret` bytes before the
    /// cut can span it, so the scan is over a fixed-size window and the cut
    /// moves back at most `longest_secret - 1` bytes.
    fn cut_clear_of_secrets(&self, cut: usize) -> usize {
        let window = self.redactions.longest_secret();
        let from = cut.saturating_sub(window);
        let mut earliest = cut;
        for (secret, _) in self.redactions.secrets() {
            let needle = secret.as_bytes();
            for start in from..cut {
                if start + needle.len() > cut
                    && self.pending.len() >= start + needle.len()
                    && self.pending[start..].starts_with(needle)
                {
                    earliest = earliest.min(start);
                }
            }
        }
        earliest
    }

    /// Write out whatever the straddle tail is still holding.
    ///
    /// Called before a flush, a roll and a close: a tail left in memory is
    /// bytes the wire really carried, and losing them would make the `.bytes`
    /// file an incomplete record -- the opposite of its purpose. It is redacted
    /// on the way out like any other chunk.
    ///
    /// # Errors
    ///
    /// Any write failure.
    ///
    /// Commands still waiting for a line end go out after it: a session that
    /// ends mid-line must not lose the commands it sent.
    fn drain_pending(&mut self) -> io::Result<()> {
        if !self.pending.is_empty() {
            let tail: Vec<u8> = std::mem::take(&mut self.pending);
            let redacted = self.redactions.apply_bytes(&tail);
            self.bytes.write_all(&redacted)?;
        }
        for record in std::mem::take(&mut self.awaiting_line_end) {
            self.bytes.write_all(&record)?;
        }
        Ok(())
    }

    /// Close the current `.bytes` part and open the next.
    ///
    /// Only the bytes file rolls. The debug log takes a handful of lines per
    /// session (`lifecycle Ready`, `lifecycle Closed`, errors), so rolling it
    /// would produce empty parts and a second thing to reassemble for no gain.
    ///
    /// # Each part is INDEPENDENTLY VALID, and that is the point
    ///
    /// A part boundary falls between two whole chunks, never inside one, so
    /// every part starts on a chunk boundary the wire really had. That is what
    /// keeps a rolled session usable as replay input: concatenating the parts
    /// in order reproduces the original stream exactly, and a single part
    /// parses on its own (`Parser::push_bytes` buffers to a newline, so a part
    /// that begins mid-tag is the one thing this must not produce).
    ///
    /// # Errors
    ///
    /// Any failure to flush the old part or create the new one. The caller
    /// swallows it -- a session must survive a failed log (`plan/12` §5.5) --
    /// but it is returned rather than hidden so the caller *can* report it.
    fn roll(&mut self) -> io::Result<()> {
        // **The pending suffix is NOT drained here**, and that reverses what
        // this line used to do.
        //
        // It read `self.drain_pending()?`, on the reasoning that those bytes
        // arrived while this part was open and belong in it. True, and it
        // defeats the buffer's whole purpose: `pending` holds back a suffix
        // precisely because it might be the FIRST HALF of a registered
        // secret. Found by review -- with `SECRET` registered and a rotation
        // between a chunk ending `SEC` and one starting `RET`, both fragments
        // were written unredacted, one per part.
        //
        // Carrying it across costs the ordering guarantee its comment claimed:
        // at most `hold` bytes move from the end of one part to the start of
        // the next. That is the right trade. A part boundary that falls a few
        // bytes early is a cosmetic imprecision in a log; a secret written in
        // clear is not recoverable, and the whole point of this file's
        // redaction is that it never happens.
        //
        // Reassembly is unaffected: concatenating the parts in order still
        // reproduces the stream exactly, because the bytes are not lost, only
        // deferred. The independently-valid-part claim above is the one that
        // weakens, and only by `hold` bytes at the seam.
        self.bytes.flush()?;
        // `part` is committed only AFTER the file exists. Incrementing first
        // and then using `?` on `File::create` spends the number on a part
        // that was never opened: the next roll takes the one after it, and the
        // sequence has a permanent hole. A reader of a capture directory
        // cannot then tell a transient ENOSPC from a part that was written and
        // later deleted -- and for a capture format whose whole claim is that
        // it is the wire verbatim, "a file is missing here" must mean exactly
        // one thing. Found by review (PL-8).
        let next_part = self.part + 1;
        let next = self.stem.with_file_name(format!(
            "{}-{:03}.bytes",
            self.stem.file_name().unwrap_or_default().to_string_lossy(),
            next_part
        ));
        self.bytes = BufWriter::new(File::create(&next)?);
        self.part = next_part;
        self.bytes_path = next;
        self.lines_written = 0;
        Ok(())
    }

    /// Append one debug line: a lifecycle change, an error -- something that
    /// is **not** on the wire.
    ///
    /// # Do not log wire content here
    ///
    /// A command, a frame or a line of game text is already in the `.bytes`
    /// file, byte-for-byte as the server saw it. Writing it again here makes
    /// two records of one fact that can disagree, and doubles the disk to do
    /// it. Author's call: *"we don't need duplicate lines timestamped, the
    /// .bytes is the whole point."*
    ///
    /// The readable timestamped view of a session is the **user log**, a
    /// different artifact above the parser. This is not a down-payment on it.
    ///
    /// # Errors
    ///
    /// Any write failure.
    pub fn event(&mut self, line: &str) -> io::Result<()> {
        // Always stamped: nothing replays this file, so a wall clock costs it
        // nothing, and "when did that happen" is the first question anyone
        // asks of it.
        writeln!(
            self.events,
            "{} {}",
            line_time(),
            self.redactions.apply(line)
        )
    }

    /// Register another launch key for redaction.
    ///
    /// # Why a sink outlives the secret it was opened with
    ///
    /// **One log spans every generation**, and a reconnect is a full re-login
    /// (`plan/10` §4.6: the SGE connection is *"strictly single-use per
    /// auth"*), so **every connection has a different launch key**. A sink
    /// whose redaction set was fixed at creation would write generation 2's
    /// key verbatim into a file opened during generation 1 -- the one secret
    /// the `.bytes` file would otherwise carry in the clear, unredacted,
    /// because it did not exist yet when the set was built.
    ///
    /// That is why this is narrow rather than a general `redactions_mut`: the
    /// launch key is the only secret that is genuinely per-connection. The
    /// account is typed once and registered at creation (the character is
    /// not registered at all: it is display text on every line).
    ///
    /// Keys accumulate. An earlier generation's key stays redacted, because
    /// the log still contains the bytes it appeared in.
    pub fn redact_key(&mut self, key: &str) {
        self.redactions.key(key);
        // **Recorded in the file, not just applied.** The header is written
        // once at creation and cannot describe a set that grows afterwards, so
        // a reader needs an in-band marker to tell which part of a log a
        // redaction covers. Everything above this line predates it.
        //
        // The KEY IS NOT LOGGED, obviously -- only the fact that one was
        // registered. It is applied to this very line, so a bug that wrote the
        // secret here would redact it on the way out.
        let _ = self.event("redaction registered (session key)");
    }

    /// Flush both files.
    ///
    /// Called at shutdown, and worth calling on a lifecycle change: a session
    /// that panics should still leave the log that explains why.
    ///
    /// # Errors
    ///
    /// Any flush failure.
    pub fn flush(&mut self) -> io::Result<()> {
        // A flush that left the straddle tail in memory would be a lie: the
        // caller flushes to make the file complete, and `plan/12` §5.5 wants a
        // session that panics to still leave the log explaining why.
        self.drain_pending()?;
        self.bytes.flush()?;
        self.events.flush()
    }

    /// Where the raw wire went.
    #[must_use]
    pub fn bytes_path(&self) -> &Path {
        &self.bytes_path
    }

    /// Where the structured log went.
    #[must_use]
    pub fn events_path(&self) -> &Path {
        &self.events_path
    }
}
