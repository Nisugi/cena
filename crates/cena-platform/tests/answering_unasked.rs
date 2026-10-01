//! The scripted game speaks unasked, as the real one does
//! (`AnsweringSource`'s module doc): a creature walking in, or a reply later
//! than the prompt that came before it. Added for the hunt driver's
//! regression test of 2026-09-30, where a prompt that was not the reply was
//! taken for it.

use std::time::Duration;

use cena_platform::AnsweringSource;
use cena_platform::bytes::ByteSource;

/// What `say` hands a parked reader, whole, owed to no write, and whatever
/// the hold on replies.
#[tokio::test]
async fn what_is_said_unasked_reaches_a_parked_reader_through_a_hold() {
    let (mut source, transcript) = AnsweringSource::new(b"<prompt>&gt;</prompt>\n");
    transcript.hold_replies();
    source.write_all(b"fire\n").await.expect("written");
    let said = b"A kobold shuffles in.\n<prompt>&gt;</prompt>\n";
    let reading = tokio::spawn(async move {
        let mut buf = [0u8; 256];
        let n = source.read(&mut buf).await.expect("read");
        buf[..n].to_vec()
    });
    tokio::task::yield_now().await;
    transcript.say(said);
    let read = tokio::time::timeout(Duration::from_secs(1), reading)
        .await
        .expect("a parked reader is woken")
        .expect("joined");
    assert_eq!(read, said, "the unasked bytes, and not the held reply");
    assert_eq!(transcript.lines(), ["fire"], "nothing said is a write");
}
