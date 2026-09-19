//! Tests for the fallback decision, without authenticating anything.
//!
//! [`authenticate_with_fallback`](super::authenticate_with_fallback) itself is
//! **never called here** -- it reaches a live service, which `CLAUDE.md`
//! forbids from this workspace. What is tested is the two pure decisions it
//! makes, which is where the cost of being wrong lives:
//!
//! - **whether to fall back at all**, read off `EaccessError::fatal`
//! - **what to report when both fail**, which must not replace the real cause
//!
//! The dispatch between them is three lines of `match`, checked by eye.

use crate::eaccess::wire::{EaccessError, err};
use crate::gemstone::weblogin::WebLoginFailure;

use super::carry_forward;

/// A transport failure: eaccess could not be reached.
fn unreachable() -> EaccessError {
    err("tls_handshake", "connection timed out")
}

/// A refusal: the server answered, and the answer was no.
fn refused() -> EaccessError {
    err("a_response", "PASSWORD").fatal()
}

#[test]
fn an_unreachable_eaccess_is_not_fatal_so_the_fallback_runs() {
    // **The case the module exists for**, and the 2026-09-08 shape: the SYN is
    // dropped, eaccess never answers, and the website is up the whole time.
    assert!(
        !unreachable().fatal,
        "a transport failure was marked fatal, which skips the fallback on \
         exactly the day it is needed"
    );
}

#[test]
fn a_credential_refusal_is_fatal_so_the_fallback_is_skipped() {
    // **The rule.** Lich: "WebLogin would reject the same credentials too, so
    // falling back would just resubmit them to a second system for no benefit."
    // The cost of getting this wrong is a second bad-password strike against
    // the account, on every retry, forever.
    assert!(
        refused().fatal,
        "a credential refusal did not stop the ladder"
    );
}

#[test]
fn an_unclassified_failure_falls_back() {
    // `fatal` defaults to false, and that default is deliberate: an
    // unclassified error retried costs a bounded ladder, while an unclassified
    // error treated as fatal costs the session (`wire.rs:160-163`).
    //
    // Asserted here because the fallback READS that default, so a change to it
    // silently changes this module's behaviour too.
    assert!(!err("some_new_stage", "something nobody has classified").fatal);
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
