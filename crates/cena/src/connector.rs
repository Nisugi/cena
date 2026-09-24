//! [`LiveConnector`]: the real login, behind the [`Connector`] trait.
//!
//! # Why this lives in the binary
//!
//! `cena-session` defines [`Connector`] and must not learn a login protocol;
//! `cena-platform` is *below* `cena-session`, so it cannot implement a trait
//! defined there. The binary is the only layer that can hold both ends --
//! verbatim the argument `crates/cena-arch-tests/tests/layering.rs` already
//! makes for the `cena -> cena-platform` edge, and the one
//! `supervisor/connect.rs`'s header predicted this file would need.
//!
//! # BUILT, NOT RUN by any test
//!
//! Every function below reaches `eaccess.play.net`. `CLAUDE.md`: *"do not log
//! into a live game service without the author present."* Nothing in this
//! workspace tests it; it is exercised by the author running the binary and
//! watching, exactly as criterion 1 was.
//!
//! What *is* testable, and is tested, is the classification it produces:
//! `cena-platform`'s `EaccessError::fatal` decides which failures stop the
//! ladder, and that decision is made where the protocol is understood rather
//! than by matching strings here.

use cena_platform::{Credentials, LiveSource};
use cena_session::{ConnectError, Connector, Generation, Retryability};

/// Logs in from scratch, every time.
///
/// # Why the credentials are retained
///
/// **The author's decision 1.** Every reconnect is a full `K/A/M/F/G/P/C/L`:
///
/// > **AUTHOR:** *"I mean I don't understand the question. When would it get a
/// > new socket that doesn't require a re-login?"*
///
/// It never would. `plan/10` §4.6: the SGE connection is *"strictly single-use
/// per auth"*, and the launch key it yields is one-shot -- so a supervisor that
/// dropped the password after the first handshake could never open a second
/// connection.
///
/// **The accepted cost is that the password stays in process memory for the
/// session's lifetime.** `main.rs` used to drop it immediately after the
/// handshake and say so; that comment is now wrong, and this is why. Note what
/// the old comment already admitted: dropping a `String` does not zero it, so
/// the bytes lingered in freed heap anyway. Retaining them makes the lifetime
/// *honest* rather than making it worse -- but it is longer, and zeroing still
/// needs the `zeroize` step `plan/12` §7.1 puts Out of scope.
pub struct LiveConnector {
    account: String,
    password: String,
    character: String,
    game_code: String,
    /// Which login provider this run uses.
    ///
    /// Carried on the connector rather than read at the call site because a
    /// RECONNECT must make the same choice as the first connect: a run started
    /// with `--web-login` that silently reverted to eaccess on reconnect would
    /// be testing something other than what was asked for.
    prefer: cena_platform::Prefer,
    /// The eaccess certificate pin, `<data dir>/simu.pem`.
    ///
    /// Recorded on the first login and checked on every one after, reconnects
    /// included -- a reconnect is a full re-login, and the one a MITM would
    /// most like to intercept. A mismatch is FATAL, so the supervisor stops
    /// rather than resubmitting the password (`cena-platform`'s `pin.rs`).
    pin: std::path::PathBuf,
    /// How many times `connect` has been called. For the log line only: the
    /// supervisor owns the real retry count.
    attempts: u32,
    /// Launch keys minted by this connector, awaiting registration with the
    /// session log.
    ///
    /// **Every generation gets a different one** (`plan/10` §4.6), and the log
    /// spans generations -- so a key issued now must be redacted in a file
    /// opened before it existed. Drained by the supervisor immediately after
    /// `connect` returns, before anything is written.
    secrets: Vec<String>,
}

impl LiveConnector {
    /// Retain what a re-login needs.
    pub fn new(
        typed: crate::ask::Typed,
        prefer: cena_platform::Prefer,
        pin: std::path::PathBuf,
    ) -> Self {
        Self {
            account: typed.account,
            password: typed.password,
            character: typed.character,
            game_code: typed.game_code,
            prefer,
            pin,
            attempts: 0,
            secrets: Vec::new(),
        }
    }

    /// The game instance code (`GS3`, `GSX`), for a data filename: two
    /// characters of one name on different instances are different
    /// characters. Not a credential.
    pub fn game_code(&self) -> &str {
        &self.game_code
    }

    /// The character this logs in, for a log filename. Not a credential.
    pub fn character(&self) -> &str {
        &self.character
    }

    /// The account name, **to register for redaction**.
    ///
    /// It IS a credential-adjacent identifier -- `plan/10` §4.6 shows it
    /// echoed in the `A` response and embedded in every character code as
    /// `W_<ACCOUNT>_<SLOT>` -- so the only legitimate caller is the one
    /// registering it with [`cena_platform::Redactions`]. Everything else
    /// should use [`Self::character`].
    pub fn account_for_redaction(&self) -> &str {
        &self.account
    }
}

/// Deliberately hand-written: the derived one would print the password.
///
/// `Credentials` redacts itself for this reason, and a struct that holds the
/// same field must not undo that by deriving `Debug`.
///
/// **CITATION CORRECTED 2026-09-19.** This cited `plan/12` §6.4, which does not
/// exist -- §6 stops at 6.3. The rule is `CLAUDE.md`'s ("Never commit
/// credentials") and `plan/10` §11.4's credential redaction; the same mistake
/// `Redactions`' own `Debug` was hand-written to avoid.
impl std::fmt::Debug for LiveConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveConnector")
            .field("account", &"<redacted>")
            .field("password", &"<redacted>")
            .field("character", &self.character)
            .field("game_code", &self.game_code)
            .field("prefer", &self.prefer)
            .field("pin", &self.pin)
            .field("attempts", &self.attempts)
            .field("secrets", &format_args!("{} pending", self.secrets.len()))
            .finish()
    }
}

impl Connector for LiveConnector {
    type Source = LiveSource;

    async fn connect(&mut self, generation: Generation) -> Result<LiveSource, ConnectError> {
        // The generation names the CONNECTION and only advances once one
        // succeeds, so a run that failed three times printed "generation 1"
        // three times and read as a loop. The attempt counter distinguishes
        // them.
        self.attempts += 1;
        // Every line names its character: with several logging in at once,
        // untagged stages from two logins interleaved and read as one
        // (author's live run, 2026-09-24).
        let who = format!("[{}] ", self.character);
        eprintln!(
            "\n{who}[connect] generation {}, attempt {}: logging in",
            generation.0, self.attempts
        );
        let credentials = Credentials {
            account: &self.account,
            password: &self.password,
            character: &self.character,
            game_code: &self.game_code,
        };
        // The fallback, not the bare eaccess call: `eaccess.play.net:7910` has
        // gone down while the website stayed up (`plan/10`, 2026-09-08), and
        // the author reports that is still the pattern. A credential rejection
        // is NOT retried through it -- see `eaccess/fallback.rs`.
        let (payload, provider) =
            cena_platform::authenticate_via(credentials, self.prefer, &self.pin, |line| {
                eprintln!("{who}{line}");
            })
            .await
            .map_err(classify)?;
        if provider == cena_platform::Provider::WebLogin {
            // Worth saying out loud: a web-login launch came through an HTML
            // scrape and a redirect chain rather than eaccess's `C`/`L`, so a
            // reader diagnosing an odd session needs to know which path
            // produced it.
            eprintln!("{who}[connect] authenticated via the web-login fallback");
        }
        // BEFORE the payload is printed or logged. `LaunchPayload`'s own
        // `Debug` redacts the key, but the supervisor is about to write log
        // lines about this connection and the game socket is about to carry
        // the key in its handshake.
        self.secrets.push(payload.key.clone());
        eprintln!("{who}[connect] {payload:?}");

        // A failure HERE is always transient: the credentials were accepted
        // and the launch key issued, so what failed is reaching the game
        // socket -- a network problem, not an account one.
        let socket = cena_platform::connect_game(&payload)
            .await
            .map_err(|error| ConnectError::transient("game_connect", error.to_string()))?;
        eprintln!("{who}[connect] game socket open\n");
        Ok(socket)
    }

    fn take_secrets(&mut self) -> Vec<String> {
        std::mem::take(&mut self.secrets)
    }
}

/// Carry `EaccessError`'s own verdict across the crate boundary.
///
/// **No string matching.** The classification is made in `cena-platform`,
/// where the protocol is understood; this only translates the type. An
/// earlier sketch matched on `stage == "a_response"` here, which would have
/// been wrong in the way `VellumFE` records: that stage covers a rejected
/// password *and* a mid-handshake EOF, and treating the EOF as fatal strands
/// the session.
fn classify(error: cena_platform::EaccessError) -> ConnectError {
    ConnectError {
        stage: error.stage,
        // Already redacted by `cena-platform`: `EaccessError::detail` never
        // contains a password or a session key. This field is logged.
        detail: error.detail,
        retryability: if error.fatal {
            Retryability::Fatal
        } else {
            Retryability::Transient
        },
    }
}
