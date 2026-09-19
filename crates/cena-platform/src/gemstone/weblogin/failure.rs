//! What went wrong, and whether anything else should be tried.
//!
//! # Why this is a type and not a string
//!
//! The supervisor has to tell two situations apart, and getting it wrong is
//! costly in both directions -- the same shape as
//! [`reject`](crate::eaccess::Rejection) for the eaccess `A` response:
//!
//! | | Retrying |
//! |---|---|
//! | the credentials or the character are wrong | pointless, and each try is a strike |
//! | the link or the server misbehaved | the only way back |
//!
//! Lich keeps this as a list of string codes checked with `include?`
//! (`authenticator.rb:29`, `:229`). Ported as an enum: the compiler then
//! enforces that every new failure states its verdict, which a string list
//! cannot.
//!
//! # The rule that matters most
//!
//! **A credential rejection is never retried through a second system.** Lich's
//! `authenticator.rb:124-129`, verbatim:
//!
//! > *"Credentials were rejected -- `WebLogin` would reject the same
//! > credentials too, so falling back would just resubmit them to a second
//! > system for no benefit. Surface the real problem."*
//!
//! That governs the fallback in both directions: eaccess must not fall back to
//! web login on a bad password, and web login must not retry one either.

use std::fmt;

/// Why a web login did not produce a launch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebLoginFailure {
    /// The account or password was rejected.
    ///
    /// Lich classifies a wrong password and a nonexistent account identically
    /// (`LOGIN_FAILED`): both land on the same `login_error.asp` redirect and
    /// differ only in a body heading, which is **not** used for control flow.
    /// The practical benefit is that a stale saved account name fails fast and
    /// fatally, rather than looping.
    LoginRejected,
    /// The account has no active subscription on this instance.
    ///
    /// Fatal: a retry cannot buy a subscription. Worth its own variant because
    /// it is the one failure whose fix is an action the player can take, so the
    /// message must not read as a password problem.
    NoSubscription,
    /// The character is not on this instance's character list.
    ///
    /// Fatal, following Lich. Note the hazard this creates, which is why
    /// [`Self::UnexpectedResponse`] exists: a non-200 response scrapes to no
    /// match, so a *transient* 503 would look exactly like a missing character
    /// and be discarded as fatal. The status is therefore checked **before**
    /// the scrape.
    CharacterNotFound,
    /// The web flow has no confirmed route for this game code.
    ///
    /// Fatal: the table is the evidence, and there is nothing to retry into.
    UnsupportedGameCode,
    /// The server did something the confirmed protocol shape does not cover.
    ///
    /// **Transient.** This is the counterpart of
    /// [`Rejection::Divergence`](crate::eaccess::Rejection::Divergence) and
    /// exists for the same reason: "we do not recognise this" is not evidence
    /// the credentials are wrong. A WAF intercept page, a 503, or a markup
    /// change all land here.
    ///
    /// Carries a short static label naming *where* it diverged, never a body.
    UnexpectedResponse(&'static str),
    /// The transport failed before an answer arrived.
    ///
    /// **Transient**, and the reason this whole module exists: an unreachable
    /// eaccess is precisely the case web login is the answer to.
    Transport(String),
}

impl WebLoginFailure {
    /// Whether the **credentials themselves** were refused.
    ///
    /// Deliberately narrower than "did this attempt fail". It is the one thing
    /// a web-login failure can say that stops a retry ladder, because it is the
    /// one thing that stays true on the next attempt and through another
    /// provider. See the note below on why there is no broader predicate.
    #[must_use]
    pub const fn is_credential_refusal(&self) -> bool {
        matches!(self, Self::LoginRejected)
    }
}

// NO `is_fatal` HERE, deliberately -- and this absence is the design, not an
// omission.
//
// One was written, and it had no caller outside its own tests. `plan/05` §-1
// forbids that, but the more useful reason is that it was answering the WRONG
// QUESTION. "Fatal for web login" and "fatal for this login attempt" are
// different, and only one variant answers both:
//
//   LoginRejected       the credentials were refused          -> stop everything
//   NoSubscription      not on THIS instance                  -> eaccess may differ
//   CharacterNotFound   the SCRAPE found nothing              -> eaccess asks `C` properly
//   UnsupportedGameCode web login's table is narrower         -> eaccess is not
//
// All four end a web-login attempt; only the first is evidence against retrying
// at all. `CharacterNotFound` is the sharpest: web login finds characters by
// scraping HTML, which fails SILENTLY when markup changes, so treating it as
// proof the character does not exist would turn a play.net markup change into a
// permanently dead session.
//
// So the caller that matters -- `eaccess::fallback::carry_forward` -- matches
// `LoginRejected` by name. A four-variant predicate there would have been
// exactly wrong while looking tidier. If a caller ever genuinely needs "did
// this web-login attempt end", add it then, with that caller in hand.

impl fmt::Display for WebLoginFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoginRejected => f.write_str("the account name or password was rejected"),
            Self::NoSubscription => {
                f.write_str("this account has no active subscription on that instance")
            }
            Self::CharacterNotFound => {
                f.write_str("no character by that name is on that instance's character list")
            }
            Self::UnsupportedGameCode => {
                f.write_str("web login has no confirmed route for that game code")
            }
            Self::UnexpectedResponse(where_) => {
                write!(f, "unexpected response from play.net at {where_}")
            }
            Self::Transport(detail) => write!(f, "could not reach play.net: {detail}"),
        }
    }
}

impl std::error::Error for WebLoginFailure {}
