//! **PL-2: an unrecognised `A` reply stranded a session permanently.**
//!
//! Every `A` response without `\tKEY\t` was marked **fatal**, which collapses
//! two cases the supervisor must tell apart:
//!
//! | The server said | Retrying |
//! |---|---|
//! | `REJECT` / `NORECORD` / `INVALID` / `PASSWORD` | pointless, and costs a strike |
//! | anything else | is the only way back |
//!
//! Fatal is right for the first and **wrong for the second**, in the direction
//! that strands a live session: a garbled reply during an unattended reconnect
//! permanently stopped a client whose credentials were good. `wire.rs` itself
//! calls that direction unsafe, and `retry.rs` quotes `VellumFE` on the opposite
//! hazard -- *"hammering the auth server with a wrong password ... could lock
//! the account"*.
//!
//! **Both hazards are real and they point opposite ways.** That is precisely why
//! the two cases cannot share a verdict, and why this is not a matter of picking
//! the safer default.
//!
//! Lich splits them identically, with the token list taken verbatim
//! (`lib/common/authentication/eaccess.rb:43`) and the no-raw-body rule from
//! `docs/eaccess-failure-diagnostics.md:31-35`.

use cena_platform::eaccess::{Rejection, classify_a_rejection};

#[test]
fn a_recognised_rejection_token_is_fatal() {
    // The credentials are wrong. Retrying sends the same wrong password again,
    // and each attempt is another strike against the account.
    for token in ["REJECT", "NORECORD", "INVALID", "PASSWORD"] {
        let response = format!("A\tACCT\t{token}");
        let verdict = classify_a_rejection(&response);

        assert_eq!(
            verdict,
            Rejection::Credentials((*token).to_owned()),
            "{token} was not recognised as a credential rejection"
        );
        assert!(verdict.is_fatal(), "{token} must stop the ladder");
    }
}

#[test]
fn an_unrecognised_reply_is_transient() {
    // **The defect.** Anything we do not recognise is "we do not know what
    // happened", which is not evidence the credentials are wrong.
    let verdict = classify_a_rejection("<html><body>403 Forbidden</body></html>");

    assert!(
        !verdict.is_fatal(),
        "an unrecognised reply stopped the session permanently. A WAF intercept \
         page is not a credential rejection, and a client that treats it as one \
         strands a session whose password is fine."
    );
    assert!(matches!(verdict, Rejection::Divergence { .. }));
}

#[test]
fn an_empty_reply_is_transient() {
    // A truncated read is the likeliest divergence of all.
    assert!(!classify_a_rejection("").is_fatal());
}

#[test]
fn the_body_of_an_unrecognised_reply_is_not_carried() {
    // Lich's rule, and its reasoning: an unrecognised response is "exactly the
    // case most likely to contain something unexpected (an HTML intercept page
    // from a WAF, a truncated fragment, etc.) that shouldn't be assumed safe to
    // log verbatim."
    //
    // So the verdict carries a LENGTH, not the text. The type is what enforces
    // it -- there is nowhere for a body to go.
    let body = "<html>Set-Cookie: session=abc123</html>";
    let verdict = classify_a_rejection(body);

    assert_eq!(
        verdict,
        Rejection::Divergence { bytes: body.len() },
        "the verdict must carry only a length"
    );
    assert!(
        !format!("{verdict:?}").contains("abc123"),
        "the raw body reached the Debug output: {verdict:?}"
    );
}

#[test]
fn a_recognised_token_is_carried_because_it_is_useful_and_not_a_secret() {
    // The rule is "print what you recognised", not "never print the response".
    // A rejection token is a short protocol word and the single most useful
    // thing to tell whoever is reading the failure.
    let verdict = classify_a_rejection("A\tACCT\tPASSWORD");
    assert!(
        format!("{verdict:?}").contains("PASSWORD"),
        "the recognised token should be visible: {verdict:?}"
    );
}

#[test]
fn a_token_is_matched_as_a_field_not_as_the_whole_string() {
    // `contains`, following Lich's `token.include?`. The response is
    // tab-delimited and the token arrives as one field, so an exact compare
    // would recognise nothing at all and every rejection would read as
    // divergence -- failing open, in the account-risking direction.
    assert!(classify_a_rejection("A\tSOMEACCT\tREJECT\textra").is_fatal());
}

#[test]
fn matching_is_case_insensitive() {
    // Defensive: the tokens are observed uppercase, but a case flip would
    // otherwise turn a real rejection into an infinite retry loop against the
    // auth server -- the exact thing `MAX_UNATTENDED_LOSSES` and the fatal
    // classification both exist to prevent.
    assert!(classify_a_rejection("A\tACCT\treject").is_fatal());
}

#[test]
fn a_reply_that_merely_mentions_a_word_in_prose_still_classifies_as_rejection() {
    // Recorded as a KNOWN LIMIT rather than fixed. `contains` cannot tell a
    // field from prose, so an intercept page containing the word "password"
    // classifies as a credential rejection and stops the ladder.
    //
    // That is the SAFE direction of the two: it stops rather than hammering the
    // auth server, which is the hazard `VellumFE` documents. Fixing it properly
    // means splitting on tabs and matching a field exactly -- but the observed
    // shapes vary (`plan/10` §12.1: no terminator, field counts differ), so a
    // stricter match risks failing open instead. Left as it is, deliberately,
    // with this test as the record.
    let verdict = classify_a_rejection("<html>Please check your password and retry</html>");
    assert!(
        verdict.is_fatal(),
        "documented behaviour changed -- see this test's comment before \
         accepting it"
    );
}
