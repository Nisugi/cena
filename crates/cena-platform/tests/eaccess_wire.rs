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
    CLIENT_BANNER, describe_launch_refusal, expect_echo, hash_password, offered_game_codes,
    parse_launch, redact, resolve_char_code, trim_ascii_whitespace,
};
use cena_platform::{Credentials, LaunchPayload};

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

/// The out-of-range error must not carry the password byte that caused it.
///
/// The spike printed `0x{p:02x}` -- one plaintext password byte, positioned.
/// That error reaches stderr through `main`'s `Box<dyn Error>`, so it landed
/// in scrollback and in any `2>` redirect. Found by adversarial review, with
/// `credentials_debug_hides_the_password` passing and asserting the opposite.
#[test]
fn the_hash_error_does_not_leak_the_password_byte() {
    // 0x61 ('a') against key 0xFF: (0x61 - 32) ^ 0xFF = 0x9E -> +32 = 190,
    // in range. Use a byte that actually overflows: (0x20 - 32) ^ 0xFF = 255.
    let e = hash_password(&[0x20], &[0xFF]).expect_err("must refuse");
    assert!(
        !e.detail.contains("0x20"),
        "the password byte must not appear in an error that reaches stderr: {}",
        e.detail
    );
    // The diagnosis must survive the redaction, or the fix traded one problem
    // for another.
    assert!(
        e.detail.contains("0xff"),
        "key byte is not secret: {}",
        e.detail
    );
    assert!(
        e.detail.contains("byte 0"),
        "the position must still be named: {}",
        e.detail
    );
}

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
    let c = "C\t1\t100\t0\t0\tW_ACCOUNT_000\tNisugi\tW_ACCOUNT_001\tOther";
    assert_eq!(resolve_char_code(c, "Nisugi"), Some("W_ACCOUNT_000"));
    assert_eq!(resolve_char_code(c, "Other"), Some("W_ACCOUNT_001"));
}

/// The `C` list is matched case-insensitively: the server capitalises
/// names and a human at a prompt does not.
#[test]
fn resolves_a_character_code_ignoring_case() {
    let c = "C\t1\t100\t0\t0\tW_ACCOUNT_000\tNisugi";
    assert_eq!(resolve_char_code(c, "nisugi"), Some("W_ACCOUNT_000"));
}

/// A character not on the account resolves to nothing rather than to the
/// wrong code -- and an odd trailing field must not panic the walk.
#[test]
fn missing_character_resolves_to_none() {
    let c = "C\t1\t100\t0\t0\tW_ACCOUNT_000\tNisugi";
    assert_eq!(resolve_char_code(c, "Nobody"), None);
    let truncated = "C\t1\t100\t0\t0\tW_ACCOUNT_000";
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
/// `A\tACCOUNT\tKEY\t<KEY-REDACTED>\tREAL NAME` -- the key was redacted, and
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

    let c = "C\t1\t100\t0\t0\tW_ACCOUNT_000\tNisugi";
    assert_eq!(redact(c), c, "a C response must pass through unchanged");
}
