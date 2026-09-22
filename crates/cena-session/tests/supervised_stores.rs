//! The stores reach a **supervised** session -- the one the binary runs.
//!
//! `character_persistence.rs` and `menu_store_wiring.rs` test the stores
//! thoroughly, and every test in them drives a plain `Session`. MEASURED
//! 2026-09-21: `SupervisedSession` had no `with_character_store` and no
//! `with_menu_store`, so no live session ever read or wrote either file, and
//! the author's machine had no data directory after several live runs. A green
//! suite reporting a feature that was not there.
//!
//! So these do not re-test the stores. They test that the supervisor hands
//! them to the actor, on every connection, and reads the snapshot once.

use std::collections::VecDeque;

use cena_platform::ReplaySource;
use cena_session::{Event, SupervisedSession, character_store, menu_store};

const WHO: &str = concat!(
    r#"<app char="Nisugi" game="GS" title="[GSIV: Nisugi, the Ranger] (Prime)"/>"#,
    "\n",
);

const SOCIETY: &str = concat!(
    "<popBold/>   You are a Master of the Guardians of Sunfist.\n",
    "<prompt time=\"1\">&gt;</prompt>\n",
);

const PUSH: &str = concat!(
    r#"<cmdlist><cli coord="9999,1" menu="frobnicate @" command="frobnicate #" "#,
    r#"menu_cat="6"/></cmdlist><cmdtimestamp data='1788300900.1.1.1'/>"#,
    "\n",
);

/// Scripted connections, then a fatal refusal that ends the session.
struct Scripted(VecDeque<Vec<u8>>);

impl Scripted {
    fn new(connections: &[&str]) -> Self {
        Self(connections.iter().map(|c| c.as_bytes().to_vec()).collect())
    }
}

impl cena_session::Connector for Scripted {
    type Source = ReplaySource;

    async fn connect(
        &mut self,
        _generation: cena_session::Generation,
    ) -> Result<ReplaySource, cena_session::ConnectError> {
        self.0
            .pop_front()
            .map(|bytes| ReplaySource::from_bytes(&bytes))
            .ok_or_else(|| cena_session::ConnectError::fatal("scripted", "no more connections"))
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-sup-stores-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn what_a_supervised_session_learns_is_on_disk_for_the_next_one() {
    let dir = temp_dir("round-trip");

    let learned = format!("{WHO}{SOCIETY}");
    let (session, _handle) = SupervisedSession::new(Scripted::new(&[&learned]));
    let _ = Box::pin(session.with_character_store(dir.clone()).run()).await;

    let stored = character_store::load(&dir, "GS", "Nisugi").expect("the snapshot was written");
    assert_eq!(stored.character, "Nisugi", "guard");

    // The next login is told WHO it is and nothing else.
    let (session, _handle) = SupervisedSession::new(Scripted::new(&[WHO]));
    let end = Box::pin(session.with_character_store(dir.clone()).run()).await;
    assert_eq!(
        end.state.character.standing.society_rank,
        Some(20),
        "restored from disk, not from the wire"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_stored_facts_are_read_once_per_session_not_once_per_connection() {
    // `<app>` is re-sent on every reconnect, and a second load would put the
    // disk's older values over what the session has learned since. The actor
    // refuses one -- and remembers that ON THE ACTOR, which the supervisor
    // replaces per connection. A load announces itself with `SyncNeeded`.
    let dir = temp_dir("load-once");
    let (session, _handle) = SupervisedSession::new(Scripted::new(&[WHO, WHO]));
    let session = session.with_character_store(dir.clone());
    let (_snapshot, mut events) = session.subscribe();
    let end = Box::pin(session.run()).await;
    assert!(end.generations.0 >= 1, "guard: it did reconnect");

    let mut loads = 0;
    while let Ok(event) = events.try_recv() {
        if matches!(event, Event::SyncNeeded(_)) {
            loads += 1;
        }
    }
    assert_eq!(loads, 1, "the reconnect read the store again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_cmdlist_push_reaches_disk_on_the_second_connection_too() {
    // On the SECOND connection, so this tells "handed to every actor" from
    // "handed to the first".
    let dir = temp_dir("menu");
    let (session, _handle) = SupervisedSession::new(Scripted::new(&["hello\n", PUSH]));
    let _ = Box::pin(session.with_menu_store(dir.clone()).run()).await;

    let loaded = menu_store::load(&dir).expect("load");
    assert_eq!(
        loaded.entry("9999,1").map(|r| r.label.as_str()),
        Some("frobnicate @")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
