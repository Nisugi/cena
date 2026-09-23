//! Tests for the fallback decision, without authenticating anything.
//!
//! [`authenticate_via`](super::authenticate_via) itself is **never called
//! here** -- it reaches a live service, which `CLAUDE.md` forbids from this
//! workspace. What is called is [`decide`], the whole of its logic with the two
//! providers passed in as stubs. That is where the cost of being wrong lives:
//!
//! - **whether to fall back at all**, read off `EaccessError::fatal`
//! - **what to report when both fail**, which must not replace the real cause
//!
//! # The first half used to test nothing
//!
//! The three tests that opened this file asserted `.fatal` on errors they had
//! built themselves -- `err(..)` is transient, `err(..).fatal()` is fatal --
//! and never called anything that READ the flag. Deleting the rule they were
//! named after, `if primary.fatal { return Err(primary) }`, left all three
//! green (review finding 5). A test of a decision must make the decision.

use std::cell::Cell;

use crate::eaccess::wire::{EaccessError, LaunchPayload, err};
use crate::gemstone::weblogin::{Launch, WebLoginFailure};

use super::{Prefer, Provider, carry_forward, decide};

/// A transport failure: eaccess could not be reached.
fn unreachable() -> EaccessError {
    err("tls_handshake", "connection timed out")
}

/// A refusal: the server answered, and the answer was no.
fn refused() -> EaccessError {
    err("a_response", "PASSWORD").fatal()
}

/// What a successful web login hands back. No key worth keeping.
fn web_launch() -> Launch {
    Launch {
        host: "web.example".to_owned(),
        port: 1,
        key: "k".to_owned(),
    }
}

/// Run [`decide`] with stub providers, and report whether web login was TRIED.
///
/// `web_calls` is the observable that matters: a fallback that ran when it
/// must not have is a bad password resubmitted to a second system, and it
/// looks identical to one that did not run if only the returned error is
/// checked -- `carry_forward` keeps the primary's error either way.
async fn run(
    prefer: Prefer,
    eaccess: Result<LaunchPayload, EaccessError>,
    web: Result<Launch, WebLoginFailure>,
) -> (Result<(LaunchPayload, Provider), EaccessError>, u32, u32) {
    let eaccess_calls = Cell::new(0);
    let web_calls = Cell::new(0);
    let result = decide(
        prefer,
        &mut |_line: &str| {},
        async |_progress| {
            eaccess_calls.set(eaccess_calls.get() + 1);
            eaccess
        },
        async |_progress| {
            web_calls.set(web_calls.get() + 1);
            web
        },
    )
    .await;
    (result, eaccess_calls.get(), web_calls.get())
}

#[tokio::test]
async fn an_unreachable_eaccess_falls_back_to_web_login() {
    // **The case the module exists for**, and the 2026-09-08 shape: the SYN is
    // dropped, eaccess never answers, and the website is up the whole time.
    let (result, _, web_calls) = run(Prefer::Eaccess, Err(unreachable()), Ok(web_launch())).await;
    assert_eq!(
        web_calls, 1,
        "a transport failure did not reach the fallback, which skips it on \
         exactly the day it is needed"
    );
    let (launch, provider) = result.expect("the fallback succeeded");
    assert_eq!(provider, Provider::WebLogin);
    assert_eq!(launch.gamehost, "web.example");
}

#[tokio::test]
async fn a_credential_refusal_never_reaches_web_login() {
    // **The rule.** Lich: "WebLogin would reject the same credentials too, so
    // falling back would just resubmit them to a second system for no benefit."
    // The cost of getting this wrong is a second bad-password strike against
    // the account, on every retry, forever.
    //
    // The web stub SUCCEEDS, deliberately: if the rule is gone, this run
    // returns Ok and the assertion below says why that is wrong.
    let (result, _, web_calls) = run(Prefer::Eaccess, Err(refused()), Ok(web_launch())).await;
    assert_eq!(
        web_calls, 0,
        "a credential refusal was resubmitted through web login"
    );
    let error = result.expect_err("the refusal is the answer");
    assert!(error.fatal);
    assert_eq!(error.stage, "a_response");
}

#[tokio::test]
async fn an_unclassified_failure_falls_back() {
    // `fatal` defaults to false, and that default is deliberate: an
    // unclassified error treated as fatal costs the session outright
    // (`wire.rs`, `EaccessError::fatal`).
    //
    // The price of the other direction is NOT "a bounded ladder", which is
    // what this comment used to say. `cena-session`'s supervisor retries a
    // transient connect failure FOREVER, on a backoff capped at 30s
    // (`SupervisedSession::run`'s docs: "There is deliberately no fourth"
    // stop). `MAX_UNATTENDED_LOSSES` counts dropped CONNECTIONS, not failed
    // logins. So a misclassified refusal is a full login every 30 seconds
    // until someone notices -- which is why findings 1 and 5 matter.
    //
    // Asserted through `decide` because the fallback READS that default, so
    // a change to it silently changes this module's behaviour too.
    let unclassified = err("some_new_stage", "something nobody has classified");
    let (_, _, web_calls) = run(Prefer::Eaccess, Err(unclassified), Ok(web_launch())).await;
    assert_eq!(web_calls, 1);
}

#[tokio::test]
async fn a_forced_web_login_never_tries_eaccess() {
    let (result, eaccess_calls, web_calls) =
        run(Prefer::WebOnly, Err(unreachable()), Ok(web_launch())).await;
    assert_eq!((eaccess_calls, web_calls), (0, 1));
    assert_eq!(result.expect("web succeeded").1, Provider::WebLogin);
}

#[tokio::test]
async fn a_forced_web_login_with_a_code_it_cannot_route_is_fatal() {
    // **Finding 1's second path.** With eaccess out of the picture, "web
    // login has no route for this code" is decided by a table compiled into
    // this binary. Nothing a retry does changes it, and a transient verdict
    // here is a supervisor re-running it every 30 seconds for good.
    let (result, _, _) = run(
        Prefer::WebOnly,
        Err(unreachable()),
        Err(WebLoginFailure::UnsupportedGameCode),
    )
    .await;
    let error = result.expect_err("no route");
    assert!(error.fatal, "{error}");
    assert_eq!(error.stage, "web_login");
}

#[tokio::test]
async fn a_forced_web_login_that_could_not_connect_stays_retryable() {
    // The converse: the forced path must not turn every failure fatal.
    let (result, _, _) = run(
        Prefer::WebOnly,
        Err(unreachable()),
        Err(WebLoginFailure::Transport("dns".to_owned())),
    )
    .await;
    assert!(!result.expect_err("transport").fatal);
}

// ---------------------------------------------------------------------------
// What is reported when both paths fail
// ---------------------------------------------------------------------------

#[test]
fn the_eaccess_stage_survives_the_fallbacks_failure() {
    // The supervisor acts on the STAGE. A web-login failure must not rewrite
    // where the eaccess attempt got to, or the diagnosis points at the wrong
    // protocol entirely.
    let carried = carry_forward(
        &unreachable(),
        &WebLoginFailure::Transport("dns".to_owned()),
    );
    assert_eq!(carried.stage, "tls_handshake");
}

#[test]
fn the_primary_cause_is_still_readable_after_both_fail() {
    // The fallback's failure is a FOOTNOTE. `L` named the primary path, so its
    // failure is the answer to "why could I not log in" -- and a reader who
    // gets only the fallback's error goes looking in the wrong place.
    let carried = carry_forward(
        &unreachable(),
        &WebLoginFailure::UnexpectedResponse("login redirect"),
    );
    assert!(
        carried.detail.contains("connection timed out"),
        "the real cause was replaced: {}",
        carried.detail
    );
    assert!(
        carried.detail.contains("web-login fallback also failed"),
        "the fallback attempt vanished from the record: {}",
        carried.detail
    );
}

#[test]
fn a_transient_failure_on_both_paths_stays_retryable() {
    // Both said "could not reach". Nothing has established that the credentials
    // are wrong, so stopping here would strand a session that a later retry
    // recovers -- the hazard `wire.rs` calls unsafe.
    let carried = carry_forward(
        &unreachable(),
        &WebLoginFailure::Transport("dns".to_owned()),
    );
    assert!(!carried.fatal);
}

#[test]
fn a_web_login_credential_refusal_makes_the_pair_fatal() {
    // **The one thing the fallback can contribute that the primary did not
    // know.** eaccess could not be reached, so it never judged the credentials
    // -- but web login did reach a server, and that server refused them.
    //
    // Both systems have now answered, so retrying is pointless and each attempt
    // is another strike. This is the single case where the secondary's verdict
    // changes the pair's.
    let carried = carry_forward(&unreachable(), &WebLoginFailure::LoginRejected);
    assert!(
        carried.fatal,
        "the credentials were refused by the system that DID answer, and the \
         ladder kept going"
    );
}

#[test]
fn a_web_login_failure_that_is_not_a_refusal_does_not_make_the_pair_fatal() {
    // The converse, and the direction that strands sessions. Only
    // `LoginRejected` is a judgement about the credentials; a missing character
    // or a WAF page is not, and must not stop a ladder that would otherwise
    // recover.
    for secondary in [
        WebLoginFailure::UnexpectedResponse("character list: not 200"),
        WebLoginFailure::Transport("timed out".to_owned()),
        WebLoginFailure::UnsupportedGameCode,
    ] {
        let carried = carry_forward(&unreachable(), &secondary);
        assert!(
            !carried.fatal,
            "{secondary:?} stopped the ladder, but it says nothing about the \
             credentials"
        );
    }
}

#[test]
fn a_fatal_primary_stays_fatal_whatever_the_fallback_said() {
    // Fatality only ever ratchets ON. If eaccess already refused the
    // credentials, no fallback outcome makes them worth retrying.
    let carried = carry_forward(&refused(), &WebLoginFailure::Transport("dns".to_owned()));
    assert!(carried.fatal);
}

#[test]
fn the_carried_detail_never_holds_a_response_body() {
    // `WebLoginFailure::UnexpectedResponse` carries a `&'static str` label, so
    // a WAF intercept page cannot reach this string. That is what makes the
    // combined detail safe to log, and it is the same rule `reject.rs` applies
    // to the eaccess `A` response.
    let carried = carry_forward(
        &unreachable(),
        &WebLoginFailure::UnexpectedResponse("character list: not 200"),
    );
    assert!(!carried.detail.contains("<html"), "{}", carried.detail);
    assert!(!carried.detail.contains("Set-Cookie"), "{}", carried.detail);
}
