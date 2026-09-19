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
/// launch synthesises a field the eaccess `L` response returns, and a reader
/// diagnosing an odd session needs to know which path produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// The `K A M F G P C L` exchange over TLS.
    Eaccess,
    /// The HTTPS web flow, after eaccess could not be reached.
    WebLogin,
}

/// Authenticate, falling back to web login if eaccess cannot be **reached**.
///
/// # The order is not arbitrary
///
/// eaccess is tried first because it is the real protocol: it enumerates
/// instances, it supports the character generator, and it returns every launch
/// field rather than having some synthesised. Web login is narrower, so it is
/// the fallback rather than the default even though it is more reliable on a
/// bad day.
///
/// # Errors
///
/// The **eaccess** error, always -- even when the fallback also failed.
/// `L` named the primary path, and its failure is the answer to "why could I
/// not log in"; the fallback's failure is a footnote that would otherwise
/// replace the real cause. The fallback's failure is passed to `progress`
/// instead, so it is visible without being mistaken for the diagnosis.
pub async fn authenticate_with_fallback(
    creds: Credentials<'_>,
    progress: impl FnMut(&str),
) -> Result<(LaunchPayload, Provider), EaccessError> {
    authenticate_via(creds, Prefer::Eaccess, progress).await
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

/// Authenticate, choosing the provider.
///
/// [`authenticate_with_fallback`] is this with [`Prefer::Eaccess`], which is
/// what production wants. [`Prefer::WebOnly`] exists so the web path can be
/// exercised on a day when eaccess is healthy -- see [`Prefer`].
///
/// # Errors
///
/// As [`authenticate_with_fallback`]. Under [`Prefer::WebOnly`] the error is
/// the **web-login** failure, since no eaccess attempt was made to have one.
pub async fn authenticate_via(
    creds: Credentials<'_>,
    prefer: Prefer,
    mut progress: impl FnMut(&str),
) -> Result<(LaunchPayload, Provider), EaccessError> {
    if prefer == Prefer::WebOnly {
        progress("[login] web login FORCED; eaccess will not be tried");
        let launch = try_web(creds, &mut progress).await.map_err(|failure| {
            // No primary error to carry, so this is the whole diagnosis.
            EaccessError {
                stage: "web_login",
                detail: failure.to_string(),
                fatal: failure.is_credential_refusal(),
            }
        })?;
        return Ok((from_web(&launch, creds.game_code), Provider::WebLogin));
    }

    let primary = match super::authenticate(creds, &mut progress).await {
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

    match try_web(creds, &mut progress).await {
        Ok(launch) => Ok((from_web(&launch, creds.game_code), Provider::WebLogin)),
        Err(secondary) => {
            // Reported, not returned. See this function's `# Errors`.
            progress(&format!("[fallback] web login also failed: {secondary}"));
            Err(carry_forward(&primary, &secondary))
        }
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
/// # Two fields are synthesised, and that is a real difference
///
/// The web flow returns a host, a port and a key -- and **not** `GAMECODE`,
/// which eaccess's `L` response carries. Lich synthesises the same defaults
/// (`web_login.rb:206-216`) and flags them: they are correct for the
/// Stormfront/Wrayth case, which is the only one Cena has.
///
/// The `game_code` is carried through from the request rather than invented,
/// because the caller asked for a specific instance and that is a fact, not a
/// default.
fn from_web(launch: &Launch, game_code: &str) -> LaunchPayload {
    LaunchPayload {
        gamehost: launch.host.clone(),
        gameport: launch.port,
        gamecode: Some(game_code.to_owned()),
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
