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

use super::config::{CLIENT_CLOSE, CLIENT_OPEN, bytes_timestamps_enabled, line_time};
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
/// `A\tACCOUNT\tKEY\t<KEY-REDACTED>\tREAL NAME` -- the key was already
/// redacted for the terminal; the account name and the author's real name were
/// not, and would have gone to disk verbatim.
#[derive(Debug, Default, Clone)]
pub struct Redactions {
    secrets: Vec<(String, &'static str)>,
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

        let bytes_path = dir.join(format!("{safe}-{stamp}.bytes"));
        let events_path = dir.join(format!("{safe}-{stamp}.log"));
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

    /// Append one structured line: a command sent, a lifecycle change, an
    /// error.
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
