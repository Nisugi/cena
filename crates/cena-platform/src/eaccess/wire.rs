//! The `EAccess` wire vocabulary: the types that cross it, and the pure
//! functions that read and write its fields.
//!
//! Split from [`super`] under `plan/05` Rule 4.1 -- **move code down, do not
//! raise the cap.** The single file reached 892 lines against a 400 cap, and
//! the architecture test caught it rather than a reviewer. The seam was
//! already there: **everything here is pure**, and everything in [`super`]
//! touches a socket.
//!
//! That split is what makes the sequence testable at all. A live login is a
//! terrible place to discover an off-by-one in the `C` walk or a `split('=')`
//! that truncates a key at its own `=` byte, and `CLAUDE.md` forbids running
//! one to find out. Every function here is checked by a test below; the
//! socket half is checked by the author, once, with their eyes.

use std::fmt;

/// The login service. `plan/10` §1.
pub(super) const EACCESS_HOST: &str = "eaccess.play.net";
/// The login service port. `plan/10` §1.
pub(super) const EACCESS_PORT: u16 = 7910;

/// The client banner sent to the **game** socket, not to eaccess.
///
/// **THIS STRING IS NOT COSMETIC.** `/FE:WRAYTH /VERSION:1.0.1.28` is what
/// makes the server serve the **extended feed** -- `<pulse>`,
/// `<exposeContainer>`, and `<inventoryManager>` in reply to
/// `_inventory manager`. There is no other negotiation: the server keys on
/// this string alone.
///
/// CONFIRMED by the author 2026-09-18, live-verified 2026-08-12, and
/// independently corroborated by
/// `crates/cena-protocol/tests/fixtures/login_setup.xml`, where the server
/// echoes `<settingsInfo client='1.0.1.28' .../>`.
///
/// The Lich-era `/FE:STORMFRONT /VERSION:1.0.1.26` gets the **reduced** feed.
pub const CLIENT_BANNER: &str = "/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML";

/// Read buffer for one handshake response.
pub(super) const READ_BUF: usize = 8192;

/// What the caller must supply. Borrowed, not owned: nothing here needs to
/// outlive the call, and an owned struct invites being stored.
#[derive(Clone, Copy)]
pub struct Credentials<'a> {
    /// The account name, not the character name.
    pub account: &'a str,
    /// The account password, in the clear. Hashed against the server's key
    /// before it reaches the socket, and never logged.
    pub password: &'a str,
    /// The character to launch, matched case-insensitively against the `C`
    /// list.
    pub character: &'a str,
    /// The instance code. **CASE-SENSITIVE on the wire** -- see
    /// [`authenticate`](super::authenticate)'s `M` check.
    pub game_code: &'a str,
}

/// Deliberately not `Debug`-derived: a derived impl prints the password, and
/// the one place a credential struct reliably leaks is a debug log written in
/// a hurry.
impl fmt::Debug for Credentials<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("account", &self.account)
            .field("password", &"<REDACTED>")
            .field("character", &self.character)
            .field("game_code", &self.game_code)
            .finish()
    }
}

/// Where the game is, and the one-shot key that opens it.
#[derive(Clone)]
pub struct LaunchPayload {
    /// The game host. A plain TCP destination -- **not** the eaccess host and
    /// **not** TLS.
    ///
    /// `String`, not `Option<String>`, **deliberately**. `plan/10` §4.9 says
    /// "Cena's type must make `gamehost`/`gameport` `Option<>` if it supports
    /// the generator" -- the character-generator path (`L\t0\tSTORM`) can omit
    /// both, and `eaccess_spec.rb:316` has `"L\tOK\tKEY=abc\n"` as a valid
    /// success. Cena does not support that path: character creation is not in
    /// `plan/12` §7.1's In column and no code here sends `L\t0`. Making these
    /// `Option` today would add a `None` arm that every caller must handle and
    /// that nothing can produce. **If the generator is ever added, these two
    /// fields become `Option` in the same commit** -- that is what §4.9 asks
    /// for, and the condition on it is not met yet.
    pub gamehost: String,
    /// The game port.
    pub gameport: u16,
    /// The instance the server actually launched, e.g. `GS` or `DR`.
    ///
    /// **This is the server's answer, not the code that was requested.** `G`
    /// selects with a code from `M` (`GS3`, `GST`, `GSX`, ...) and `L` answers
    /// with a shorter family code, so a `GS3` login returns `GAMECODE=GS`.
    /// `plan/10` §4.9 lists this among the four fields "Cena needs" -- the
    /// other four (`UPPORT`, `GAME`, `FULLGAMENAME`, `GAMEFILE`) "exist solely
    /// to tell a Simutronics launcher which `.EXE` to run" and are dropped.
    ///
    /// Absent on the generator path, so `Option` -- unlike the two above, this
    /// one costs nothing, because nothing dereferences it.
    pub gamecode: Option<String>,
    /// The session key. One shot, short-lived, and a credential: see this
    /// type's `Debug`.
    pub key: String,
}

/// Redacts the key. The payload is the natural thing to log on a successful
/// login -- "connected to X:Y" -- and the key sits beside the host.
impl fmt::Debug for LaunchPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchPayload")
            .field("gamehost", &self.gamehost)
            .field("gameport", &self.gameport)
            .field("gamecode", &self.gamecode)
            .field("key", &"<REDACTED>")
            .finish()
    }
}

/// A login failure, naming the stage it happened at.
///
/// The stage is the whole point. `plan/10` §11.2 requires a wrong password to
/// "fail cleanly in under 2s, **naming the stage**" -- because every failure
/// in this sequence that does *not* name its stage points at the credential,
/// and on 2026-09-18 three of them were something else entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EaccessError {
    /// Which step failed: `tls_handshake`, `a_response`, `l_response`, ...
    pub stage: &'static str,
    /// What went wrong. Never contains a password or a key.
    pub detail: String,
    /// Whether retrying is pointless: the account or the request was REFUSED,
    /// as against the link failing on the way to asking.
    ///
    /// # Why this is a field and not a function of `stage`
    ///
    /// Because **one stage is both**. `a_response` covers the server saying
    /// "authentication rejected" *and* `read_response` reporting "connection
    /// closed by peer (0 bytes)" or a stage timeout -- a credential rejection
    /// and two transport failures, under one name.
    ///
    /// `VellumFE` records both directions of getting this wrong, and they are
    /// the same mistake made twice:
    ///
    /// > *"The headless reconnect supervisor stops retrying when it finds this
    /// > in an error chain -- hammering the auth server with a wrong password
    /// > would be pointless and **could lock the account**."*
    ///
    /// > *"EOF: ... a transient DROP, not a credential rejection -- it must NOT
    /// > surface as `AuthFailed`, or the ... supervisor treats it as 'bad
    /// > credentials, stop retrying' and **strands the session**."*
    ///
    /// So classifying on `stage` alone would strand a session on every drop
    /// that happened to land mid-handshake. The layer that knows which it was
    /// is the one that built the error, which is why it says so here rather
    /// than leaving a caller to match on the message.
    ///
    /// **Defaults to `false`** (`err` sets it), so a failure nobody has
    /// classified is retried. That is the direction that fails safe: an
    /// unclassified error retried costs a bounded ladder, while an
    /// unclassified error treated as fatal costs the session.
    pub fatal: bool,
}

impl EaccessError {
    /// Mark this failure as one no retry can fix.
    ///
    /// Used at the two places that are genuinely a refusal rather than a
    /// transport failure: a rejected `A` response, and an `L PROBLEM` launch
    /// refusal.
    #[must_use]
    pub fn fatal(mut self) -> Self {
        self.fatal = true;
        self
    }
}

impl fmt::Display for EaccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.stage, self.detail)
    }
}

impl std::error::Error for EaccessError {}

pub(super) fn err<E: fmt::Display>(stage: &'static str, e: E) -> EaccessError {
    EaccessError {
        stage,
        detail: e.to_string(),
        // Transient unless a caller says otherwise -- see the `fatal` field.
        fatal: false,
    }
}

/// The password hash: `((pw[i] - 32) ^ key[i]) + 32`, in `i32`.
///
/// # Why this refuses instead of wrapping
///
/// `plan/10` §10.3: Rust `u8` wraps in release and panics in debug, and
/// **neither matches Ruby**, which raises on both overflow and underflow --
/// 14,336 of the 65,536 byte pairs. Lich has therefore **never successfully
/// sent a byte outside `0..=255`**, so the server's behaviour there is
/// completely unobserved. Masking with `& 0xFF` would emit bytes no server has
/// been seen to accept: an untested protocol change disguised as a port.
///
/// It also closes the short-key case Ruby leaves open. Ruby indexes `key[i]`
/// over the *password's* length and raises on `nil`; Rust's `zip` would
/// silently stop at the shorter of the two, producing a **wrong password**
/// rather than an error.
///
/// # Errors
///
/// [`EaccessError`] with stage `hash` if the key is shorter than the password,
/// or if any byte falls outside `0..=255`.
pub fn hash_password(password: &[u8], key: &[u8]) -> Result<Vec<u8>, EaccessError> {
    if key.len() < password.len() {
        // FATAL: the password is longer than the protocol's key can hash. That
        // is a property of the password, not of this attempt.
        return Err(err(
            "hash",
            format!(
                "key ({} bytes) shorter than password ({} bytes) -- Ruby raises \
                 here; refusing to truncate, which would send a wrong password \
                 rather than fail",
                key.len(),
                password.len()
            ),
        )
        .fatal());
    }

    let mut out = Vec::with_capacity(password.len());
    for (i, (&p, &k)) in password.iter().zip(key.iter()).enumerate() {
        let result = ((i32::from(p) - 32) ^ i32::from(k)) + 32;
        if !(0..=255).contains(&result) {
            // The password byte `p` is deliberately NOT in this message.
            //
            // It was, ported verbatim from the spike (`:88`), which printed
            // `0x{p:02x}` -- one plaintext password byte, in hex, with its
            // index. That error propagates out of `authenticate` to `main`,
            // which returns `Box<dyn Error>`, so the runtime prints it: the
            // byte lands on stderr, in scrollback, and in any `2>` redirect.
            // Fine in a throwaway spike; not in a shipped library whose own
            // test asserts the password does not leak.
            //
            // **Only the INDEX survives.** An earlier version printed the key
            // byte and the result and claimed the password byte was withheld --
            // and the comment right here admitted the arithmetic was
            // "recoverable from the other three", which is exactly right:
            // `p = ((result - 32) ^ k) + 32`. Two reviews found it: one added the
            // comment, one noticed the comment contradicted the message.
            //
            // The index is what a diagnosis actually needs ("the ninth character
            // of your password") and reveals nothing about the byte.
            let _ = (p, k, result);
            return Err(err(
                "hash",
                format!(
                    "password byte {i} hashes out of range, outside 0..=255. \
                     Ruby raises here and Lich has never sent such a byte, so \
                     the server's behavior is UNOBSERVED (plan/10 §12.1 S3). \
                     Refusing to guess. Neither the byte nor the arithmetic is \
                     shown: the key byte and the result together recover it."
                ),
            )
            .fatal());
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the range check immediately above proves 0..=255"
        )]
        out.push(result as u8);
    }
    Ok(out)
}

/// Replace the account name inside a character code with `<ACCOUNT>`.
///
/// A character code has the shape `W_<ACCOUNT>_<SLOT>` (`plan/10:472`, VERIFIED
/// in the capture: *"the same character, and the same code `W_<ACCOUNT>_000`"*),
/// so printing one prints the account -- and the `resolve_char` progress line
/// did exactly that, on every login, to stderr and into the scrollback.
///
/// That is the same exposure [`redact`] was written for after the first live run
/// printed the account and the account holder's real name in full. It closed the
/// `A` response and left this one open, because here the account is INSIDE a
/// field rather than a field of its own.
///
/// Positional, not a guess: keep the leading `W` and the trailing slot, blank
/// what is between. The slot is taken from the END rather than from index 2, so
/// an account name containing `_` still redacts whole. A code that does not have
/// this shape is returned **unchanged** rather than blanked: an unrecognised
/// code is a diagnostic, and destroying it would remove the thing a reader needs.
#[must_use]
pub fn redact_char_code(code: &str) -> String {
    let parts: Vec<&str> = code.split('_').collect();
    if parts.len() < 3 || parts[0] != "W" {
        return code.to_owned();
    }
    let slot = parts[parts.len() - 1];
    format!("W_<ACCOUNT>_{slot}")
}

/// Redact anything that looks like a credential before printing.
///
/// Two shapes: a bare 32-hex-digit field (a session key on its own), and any
/// `KEY=` field. Everything else passes through, because the *point* of
/// printing these lines is diagnosis -- `plan/10` §4.7 records that parsing a
/// response before seeing it fail turned the server's bare `?` into an
/// innocuous-looking `tier="?"`.
#[must_use]
pub fn redact(s: &str) -> String {
    // The `A` success response has a FIXED shape:
    //
    //     A <tab> <ACCOUNT> <tab> KEY <tab> <session key> <tab> <Real Name>
    //
    // so the account name and the account holder's real name are POSITIONAL,
    // and removing them is exact rather than a guess.
    //
    // ADDED 2026-09-18, after the first live run printed both in full. Only
    // the key was redacted; the account and the author's real name went to the
    // terminal, the scrollback, and into a transcript pasted for review. They
    // are personal data and nothing downstream needs them.
    let fields: Vec<&str> = s.split('\t').collect();
    let is_a_response = fields.first() == Some(&"A") && fields.get(2) == Some(&"KEY");

    fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            if is_a_response && i == 1 {
                "<ACCOUNT>".to_owned()
            } else if is_a_response && i == 4 {
                "<NAME>".to_owned()
            } else if field.len() == 32 && field.chars().all(|c| c.is_ascii_hexdigit()) {
                "<KEY-REDACTED>".to_owned()
            } else if field.starts_with("KEY=") {
                "KEY=<REDACTED>".to_owned()
            } else {
                (*field).to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\t")
}

/// Find a character's launch code in a `C` response.
///
/// Format: `C \t n \t n \t n \t n \t <code> \t <Name> [\t <code> \t <Name>]...`
/// -- four counts, then code/name pairs from field 5.
///
/// Split out of [`authenticate`](super::authenticate) so it can be tested
/// without a socket: this is
/// the one piece of parsing in the sequence with an off-by-one to get wrong,
/// and a live login is a poor place to discover it.
#[must_use]
pub fn resolve_char_code<'a>(c_response: &'a str, character: &str) -> Option<&'a str> {
    let fields: Vec<&str> = c_response.trim().split('\t').collect();
    let mut i = 5;
    while i + 1 < fields.len() {
        if fields[i + 1].eq_ignore_ascii_case(character) {
            return Some(fields[i]);
        }
        i += 2;
    }
    None
}

/// The instance codes an `M` response offers.
///
/// Format: `M \t <code> \t <name> [\t <code> \t <name>]...`
#[must_use]
pub fn offered_game_codes(m_response: &str) -> Vec<&str> {
    m_response.trim().split('\t').skip(1).step_by(2).collect()
}

/// Whether an `L` response reports success.
///
/// # The guard MUST be `L<TAB>OK`, not `^L<TAB>`
///
/// A refusal is `L<TAB>PROBLEM<TAB><n>`, which **also** starts with `L<TAB>`. Lich's
/// analysis records that a loose guard accepts it and then parses a garbage
/// launch payload (`plan/10` §4.7 item 2) -- so the client connects somewhere
/// meaningless with a key that was never issued.
///
/// # Why this is a function rather than an inline `starts_with`
///
/// Because it was inline, in `handshake::launch_character`, which needs a
/// socket -- so no test could reach it. The test that existed compared two
/// string LITERALS to each other and asserted the test file, not the parser
/// (review finding PL-6). Pulling the guard out is what makes it testable at
/// all, which is the same `wire`/`handshake` seam this module already draws.
#[must_use]
pub fn is_launch_ok(l_response: &str) -> bool {
    l_response.starts_with("L	OK")
}

/// Parse the `GAMEHOST` / `GAMEPORT` / `KEY` triple out of an `L\tOK` line.
///
/// `plan/10` §12.3: `splitn(2, '=')`, because a `KEY` value could itself
/// contain `=`.
///
/// # Errors
///
/// [`EaccessError`] with stage `l_response` if any of the three is absent.
pub fn parse_launch(l_response: &str) -> Result<LaunchPayload, EaccessError> {
    let mut gamehost = None;
    let mut gameport = None;
    let mut gamecode = None;
    let mut key = None;
    for field in l_response.trim().split('\t') {
        let mut kv = field.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("GAMEHOST"), Some(v)) => gamehost = Some(v.to_owned()),
            (Some("GAMEPORT"), Some(v)) => gameport = v.parse::<u16>().ok(),
            // `plan/10` §4.9's fourth needed field. The other four in the
            // response -- UPPORT, GAME, FULLGAMENAME, GAMEFILE -- tell a
            // Simutronics launcher which .EXE to run, and are dropped.
            (Some("GAMECODE"), Some(v)) => gamecode = Some(v.to_owned()),
            (Some("KEY"), Some(v)) => key = Some(v.to_owned()),
            _ => {}
        }
    }
    Ok(LaunchPayload {
        gamehost: gamehost.ok_or_else(|| err("l_response", "no GAMEHOST in launch payload"))?,
        gameport: gameport.ok_or_else(|| err("l_response", "no GAMEPORT in launch payload"))?,
        gamecode,
        key: key.ok_or_else(|| err("l_response", "no KEY in launch payload"))?,
    })
}

/// Check a response answers the command that was sent.
///
/// Every response in this sequence is `<letter>\t...`, echoing its command, so
/// a mismatch means the read stream has **slipped out of step with the write
/// stream** -- and every field read after that point is meaningless.
///
/// This is the check whose absence made a lowercase game code look first like
/// an entitlement problem, then a pricing problem, then a session-state
/// problem (`plan/10` §4.7).
///
/// # Errors
///
/// [`EaccessError`] at `stage` if the response does not begin `<letter>\t`.
pub fn expect_echo(response: &str, letter: char, stage: &'static str) -> Result<(), EaccessError> {
    let want = format!("{letter}\t");
    if response.starts_with(&want) {
        return Ok(());
    }
    Err(err(
        stage,
        format!(
            "expected a {letter} response, got {:?} -- the read and write \
             streams are out of step, and nothing parsed after this point is \
             meaningful",
            response.trim()
        ),
    ))
}

// `trim_ascii_whitespace` WAS HERE, and its deletion is the point.
//
// It wrapped `[u8]::trim_ascii` and returned it unchanged, while its own doc
// claimed the behaviour was "spelled out here" -- so its test asserted the
// standard library, not Cena (review finding PL-6).
//
// Worse, its last caller went away with PL-3: the handshake used to trim the
// K key, and `plan/10:1734,1749` establishes that the key is 32 bytes of random
// BINARY with no terminator, so a leading 0x20 is data and trimming it shifts
// every XOR index. The test that guarded this function opened with "The K key
// is trimmed before hashing" -- documenting, and protecting, the exact bug that
// was removed.
//
// A dead function whose test asserts a behaviour we deliberately eliminated is
// worse than no function: it reads as a specification. `handshake.rs:177`
// records why nothing is trimmed.
