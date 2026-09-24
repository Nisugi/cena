//! Tests for the pure half of the web-login flow.
//!
//! # Why these live in `src/gemstone/` and not in `tests/`
//!
//! Every fixture here spells a game hostname or an `/gs4/` path, and
//! `cena-arch-tests`' Rule 3.4 scan covers integration tests as well as `src`.
//! Under `src/gemstone/` they are exempt by path -- the same reason the
//! instance table is here. See `crate::gemstone`'s header.
//!
//! # What these tests are FOR
//!
//! Six behaviours in this protocol were only discoverable by running a
//! non-browser client against several live accounts
//! (`lich-5/docs/web-login-protocol-analysis.md`). `CLAUDE.md` forbids
//! re-observing any of them from this workspace, so **these fixtures are the
//! only record Cena has**, and a test that stops holding here is a signal to go
//! back to the live protocol doc rather than to edit the assertion.

use super::failure::WebLoginFailure;
use super::instance::instance_for;
use super::scrape::{
    Hop, LoginRedirect, character_entries, classify_login_redirect, find_char_code,
    is_subscription_needed, next_hop, parse_launch,
};

/// `GemStone` Prime, the instance with the code divergence.
fn prime() -> &'static super::instance::Instance {
    instance_for("GS3").expect("GS3 is a confirmed instance")
}

// ---------------------------------------------------------------------------
// The instance table
// ---------------------------------------------------------------------------

#[test]
fn prime_asks_the_web_flow_for_gs4_not_gs3() {
    // **The divergence, and the reason this is a table rather than a
    // passthrough.** GemStone Prime is `GS3` over eaccess and `GS4` here; Lich
    // records that `GS3` is simply not accepted by the web layer. Because the
    // rejection is silent rather than an error, guessing would fail far away
    // from the cause.
    assert_eq!(prime().game_code, "GS3");
    assert_eq!(prime().web_game_code, "GS4");
}

#[test]
fn the_other_instances_pass_their_code_through_unchanged() {
    // Every instance except Prime uses the same code both ways. Asserted so
    // that a future row copied from Prime's shape is caught.
    for code in ["GST", "GSF"] {
        let instance = instance_for(code).expect("a confirmed instance");
        assert_eq!(
            instance.game_code, instance.web_game_code,
            "{code} diverged; if that is real, the protocol doc needs updating"
        );
    }
}

#[test]
fn each_instance_scrapes_its_own_character_page() {
    // **Discovery 4**, confirmed live: a Shattered-only character appears on
    // `/gs4/play/playf.asp` and NOT AT ALL on GemStone Prime's page. Scraping
    // the family's generic page would report "no such character" for a
    // character that exists -- and that failure is classified fatal.
    let paths: Vec<&str> = ["GS3", "GST", "GSF"]
        .iter()
        .map(|code| {
            instance_for(code)
                .expect("a confirmed instance")
                .character_list_path
        })
        .collect();

    let mut unique = paths.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        paths.len(),
        unique.len(),
        "two instances share a character page: {paths:?}. That is the \
         assumption discovery 4 disproved."
    );
}

#[test]
fn an_unknown_game_code_is_refused_rather_than_guessed() {
    // Lich raises `UNSUPPORTED_GAME_CODE` rather than assuming passthrough,
    // because the web layer's codes are not always the eaccess ones. `GSX` is
    // the concrete case: GemStone Platinum is RETIRED.
    assert!(instance_for("GSX").is_none());
    assert!(instance_for("DR").is_none(), "DragonRealms is deferred");
    assert!(instance_for("").is_none());
}

// ---------------------------------------------------------------------------
// The login redirect -- the pass/fail signal
// ---------------------------------------------------------------------------

#[test]
fn the_okay_page_means_authenticated() {
    assert_eq!(
        classify_login_redirect(Some("/gs4/play/home.asp"), prime()),
        LoginRedirect::Authenticated
    );
}

#[test]
fn the_security_question_page_is_also_a_success() {
    // **Discovery 3**, and the one most likely to be "fixed" into a bug by
    // someone reading the code without this note. An account that has never set
    // a security question is redirected HERE instead of to `okay_page` -- but
    // the session is already fully authenticated: the account name renders in
    // the banner and the real session cookie is set.
    //
    // Treating it as a failure would lock out every such account.
    assert_eq!(
        classify_login_redirect(Some("/playdotnet/account/security_qa.asp"), prime()),
        LoginRedirect::Authenticated
    );
}

#[test]
fn the_error_page_means_rejected_whatever_its_query_says() {
    // The two confirmed failure shapes differ ONLY in the query string -- a
    // wrong password gives `returnto=/gs4/`, a nonexistent account
    // `returnto=/gs4/play/home.asp` -- and mean the same thing. Lich
    // deliberately classifies both identically rather than reading the body
    // heading, so a stale saved account name fails fast instead of looping.
    for query in [
        "",
        "?error=&returnto=/gs4/",
        "?error=&returnto=/gs4/play/home.asp",
    ] {
        assert_eq!(
            classify_login_redirect(Some(&format!("/gs4/login_error.asp{query}")), prime()),
            LoginRedirect::Rejected,
            "query {query:?} changed the verdict"
        );
    }
}

#[test]
fn a_missing_location_is_unexpected_not_a_rejection() {
    // The direction matters: `Unexpected` is transient and `Rejected` is fatal,
    // so reading a header-less response as a rejection would strand a session
    // whose password is fine.
    assert_eq!(
        classify_login_redirect(None, prime()),
        LoginRedirect::Unexpected
    );
}

#[test]
fn an_unrecognised_redirect_is_unexpected() {
    // A WAF intercept, a maintenance page, or a protocol change. All transient:
    // "we do not recognise this" is not evidence the credentials are wrong.
    assert_eq!(
        classify_login_redirect(Some("/somewhere/else.asp"), prime()),
        LoginRedirect::Unexpected
    );
}

// ---------------------------------------------------------------------------
// The subscription redirect -- discoveries 5 and 6
// ---------------------------------------------------------------------------

#[test]
fn the_plain_subscription_page_is_recognised() {
    // **Discovery 5.** Login itself SUCCEEDED and landed on `okay_page`; it is
    // GETting that page which redirects again. A non-following client never
    // sees it and falls through to an empty scrape, reporting
    // `CharacterNotFound` -- fatal, and the wrong cause entirely.
    assert!(is_subscription_needed(
        "/gs4/play/subscription_needed.asp",
        prime()
    ));
}

#[test]
fn an_instance_specific_subscription_page_is_recognised_by_shape() {
    // **Discovery 6.** The path is not one fixed string: DragonRealms Platinum
    // uses `subscription_to_plat_needed.asp` and Fallen
    // `subscription_to_fall_needed.asp`. No GemStone instance has been seen
    // with a `_to_` variant, which is exactly why the loose match is kept --
    // assuming GemStone will never have one is the guess being avoided.
    assert!(is_subscription_needed(
        "/gs4/play/subscription_to_plat_needed.asp",
        prime()
    ));
    assert!(is_subscription_needed(
        "/gs4/play/subscription_to_anything_needed.asp",
        prime()
    ));
}

#[test]
fn a_similar_path_is_not_a_subscription_page() {
    // The shape match must not become a substring match. Each of these is a
    // different page, and calling it `NoSubscription` would report a fatal,
    // actionable failure for something else entirely.
    for path in [
        "/gs4/play/subscription_needed.asp.evil",
        "/gs4/play/subscription/needed.asp",
        "/gs4/play/subscriptions_needed.asp",
        "/gs4/play/subscription_to__needed.asp",
        "/dr/play/subscription_needed.asp",
        "/gs4/play/home.asp",
        // A path separator INSIDE the `_to_` middle. These are the cases the
        // separator guard actually exists for, and the only ones that reach it:
        // `subscription/needed.asp` above fails for an unrelated reason (its
        // tail is `/needed.asp`, which matches neither branch), so it does not
        // exercise the guard at all.
        //
        // FOUND by deleting the guard and watching every test still pass. The
        // original list looked like it covered this and did not.
        "/gs4/play/subscription_to_a/b_needed.asp",
        "/gs4/play/subscription_to_../../evil_needed.asp",
    ] {
        assert!(
            !is_subscription_needed(path, prime()),
            "{path} was misread as a subscription page"
        );
    }
}

// ---------------------------------------------------------------------------
// The character scrape -- the most fragile part of the protocol
// ---------------------------------------------------------------------------

/// The markup Lich captured live, with a second character added.
const CHARACTER_PAGE: &str = r#"
<form name="pickchar">
<input type=radio name="charID" id="W_TESTACCOUNT_000" value="W_TESTACCOUNT_000" checked  >
<label for="W_TESTACCOUNT_000"><span class="normS1">Raiyen</span></label><br>
<input type=radio name="charID" id="W_TESTACCOUNT_001" value="W_TESTACCOUNT_001"   >
<label for="W_TESTACCOUNT_001"><span class="normS1">Tanwen</span></label><br>
</form>
"#;

#[test]
fn a_character_resolves_to_its_own_code() {
    assert_eq!(
        find_char_code(CHARACTER_PAGE, "Tanwen").as_deref(),
        Some("W_TESTACCOUNT_001"),
        "the SECOND entry resolved wrongly, which is how an off-by-one here \
         looks: the player launches someone else's character"
    );
}

#[test]
fn the_name_match_ignores_case_and_surrounding_space() {
    // Lich uses `casecmp?` on a stripped name. A player typing `raiyen` must
    // not be told their character does not exist.
    for spelling in ["raiyen", "RAIYEN", "  Raiyen  "] {
        assert_eq!(
            find_char_code(CHARACTER_PAGE, spelling).as_deref(),
            Some("W_TESTACCOUNT_000"),
            "{spelling:?} did not match"
        );
    }
}

#[test]
fn a_character_not_on_this_page_does_not_resolve() {
    assert_eq!(find_char_code(CHARACTER_PAGE, "Nobody"), None);
}

#[test]
fn a_name_must_match_whole_and_not_merely_start_a_real_one() {
    // A prefix match would launch SOMEONE ELSE'S CHARACTER: ask for "Rai" and
    // get "Raiyen". It also makes two characters whose names share a prefix
    // ambiguous, resolving by page order rather than by name.
    //
    // FOUND by breaking `find_char_code` into a `starts_with` and watching
    // every test pass: no fixture name was a prefix of another, so nothing
    // could tell the two behaviours apart.
    assert_eq!(find_char_code(CHARACTER_PAGE, "Rai"), None);
    assert_eq!(find_char_code(CHARACTER_PAGE, "Tan"), None);

    // And the converse: a name that merely CONTAINS a real one is not it
    // either.
    assert_eq!(find_char_code(CHARACTER_PAGE, "Raiyena"), None);

    // Two characters sharing a prefix must each resolve to their own code.
    let page = r#"
<input type=radio name="charID" id="W_ACCT_000" value="W_ACCT_000">
<label for="W_ACCT_000"><span class="normS1">Rai</span></label><br>
<input type=radio name="charID" id="W_ACCT_001" value="W_ACCT_001">
<label for="W_ACCT_001"><span class="normS1">Raiyen</span></label><br>
"#;
    assert_eq!(find_char_code(page, "Rai").as_deref(), Some("W_ACCT_000"));
    assert_eq!(
        find_char_code(page, "Raiyen").as_deref(),
        Some("W_ACCT_001")
    );
}

#[test]
fn an_input_without_a_matching_label_does_not_borrow_the_next_name() {
    // The id and the display name are in SEPARATE elements, so a scan that
    // simply takes "the next label" pairs a character code with someone else's
    // name -- and the player launches the wrong character.
    // The orphan carries a `<span>` of its OWN -- a heading, a tooltip, any
    // markup at all between the two inputs. Without one, "the next span" and
    // "this code's label" happen to coincide and the fixture cannot tell a
    // correct scan from a broken one.
    //
    // FOUND by deliberately breaking `character_entries` to take the next
    // `<span>`: the first version of this test PASSED against that break,
    // because its orphan had no span. A fixture that cannot distinguish the
    // behaviours it names is a test that cannot fail (`plan/19` pattern D).
    let page = r#"
<input type=radio name="charID" id="W_ACCT_000" value="W_ACCT_000">
<span class="alert">Slot unavailable</span>
<input type=radio name="charID" id="W_ACCT_001" value="W_ACCT_001">
<label for="W_ACCT_001"><span class="normS1">Tanwen</span></label><br>
"#;
    assert_eq!(
        character_entries(page),
        vec![("W_ACCT_001".to_owned(), "Tanwen".to_owned())],
        "the orphaned input borrowed a name that is not a character"
    );
    assert_eq!(
        find_char_code(page, "Slot unavailable"),
        None,
        "page furniture resolved as a character"
    );
}

#[test]
fn a_markup_change_yields_no_entries_rather_than_a_wrong_one() {
    // **The failure Lich warns is silent.** This asserts the shape of that
    // silence: nothing is invented. The caller turns an empty result into
    // `CharacterNotFound`, so the protective thing is that the STATUS is
    // checked before the scrape -- see `only_a_200_is_scraped` below.
    let changed = r#"<div data-char="W_ACCT_000">Raiyen</div>"#;
    assert!(character_entries(changed).is_empty());
    assert_eq!(find_char_code(changed, "Raiyen"), None);
}

#[test]
fn an_empty_page_scrapes_to_nothing() {
    assert!(character_entries("").is_empty());
}

// ---------------------------------------------------------------------------
// The launch URL -- a security boundary
// ---------------------------------------------------------------------------

/// A well-formed launch URL for Prime, matching the pinned host and port.
fn prime_launch(query: &str) -> String {
    format!("https://www.play.net/play/home.asp?{query}")
}

#[test]
fn a_well_formed_launch_url_parses() {
    let launch = parse_launch(
        &prime_launch("host=storm.gs4.game.play.net&port=10024&key=abc123"),
        prime(),
    )
    .expect("a confirmed-shape launch url");

    assert_eq!(launch.port, 10024);
    assert_eq!(launch.key, "abc123");
}

#[test]
fn the_launch_key_is_not_in_the_debug_output() {
    // The key is the live game credential, exactly as `LaunchPayload`'s is.
    let launch = parse_launch(
        &prime_launch("host=storm.gs4.game.play.net&port=10024&key=SECRETKEY"),
        prime(),
    )
    .expect("parses");

    let rendered = format!("{launch:?}");
    assert!(
        !rendered.contains("SECRETKEY"),
        "the launch key reached Debug output: {rendered}"
    );
}

#[test]
fn a_host_that_is_not_the_pinned_one_is_refused() {
    // **The point of pinning.** By this stage the URL has come through a
    // redirect chain, and it decides where the client opens a socket and hands
    // over a key. Every GemStone row is confirmed live, so there is no reason
    // to accept anything else.
    let result = parse_launch(
        &prime_launch("host=evil.example.com&port=10024&key=abc"),
        prime(),
    );
    assert_eq!(
        result,
        Err(WebLoginFailure::UnexpectedResponse(
            "launch url: host or port is not this instance's pinned pair"
        ))
    );
}

#[test]
fn a_trusted_host_on_the_wrong_port_is_refused() {
    // The pair must match whole. `storm.gs4.game.play.net` is also Shattered's
    // host on a different port, so a host-only check would let a Prime login
    // land on the wrong instance.
    assert!(
        parse_launch(
            &prime_launch("host=storm.gs4.game.play.net&port=10324&key=abc"),
            prime(),
        )
        .is_err()
    );
}

#[test]
fn a_duplicated_parameter_is_refused() {
    // **The subtlest check here.** Last-value-wins is the ordinary behaviour of
    // a query parser, and it would let a repeated parameter silently override
    // the real one -- appending `&host=evil` to a legitimate URL. Lich's
    // `DUPLICATE_QUERY_PARAM`.
    for query in [
        "host=storm.gs4.game.play.net&port=10024&key=abc&host=evil.example.com",
        "host=storm.gs4.game.play.net&port=10024&key=abc&key=other",
        "host=storm.gs4.game.play.net&port=10024&port=10324&key=abc",
    ] {
        assert_eq!(
            parse_launch(&prime_launch(query), prime()),
            Err(WebLoginFailure::UnexpectedResponse(
                "launch url: duplicate host, port or key"
            )),
            "a duplicate survived in {query:?}"
        );
    }
}

#[test]
fn a_blank_or_missing_value_is_refused() {
    for query in [
        "host=&port=10024&key=abc",
        "host=storm.gs4.game.play.net&port=10024&key=",
        "host=storm.gs4.game.play.net&key=abc",
        "",
    ] {
        assert!(
            parse_launch(&prime_launch(query), prime()).is_err(),
            "{query:?} was accepted"
        );
    }
}

#[test]
fn a_url_with_no_query_is_refused() {
    assert!(parse_launch("https://www.play.net/play/home.asp", prime()).is_err());
}

#[test]
fn a_percent_encoded_host_still_has_to_match() {
    // Decoding happens BEFORE the comparison, so an encoded spelling of the
    // right host is accepted and an encoded spelling of a wrong one is not --
    // encoding must not be a way past the pin.
    let encoded = parse_launch(
        &prime_launch("host=storm%2Egs4%2Egame%2Eplay%2Enet&port=10024&key=abc"),
        prime(),
    );
    assert!(encoded.is_ok(), "an encoded form of the pinned host failed");

    assert!(
        parse_launch(
            &prime_launch("host=evil%2Eexample%2Ecom&port=10024&key=abc"),
            prime(),
        )
        .is_err()
    );
}

// ---------------------------------------------------------------------------
// Failure classification -- what the supervisor acts on
// ---------------------------------------------------------------------------

// The fatality of each variant is asserted at its one real decision point --
// `eaccess::fallback`'s tests -- rather than here. See `failure.rs`'s note on
// why there is no `is_fatal`: a predicate over all four variants answers a
// question no caller asks, and answering it here would re-create it in
// assertion form.

#[test]
fn a_failure_message_never_carries_the_response_body() {
    // `UnexpectedResponse` takes a `&'static str`, so the TYPE enforces this --
    // there is nowhere for a body to go. Asserted anyway, because the guarantee
    // is what lets this be logged freely.
    let failure = WebLoginFailure::UnexpectedResponse("character list: not 200");
    let rendered = format!("{failure}");
    assert!(rendered.contains("character list"), "{rendered}");
    assert!(!rendered.contains("<html"), "{rendered}");
}

// ---------------------------------------------------------------------------
// The selection chain's redirects -- a security boundary too (finding 3)
// ---------------------------------------------------------------------------

#[test]
fn the_real_final_redirect_ends_the_chain() {
    let url = prime_launch("host=storm.gs4.game.play.net&port=10024&key=abc123");
    assert_eq!(next_hop(&url), Ok(Hop::Launch(url.clone())));
}

#[test]
fn a_relative_hop_is_followed_on_play_net() {
    // The confirmed chain is relative: goplay2.asp -> playing_web.asp -> ...
    assert_eq!(
        next_hop("/includes/common/play/playing_web.asp"),
        Ok(Hop::Follow(
            "https://www.play.net/includes/common/play/playing_web.asp".to_owned()
        ))
    );
}

#[test]
fn a_redirect_off_play_net_is_refused_not_followed() {
    // **The defect.** Every one of these was followed before: the loop took
    // any `Location` starting with `http` verbatim, cookie jar attached.
    for hostile in [
        // Off-site, with a launch-shaped query so the old recogniser took it.
        "https://evil.example/play/home.asp?host=h&port=1&key=k",
        // The downgrade Lich names: plain http to the right host and path.
        "http://www.play.net/play/home.asp?host=h&port=1&key=k",
        // A lookalike host, and userinfo smuggling the real one in front.
        "https://www.play.net.evil.example/play/home.asp",
        "https://www.play.net@evil.example/play/home.asp",
        "https://user@www.play.net/play/home.asp",
        // The right origin on the wrong port.
        "https://www.play.net:8443/play/home.asp",
        // A scheme-relative "path" that resolves to another host.
        "//evil.example/includes/common/play/playing_web.asp",
        // An absolute URL that is not the final page. Lich treats EVERY
        // absolute Location as the end of the chain, and pins its path.
        "https://www.play.net/includes/common/play/playing_web.asp",
    ] {
        assert_eq!(
            next_hop(hostile),
            Err(WebLoginFailure::UnexpectedResponse(
                "character selection: untrusted redirect"
            )),
            "{hostile} was not refused"
        );
    }
}

#[test]
fn the_trusted_origin_is_matched_case_insensitively() {
    // A URL's scheme and host are case-insensitive; `Url` normalises both, so
    // a server that capitalises them is not mistaken for an attacker.
    let url = "HTTPS://WWW.PLAY.NET/play/home.asp?host=h&port=1&key=k";
    assert_eq!(next_hop(url), Ok(Hop::Launch(url.to_owned())));
}
