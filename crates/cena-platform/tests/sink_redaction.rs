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

/// **The log header must not contradict the log body.**
///
/// FOUND 2026-09-19 by reading the log from the first live web-login run. The
/// header said *"Credentials ... are redacted: NO -- nothing was registered,
/// treat this file as raw"*, and eleven lines later the same file said
/// `redaction registered (session key)`.
///
/// It was not an edge case: the launch key is minted by the connect and
/// registered afterwards, so the creation-time set is empty in **every live
/// session**. The header was therefore wrong every time, and wrong in the
/// direction that matters -- it tells a future reader to treat a redacted log
/// as raw, which is how a log gets shared that should not be.
///
/// No test asserted the header, which is why it shipped. This is that test.
#[test]
fn the_header_does_not_claim_nothing_is_redacted_when_a_key_arrives_later() {
    let dir = std::env::temp_dir().join("cena-sink-header");
    let _ = std::fs::remove_dir_all(&dir);
    // An EMPTY set at creation, which is exactly the live case.
    let mut sink = cena_platform::SessionSink::create(&dir, "Tester", "stamp", Redactions::new())
        .expect("the sink must open");

    // The key arrives after the file exists, as it does on every connect.
    sink.redact_key("a-launch-key-long-enough-to-register");
    sink.flush().expect("flush must work");
    let path = sink.events_path().to_owned();
    drop(sink);

    let log = std::fs::read_to_string(&path).expect("the log must be readable");
    let (header, body) = log
        .split_once("redaction registered")
        .expect("the in-band marker must be present");

    assert!(
        !header.contains("treat this file as raw"),
        "the header told a reader to treat a redacted log as raw:\n{header}"
    );
    assert!(
        header.contains("registered LATER"),
        "the header must point at the in-band marker, since the set grows \
         after creation:\n{header}"
    );
    assert!(
        body.contains("(session key)"),
        "the marker must name what was registered"
    );
}

/// **PL-5: a secret split across two reads was written in clear.**
///
/// `apply_bytes` is called once per `read`, and a TCP read boundary falls
/// wherever the network puts it. The launch key is ~32 bytes and arrives in the
/// game handshake, so a chunk boundary landing inside it is ordinary, not
/// exotic -- and the result is the live credential sitting in a `.bytes` file
/// in the clear.
///
/// The review called this out with "No test covers it." This is that test.
#[test]
fn a_secret_split_across_two_writes_is_still_redacted() {
    const KEY: &str = "abcdefghijklmnopqrstuvwxyz012345";
    let dir = std::env::temp_dir().join("cena-sink-straddle");
    let _ = std::fs::remove_dir_all(&dir);
    let mut sink = cena_platform::SessionSink::create(&dir, "Tester", "stamp", Redactions::new())
        .expect("the sink must open");
    sink.redact_key(KEY);

    // The boundary falls INSIDE the key, as a read boundary may.
    let (head, tail) = KEY.split_at(14);
    sink.wire(true, format!("<login key=\"{head}").as_bytes())
        .expect("write must work");
    sink.wire(true, format!("{tail}\"/>\n").as_bytes())
        .expect("write must work");
    sink.flush().expect("flush must work");
    let path = sink.bytes_path().to_owned();
    drop(sink);

    let written = std::fs::read(&path).expect("the wire file must be readable");
    let text = String::from_utf8_lossy(&written);
    assert!(
        !text.contains(KEY),
        "the launch key was written in CLEAR across a chunk boundary:\n{text}"
    );
}

/// **PL-5: a match must not corrupt unrelated bytes in the same chunk.**
///
/// On any match the whole chunk used to round-trip through
/// `String::from_utf8_lossy`, so non-UTF-8 bytes *elsewhere in that chunk*
/// became U+FFFD. `bytes_without_a_secret_are_returned_unchanged` covered only
/// the no-match path, so nothing caught it.
///
/// This matters because the `.bytes` file is meant to be the WIRE. A fixture
/// cut from a chunk that happened to contain a secret would differ from what
/// the server actually sent, and the corruption is silent.
#[test]
fn redacting_a_chunk_leaves_its_other_bytes_byte_exact() {
    let mut redactions = Redactions::new();
    redactions.key("supersecretkey-0123456789");

    // A latin-1 byte that is not valid UTF-8, beside the secret.
    let mut chunk = b"before \xff ".to_vec();
    chunk.extend_from_slice(b"supersecretkey-0123456789");
    chunk.extend_from_slice(b" \xfe after");

    let out = redactions.apply_bytes(&chunk);

    assert!(
        !out.windows(25).any(|w| w == b"supersecretkey-0123456789"),
        "the secret survived"
    );
    assert!(
        out.contains(&0xff) && out.contains(&0xfe),
        "non-UTF-8 bytes elsewhere in the chunk were replaced with U+FFFD, so \
         the `.bytes` file no longer holds what the server sent"
    );
}

/// **Holding bytes back must never LOSE them.**
///
/// The straddle fix defers up to `longest_secret - 1` bytes. That is only
/// acceptable if every deferred byte still reaches the file -- a `.bytes` file
/// silently short is worse than one with a late boundary, because it is what
/// fixtures are cut from and what criterion 7 replays.
#[test]
fn every_byte_still_reaches_the_file_despite_the_held_back_tail() {
    let dir = std::env::temp_dir().join("cena-sink-noloss");
    let _ = std::fs::remove_dir_all(&dir);
    let mut sink = cena_platform::SessionSink::create(&dir, "Tester", "stamp", Redactions::new())
        .expect("the sink must open");
    sink.redact_key("a-key-that-never-appears-in-the-payload");

    let mut expected = Vec::new();
    for i in 0..20u8 {
        let chunk = format!("<line n='{i}'/>");
        expected.extend_from_slice(chunk.as_bytes());
        sink.wire(true, chunk.as_bytes()).expect("write must work");
    }
    sink.flush().expect("flush must work");
    let path = sink.bytes_path().to_owned();
    drop(sink);

    let written = std::fs::read(&path).expect("readable");
    assert_eq!(
        written, expected,
        "bytes were lost or altered by the straddle tail"
    );
}

/// A secret split across MANY chunks, one byte at a time.
///
/// The pathological case for a carry-over tail: if the hold were ever computed
/// per-chunk rather than against the longest secret, this is what would defeat
/// it.
#[test]
fn a_secret_dribbled_one_byte_at_a_time_is_still_redacted() {
    const KEY: &str = "dribbled-secret-0123456789abcdef";
    let dir = std::env::temp_dir().join("cena-sink-dribble");
    let _ = std::fs::remove_dir_all(&dir);
    let mut sink = cena_platform::SessionSink::create(&dir, "Tester", "stamp", Redactions::new())
        .expect("the sink must open");
    sink.redact_key(KEY);

    for byte in format!("<k>{KEY}</k>\n").into_bytes() {
        sink.wire(true, &[byte]).expect("write must work");
    }
    sink.flush().expect("flush must work");
    let path = sink.bytes_path().to_owned();
    drop(sink);

    let written = String::from_utf8_lossy(&std::fs::read(&path).expect("readable")).into_owned();
    assert!(
        !written.contains(KEY),
        "a secret arriving one byte per read was written in clear:\n{written}"
    );
    assert!(written.contains("<KEY>"), "{written}");
}

/// Dropping a sink without flushing must still write the held-back tail.
///
/// `BufWriter` flushes its own buffer on drop, but the straddle tail is ours.
/// Without a `Drop` impl the last `longest_secret - 1` bytes of every session
/// would simply vanish.
#[test]
fn dropping_the_sink_writes_the_tail() {
    let dir = std::env::temp_dir().join("cena-sink-drop");
    let _ = std::fs::remove_dir_all(&dir);
    let path = {
        let mut sink =
            cena_platform::SessionSink::create(&dir, "Tester", "stamp", Redactions::new())
                .expect("the sink must open");
        sink.redact_key("some-registered-key-value-here-32");
        sink.wire(true, b"tail bytes that were never flushed")
            .expect("write must work");
        let path = sink.bytes_path().to_owned();
        // NO flush: the drop must do it.
        path
    };

    let written = std::fs::read(&path).expect("readable");
    assert_eq!(
        written, b"tail bytes that were never flushed",
        "the held-back tail was lost when the sink was dropped"
    );
}

/// **PL-5: the account name reached the log because nothing registered it.**
///
/// `Redactions::account` existed with no production caller. The account is not
/// a password, but it is not innocuous either: every character code on the wire
/// is `W_<ACCOUNT>_<SLOT>` (`plan/10` §4.6), so it appears in ordinary game
/// traffic, and the review's PL-4 already closed the same exposure for stderr.
///
/// The header must also NAME what it covers. Saying "yes" when only a key was
/// registered told a reader the account was scrubbed when it was not.
#[test]
fn a_registered_account_is_redacted_and_the_header_names_it() {
    let dir = std::env::temp_dir().join("cena-sink-account");
    let _ = std::fs::remove_dir_all(&dir);

    let mut redactions = Redactions::new();
    redactions.account("someaccount");
    let mut sink = cena_platform::SessionSink::create(&dir, "Tester", "stamp", redactions)
        .expect("the sink must open");

    // The shape the wire really carries.
    sink.wire(true, b"<charID id=\"W_SOMEACCOUNT_001\"/>\n")
        .expect("write must work");
    sink.flush().expect("flush must work");
    let bytes_path = sink.bytes_path().to_owned();
    let events_path = sink.events_path().to_owned();
    drop(sink);

    let wire = String::from_utf8_lossy(&std::fs::read(&bytes_path).expect("readable")).into_owned();
    assert!(
        !wire.to_ascii_uppercase().contains("SOMEACCOUNT"),
        "the account name reached the wire log:\n{wire}"
    );

    let header = std::fs::read_to_string(&events_path).expect("readable");
    assert!(
        header.contains("account and character"),
        "the header must name WHAT is registered, not just say yes:\n{header}"
    );
}
