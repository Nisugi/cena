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
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

/// The exact strings to remove before anything is written.
///
/// **A closed set, known in advance.** That is what makes this exact rather
/// than a guess: the account and character are typed at a prompt, and the key
/// arrives in the `L` response. Nothing here has to decide whether a word
/// *looks like* a credential.
///
/// The live run of 2026-09-18 printed
/// `A\t<ACCOUNT>\tKEY\t<KEY-REDACTED>\t<NAME>` -- the key was already
/// redacted for the terminal; the account name and the author's real name were
/// not, and would have gone to disk verbatim.
#[derive(Default, Clone)]
pub struct Redactions {
    secrets: Vec<(String, &'static str)>,
}

/// Hand-written, because the derive **printed every secret**.
///
/// `Redactions` exists to keep launch keys out of files, and deriving `Debug`
/// meant any `{:?}` -- on it, on a `SessionSink`, or on a `SessionEnd` that
/// contains one -- dumped the whole `Vec<(String, _)>` in the clear. Found by
/// review. The same mistake `Credentials` and `LaunchPayload` were already
/// written by hand to avoid.
///
/// The COUNT is shown, which is what a reader legitimately wants ("was anything
/// registered?") and reveals nothing.
impl std::fmt::Debug for Redactions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redactions")
            .field(
                "secrets",
                &format_args!("{} registered", self.secrets.len()),
            )
            .finish()
    }
}

impl Redactions {
    /// Nothing redacted yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Redact one exact string wherever it appears.
    ///
    /// Ignores empties and very short strings: a one- or two-character secret
    /// would match everywhere and turn the log into noise, which is a worse
    /// outcome than not redacting a string that short. Three is the floor.
    pub fn add(&mut self, secret: &str, replacement: &'static str) {
        let secret = secret.trim();
        if secret.len() >= 3 {
            self.secrets.push((secret.to_owned(), replacement));
        }
    }

    /// The account name, its `A`-response echo, and the real name beside it.
    pub fn account(&mut self, account: &str) {
        self.add(account, "<ACCOUNT>");
        // The server echoes the account uppercased in `A\t<ACCOUNT>\tKEY...`,
        // which a case-sensitive match would miss.
        self.add(&account.to_uppercase(), "<ACCOUNT>");
    }

    /// The session key from `L`. One-shot and short-lived, but it is a
    /// credential for as long as it is valid.
    pub fn key(&mut self, key: &str) {
        self.add(key, "<KEY>");
    }

    /// The account holder's real name, as the `A` response carries it.
    pub fn real_name(&mut self, name: &str) {
        self.add(name, "<NAME>");
        self.add(&name.to_uppercase(), "<NAME>");
    }

    /// Apply every redaction to a string.
    #[must_use]
    pub fn apply(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (secret, replacement) in &self.secrets {
            if out.contains(secret.as_str()) {
                out = out.replace(secret.as_str(), replacement);
            }
        }
        out
    }

    /// Apply every redaction to raw wire bytes, **byte for byte**.
    ///
    /// # Why this does not go through `str`
    ///
    /// It used to: on a match the whole chunk round-tripped through
    /// `String::from_utf8_lossy`, which replaces every invalid byte with
    /// U+FFFD. So redacting a secret silently corrupted **unrelated bytes in
    /// the same chunk** -- and the `.bytes` file is meant to be the wire, so a
    /// fixture cut from such a chunk would differ from what the server sent,
    /// with nothing to indicate it.
    ///
    /// The no-match path was always byte-exact, and
    /// `bytes_without_a_secret_are_returned_unchanged` only covered that path,
    /// so the corruption had no test. Review finding PL-5.
    ///
    /// Scanning bytes also removes the question of whether a secret is valid
    /// UTF-8: the old comment reasoned that every secret is ASCII "so that case
    /// does not arise", which was true but load-bearing. Now it is irrelevant.
    #[must_use]
    pub fn apply_bytes(&self, bytes: &[u8]) -> Vec<u8> {
        if self.secrets.is_empty() {
            return bytes.to_vec();
        }
        let mut out = Vec::with_capacity(bytes.len());
        let mut at = 0;
        'outer: while at < bytes.len() {
            for (secret, replacement) in &self.secrets {
                let needle = secret.as_bytes();
                if bytes[at..].starts_with(needle) {
                    out.extend_from_slice(replacement.as_bytes());
                    at += needle.len();
                    continue 'outer;
                }
            }
            out.push(bytes[at]);
            at += 1;
        }
        out
    }

    /// The registered secrets, for the sink's boundary arithmetic.
    ///
    /// Crate-visible, not public: the values are credentials, and the only
    /// legitimate caller is the sink deciding where a chunk may be cut.
    pub(crate) fn secrets(&self) -> &[(String, &'static str)] {
        &self.secrets
    }

    /// The length of the longest registered secret, in bytes.
    ///
    /// The sink uses this to size the tail it carries between chunks: a secret
    /// can straddle a read boundary, and holding back `longest - 1` bytes
    /// guarantees any secret is whole in some chunk. Zero when nothing is
    /// registered.
    #[must_use]
    pub fn longest_secret(&self) -> usize {
        self.secrets
            .iter()
            .map(|(secret, _)| secret.len())
            .max()
            .unwrap_or(0)
    }

    /// Whether anything is being redacted. For a startup line that says so,
    /// because a log that *claims* to be scrubbed and is not is worse than one
    /// that admits it is raw.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }
}

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
                "none -- no account or character was registered"
            } else {
                "account and character"
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
        // `None` means every byte is still held back waiting for more: nothing
        // to write yet, and writing a stamp for it would invent a line.
        let Some(redacted) = self.redact_across_chunks(inbound, bytes) else {
            return Ok(());
        };
        if self.stamp_bytes {
            write!(self.bytes, "{}: ", line_time())?;
        }
        if inbound {
            self.bytes.write_all(&redacted)?;
        } else {
            // Client input is WRAPPED, not prefixed -- the same markers
            // `logxml.lic` uses ("Messages from the client will be wrapped in
            // <!-- CLIENT -->...<!-- ENDCLIENT --> tags"). That script produced
            // the 49.55 GB corpus every fixture in this repo came from, so
            // matching it means a Cena capture can be read by anything that
            // already reads those, and vice versa. Inventing a private framing
            // here would have made our own logs the odd ones out.
            self.bytes.write_all(CLIENT_OPEN)?;
            self.bytes.write_all(redacted.trim_ascii_end())?;
            self.bytes.write_all(CLIENT_CLOSE)?;
            // **The wrapper gets its own terminator, unconditionally**, because
            // the command's own newline was just trimmed off. Without it the
            // next inbound line is glued to `ENDCLIENT -->` and a reader
            // splitting on lines sees one line where there were two.
            //
            // The old check tested `redacted` -- the UNTRIMMED buffer -- while
            // writing the trimmed one, so a command ending in a newline took the
            // "already terminated" branch and got nothing.
            self.bytes.write_all(b"\n")?;
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
    /// **Outbound writes are not deferred.** They are whole commands, framed by
    /// the caller rather than by a read boundary, so there is nothing to
    /// straddle -- and deferring one would move it after inbound bytes that
    /// really did arrive later, corrupting the order the file records.
    fn redact_across_chunks(&mut self, inbound: bool, bytes: &[u8]) -> Option<Vec<u8>> {
        let hold = self.redactions.longest_secret().saturating_sub(1);
        if hold == 0 || !inbound {
            // Nothing registered, or an outbound command: redact in place.
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
    fn drain_pending(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let tail: Vec<u8> = std::mem::take(&mut self.pending);
        let redacted = self.redactions.apply_bytes(&tail);
        self.bytes.write_all(&redacted)
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
        // BEFORE the part closes: these bytes arrived while this part was
        // open, so they belong in it. Deferring them to the next part would
        // reorder the record across a part boundary.
        self.drain_pending()?;
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
    /// account and character are typed once and registered at creation.
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
