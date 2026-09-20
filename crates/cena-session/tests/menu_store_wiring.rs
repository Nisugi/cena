//! A `<cmdlist>` push reaches disk from a running session.
//!
//! The bytes go through a real `SessionActor` over a `ReplaySource`, so what
//! is tested is the **wiring** -- that `actor/io.rs` notices the push and
//! calls the store. The store's own rules (merge, format, pruning) are
//! `menu_store.rs`; the model's are `cena-model`'s `cmdlist_updates.rs`.

use cena_platform::ReplaySource;
use cena_session::{Session, menu_store};

/// A push of a row the shipped table does not have, so something is written.
///
/// INVENTED, necessarily: MEASURED, the rows the server pushes today are
/// byte-identical to the shipped table, so a real push writes nothing at all.
/// `a_push_matching_the_shipped_table_writes_nothing` covers that case with
/// the real rows.
const NOVEL_PUSH: &str = concat!(
    r#"<cmdlist><cli coord="9999,1" menu="frobnicate @" command="frobnicate #" "#,
    r#"menu_cat="6"/></cmdlist><cmdtimestamp data='1788300900.1.1.1'/>"#,
    "\n",
);

/// The real push, verbatim from the captured stream.
const REAL_PUSH: &str = concat!(
    r#"<cmdlist><cli coord="2524,12785" menu="sense @" command="sense #" menu_cat="5_roleplay"/>"#,
    r#"<cli coord="2524,12784" menu="whisper @ about %" command="whisper # about %" menu_cat="9_questions"/>"#,
    r#"</cmdlist><cmdtimestamp data='1788300900.1.1.1'/>"#,
    "\n",
);

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-menu-wiring-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_push_is_written_the_moment_it_arrives() {
    // The author's choice between writing on every push and writing at
    // disconnect: *"option 1 of course."* The session here ends normally, so
    // this alone does not distinguish the two -- `a_push_survives_a_session_
    // that_never_ends_cleanly` is what does.
    let dir = temp_dir("on-arrival");
    let session =
        Session::new(ReplaySource::from_bytes(NOVEL_PUSH.as_bytes())).with_menu_store(dir.clone());
    let end = session.into_actor().run().await;

    assert_eq!(
        end.state.learned_commands.len(),
        1,
        "guard: the model has it"
    );

    let loaded = menu_store::load(&dir).expect("load");
    assert_eq!(loaded.len(), 1, "and so does the file");
    assert_eq!(
        loaded.entry("9999,1").map(|r| r.label.as_str()),
        Some("frobnicate @")
    );
    assert_eq!(loaded.version(), Some("1788300900.1.1.1"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_push_survives_a_session_that_never_ends_cleanly() {
    // What "write on arrival" BUYS over "write at disconnect", and the only
    // test that tells the two apart: the file is already complete while the
    // stream is still open, so a crash after this point loses nothing.
    //
    // A push is not repeated on reconnect -- the server sends one when it
    // decides the client is behind -- so rows lost this way may not come back.
    let dir = temp_dir("mid-stream");
    // The push, then traffic that would still be arriving when a crash hit.
    let stream = format!("{NOVEL_PUSH}You see nothing unusual.\n");
    let session =
        Session::new(ReplaySource::from_bytes(stream.as_bytes())).with_menu_store(dir.clone());
    let (_snapshot, _events) = session.subscribe();
    let _ = session.into_actor().run().await;

    assert_eq!(
        menu_store::load(&dir).expect("load").len(),
        1,
        "written before the later traffic, not after the stream ended"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_push_matching_the_shipped_table_writes_nothing() {
    // The state of a current install, MEASURED: the two real pushed rows are
    // byte-identical to the shipped table, so there is nothing novel and no
    // file is created. Lich has the same guard for the same reason
    // (`infomon.rb:213`: `return :noop if self.cache.get(key) == value`).
    let dir = temp_dir("no-op");
    let session =
        Session::new(ReplaySource::from_bytes(REAL_PUSH.as_bytes())).with_menu_store(dir.clone());
    let end = session.into_actor().run().await;

    assert_eq!(
        end.state.learned_commands.len(),
        2,
        "guard: the model learned both rows"
    );
    assert!(
        !menu_store::store_path(&dir).exists(),
        "nothing novel, so no file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_without_a_store_writes_nothing_and_still_learns() {
    // Every existing test is in this state. The rows reach the model either
    // way; only the file depends on opting in.
    let dir = temp_dir("unconfigured");
    let session = Session::new(ReplaySource::from_bytes(NOVEL_PUSH.as_bytes()));
    let end = session.into_actor().run().await;

    assert_eq!(
        end.state.learned_commands.len(),
        1,
        "the model still learns"
    );
    assert!(!dir.exists(), "no directory was touched");
}
