//! The handshake, driven end to end over a scripted server. No network.
//!
//! # Why these exist when `tests/eaccess_*.rs` already do
//!
//! Every test there calls a PURE function -- `hash_password`, `redact`,
//! `describe_launch_refusal` -- and asserts what it returns. None of them can
//! see what the handshake does with that function, and three review findings
//! lived exactly in that gap:
//!
//! - **PL-3's guard could not fail.** "The key is used whole" was pinned by
//!   calling `hash_password` with a whole key. The trim it forbids lived at the
//!   CALL SITE in `prove_identity`, so putting `.trim_ascii()` back there left
//!   the whole suite green (finding 4).
//! - **The `M` refusal said FATAL and was not** (finding 1). The comment was
//!   right, the code never called `.fatal()`, and no test built the error the
//!   handshake builds.
//! - **An unrecognised `L` reached stderr unredacted** (finding 2), through a
//!   function whose own tests only ever fed it `PROBLEM` lines.
//!
//! So these drive [`converse`] -- the conversation `authenticate` runs, minus
//! the TLS connect -- over [`AnsweringSource`], which answers each command with
//! a scripted reply and records every byte written. `CLAUDE.md`'s rule is
//! untouched: nothing here opens a socket, and `authenticate` itself is still
//! never called.

use super::converse;
use crate::answering::{AnsweringSource, TranscriptHandle};
use crate::eaccess::wire::{Credentials, EaccessError, LaunchPayload, hash_password};

/// Test credentials. Nothing real: `CLAUDE.md` forbids them in the tree.
///
/// The password is DIGITS on purpose. `TranscriptHandle::lines` reads the
/// written bytes through `from_utf8_lossy`, so a hashed byte above 0x7F would
/// collapse to U+FFFD and two different wrong hashes could compare equal. With
/// `p - 32` in `0x10..=0x19` and every key byte below `0x40`, each hashed byte
/// is `((p - 32) ^ k) + 32`, which stays in printable ASCII -- so the
/// comparison below is byte-exact, not merely lossy-equal.
fn creds(game_code: &str) -> Credentials<'_> {
    Credentials {
        account: "TESTACCT",
        password: "0123456789",
        character: "Testchar",
        game_code,
    }
}

/// A 32-byte key whose FIRST byte is `0x20`.
///
/// The exact case PL-3 is about: 32 bytes of random binary in which a leading
/// space is data, not framing (`plan/10:1734`, `:1749`). The rest are distinct
/// so a one-byte shift changes every hashed byte, not just the first.
fn key_with_leading_space() -> Vec<u8> {
    let mut key = vec![b' '];
    key.extend(1u8..32);
    key
}

/// The server's side of a successful login on `GS3`, with `key` as `K`'s reply.
///
/// The `A` reply is the source's DEFAULT reply rather than a scripted one,
/// because its command line carries the hash and so cannot be matched by an
/// exact string. Every other command's reply is matched on the command sent,
/// which is itself an assertion: if the handshake sends `F\tgs3`, nothing
/// answers it and the read times out naming the stage.
fn scripted_server(key: &[u8], l_reply: &str) -> (AnsweringSource, TranscriptHandle) {
    let (source, script) =
        AnsweringSource::new(b"A\tTESTACCT\tKEY\t0123456789abcdef0123456789abcdef\tTest Holder");
    script.answer("K", key);
    // Instance NAMES are placeholders: `cena-arch-tests` keeps game names in
    // the game modules, and nothing here reads them.
    script.answer("M", b"M\tGS3\tPrime\tGST\tTest");
    script.answer("F\tGS3", b"F\tNORMAL");
    script.answer("G\tGS3", b"G\tPrime\t0\t");
    script.answer("P\tGS3", b"P\tGS3\t1495\tGS3.P\t2500");
    script.answer("C", b"C\t1\t1\t0\t0\tW_TESTACCT_000\tTestchar");
    script.answer("L\tW_TESTACCT_000\tSTORM", l_reply.as_bytes());
    (source, script)
}

const L_OK: &str = "L\tOK\tUPPORT=5535\tGAME=STORM\tGAMECODE=GS\tFULLGAMENAME=Prime\t\
                    GAMEFILE=STORM.EXE\tGAMEHOST=gamehost.example.net\tGAMEPORT=10024\t\
                    KEY=0123456789abcdef0123456789abcdef";

/// Run the conversation, collecting the progress lines it would have printed.
async fn run(
    source: &mut AnsweringSource,
    creds: Credentials<'_>,
) -> (Result<LaunchPayload, EaccessError>, Vec<String>) {
    let mut printed = Vec::new();
    let result = converse(source, creds, &mut |line: &str| {
        printed.push(line.to_owned());
    })
    .await;
    (result, printed)
}

#[tokio::test(start_paused = true)]
async fn the_a_request_carries_the_hash_of_the_whole_key_as_sent() {
    // **Finding 4, closed at the call site.** The key's first byte is 0x20.
    // What reaches the wire must be the hash against all 32 bytes -- so a trim
    // anywhere between the read and the XOR changes this line, and fails it.
    let key = key_with_leading_space();
    let (mut source, script) = scripted_server(&key, L_OK);

    let (result, _) = run(&mut source, creds("GS3")).await;
    let launch = result.expect("the scripted login must succeed");
    assert_eq!(launch.gamehost, "gamehost.example.net");

    let whole = hash_password(b"0123456789", &key).expect("the whole key hashes");
    let trimmed = hash_password(b"0123456789", &key[1..]).expect("the trimmed key hashes");
    assert_ne!(
        whole, trimmed,
        "the fixture must make a trim VISIBLE, or this test proves nothing"
    );
    let expected = format!(
        "A\tTESTACCT\t{}",
        String::from_utf8(whole).expect("the digits password hashes to ASCII")
    );

    let lines = script.lines();
    assert_eq!(lines.first().map(String::as_str), Some("K"));
    assert_eq!(
        lines.get(1),
        Some(&expected),
        "the A request is not the hash of the key AS SENT. The K reply is 32 \
         bytes of random binary with no terminator (plan/10:1734, :1749): a \
         leading 0x20 is data, and trimming it sends a wrong password -- a \
         FATAL stop and a bad-password strike on roughly one login in 64."
    );
}

#[tokio::test(start_paused = true)]
async fn a_game_code_the_server_does_not_offer_is_fatal() {
    // **Finding 1.** A mistyped code -- web login's spelling of Prime is the
    // likeliest. M does not list it. Transient here meant: fall back to web login,
    // which refuses it as `UnsupportedGameCode`, which does not stop the
    // ladder -- so a full login, password and all, every 30 seconds, forever.
    let (mut source, script) = scripted_server(&key_with_leading_space(), L_OK);

    let (result, _) = run(&mut source, creds("GSQ")).await;
    let error = result.expect_err("GSQ is not offered by M");

    assert_eq!(error.stage, "m_response");
    assert!(
        error.fatal,
        "a code the server's own instance list does not contain was marked \
         retryable. No retry adds it to the list; each one resends the \
         password. {error}"
    );
    // And it stopped AT M: nothing after it was sent against the wrong
    // instance. K, A, M -- three writes.
    assert_eq!(script.written_count(), 3, "{:?}", script.lines());
}

#[tokio::test(start_paused = true)]
async fn an_unrecognised_l_reply_is_redacted_before_it_is_quoted() {
    // **Finding 2.** Neither `L\tOK` nor `L\tPROBLEM`, and carrying a key --
    // the shape nobody planned for is the one nobody redacted. The detail goes
    // to stderr and into the session's `.log`.
    let l_reply = "L\tHUH\tGAMEHOST=h\tKEY=SECRET-LAUNCH-KEY";
    let (mut source, _script) = scripted_server(&key_with_leading_space(), l_reply);

    let (result, printed) = run(&mut source, creds("GS3")).await;
    let error = result.expect_err("an L that is not OK is a refusal");

    assert_eq!(error.stage, "l_response");
    assert!(
        !error.detail.contains("SECRET-LAUNCH-KEY"),
        "the launch key reached the error detail in the clear: {}",
        error.detail
    );
    assert!(
        error.detail.contains("KEY=<REDACTED>"),
        "redaction must blank the value and keep the shape, so the reply is \
         still diagnosable: {}",
        error.detail
    );
    assert!(
        printed
            .iter()
            .all(|line| !line.contains("SECRET-LAUNCH-KEY")),
        "{printed:?}"
    );
}
