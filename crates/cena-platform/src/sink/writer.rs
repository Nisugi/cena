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

    /// Apply every redaction to raw wire bytes.
    ///
    /// Works on the UTF-8 lossy view and returns bytes. A credential that is
    /// not valid UTF-8 would not survive the round trip -- but every secret
    /// here is one a human typed or the server sent as ASCII, so that case
    /// does not arise. Stated rather than assumed.
    #[must_use]
    pub fn apply_bytes(&self, bytes: &[u8]) -> Vec<u8> {
        if self.secrets.is_empty() {
            return bytes.to_vec();
        }
        let text = String::from_utf8_lossy(bytes);
        if self.secrets.iter().any(|(s, _)| text.contains(s.as_str())) {
            return self.apply(&text).into_bytes();
        }
        bytes.to_vec()
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
    /// Whether the bytes file carries per-line times. Read once at creation
    /// rather than per line: an env var that changed mid-session would produce
    /// a file that is half one format.
    stamp_bytes: bool,
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
        writeln!(
            events,
            "# cena session log -- PRIVATE DEVELOPMENT LOG, not automatically shareable.\n\
             # Credentials (account, real name, session key) are redacted: {}.\n\
             # OTHER PLAYERS' NAMES ARE NOT REDACTED. Cut fixtures through\n\
             # cena_protocol::scrub with the names named, as CLAUDE.md requires.",
            if redactions.is_empty() {
                "NO -- nothing was registered, treat this file as raw"
            } else {
                "yes"
            }
        )?;

        Ok(Self {
            bytes,
            events,
            redactions,
            bytes_path,
            events_path,
            lines_written: 0,
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
        let redacted = self.redactions.apply_bytes(bytes);
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
        }
        if !redacted.ends_with(b"\n") {
            self.bytes.write_all(b"\n")?;
        }
        self.lines_written += 1;
        if self.lines_written >= self.rotate_after {
            self.roll()?;
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
        self.bytes.flush()?;
        self.part += 1;
        let next = self.stem.with_file_name(format!(
            "{}-{:03}.bytes",
            self.stem.file_name().unwrap_or_default().to_string_lossy(),
            self.part
        ));
        self.bytes = BufWriter::new(File::create(&next)?);
        self.bytes_path = next;
        self.lines_written = 0;
        Ok(())
    }

    /// How many lines have been written, for the caller's rotation check.
    ///
    /// Rotation is the caller's because it needs a clock and a path, and this
    /// type deliberately has neither (see [`Self::create`]).
    #[must_use]
    pub fn lines_written(&self) -> u64 {
        self.lines_written
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
