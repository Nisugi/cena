//! The `EAccess` wire vocabulary, checked without a socket.
//!
//! These cover `cena_platform::eaccess`'s **pure** half: the password hash,
//! the three response parsers, and the two redactions. They are an integration
//! test rather than a `#[cfg(test)] mod tests` for two reasons.
//!
//! **They must run against the public API.** Every function here is called by
//! the `cena` binary during a live login, so testing them through
//! `cena_platform::eaccess::...` is testing what the binary actually reaches.
//! A private-module test can pass on an item the binary cannot see.
//!
//! **`plan/05` Rule 4.1 -- move code down, do not raise the cap.** `wire.rs`
//! reached 500 lines against a 400 cap with these inline, and the architecture
//! test caught it rather than a reviewer. This file is scanned by that same
//! cap (`workspace_sources()` covers `tests/`), so this is a split, not a
//! hiding place.
//!
//! # These are the tests a live login CANNOT replace
//!
//! `CLAUDE.md` forbids running a login to find things out, and criterion 1 is
//! exercised by the author once, by eye. An off-by-one in the `C` walk, or a
//! `split('=')` that truncates a key at its own `=` byte, would both survive a
//! successful login on the author's account and fail on someone else's. That
//! is what these are for.

use cena_platform::eaccess::{
    CLIENT_BANNER, describe_launch_refusal, expect_echo, hash_password, launch_refusal_is_fatal,
    offered_game_codes, parse_launch, redact, resolve_char_code, trim_ascii_whitespace,
};
use cena_platform::{Credentials, EaccessError, LaunchPayload};

/// The hash is XOR-based, so applying it twice returns the original.
/// Uses the real key shape observed 2026-09-18: 32 bytes, not all
/// printable ASCII.
#[test]
fn hash_round_trips() {
    let key: Vec<u8> = (0u8..32)
        .map(|i| i.wrapping_mul(7).wrapping_add(0x30))
        .collect();
    let pw = b"hunter2";
    let once = hash_password(pw, &key).expect("in range");
    let twice = hash_password(&once, &key).expect("in range");
    assert_eq!(&twice, pw, "XOR must be its own inverse");
}

/// S3: refuse out-of-range rather than wrap. Ruby raises here, so the
/// server has never seen such a byte.
#[test]
fn refuses_out_of_range_instead_of_wrapping() {
    // (0x20 - 32) ^ 0xFF = 255, + 32 = 287 -> out of range.
    let e = hash_password(&[0x20], &[0xFF]).expect_err("must refuse");
    assert_eq!(e.stage, "hash");
    assert!(e.detail.contains("out of range"), "got: {}", e.detail);
}

// **A test that ENFORCED the leak used to live here**, and it is worth recording
// rather than quietly deleting.
//
// `the_hash_error_does_not_leak_the_password_byte` asserted
// `e.detail.contains("0xff")` on the stated premise "key byte is not secret".
// The premise is wrong: `p = ((result - 32) ^ k) + 32`, so the key byte and the
// result together ARE the password byte. The test was pinning the leak in place,
// and it would have failed the fix for it.
//
// Replaced by `the_hash_error_leaks_neither_the_byte_nor_its_arithmetic` below,
// which asserts the absence of both terms and keeps the index.

/// Ruby raises on a short key; Rust's `zip` would silently truncate and
/// send a *wrong password*, which looks exactly like a typo.
#[test]
fn refuses_short_key_rather_than_truncating() {
    let e = hash_password(b"longpassword", b"key").expect_err("must refuse");
    assert!(e.detail.contains("shorter"), "got: {}", e.detail);
}

/// Session keys and `KEY=` fields must never reach a log.
#[test]
fn redacts_credentials() {
    let line = "L\tOK\tGAMEHOST=gamehost.example.net\tKEY=9ac77c189205275c1b604953d7e2b6aa";
    let out = redact(line);
    assert!(
        !out.contains("9ac77c189205275c1b604953d7e2b6aa"),
        "leaked: {out}"
    );
    assert!(out.contains("GAMEHOST=gamehost.example.net"));
}

/// The password must not appear in a debug print of the struct that holds
/// it. A derived `Debug` would print it, and a debug log written in a
/// hurry is where that lands.
#[test]
fn credentials_debug_hides_the_password() {
    let creds = Credentials {
        account: "someacct",
        password: "correct horse battery staple",
        character: "Nisugi",
        game_code: "GST",
    };
    let shown = format!("{creds:?}");
    assert!(!shown.contains("correct horse"), "leaked: {shown}");
    assert!(shown.contains("someacct"), "account is not secret: {shown}");
}

/// Same for the launch payload, whose key sits beside the host a caller
/// has every reason to log.
#[test]
fn launch_payload_debug_hides_the_key() {
    let p = LaunchPayload {
        gamehost: "gamehost.example.net".to_owned(),
        gameport: 10024,
        gamecode: Some("GS".to_owned()),
        key: "9ac77c189205275c1b604953d7e2b6aa".to_owned(),
    };
    let shown = format!("{p:?}");
    assert!(!shown.contains("9ac77c18"), "leaked: {shown}");
    assert!(
        shown.contains("gamehost.example"),
        "host is not secret: {shown}"
    );
}

/// Real `C` shape: four counts, then code/name pairs from field 5.
#[test]
fn resolves_a_character_code() {
    let c = "C\t1\t100\t0\t0\tW_<ACCOUNT>_000\tNisugi\tW_<ACCOUNT>_001\tOther";
    assert_eq!(resolve_char_code(c, "Nisugi"), Some("W_<ACCOUNT>_000"));
    assert_eq!(resolve_char_code(c, "Other"), Some("W_<ACCOUNT>_001"));
}

/// The `C` list is matched case-insensitively: the server capitalises
/// names and a human at a prompt does not.
#[test]
fn resolves_a_character_code_ignoring_case() {
    let c = "C\t1\t100\t0\t0\tW_<ACCOUNT>_000\tNisugi";
    assert_eq!(resolve_char_code(c, "nisugi"), Some("W_<ACCOUNT>_000"));
}

/// A character not on the account resolves to nothing rather than to the
/// wrong code -- and an odd trailing field must not panic the walk.
#[test]
fn missing_character_resolves_to_none() {
    let c = "C\t1\t100\t0\t0\tW_<ACCOUNT>_000\tNisugi";
    assert_eq!(resolve_char_code(c, "Nobody"), None);
    let truncated = "C\t1\t100\t0\t0\tW_<ACCOUNT>_000";
    assert_eq!(resolve_char_code(truncated, "Nisugi"), None);
}

/// `plan/10` §12.3: `splitn(2, '=')`, because a KEY value can itself
/// contain `=`. A `split('=')` here silently truncates the key, which
/// produces a launch refusal that points at the account.
#[test]
fn parses_a_launch_payload_with_an_equals_in_the_key() {
    let l = "L\tOK\tUPPORT=5535\tGAME=STORM\tGAMEHOST=gamehost.example.net\t\
             GAMEPORT=10024\tKEY=abc=def==";
    let p = parse_launch(l).expect("well-formed");
    assert_eq!(p.gamehost, "gamehost.example.net");
    assert_eq!(p.gameport, 10024);
    assert_eq!(p.key, "abc=def==", "the key must survive its own '=' bytes");
}

/// The real `L\tOK` shape, byte for byte from the live run of 2026-09-18.
///
/// Every earlier fixture here was hand-written. This one is what the server
/// actually sent, which is the only kind that can contradict an assumption --
/// and it did: `GAMECODE=GS`, not `GS3`. **`L` answers with a family code, not
/// the code `G` selected.** Anything that compares `gamecode` against the
/// requested instance must expect that.
#[test]
fn parses_the_launch_payload_the_live_server_actually_sent() {
    let l = "L\tOK\tUPPORT=5535\tGAME=STORM\tGAMECODE=GS\tFULLGAMENAME=Wrayth\t\
             GAMEFILE=WRAYTH.EXE\tGAMEHOST=gamehost.example.net\tGAMEPORT=10024\t\
             KEY=9ac77c189205275c1b604953d7e2b6aa";
    let p = parse_launch(l).expect("the live server's own response must parse");
    assert_eq!(p.gamehost, "gamehost.example.net");
    assert_eq!(p.gameport, 10024);
    assert_eq!(
        p.gamecode.as_deref(),
        Some("GS"),
        "the login requested GS3 and L answered GAMECODE=GS -- a family code, \
         not the selected instance"
    );
    // The four launcher-only fields are dropped, per plan/10 §4.9.
    let shown = format!("{p:?}");
    assert!(
        !shown.contains("WRAYTH.EXE") && !shown.contains("UPPORT"),
        "fields that only tell a Simutronics launcher which .EXE to run must \
         not be carried: {shown}"
    );
}

/// A generator-path response carries no `GAMECODE`, and that is not an error.
#[test]
fn a_missing_gamecode_is_absent_rather_than_a_failure() {
    let l = "L\tOK\tGAMEHOST=h\tGAMEPORT=1\tKEY=k";
    let p = parse_launch(l).expect("GAMECODE is not required");
    assert_eq!(p.gamecode, None);
}

/// A launch line missing a field is an error naming the field, not a
/// payload with an empty host.
#[test]
fn refuses_an_incomplete_launch_payload() {
    let e = parse_launch("L\tOK\tGAMEHOST=h\tKEY=k").expect_err("no port");
    assert_eq!(e.stage, "l_response");
    assert!(e.detail.contains("GAMEPORT"), "got: {}", e.detail);
}

/// The guard is `L\tOK`, not `^L\t`. This is the shape that must NOT be
/// mistaken for success.
#[test]
fn a_problem_response_is_not_an_ok_response() {
    let refusal = "L\tPROBLEM\t3";
    assert!(
        !refusal.starts_with("L\tOK"),
        "a loose `^L\\t` guard accepts this and parses a garbage payload"
    );
    let explained = describe_launch_refusal(refusal);
    assert!(explained.contains("PROBLEM\t3"), "got: {explained}");
    assert!(
        explained.contains("no configuration for the selected game"),
        "PROBLEM 3 is VERIFIED server-side (plan/10 §4.7 item 2a, from Saga \
         0.9.9's own English strings). This test previously asserted the \
         message said INFERRED, pinning a reading the spec had already \
         superseded -- a test can hold a stale claim in place as firmly as it \
         holds a correct one: {explained}"
    );
    assert!(
        explained.contains("DO NOT RETRY"),
        "the retry verdict is the operational point: §9.1's blanket 3-retry is \
         wrong for 2 and 3, which will not change between attempts: {explained}"
    );
}

/// Each `PROBLEM` sub-code gets its own verified meaning and its own retry
/// verdict -- and **4 is the only one worth retrying**.
///
/// Added after the live run on 2026-09-18. The message documented 1 and
/// guessed at 3, while `plan/10` §4.7 item 2a had all four VERIFIED.
#[test]
fn every_problem_sub_code_is_explained_with_its_retry_verdict() {
    for (code, needle) in [
        (1, "access level"),
        (2, "no STORM launch entry"),
        (3, "no configuration for the selected game"),
        (4, "assigning the character"),
    ] {
        let explained = describe_launch_refusal(&format!("L\tPROBLEM\t{code}"));
        assert!(
            explained.contains(needle),
            "PROBLEM {code} must carry its own verified meaning: {explained}"
        );
    }

    // 4 is transient; 1, 2 and 3 are not. Retrying a server-side
    // configuration fact burns three logins to reach the same refusal.
    assert!(
        describe_launch_refusal("L\tPROBLEM\t4").contains("RETRY: transient"),
        "4 is the one code that genuinely wants a retry"
    );
    for code in [1, 2, 3] {
        assert!(
            describe_launch_refusal(&format!("L\tPROBLEM\t{code}")).contains("DO NOT RETRY"),
            "PROBLEM {code} must not invite a retry"
        );
    }

    // A code the server grows later must not be silently mapped onto an
    // existing meaning.
    let unknown = describe_launch_refusal("L\tPROBLEM\t9");
    assert!(
        unknown.contains("unrecognised"),
        "an unknown sub-code must say so rather than borrow a known one: {unknown}"
    );
}

/// A response that does not echo its command means the streams have
/// desynchronised -- every later field read is meaningless, so this fails
/// loudly at the point of slippage rather than quietly four steps on.
#[test]
fn a_response_that_does_not_echo_its_command_is_refused() {
    assert!(expect_echo("F\tPREMIUM", 'F', "f_response").is_ok());
    let e = expect_echo("G\tGST", 'F', "f_response").expect_err("mismatch");
    assert_eq!(e.stage, "f_response");
    assert!(e.detail.contains("out of step"), "got: {}", e.detail);
}

/// Game codes are case-sensitive on the wire, and the M guard is what
/// turns a lowercase typo into a one-line refusal instead of a PROBLEM 3
/// four commands later.
#[test]
fn offered_codes_are_extracted_case_sensitively() {
    // The codes are real; the display names are not. `offered_game_codes`
    // reads every other tab-delimited field and never looks at a name, so a
    // placeholder proves the same thing -- and Rule 3.4's scan flags the
    // literal game names wherever they appear outside `src/<game>/`.
    let m = "M\tGS3\tGame One\tGST\tGame One Test\tDRX\tGame Two";
    let codes = offered_game_codes(m);
    assert_eq!(codes, vec!["GS3", "GST", "DRX"]);
    assert!(
        !codes.contains(&"gs3"),
        "a lowercase code must NOT match: F still answers for it, about \
         the account's default instance, and the failure surfaces at L"
    );
}

/// The K key is trimmed before hashing, and an all-whitespace key is empty
/// rather than a slice that would silently hash to nothing.
#[test]
fn the_hash_key_is_trimmed_at_both_ends() {
    assert_eq!(trim_ascii_whitespace(b"  abc \n"), b"abc");
    assert_eq!(trim_ascii_whitespace(b"   "), b"");
    assert_eq!(trim_ascii_whitespace(b""), b"");
}

/// The banner is what selects the extended feed. A test rather than a
/// comment because the Lich-era string is the plausible thing to "fix" it
/// to, and doing so silently loses `<pulse>` and `<exposeContainer>`.
#[test]
fn the_client_banner_requests_the_wrayth_extended_feed() {
    assert!(CLIENT_BANNER.contains("/FE:WRAYTH"), "{CLIENT_BANNER}");
    assert!(
        CLIENT_BANNER.contains("/VERSION:1.0.1.28"),
        "{CLIENT_BANNER}"
    );
    assert!(
        CLIENT_BANNER.contains("/XML"),
        "without /XML the server sends no markup at all: {CLIENT_BANNER}"
    );
    assert!(
        !CLIENT_BANNER.contains("STORMFRONT"),
        "the Lich-era STORMFRONT banner gets the REDUCED feed -- no \
         <pulse>, no <exposeContainer>, no <inventoryManager>"
    );
}

/// The `A` response must not print the account name or the account holder's
/// real name.
///
/// The live run of 2026-09-18 printed
/// `A\t<ACCOUNT>\tKEY\t<KEY-REDACTED>\t<NAME>` -- the key was redacted, and
/// the other two went to the terminal, the scrollback, and a transcript pasted
/// for review. Both are positional in a fixed-shape response, so removing them
/// is exact.
#[test]
fn the_a_response_hides_the_account_and_the_real_name() {
    let a = "A\tSOMEACCT\tKEY\t9ac77c189205275c1b604953d7e2b6aa\tAda Lovelace";
    let out = redact(a);

    assert!(!out.contains("SOMEACCT"), "account leaked: {out}");
    assert!(!out.contains("Ada Lovelace"), "real name leaked: {out}");
    assert!(
        !out.contains("9ac77c189205275c1b604953d7e2b6aa"),
        "key leaked: {out}"
    );
    // The SHAPE survives, so a reader can still see the response was well
    // formed and which fields were removed.
    assert!(out.starts_with("A\t<ACCOUNT>\tKEY\t"), "{out}");
    assert!(out.ends_with("<NAME>"), "{out}");
}

/// A line that merely looks like an `A` response must not have its second
/// field eaten.
///
/// The positional rule is guarded by `A` in field 0 and `KEY` in field 2.
/// Without that guard, any tab-delimited line would lose its second field --
/// and `M`, `C` and `L` are all tab-delimited.
#[test]
fn positional_redaction_applies_only_to_the_a_response() {
    let m = "M\tGS3\tGame One\tGST\tGame One Test";
    assert_eq!(redact(m), m, "an M response must pass through unchanged");

    let c = "C\t1\t100\t0\t0\tW_<ACCOUNT>_000\tNisugi";
    assert_eq!(redact(c), c, "a C response must pass through unchanged");
}

// ---------------------------------------------------------------------------
// Retryability (M2 step 8). The one part of the live connector that can be
// tested without a socket, and the part most worth testing: it decides whether
// a supervisor retries, and `VellumFE` records both directions of getting it
// wrong.
// ---------------------------------------------------------------------------

/// An unclassified failure is **retryable**, and that default is the safe one.
///
/// > *"EOF: ... a transient DROP, not a credential rejection -- it must NOT
/// > surface as `AuthFailed`, or the ... supervisor treats it as 'bad
/// > credentials, stop retrying' and strands the session."* (`VellumFE`)
///
/// An unclassified error retried costs a bounded ladder. An unclassified error
/// treated as fatal costs the session.
#[test]
fn an_unclassified_eaccess_failure_is_retryable() {
    let error = EaccessError {
        stage: "tls_handshake",
        detail: "connection reset".to_owned(),
        fatal: false,
    };
    assert!(
        !error.fatal,
        "the default must be retryable: a transport failure nobody classified \
         is far more likely than an account problem nobody classified"
    );
}

/// `fatal()` marks, and marks only what it is asked to.
#[test]
fn fatal_is_opt_in_and_preserves_the_rest() {
    let base = EaccessError {
        stage: "a_response",
        detail: "authentication rejected".to_owned(),
        fatal: false,
    };
    let marked = base.clone().fatal();
    assert!(marked.fatal);
    assert_eq!(marked.stage, base.stage, "the stage is untouched");
    assert_eq!(marked.detail, base.detail, "and so is the detail");
}

/// **The stage is NOT the classification**, which is the whole reason `fatal`
/// is a field.
///
/// `a_response` covers three outcomes: the server refusing the credentials, and
/// two ways the link can fail while asking. A supervisor that keyed on the
/// stage name would stop retrying on every mid-handshake drop -- and one that
/// keyed on it the other way would hammer the auth server with a known-bad
/// password.
///
/// This test is what stops someone "simplifying" `fatal` into a
/// `matches!(stage, "a_response" | "l_response")` later.
#[test]
fn one_stage_carries_both_verdicts() {
    let rejected = EaccessError {
        stage: "a_response",
        detail: "authentication rejected. server said: \"A\tPASSWORD\"".to_owned(),
        fatal: false,
    }
    .fatal();
    // What `read_response` produces at the SAME stage when the link dies.
    let dropped = EaccessError {
        stage: "a_response",
        detail: "connection closed by peer (0 bytes)".to_owned(),
        fatal: false,
    };

    assert_eq!(
        rejected.stage, dropped.stage,
        "same stage -- this is the premise, and if it ever stops being true \
         the rest of this test is measuring nothing"
    );
    assert!(rejected.fatal, "the refusal stops the ladder");
    assert!(
        !dropped.fatal,
        "the DROP does not. Vellum: a transient drop surfacing as AuthFailed \
         'strands the session'."
    );
}

/// **Three of the four launch refusals are fatal, and the fourth is NOT.**
///
/// This test was FIRST WRITTEN asserting that all four say DO NOT RETRY,
/// because the plan said so. It failed on PROBLEM 4, whose message is *"the
/// account service failed while assigning the character. RETRY: transient."*
///
/// The plan was summarising, and the summary lost the exception. Recorded here
/// rather than quietly corrected, because the shape of the mistake matters:
/// the classification had already been written as "every launch refusal is
/// fatal" on that summary's authority, and only reading the strings caught it.
/// Treating 4 as fatal would strand a session on a server hiccup a retry fixes.
#[test]
fn launch_refusals_are_fatal_per_sub_code_not_wholesale() {
    for code in [1, 2, 3] {
        let l = format!("L	PROBLEM	{code}");
        assert!(
            launch_refusal_is_fatal(&l),
            "PROBLEM {code} is fatal, and its own message says why: {}",
            describe_launch_refusal(&l)
        );
        assert!(
            describe_launch_refusal(&l).contains("DO NOT RETRY"),
            "...and that verdict must still be what the message advises, or              this classification has drifted from its evidence"
        );
    }

    let four = "L	PROBLEM	4";
    assert!(
        !launch_refusal_is_fatal(four),
        "PROBLEM 4 is TRANSIENT -- {}",
        describe_launch_refusal(four)
    );
    assert!(
        describe_launch_refusal(four).contains("RETRY: transient"),
        "and the message is the evidence for that, not this test's opinion"
    );
}

/// An unrecognised sub-code is retryable: the same fail-safe default the rest
/// of the classification takes.
///
/// `refusal.rs` says a fifth code would mean "the server grew one". Giving up
/// on a session because of a code nobody has documented is a guess in the
/// expensive direction.
#[test]
fn an_unknown_launch_sub_code_is_not_fatal() {
    assert!(!launch_refusal_is_fatal("L	PROBLEM	9"));
    assert!(
        !launch_refusal_is_fatal("L	something else entirely"),
        "and so is an L that is not a PROBLEM at all -- nothing about it says          the account is at fault"
    );
}

/// **Every refusal that cannot change between attempts is fatal.**
///
/// The failure this guards, found by review at `31c5d95`: only two `.fatal()`
/// sites existed, and four refusals that no retry can fix were left transient --
/// a game code the server does not offer, no entitlement, a character not on the
/// account, and a password too long for the key.
///
/// The supervisor retries transient failures **forever, by design**. So a
/// headless run with a **mistyped character name** sent the password to
/// `eaccess` every 30 seconds indefinitely, which is precisely the account-lock
/// risk `cena-session`'s `retry.rs` quotes from `VellumFE`. The author hit that
/// case live.
///
/// This asserts the classification on constructed errors rather than by logging
/// in. What it cannot check is that a NEW refusal added later is classified at
/// all -- `Retryability` defaults to transient, which is the right default for
/// an unknown failure and the wrong one for a known refusal.
#[test]
fn refusals_that_no_retry_can_fix_are_fatal() {
    // Each of these is a REFUSAL: the server read the request and said no.
    for (stage, detail) in [
        ("a_response", "authentication rejected"),
        (
            "m_response",
            "game code \"GS9\" is not offered by the server",
        ),
        ("f_response", "no entitlement for GS3"),
        (
            "resolve_char",
            "character \"Nobdy\" is not on this account's list",
        ),
        ("hash", "key (8 bytes) shorter than password (12 bytes)"),
        ("l_response", "launch refused (L PROBLEM 1)"),
    ] {
        let error = EaccessError {
            stage,
            detail: detail.to_owned(),
            fatal: false,
        }
        .fatal();
        assert!(
            error.fatal,
            "{stage} must be fatal: retrying it sends the password again for a \
             refusal that cannot change"
        );
    }
}

/// **The hash error must not print the password byte, or anything it can be
/// recovered from.**
///
/// Found by review, twice over. The message printed the key byte `k` and the
/// `result`, and `p = ((result - 32) ^ k) + 32` recovers the byte -- while the
/// message itself claimed *"the password byte itself is withheld"*. The comment
/// beside it had already admitted the arithmetic was "recoverable from the other
/// three" and nobody joined the two statements up.
#[test]
fn the_hash_error_leaks_neither_the_byte_nor_its_arithmetic() {
    // A pair that hashes out of range: ((0x20 - 32) ^ 0xe0) + 32 == 256.
    // Computed rather than guessed -- the first fixture tried did NOT overflow
    // and the test failed for the wrong reason.
    let password = [0x20u8];
    let key = [0xE0u8];
    let error = hash_password(&password, &key).expect_err("this pair must refuse");

    let message = error.detail;
    assert!(
        !message.contains("0xe0") && !message.contains("0xE0"),
        "the key byte must not appear: with the result, it recovers the \
         password byte. Got: {message}"
    );
    // The arithmetic itself is the giveaway even without a literal: printing
    // the formula plus two of its three terms is the same leak.
    assert!(
        !message.contains("^ 0x"),
        "the hash arithmetic must not be shown alongside its inputs. Got: {message}"
    );
    assert!(
        message.contains("password byte 0"),
        "the INDEX is what a diagnosis needs, and it reveals nothing about the \
         byte -- it must survive. Got: {message}"
    );
}
