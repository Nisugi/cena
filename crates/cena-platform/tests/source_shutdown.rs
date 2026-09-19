//! **PL-6: `AnsweringSource` ignored its own `shutdown`.**
//!
//! `ReplaySource` and `LiveSource` both treat a shut-down source as closed:
//! reads report end of stream and writes fail. `AnsweringSource` set a flag and
//! carried on -- reads stayed parked, writes succeeded.
//!
//! That is not a cosmetic inconsistency. `AnsweringSource` is what most session
//! tests run against, so a **write-after-shutdown bug in the session would pass
//! every one of them** while failing against the two sources nothing tests
//! with. The mock was more permissive than production, in the direction that
//! hides bugs.
//!
//! These tests hold the three sources to the same contract, which is the point:
//! a mock that does not is a mock that lies.

use cena_platform::bytes::ByteSource;

/// A source that has been shut down is at end of stream.
///
/// Without this a session that closed its own source parks in `read` forever
/// instead of ending -- and `AnsweringSource::read` waits FOREVER by design, so
/// there is nothing else to break the wait.
#[tokio::test]
async fn an_answering_source_reads_end_of_stream_after_shutdown() {
    let (mut source, _transcript) = cena_platform::AnsweringSource::new(b"<prompt>&gt;</prompt>\n");
    source.shutdown().await.expect("shutdown must succeed");

    let mut buf = [0u8; 64];
    let read = tokio::time::timeout(std::time::Duration::from_secs(1), source.read(&mut buf))
        .await
        .expect("a shut-down source must not park the reader forever")
        .expect("read must not error");

    assert_eq!(read, 0, "a shut-down source must report end of stream");
}

/// Writing to a shut-down source fails, as it does on a real socket.
#[tokio::test]
async fn an_answering_source_refuses_a_write_after_shutdown() {
    let (mut source, _transcript) = cena_platform::AnsweringSource::new(b"reply\n");
    source.shutdown().await.expect("shutdown must succeed");

    let error = source
        .write_all(b"look\n")
        .await
        .expect_err("a write to a closed source must fail");
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::NotConnected,
        "the same kind `ReplaySource` reports, so a caller can treat them alike"
    );
}

/// The same contract, on the source it was already honoured by.
///
/// Asserted here rather than taken on trust: this file's premise is that the
/// three sources agree, and a test of only the one that was wrong cannot show
/// that.
#[tokio::test]
async fn a_replay_source_honours_the_same_contract() {
    let mut source = cena_platform::ReplaySource::from_bytes(b"<prompt>&gt;</prompt>\n");
    source.shutdown().await.expect("shutdown must succeed");

    let mut buf = [0u8; 64];
    assert_eq!(
        source.read(&mut buf).await.expect("read must not error"),
        0,
        "a shut-down replay source must report end of stream"
    );
    assert_eq!(
        source
            .write_all(b"look\n")
            .await
            .expect_err("a write to a closed source must fail")
            .kind(),
        std::io::ErrorKind::NotConnected
    );
}
