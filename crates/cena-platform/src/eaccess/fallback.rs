//! Two ways in, and the rule for choosing between them.
//!
//! # Why there is a second way in at all
//!
//! `eaccess.play.net:7910` has gone down while the website stayed up. `plan/10`
//! records 2026-09-08: `nc -vz eaccess.play.net 7910` exited 124 -- the SYN
//! silently dropped, no RST -- and *"website auth and Play-in-Browser stayed up
//! throughout"*. The author, 2026-09-19: they have been *"having issues with
//! the normal login, and the web one seems to stay up during these times
//! allowing access to the game."*
//!
//! So the fallback is not redundancy for its own sake. It is the path that
//! works on the days the main one does not.
//!
//! # The rule, and why it is the whole design
//!
//! **A credential rejection is never retried through the second system.**
//! Lich's `authenticator.rb:124-129`, verbatim:
//!
//! > *"Credentials were rejected -- `WebLogin` would reject the same
//! > credentials too, so falling back would just resubmit them to a second
//! > system for no benefit. Surface the real problem."*
//!
//! Two failures that both read as "login didn't work" need opposite responses,
//! and both mistakes are expensive:
//!
//! | eaccess said | fall back? | cost of getting it wrong |
//! |---|---|---|
//! | the password is wrong | **no** | a second bad-password strike, and the real error is hidden behind a second failure |
//! | I could not reach the server | **yes** | the session stays down through an outage the fallback would have survived |
//!
//! [`EaccessError::fatal`] already carries this distinction -- it exists
//! because *one stage is both* -- so this module reads a decision that has
//! already been made rather than making a new one. Note its default is
//! `false`: an unclassified failure falls back, which is the direction that
//! fails safe.
//!
//! # What the fallback cannot do
//!
//! Web login resolves **one character on one instance**. It has no equivalent
//! of eaccess's `M` enumeration or the character generator, so those paths have
//! no fallback and Lich gives them the full retry budget instead
//! (`authenticator.rb:191`). Cena does not implement either yet; when it does,
//! this is the constraint.

use crate::gemstone::weblogin::{Launch, WebLoginFailure, WebLoginRequest, authenticate_via_web};

use super::wire::{Credentials, EaccessError, LaunchPayload};

/// Which provider produced a launch.
///
/// Reported rather than hidden, because the two are not equivalent: a web-login
/// launch came through an HTML scrape and a redirect chain rather than the
/// `C`/`L` exchange, and a reader diagnosing an odd session needs to know
/// which path produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// The `K A M F G P C L` exchange over TLS.
    Eaccess,
    /// The HTTPS web flow, after eaccess could not be reached.
    WebLogin,
}

/// Which provider to try first, or to use alone.
///
/// # Why a forced web path exists at all
///
/// Because the fallback is otherwise **unreachable while eaccess is up**, and
/// eaccess is up almost always -- which is the whole point of it. Without this,
/// the web path could only ever be exercised during an outage, i.e. at the
/// worst possible moment to discover a bug in it.
///
/// Lich has the same escape hatch for the same reason (`auth_provider: :web`,
/// `authenticator.rb:105`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefer {
    /// The normal path: eaccess, with web login if it cannot be reached.
    Eaccess,
    /// **Web login only.** eaccess is not tried, so there is nothing to fall
    /// back to and a failure here is final.
    WebOnly,
}

/// Authenticate, choosing the provider -- and falling back to web login if
/// eaccess cannot be **reached**.
///
/// # The order is not arbitrary
///
/// eaccess is tried first because it is the real protocol: it enumerates
/// instances, it supports the character generator, and it returns what the
/// launch needs without a scrape in between. Web login is narrower, so it is
/// the fallback rather than the default even though it is more reliable on a
/// bad day. [`Prefer::WebOnly`] exists so the web path can be exercised on a
/// day when eaccess is healthy -- see [`Prefer`].
///
/// # There used to be a second entry point
///
/// `authenticate_with_fallback` was this with [`Prefer::Eaccess`] fixed. Its
/// only production caller moved to this function when `--web-login` arrived,
/// and it was left exported with none (review finding 12, `plan/05` §-1).
/// Removed rather than kept as a convenience: a second name for one behaviour
/// is a second place a reader must check is the same.
///
/// # Errors
///
/// The **eaccess** error, always -- even when the fallback also failed.
/// `L` named the primary path, and its failure is the answer to "why could I
/// not log in"; the fallback's failure is a footnote that would otherwise
/// replace the real cause. The fallback's failure is passed to `progress`
/// instead, so it is visible without being mistaken for the diagnosis.
///
/// Under [`Prefer::WebOnly`] the error is the **web-login** failure, since no
/// eaccess attempt was made to have one.
pub async fn authenticate_via(
    creds: Credentials<'_>,
    prefer: Prefer,
    mut progress: impl FnMut(&str),
) -> Result<(LaunchPayload, Provider), EaccessError> {
    decide(
        prefer,
        &mut progress,
        async |progress| super::authenticate(creds, progress).await,
        async |progress| try_web(creds, progress).await,
    )
    .await
}

/// The choice between the two providers, with the providers passed in.
///
/// # Why the stages are parameters
///
/// So that the RULE can be tested -- the thing this module exists for -- with
/// no network. [`authenticate_via`] passes the real two; `fallback_tests.rs`
/// passes stubs that fail on command and record whether they were called.
///
/// The tests before this could not reach the rule at all. They asserted
/// `.fatal` on errors they had built themselves and never called anything that
/// read it, so deleting the `if primary.fatal { return Err(primary) }` below --
/// the line that stops a bad password being resubmitted to a second system --
/// left every one of them green (review finding 5).
async fn decide<P: FnMut(&str)>(
    prefer: Prefer,
    progress: &mut P,
    eaccess: impl AsyncFnOnce(&mut P) -> Result<LaunchPayload, EaccessError>,
    web: impl AsyncFnOnce(&mut P) -> Result<Launch, WebLoginFailure>,
) -> Result<(LaunchPayload, Provider), EaccessError> {
    if prefer == Prefer::WebOnly {
        progress("[login] web login FORCED; eaccess will not be tried");
        let launch = web(progress).await.map_err(|f| web_only_error(&f))?;
        return Ok((from_web(&launch), Provider::WebLogin));
    }

    let primary = match eaccess(progress).await {
        Ok(launch) => return Ok((launch, Provider::Eaccess)),
        Err(error) => error,
    };

    // **The rule.** A refusal is an answer, and the same credentials will be
    // refused again. Falling back here would spend a second bad-password strike
    // and bury the real cause under a second, less relevant failure.
    if primary.fatal {
        return Err(primary);
    }

    progress(&format!(
        "[fallback] eaccess unreachable at stage {}; trying web login",
        primary.stage
    ));

    match web(progress).await {
        Ok(launch) => Ok((from_web(&launch), Provider::WebLogin)),
        Err(secondary) => {
            // Reported, not returned. See `authenticate_via`'s `# Errors`.
            progress(&format!("[fallback] web login also failed: {secondary}"));
            Err(carry_forward(&primary, &secondary))
        }
    }
}

/// A forced web login's failure, as the whole diagnosis.
///
/// No primary error to carry, so this is all there is -- and its fatality is
/// decided differently from [`carry_forward`]'s, for one variant.
///
/// # `UnsupportedGameCode` is fatal HERE, and only here
///
/// In the fallback it is not: web login's table is narrower than eaccess's, so
/// "web cannot route this code" says nothing about whether eaccess will, once
/// it answers. Under [`Prefer::WebOnly`] there is no eaccess to answer. The
/// table is compiled into this binary, so the verdict is identical on every
/// attempt -- and a supervisor told "transient" re-runs it every 30 seconds
/// forever, for a mistyped `GS4` (review finding 1, its second path).
///
/// `LoginRejected` is fatal for the reason it is everywhere: the credentials
/// were refused. The rest stay transient for the reasons `carry_forward` gives.
fn web_only_error(failure: &WebLoginFailure) -> EaccessError {
    EaccessError {
        stage: "web_login",
        detail: failure.to_string(),
        fatal: failure.is_credential_refusal()
            || matches!(failure, WebLoginFailure::UnsupportedGameCode),
    }
}

/// Run the web-login flow, narrating each stage.
///
/// Shared by both paths so the forced run exercises **exactly** the code the
/// fallback would -- a forced path that differs from the real one tests the
/// wrong thing.
async fn try_web(
    creds: Credentials<'_>,
    progress: &mut impl FnMut(&str),
) -> Result<Launch, WebLoginFailure> {
    let request = WebLoginRequest {
        account: creds.account,
        password: creds.password,
        character: creds.character,
        game_code: creds.game_code,
    };
    let launch = authenticate_via_web(request, progress).await?;
    progress("[web] authenticated via web login");
    Ok(launch)
}

/// A web-login [`Launch`] as a [`LaunchPayload`].
///
/// # Nothing is synthesised, and that used to be untrue
///
/// The web flow returns a host, a port and a key, and a [`LaunchPayload`] is
/// now exactly those three. It used to carry a `gamecode` too, which the web
/// flow does not return -- so this filled it with the REQUESTED code (`GS3`),
/// where eaccess's `L` fills it with the server's family code (`GS`). One field,
/// two vocabularies, and no reader (review finding 11). `wire.rs` records why
/// it went rather than being made `None` here.
fn from_web(launch: &Launch) -> LaunchPayload {
    LaunchPayload {
        gamehost: launch.host.clone(),
        gameport: launch.port,
        key: launch.key.clone(),
    }
}

/// The eaccess error, noting that the fallback was tried and also failed.
///
/// The **stage and fatality are the primary's**, unchanged: the supervisor
/// acts on those, and a web-login failure must not change what it does about an
/// eaccess one. Only the human-readable detail gains a clause.
///
/// One exception, and it is the case that matters: if the fallback says the
/// **credentials** are wrong, then both systems have now refused them and
/// retrying is pointless. That is the one piece of information the fallback can
/// contribute which the primary did not have.
///
/// # Why this matches one variant by name rather than asking a predicate
///
/// Because "fatal **for web login**" and "fatal **for this login attempt**" are
/// different questions, and only one variant answers both. Four
/// [`WebLoginFailure`] variants end a web-login attempt; three of them say
/// nothing about whether eaccess would have succeeded:
///
/// | | fatal for web login | evidence against retrying at all |
/// |---|---|---|
/// | `LoginRejected` | yes | **yes** -- the credentials were refused |
/// | `NoSubscription` | yes | no -- this instance, not these credentials |
/// | `CharacterNotFound` | yes | no -- the scrape is the fragile part; eaccess enumerates properly |
/// | `UnsupportedGameCode` | yes | no -- web login's table is narrower than eaccess's |
///
/// `CharacterNotFound` is the sharpest case. Web login finds characters by
/// **scraping HTML**, which fails silently when the markup changes; eaccess
/// asks the `C` command. So "web login could not find the character" is a
/// statement about the scraper at least as often as about the account, and
/// letting it stop the ladder would turn a play.net markup change into a
/// permanently dead session.
///
/// Matching the one variant by name therefore is not a missed abstraction --
/// it IS the distinction. `failure.rs` records why no four-variant predicate
/// exists to be tempted by.
fn carry_forward(primary: &EaccessError, secondary: &WebLoginFailure) -> EaccessError {
    let credentials_refused = secondary.is_credential_refusal();
    EaccessError {
        stage: primary.stage,
        detail: format!(
            "{} (web-login fallback also failed: {secondary})",
            primary.detail
        ),
        fatal: primary.fatal || credentials_refused,
    }
}

#[cfg(test)]
#[path = "fallback_tests.rs"]
mod fallback_tests;
