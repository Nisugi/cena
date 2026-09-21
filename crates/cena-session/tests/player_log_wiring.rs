//! `plan/25` step 2b: a running session feeds the player log.
//!
//! Steps 1 and 2 built a channel and a writer that nothing called. These drive
//! real bytes through a real `SessionActor` and read what reaches the sink, so
//! what is tested is the **wiring** and the routing rule -- the writer's own
//! behaviour is `player_log_writer.rs`.

use std::time::Duration;

use cena_platform::{AnsweringSource, ReplaySource};
use cena_session::player_log::{Capture, LogSink};
use cena_session::{CommandId, Origin, PlayerLog, Session};

/// Everything queued, as `(tag, text)`.
fn drain(sink: &mut LogSink) -> Vec<(String, String)> {
    std::iter::from_fn(|| sink.try_recv())
        .map(|l| (l.stream, l.text))
        .collect()
}

async fn logged(wire: &str) -> Vec<(String, String)> {
    logged_with(wire, Capture::default()).await
}

async fn logged_with(wire: &str, capture: Capture) -> Vec<(String, String)> {
    let (log, mut sink) = PlayerLog::new();
    let session =
        Session::new(ReplaySource::from_bytes(wire.as_bytes())).with_player_log(log, capture, None);
    let _ = session.into_actor().run().await;
    drain(&mut sink)
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn main_window_text_is_logged_as_main() {
    let lines = logged("You see nothing unusual.\n").await;
    assert_eq!(
        lines,
        vec![("main".to_owned(), "You see nothing unusual.".to_owned())]
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_line_split_at_a_link_is_one_line_with_its_markup_gone() {
    // The shape that printed the author's inventory as `a` / `pebbled grey
    // leather doublet` on two lines: one displayed line, three frames.
    //
    // **A completed line comes first, and the test needs it.** MEASURED by
    // mutation: logging on EVERY text frame passed without it, because the
    // mid-line frames found no completed line to log. With one in the buffer,
    // that mutant writes `before` again for each of them.
    let lines = logged("before\nYou see <a exist=\"1\" noun=\"rock\">a rock</a> here.\n").await;
    assert_eq!(
        lines,
        vec![
            ("main".to_owned(), "before".to_owned()),
            ("main".to_owned(), "You see a rock here.".to_owned()),
        ]
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_readout_is_off_by_default_and_a_text_stream_is_on() {
    // AUTHOR, 2026-09-21: every feed is available, and which are WRITTEN is
    // configurable because the readouts eat disk. `inv` and `thoughts` side by
    // side, so the test tells a default from a rule that drops every pushed
    // stream.
    let wire = concat!(
        "<clearStream id='inv'/><pushStream id='inv'/>Your worn items are:\n",
        "  a pebbled grey leather doublet\n<popStream/>",
        "<pushStream id='thoughts'/>[OOC] someone: hi\n<popStream/>",
        "after\n",
    );
    let lines = logged(wire).await;
    assert_eq!(
        lines,
        vec![
            ("thoughts".to_owned(), "[OOC] someone: hi".to_owned()),
            ("main".to_owned(), "after".to_owned()),
        ]
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn blank_lines_are_not_logged() {
    let lines = logged("first\n\n   \nsecond\n").await;
    assert_eq!(lines.len(), 2, "{lines:?}");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_sent_command_is_logged_as_cmd_without_its_newline() {
    let (source, _transcript) = AnsweringSource::new(b"<prompt time=\"1\">&gt;</prompt>\n");
    let (log, mut sink) = PlayerLog::new();
    let session = Session::new(source).with_player_log(log, Capture::default(), None);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.into_actor().run());

    let _ = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    cancel.cancel();
    task.await.expect("the actor task must not panic");

    let lines = drain(&mut sink);
    assert!(
        lines.contains(&("cmd".to_owned(), "look".to_owned())),
        "{lines:?}"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_with_no_log_attached_runs_as_before() {
    let session = Session::new(ReplaySource::from_bytes(b"hello\n"));
    let end = session.into_actor().run().await;
    assert_eq!(end.state.lines_seen(), 1);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn what_hydra_says_is_logged_in_its_own_voice() {
    // A notice never passes through the actor -- `SessionHandle::say` goes
    // straight to the event stream -- so the actor's feed cannot see it. The
    // handle is CLONED BEFORE the log is attached, which is the order the
    // binary does it in, and the order a slot copied at clone time would fail.
    use cena_session::notice::{Notice, NoticeKind};

    let session = Session::new(ReplaySource::from_bytes(b""));
    let early = session.handle();
    let (log, mut sink) = PlayerLog::new();
    let session = session.with_player_log(log, Capture::default(), None);

    early.say(Notice::line(NoticeKind::Warn, "the trip stopped: no route"));
    early.say(Notice::table(
        NoticeKind::Info,
        vec![
            "step  room".to_owned(),
            String::new(),
            "1     228".to_owned(),
        ],
    ));
    drop(session);

    assert_eq!(
        drain(&mut sink),
        vec![
            ("hydra".to_owned(), "the trip stopped: no route".to_owned()),
            ("hydra".to_owned(), "step  room".to_owned()),
            ("hydra".to_owned(), "1     228".to_owned()),
        ]
    );
}

#[test]
fn a_notice_with_no_log_attached_is_still_said() {
    use cena_session::notice::{Notice, NoticeKind};
    let session = Session::new(ReplaySource::from_bytes(b""));
    let (_snapshot, mut events) = session.subscribe();
    session
        .handle()
        .say(Notice::line(NoticeKind::Info, "hello"));
    assert!(matches!(
        events.try_recv(),
        Ok(cena_session::Event::Notice(_))
    ));
}

/// Two scripted connections, then a fatal refusal that ends the session.
struct TwoConnections(std::collections::VecDeque<Vec<u8>>);

impl cena_session::Connector for TwoConnections {
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

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn one_log_spans_a_reconnect_and_each_line_says_which_connection() {
    // The path the BINARY uses. The supervisor builds a new actor per
    // connection, so a log handed to the first alone would stop at the drop --
    // exactly where a reader most wants to know what happened.
    let connector = TwoConnections(
        [b"before the drop\n".to_vec(), b"after it\n".to_vec()]
            .into_iter()
            .collect(),
    );
    let (log, mut sink) = PlayerLog::new();
    let (session, handle) = cena_session::SupervisedSession::new(connector);
    let session = session.with_player_log(log, Capture::default(), None);
    let end = session.run().await;
    assert!(end.generations.0 >= 1, "guard: it did reconnect");

    // The handle was returned BEFORE the log was attached, and still reaches it.
    handle.say(cena_session::Notice::line(
        cena_session::NoticeKind::Info,
        "said after",
    ));

    let lines: Vec<_> = std::iter::from_fn(|| sink.try_recv())
        .map(|l| (l.generation.0, l.stream, l.text))
        .collect();
    let texts: Vec<_> = lines.iter().map(|l| l.2.as_str()).collect();
    assert_eq!(texts, ["before the drop", "after it", "said after"]);
    assert!(
        lines[0].0 < lines[1].0,
        "the reconnect is not visible in the log: {lines:?}"
    );
}

const INV: &str = concat!(
    "<clearStream id='inv'/><pushStream id='inv'/>Your worn items are:\n",
    "  a pebbled grey leather doublet\n<popStream/>",
    "<prompt time=\"1\">&gt;</prompt>\n",
);

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_readout_switched_on_is_written_once_until_it_changes() {
    // The bloat the author named: the game rewrites the whole table over and
    // over. Three identical rewrites, then one that differs.
    let changed = INV.replace("doublet", "jerkin");
    let wire = format!("{INV}{INV}{INV}{changed}");
    let lines = logged_with(&wire, Capture::default().set("inv", true)).await;
    let items: Vec<_> = lines
        .iter()
        .filter(|(tag, text)| tag == "inv" && text.contains("leather"))
        .map(|(_, text)| text.trim())
        .collect();
    assert_eq!(
        items,
        [
            "a pebbled grey leather doublet",
            "a pebbled grey leather jerkin"
        ],
        "{lines:?}"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_feed_switched_off_is_not_written() {
    let wire = "<pushStream id='thoughts'/>chatter\n<popStream/>kept\n";
    let lines = logged_with(wire, Capture::default().set("thoughts", false)).await;
    assert_eq!(lines, vec![("main".to_owned(), "kept".to_owned())]);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_chunk_the_combat_definitions_recognise_is_tagged_combat() {
    // The author's goal: a combat feed separable from the main feed. A REAL
    // exchange (`combat_exchange.xml`, cut from live traffic), then a chunk of
    // ordinary text, so the test tells "this chunk was combat" from "everything
    // after the first swing is combat".
    let exchange = include_str!("../../cena-protocol/tests/fixtures/combat_exchange.xml");
    //
    // **A thought arrives inside the combat chunk, and the test needs it.**
    // MEASURED by mutation: applying the class to EVERY feed passed without
    // one, because nothing but main-window text was in the chunk.
    let wire = format!(
        "<pushStream id='thoughts'/>mid-swing chatter\n<popStream/>{exchange}\n\
         You glance around.\n<prompt time=\"1788293799\">&gt;</prompt>\n"
    );
    let lines = logged(&wire).await;

    let thought = lines
        .iter()
        .find(|(_, text)| text == "mid-swing chatter")
        .expect("guard: the thought reached the log");
    assert_eq!(
        thought.0, "thoughts",
        "someone else's words were classed as this character's combat"
    );

    let aim = lines
        .iter()
        .find(|(_, text)| text.starts_with("You take aim and fire"))
        .expect("guard: the swing reached the log");
    assert_eq!(aim.0, "main/combat", "{lines:?}");

    let after = lines
        .iter()
        .find(|(_, text)| text == "You glance around.")
        .expect("guard: the later chunk reached the log");
    assert_eq!(after.0, "main", "combat leaked into the next chunk");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_characters_own_settings_file_switches_feeds() {
    // The file is named by what `<app>` says (`Prime`, not the login code), so
    // it cannot be read until the game has spoken. `early` arrives BEFORE the
    // `<app>` tag and `thoughts` is switched off: a capture judged on arrival
    // would let it through, because the file had not been read yet.
    use cena_session::player_log::LogSettings;
    use cena_session::settings_store::{self, SettingsFile};

    let dir = std::env::temp_dir().join(format!("cena-log-settings-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut file = SettingsFile::new("Prime", "Nisugi");
    let feeds = [("inv".to_owned(), true), ("thoughts".to_owned(), false)].into();
    file.set_section("player_log", &LogSettings { feeds })
        .expect("set");
    settings_store::save(&dir, &file).expect("save");

    let wire = format!(
        "<pushStream id='thoughts'/>early\n<popStream/>\
         <app char=\"Nisugi\" game=\"Prime\" title=\"x\"/>\n{INV}"
    );
    let (log, mut sink) = PlayerLog::new();
    let session = Session::new(ReplaySource::from_bytes(wire.as_bytes())).with_player_log(
        log,
        Capture::default(),
        Some(dir.clone()),
    );
    let _ = session.into_actor().run().await;
    let tags: Vec<_> = drain(&mut sink).into_iter().map(|(tag, _)| tag).collect();

    assert!(
        tags.contains(&"inv".to_owned()),
        "inv was switched ON: {tags:?}"
    );
    assert!(
        !tags.contains(&"thoughts".to_owned()),
        "thoughts was switched OFF: {tags:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_supervised_session_reads_the_settings_file_too() {
    // **The path the binary runs, and the one that was broken.** The settings
    // were first loaded from a hook on the snapshot's load -- which
    // `SupervisedSession` never reaches, having no `with_character_store`. It
    // passed every test above and would never have worked in a real session.
    use cena_session::player_log::LogSettings;
    use cena_session::settings_store::{self, SettingsFile};

    let dir = std::env::temp_dir().join(format!("cena-log-settings-sup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut file = SettingsFile::new("Prime", "Nisugi");
    let feeds = [("inv".to_owned(), true)].into();
    file.set_section("player_log", &LogSettings { feeds })
        .expect("set");
    settings_store::save(&dir, &file).expect("save");

    let wire = format!("<app char=\"Nisugi\" game=\"Prime\" title=\"x\"/>\n{INV}");
    let connector = TwoConnections([wire.into_bytes()].into_iter().collect());
    let (log, mut sink) = PlayerLog::new();
    let (session, _handle) = cena_session::SupervisedSession::new(connector);
    let session = session.with_player_log(log, Capture::default(), Some(dir.clone()));
    let _ = session.run().await;

    let tags: Vec<_> = drain(&mut sink).into_iter().map(|(tag, _)| tag).collect();
    assert!(tags.contains(&"inv".to_owned()), "{tags:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
