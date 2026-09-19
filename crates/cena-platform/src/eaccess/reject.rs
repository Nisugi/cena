//! Classifying an `A` response that carried no session key.
//!
//! Split out of [`handshake`](super::handshake) because it is pure -- it takes a
//! string and returns a verdict -- and the handshake around it needs a socket.
//! That is the same seam [`wire`](super::wire) already draws.
//!
//! # Two cases wearing one answer
//!
//! An `A` response without `\tKEY\t` was treated as **fatal**, full stop. That
//! collapses two situations the supervisor must tell apart:
//!
//! | The server said | It means | Retrying |
//! |---|---|---|
//! | `REJECT`, `NORECORD`, `INVALID`, `PASSWORD` | the credentials are wrong | pointless, and risks the account |
//! | anything else | we do not know what happened | is the only way back |
//!
//! Fatal is right for the first and **wrong for the second**, in the direction
//! that strands a session: a garbled reply during an unattended reconnect
//! permanently stops a client whose credentials are good. `wire.rs` itself calls
//! that direction unsafe, and `retry.rs` quotes `VellumFE` on the other one --
//! *"hammering the auth server with a wrong password ... could lock the
//! account"*. Both hazards are real and they point opposite ways, which is
//! exactly why the two cases cannot share a verdict.
//!
//! Lich splits them the same way, and its token list is taken verbatim:
//! `KNOWN_REJECTION_TOKENS = %w[REJECT NORECORD INVALID PASSWORD]`
//! (`reference/lich-5/lib/common/authentication/eaccess.rb:43`).
//!
//! # The raw body of an unrecognised reply is NOT logged
//!
//! This is Lich's other rule here, and its reasoning is the whole argument
//! (`docs/eaccess-failure-diagnostics.md:31-35`):
//!
//! > *"The `A` response is different: it can be a real Simutronics rejection
//! > token or an unrecognized/garbled response, and only the derived
//! > classification is logged, not the raw text -- an unrecognized response is
//! > exactly the case most likely to contain something unexpected (an HTML
//! > intercept page from a WAF, a truncated fragment, etc.) that shouldn't be
//! > assumed safe to log verbatim."*
//!
//! A **recognised** token is still printed: it is a short protocol word, it is
//! the most useful thing a reader can be told, and it is not a secret. So the
//! rule is not "never print the response" -- it is "print what you recognised".

/// The rejection words Simutronics sends, verbatim from Lich
/// (`lib/common/authentication/eaccess.rb:43`).
///
/// Ported rather than invented: these are observed server behaviour, which
/// `plan/13` §4a says to port rather than re-derive.
const KNOWN_REJECTION_TOKENS: &[&str] = &["REJECT", "NORECORD", "INVALID", "PASSWORD"];

/// What an `A` response with no session key turned out to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// The server named a rejection we recognise: the credentials are wrong.
    ///
    /// Carries the token, which is safe to print and is the single most useful
    /// thing to tell whoever is reading the failure.
    Credentials(String),
    /// The server said something we do not recognise.
    ///
    /// **No body.** It is the case most likely to be a WAF intercept page or a
    /// truncated fragment, and a length is all a diagnostic needs.
    Divergence { bytes: usize },
}

impl Rejection {
    /// Whether retrying could ever succeed.
    ///
    /// `false` for [`Self::Credentials`] -- the same credentials will be
    /// rejected again, and each attempt is another strike. `true` for
    /// [`Self::Divergence`], because "we do not know what that was" is not
    /// evidence the credentials are wrong, and a client that stops on it
    /// strands a session that a retry would have recovered.
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(self, Self::Credentials(_))
    }
}

/// Classify an `A` response that carried no `\tKEY\t`.
///
/// Matched with `contains` rather than an exact compare, following Lich
/// (`token.include?`): the response is tab-delimited and the token arrives as a
/// field, so requiring the whole string to equal it would recognise nothing.
#[must_use]
pub fn classify_a_rejection(response: &str) -> Rejection {
    let upper = response.to_ascii_uppercase();
    KNOWN_REJECTION_TOKENS
        .iter()
        .find(|token| upper.contains(*token))
        .map_or_else(
            || Rejection::Divergence {
                bytes: response.len(),
            },
            |token| Rejection::Credentials((*token).to_owned()),
        )
}
