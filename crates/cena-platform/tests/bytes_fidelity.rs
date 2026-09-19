//! **The `.bytes` file is the wire, byte for byte.**
//!
//! # Why this file exists
//!
//! `Recorder`'s docs and `SessionSink::wire`'s both say chunk boundaries are
//! preserved, because *"replaying the split that happened live is what drives
//! `Parser::push_bytes`'s partial-line path with a real boundary rather than an
//! invented one"*. The implementation appended a newline to any chunk that did
//! not already end in one, which is the opposite of that.
//!
//! **This is the file M2's golden corpus is cut from**, and criterion 7's replay
//! reads it, so a byte the sink invents is a byte every future fixture inherits.
//!
//! Found by review. The existing reassembly test could not see it
//! (`sink_redaction.rs`): every chunk it feeds already ends in a newline.

use cena_platform::{Redactions, SessionSink};

/// A fresh sink in its own temp directory, and the path to its bytes file.
///
/// Returns a `Result` rather than `expect`ing: `expect_used` is denied
/// workspace-wide and clippy applies it to helpers even in a test target, which
/// is the right call -- a helper that panics reports the wrong file.
fn sink(name: &str) -> std::io::Result<(SessionSink, std::path::PathBuf)> {
    let dir = std::env::temp_dir().join(format!("cena-bytes-fidelity-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    let sink = SessionSink::create(&dir, "Tester", "stamp", Redactions::new())?;
    let path = sink.bytes_path().to_path_buf();
    Ok((sink, path))
}

/// **A tag split across two reads must not gain a newline in the middle.**
///
/// The reproduction from the review: `<pushStream id='ro` + `om'/>` became
/// `<pushStream id='ro\nom'/>` on disk. A replay of that file parses something
/// the live session never saw.
#[test]
fn a_chunk_that_ends_mid_tag_is_written_verbatim() {
    let (mut sink, path) = sink("mid-tag").expect("the sink must open");

    // A real split: the game's chunk boundary landed inside an attribute.
    sink.wire(true, b"<pushStream id='ro").expect("write");
    sink.wire(true, b"om'/>Some text\n").expect("write");
    sink.flush().expect("flush");
    drop(sink);

    let written = std::fs::read(&path).expect("the bytes file must exist");
    let text = String::from_utf8_lossy(&written);

    assert!(
        text.contains("<pushStream id='room'/>"),
        "the tag must survive the chunk boundary intact. A newline injected \
         between the two reads makes the recording parse differently from the \
         live session it recorded. Got: {text:?}"
    );
    assert_eq!(
        written, b"<pushStream id='room'/>Some text\n",
        "and the file must be the two chunks CONCATENATED, with nothing added"
    );
}

/// **A command that already ends in a newline must not lose it.**
///
/// The second half of the same defect, in the other direction: the check looked
/// at the untrimmed buffer while the write was trimmed, so `look\n` produced
/// `<!-- CLIENT -->look<!-- ENDCLIENT -->` with **no** trailing newline -- and
/// the next inbound line was glued onto it.
#[test]
fn a_client_command_is_followed_by_exactly_one_newline() {
    let (mut sink, path) = sink("client-newline").expect("the sink must open");

    sink.wire(false, b"look\n").expect("write");
    sink.wire(true, b"You see nothing special.\n")
        .expect("write");
    sink.flush().expect("flush");
    drop(sink);

    let written = std::fs::read(&path).expect("the bytes file must exist");
    let text = String::from_utf8_lossy(&written);

    assert!(
        !text.contains("ENDCLIENT -->You see"),
        "the server's reply must not be glued to the ENDCLIENT marker -- a \
         reader splitting on lines would see one line where there were two. \
         Got: {text:?}"
    );
    assert!(
        text.contains("ENDCLIENT -->\n"),
        "the client wrapper needs its own line terminator. Got: {text:?}"
    );
}

/// The inbound path adds nothing at all, even for a chunk with no newline
/// anywhere in it.
#[test]
fn inbound_bytes_are_never_padded() {
    let (mut sink, path) = sink("no-padding").expect("the sink must open");

    sink.wire(true, b"abc").expect("write");
    sink.wire(true, b"def").expect("write");
    sink.flush().expect("flush");
    drop(sink);

    assert_eq!(
        std::fs::read(&path).expect("the bytes file must exist"),
        b"abcdef",
        "two newline-free chunks concatenate to six bytes. Anything longer is \
         the sink inventing framing the wire did not have."
    );
}
