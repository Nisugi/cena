//! The wire log's redaction, checked on the shapes the live server actually
//! sent.
//!
//! These exist because this is the one part of logging where being wrong is
//! not recoverable: a credential written to disk stays there, and no later fix
//! un-writes it. Everything else in `sink` fails loudly (a file that will
//! not open) or harmlessly (a missing line).
//!
//! The fixtures below are the real shapes from the live run of 2026-09-18,
//! with the author's own values replaced by stand-ins of the same form.

use cena_platform::Redactions;

/// The `A` response is the reason this module exists.
///
/// The live run printed `A\t<ACCOUNT>\tKEY\t<redacted>\t<NAME>`. The key
/// was already redacted for the terminal; **the account name and the account
/// holder's real name were not**, and would have reached disk verbatim.
#[test]
fn the_a_response_loses_the_account_and_the_real_name() {
    let mut r = Redactions::new();
    r.account("someacct");
    r.real_name("Ada Lovelace");

    let a = "A\tSOMEACCT\tKEY\t9ac77c189205275c1b604953d7e2b6aa\tAda Lovelace";
    let out = r.apply(a);

    assert!(!out.contains("SOMEACCT"), "account leaked: {out}");
    assert!(!out.contains("Ada Lovelace"), "real name leaked: {out}");
    assert!(out.contains("<ACCOUNT>"), "{out}");
    assert!(out.contains("<NAME>"), "{out}");
}

/// The server echoes the account **uppercased**, and the human types it
/// lowercase. A case-sensitive match would miss the one that reaches disk.
///
/// This is the whole reason `account()` registers both forms.
#[test]
fn the_account_is_redacted_whatever_case_the_server_echoes() {
    let mut r = Redactions::new();
    r.account("someacct");

    for form in ["someacct", "SOMEACCT"] {
        let out = r.apply(&format!("A\t{form}\tKEY\tabc"));
        assert!(
            !out.contains(form),
            "the {form:?} form survived: {out}. The account is typed in one \
             case and echoed in another; both reach disk."
        );
    }
}

/// The session key must not survive into the bytes file either.
///
/// It is short-lived, but it is a credential for as long as it is valid, and
/// the bytes file is the one that gets copied around to cut fixtures from.
#[test]
fn the_session_key_is_redacted_in_raw_wire_bytes() {
    let mut r = Redactions::new();
    r.key("9ac77c189205275c1b604953d7e2b6aa");

    let wire = b"L\tOK\tGAMEHOST=h\tGAMEPORT=1\tKEY=9ac77c189205275c1b604953d7e2b6aa\n";
    let out = r.apply_bytes(wire);
    let text = String::from_utf8_lossy(&out);

    assert!(!text.contains("9ac77c18"), "key leaked into bytes: {text}");
    assert!(text.contains("<KEY>"), "{text}");
    // The surrounding structure must survive, or the bytes file stops being
    // usable as replay input.
    assert!(text.contains("GAMEHOST=h"), "{text}");
    assert!(text.starts_with("L\tOK"), "{text}");
}

/// Bytes with no secret in them come back **byte-identical**.
///
/// This is load-bearing for criterion 7. The bytes file is replay input, and a
/// redactor that rewrote every chunk -- re-encoding UTF-8, normalising, or
/// altering a chunk boundary -- would silently change what a replay replays.
/// The overwhelming majority of chunks contain no credential at all, so the
/// no-match path is the one that must be exact.
#[test]
fn bytes_without_a_secret_are_returned_unchanged() {
    let mut r = Redactions::new();
    r.account("someacct");
    r.key("9ac77c189205275c1b604953d7e2b6aa");

    // A real chunk from the login burst, including a high byte that a lossy
    // round trip would mangle.
    let wire: &[u8] = b"<pushBold/>You also see a bent \xC2\xA0sign<popBold/>\r\n";
    let out = r.apply_bytes(wire);

    assert_eq!(
        out, wire,
        "a chunk containing no secret must be byte-identical, or the bytes \
         file stops reproducing the session it recorded"
    );
}

/// Nothing registered means nothing rewritten -- and `is_empty` says so, which
/// is what lets the log file's own header admit it is raw.
#[test]
fn an_empty_redaction_set_changes_nothing_and_admits_it() {
    let r = Redactions::new();
    assert!(r.is_empty());
    let wire = b"anything at all\n";
    assert_eq!(r.apply_bytes(wire), wire);
    assert_eq!(r.apply("anything at all"), "anything at all");
}

/// A very short secret is refused rather than applied.
///
/// A two-character account name would match inside half the words on the wire
/// and turn the log into unreadable noise -- which is a worse outcome than not
/// redacting a string that short, because an unreadable log gets deleted and
/// then there is no log at all.
#[test]
fn a_secret_too_short_to_be_safe_is_refused() {
    let mut r = Redactions::new();
    r.add("ab", "<X>");
    r.add("", "<X>");
    r.add("   ", "<X>");
    assert!(
        r.is_empty(),
        "secrets under three characters must not be registered"
    );
    assert_eq!(r.apply("a cab in the abbey"), "a cab in the abbey");
}

/// Whitespace around a typed credential must not defeat the match.
///
/// The prompt trims, but a credential arriving from a config file or an
/// environment variable might not, and a trailing space would make every
/// comparison fail silently.
#[test]
fn a_secret_is_trimmed_before_it_is_registered() {
    let mut r = Redactions::new();
    r.account("  someacct \n");
    let out = r.apply("A\tsomeacct\tKEY");
    assert!(
        !out.contains("someacct"),
        "untrimmed secret did not match: {out}"
    );
}

/// The bytes file rolls at the threshold, and the parts REASSEMBLE.
///
/// Rotation is easy to write and easy to get subtly wrong: a part boundary
/// that fell inside a chunk would produce a part beginning mid-tag, and
/// `Parser::push_bytes` buffers to a newline, so that part would parse
/// differently on its own than it did in the stream. The reassembly assertion
/// is what makes this a test of replayability rather than of file creation.
///
/// **It earned that immediately.** The first implementation left part 0
/// unnumbered (`Tester-stamp.bytes`, then `Tester-stamp-001.bytes`), which
/// sorts wrong: `-` (0x2D) precedes `.` (0x2E), so the first part sorted last
/// and the rejoined stream was chunks 4-9 followed by 0-3. Every part existed
/// and held the right bytes; only the order was wrong. A test that counted
/// files would have passed, and the corruption would have surfaced as an
/// unreplayable session weeks later.
#[test]
fn the_bytes_file_rolls_and_the_parts_reassemble() {
    let dir = std::env::temp_dir().join("cena-sink-rotation");
    let _ = std::fs::remove_dir_all(&dir);
    // The threshold is PASSED, not set in the environment: `unsafe_code =
    // "deny"` makes `set_var` unavailable, and a test that mutated global
    // state would race every other test in this binary.
    let mut sink = cena_platform::SessionSink::create_with_rotation(
        &dir,
        "Tester",
        "stamp",
        Redactions::new(),
        4,
    )
    .expect("the sink must open");

    // Ten chunks against a threshold of four: part 0, then -001, then -002.
    let mut expected = Vec::new();
    for i in 0..10u8 {
        let chunk = format!("chunk {i} with some payload\n");
        expected.extend_from_slice(chunk.as_bytes());
        sink.wire(true, chunk.as_bytes()).expect("write must work");
    }
    sink.flush().expect("flush must work");
    drop(sink);

    let mut parts: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("the directory must exist")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "bytes"))
        .collect();
    parts.sort();

    assert!(
        parts.len() >= 3,
        "ten chunks at a threshold of four must produce at least three parts, \
         got {}: {parts:?}",
        parts.len()
    );

    // THE PROPERTY THAT MATTERS: the parts in order are the original stream.
    let mut rejoined = Vec::new();
    for part in &parts {
        rejoined.extend_from_slice(&std::fs::read(part).expect("part must read"));
    }
    assert_eq!(
        rejoined, expected,
        "concatenating the parts in order must reproduce the stream exactly, \
         or a rolled session stops being usable as replay input"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **The redaction store must not print its own secrets.**
///
/// Found by review: `Redactions` derived `Debug`, so any `{:?}` on it -- or on a
/// `SessionSink`, or on a `SessionEnd` holding one -- dumped every registered
/// launch key in the clear. The type whose entire job is keeping secrets out of
/// files was printing them on request.
#[test]
fn the_redaction_store_does_not_print_its_secrets() {
    let mut redactions = Redactions::new();
    redactions.key("SUPERSECRETLAUNCHKEY");
    redactions.account("someaccount");

    let shown = format!("{redactions:?}");
    assert!(
        !shown.contains("SUPERSECRETLAUNCHKEY"),
        "the launch key must not appear in Debug output: {shown}"
    );
    assert!(
        !shown.contains("someaccount"),
        "nor the account name: {shown}"
    );
    // THREE, not two: `account()` registers the typed form and its uppercase
    // echo, because the server sends the account name uppercased in `A`.
    assert!(
        shown.contains("3 registered"),
        "the COUNT is what a reader legitimately wants, and it must survive: \
         {shown}"
    );
}
