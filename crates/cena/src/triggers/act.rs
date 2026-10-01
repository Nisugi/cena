//! A trigger's send, sent (`plan/45` Stage 5): the session decided it and
//! paced it ([`Event::Act`]); here, where the character's command table is,
//! it goes as if the player had typed it -- to Hydra's `;` commands first,
//! otherwise to the game as [`Origin::Trigger`], which never counts as the
//! player being there. What the game or Hydra refuses is said, naming the
//! trigger. Each character's triggers task ([`super::follow`]) hears the
//! sends and sends them here.
//!
//! A send names the connection whose line set it off. One heard after that
//! connection was replaced is not sent -- not to the game, and not as a `;`
//! command, which could start a behavior on the new connection from a line
//! of the old (the crate review of 2026-09-28, R2). It is not said either:
//! the connection's end was, as a send it interrupted is not.
//!
//! **A trigger acts for its own character only** (the author, 2026-09-29: *"I
//! would expect each character to be set up to fend for themselves, and so
//! they should have their own trigger"*), **and never decides what may act on
//! it.** Hydra's command line hears a send as [`Origin::Trigger`], and refuses
//! it the commands only the player may type, `;agent`, `;trigger`, `;lich`,
//! `;to` and `;all` (`crate::commands::PLAYERS_OWN`), in a `;multi` or a
//! `;foreach` too, and says so. This replaced a check here that read `;all`
//! and `;to` off the line split at commas, which a `;foreach` or a `;multi`
//! split at semicolons walked past (the crate review of 2026-10-01, BI-D-1),
//! and which left `;agent level takeover` to anyone whose words a send's
//! regex group took in (BI-D-2).
//!
//! # What the game's words may fill in
//!
//! A regex group fills in what a command is given, **never which command**
//! (MO-F-3). A send whose groups would put the game's text where a command's
//! word is -- the line's first word, after its symbol if it has one, or, in a
//! Hydra command, an entry's after a `,`, `;`, `|` or `/` -- or whose groups
//! carry one of those separators into a Hydra command, is not sent, and is
//! said. So `send = "$1"` on `Baelor whispers, "(.+)"` is not a puppet for
//! whoever can make that line appear, and `.multi 1,say $1` cannot be made
//! into two commands by a comma in what was said; `say $1` and `weapon $1`
//! send as before. Approving a send from elsewhere approves its template
//! (`cena_behavior::triggers`), and with this the template is every command
//! the send can give.
//!
//! [`Event::Act`]: cena_session::Event::Act

use std::ops::Range;

use cena_session::command::claimant::Claimed;
use cena_session::{Gate, Generation, Notice, NoticeKind, Origin, Sent, SessionHandle};

/// What splits a Hydra command into commands: `;multi`'s entries and
/// `;foreach`'s command.
const SEPARATORS: [char; 4] = [',', ';', '|', '/'];

/// Send `trigger`'s `line`, as typed, if the connection `generation` that
/// set it off is still the session's. `captured`: the spans of `line` its
/// regex groups filled in.
pub(super) async fn send(
    handle: &SessionHandle,
    generation: Generation,
    trigger: &str,
    line: &str,
    captured: &[Range<usize>],
) {
    if handle.generation() != generation {
        return;
    }
    let symbol = handle
        .command_symbol()
        .unwrap_or(cena_session::command::claimant::DEFAULT_SYMBOL);
    if chosen_by_the_game(line, captured, symbol) {
        handle.say(Notice::line(
            NoticeKind::Warn,
            format!(
                "Trigger `{trigger}`: `{}` not sent: what the game said would choose the command, not fill it in.",
                line.trim()
            ),
        ));
        return;
    }
    let said = match handle.typed(line, Origin::Trigger) {
        Some(Claimed::Unknown) => Some(format!(
            "Trigger `{trigger}`: Hydra has no command `{}`.",
            line.trim()
        )),
        Some(_) => None,
        None => match handle
            .send_at(generation, line, Origin::Trigger, Gate::None)
            .await
        {
            Sent::Refused(refusal) => Some(format!(
                "Trigger `{trigger}`: `{line}` not sent: {refusal:?}."
            )),
            Sent::Ok { .. } | Sent::Dead | Sent::Interrupted => None,
        },
    };
    if let Some(said) = said {
        handle.say(Notice::line(NoticeKind::Warn, said));
    }
}

/// Whether the spans `captured` of `line` -- the game's text -- would choose
/// a command rather than fill one in. See the module docs.
fn chosen_by_the_game(line: &str, captured: &[Range<usize>], symbol: char) -> bool {
    if captured.is_empty() {
        return false;
    }
    let hydras = line.trim_start().starts_with(symbol);
    let mut words = vec![word_at(line, 0, symbol)];
    if hydras {
        words.extend(
            line.match_indices(SEPARATORS)
                .map(|(at, separator)| word_at(line, at + separator.len(), symbol)),
        );
    }
    captured.iter().any(|span| {
        words.iter().any(|word| span.contains(word))
            || (hydras
                && line
                    .get(span.clone())
                    .is_some_and(|text| text.contains(SEPARATORS)))
    })
}

/// Where the command word after byte `from` of `line` starts: past
/// whitespace, and a command symbol and the whitespace after it.
fn word_at(line: &str, from: usize, symbol: char) -> usize {
    let rest = line.get(from..).unwrap_or_default();
    let past_space = rest.trim_start();
    let past_symbol = past_space
        .strip_prefix(symbol)
        .map_or(past_space, str::trim_start);
    line.len() - past_symbol.len()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use cena_platform::AnsweringSource;
    use cena_session::{Event, Session, State};
    use tokio::sync::broadcast;

    use super::*;
    use crate::commands::Commands;

    const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

    fn told(events: &mut broadcast::Receiver<Event>) -> (Vec<(String, Origin)>, Vec<String>) {
        let (mut sent, mut notices) = (Vec::new(), Vec::new());
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Sent { line, origin, .. } => sent.push((line, origin)),
                Event::Notice(notice) => notices.push(notice.lines().join(" ")),
                _ => {}
            }
        }
        (sent, notices)
    }

    /// A game line goes to the game as a trigger's; a Hydra command runs,
    /// and never reaches the game; one Hydra lacks is said.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_send_goes_as_typed_and_is_marked_a_triggers() {
        let (source, transcript) = AnsweringSource::logged_in(PROMPT);
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut waiting) = session.subscribe();
        let (_, mut events) = session.subscribe();
        let commands = Commands::install(&handle);
        crate::sorter::open(&handle, &commands, None);
        tokio::spawn(session.into_actor().run());
        while !matches!(waiting.recv().await, Ok(Event::StateChanged(State::Ready))) {}

        let now = handle.generation();
        send(&handle, now, "stun", "stand", &[]).await;
        send(&handle, now, "sort", ".sorter on", &[]).await;
        send(&handle, now, "odd", ".frobnicate", &[]).await;
        send(&handle, now, "fan", ".all kneel", &[]).await;
        tokio::time::sleep(Duration::from_secs(1)).await;

        assert_eq!(
            transcript.lines(),
            ["stand"],
            "only the game's line reached it"
        );
        let (sent, notices) = told(&mut events);
        assert!(
            sent.contains(&("stand".to_owned(), Origin::Trigger)),
            "{sent:?}"
        );
        assert!(handle.sorts_containers(), "the Hydra command ran");
        assert!(
            notices
                .iter()
                .any(|notice| notice.contains("Trigger `odd`") && notice.contains(".frobnicate")),
            "{notices:?}"
        );
        assert!(
            notices
                .iter()
                .any(|notice| notice.contains("`.all kneel`")
                    && notice.contains("a trigger sent it")),
            "a trigger's .all is said, and sends nothing: {notices:?}"
        );
    }

    /// What every connection answers each line with: a line a trigger
    /// sends on.
    const TACKLE: &[u8] =
        b"You could use this opportunity to tackle!\n<prompt time=\"1\">&gt;</prompt>\n";

    /// A connection that logs in every time, its lines kept.
    #[derive(Clone, Default)]
    struct Connections(std::sync::Arc<std::sync::Mutex<Vec<cena_platform::TranscriptHandle>>>);

    impl Connections {
        fn latest(&self) -> Option<cena_platform::TranscriptHandle> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .last()
                .cloned()
        }
    }

    impl cena_session::Connector for Connections {
        type Source = AnsweringSource;

        async fn connect(
            &mut self,
            _generation: Generation,
        ) -> Result<AnsweringSource, cena_session::ConnectError> {
            let (source, transcript) = AnsweringSource::logged_in(TACKLE);
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(transcript);
            Ok(source)
        }
    }

    /// Until the next `Ready`; false if the session ends, or an hour goes
    /// by without one.
    async fn ready(events: &mut broadcast::Receiver<Event>) -> bool {
        let next = async {
            loop {
                match events.recv().await {
                    Ok(Event::StateChanged(State::Ready)) => return true,
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return false,
                }
            }
        };
        tokio::time::timeout(Duration::from_hours(1), next)
            .await
            .unwrap_or(false)
    }

    /// A send set off on one connection and heard after it was replaced is
    /// not made on the new one: not to the game, and not as a `;` command
    /// (the crate review of 2026-09-28, R2). One set off on the new
    /// connection is, and names it.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_send_from_a_replaced_connection_is_not_made() {
        let connections = Connections::default();
        let (session, handle) = cena_session::SupervisedSession::new(connections.clone());
        let (_, mut events) = session.subscribe();
        let commands = Commands::install(&handle);
        crate::sorter::open(&handle, &commands, None);
        let tackle = cena_session::trigger::Trigger {
            name: "react".into(),
            rule: cena_session::trigger::Rule {
                pattern: Some(cena_session::trigger::Pattern::Regex(
                    r"You could use this opportunity to (\w+)".into(),
                )),
                send: Some("weapon $1".into()),
                ..cena_session::trigger::Rule::default()
            },
        };
        handle.set_triggers(cena_session::trigger::Matcher::new(vec![tackle]).unwrap_or_default());
        let cancel = session.cancel_token();
        let task = tokio::spawn(session.run());
        assert!(ready(&mut events).await);
        let then = handle.generation();

        connections.latest().expect("a connection").hang_up();
        assert!(ready(&mut events).await, "the session reconnects");
        assert_ne!(handle.generation(), then);
        send(&handle, then, "stun", "stand", &[]).await;
        send(&handle, then, "sort", ".sorter on", &[]).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let now = connections.latest().expect("the new connection");
        assert!(now.lines().is_empty(), "{:?}", now.lines());
        assert!(!handle.sorts_containers(), "no command ran");
        assert!(matches!(
            handle
                .send_at(then, "stand", Origin::Trigger, Gate::None)
                .await,
            Sent::Interrupted
        ));

        while events.try_recv().is_ok() {}
        send(&handle, handle.generation(), "stun", "stand", &[]).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(now.lines(), ["stand"], "the new connection's own");
        let answered: Vec<Generation> = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                Event::Act { generation, .. } => Some(generation),
                _ => None,
            })
            .collect();
        assert_eq!(answered, [handle.generation()], "its answer set off a send");
        cancel.cancel();
        let _ = task.await;
    }

    /// A send reaching a session still learning its character is refused,
    /// and said; the player's own line is not (the crate review of
    /// 2026-09-28, R3: what the player may type is no policy for what a
    /// trigger may send).
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_send_waits_for_ready_as_the_player_does_not() {
        let (source, transcript) = AnsweringSource::new(PROMPT);
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        tokio::spawn(session.into_actor().run());
        tokio::time::sleep(Duration::from_secs(1)).await;

        send(&handle, handle.generation(), "stun", "stand", &[]).await;
        let typed = handle.send_now("look", Origin::Manual, Gate::None).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(matches!(typed, Sent::Ok { .. }), "{typed:?}");
        assert_eq!(transcript.lines(), ["look"], "the trigger's line refused");
        let (_, notices) = told(&mut events);
        assert!(
            notices
                .iter()
                .any(|notice| notice.contains("Trigger `stun`") && notice.contains("not sent")),
            "{notices:?}"
        );
    }

    /// What a trigger sends never gives the commands only the player may:
    /// as written, in a `.multi` split by commas or by semicolons, or filled
    /// in from what another player said. The player's own typing still does
    /// (the crate review of 2026-10-01, BI-D-1, BI-D-2, MO-F-3).
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_triggers_send_never_gives_the_players_own_commands() {
        use cena_session::trigger::{Matcher, Pattern, Rule, Trigger};
        let (source, transcript) = AnsweringSource::logged_in(PROMPT);
        let session = Session::new(source);
        let handle = session.handle();
        let observer = session.observer();
        let (_, mut waiting) = session.subscribe();
        let (_, mut events) = session.subscribe();
        let commands = Commands::install(&handle);
        crate::batch::open(&handle, &observer, &commands);
        let ran = crate::commands::stand_ins(&commands);
        tokio::spawn(session.into_actor().run());
        while !matches!(waiting.recv().await, Ok(Event::StateChanged(State::Ready))) {}
        let now = handle.generation();

        for line in [
            ".agent level takeover",
            ".trigger approve theirs",
            ".lich on",
            ".to Baelor look",
            ".all stand",
            ".multi 1,look,.agent level takeover",
            ".multi 1;look;.all stand",
        ] {
            send(&handle, now, "puppet", line, &[]).await;
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        // Another player's words, through a regex group.
        let heard = |send: &str| Trigger {
            name: "puppet".into(),
            rule: Rule {
                pattern: Some(Pattern::Regex(r#"Baelor whispers, "(.+)""#.into())),
                send: Some(send.into()),
                ..Rule::default()
            },
        };
        for (template, said) in [
            ("$1", ".agent level takeover"),
            (".multi 1,say $1", "hi,.agent level takeover"),
            (".multi 1,say $1", "hi;.all stand"),
            (".multi 1,say $1", "hello"),
        ] {
            let matcher = Matcher::new(vec![heard(template)]).unwrap();
            let line = cena_session::Line::new(
                "",
                cena_session::ChunkLine::plain(&format!("Baelor whispers, \"{said}\"")).runs,
            );
            for act in matcher
                .answer(&line, &cena_session::GameState::default())
                .acts
            {
                send(&handle, now, &act.trigger, &act.line, &act.captured).await;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        assert!(ran.lock().unwrap().is_empty(), "{:?}", ran.lock().unwrap());
        assert_eq!(
            transcript.lines(),
            ["look", "look", "say hello"],
            "each multi's game line before its refusal, and the say a group filled in"
        );
        let (_, notices) = told(&mut events);
        assert!(
            notices
                .iter()
                .any(|notice| notice.contains("is only for you to type")),
            "{notices:?}"
        );
        assert!(
            notices
                .iter()
                .any(|notice| notice.contains("would choose the command")),
            "{notices:?}"
        );

        // The player's own typing still reaches each.
        for line in [".agent level takeover", ".all stand"] {
            handle
                .send_manual_at(now, line, Duration::from_secs(5))
                .await;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(*ran.lock().unwrap(), ["agent level takeover", "all stand"]);
    }

    /// Where each `{...}` is, the braces gone: a line and its captured spans.
    fn marked(written: &str) -> (String, Vec<Range<usize>>) {
        let (mut line, mut spans, mut from) = (String::new(), Vec::new(), None);
        for c in written.chars() {
            match c {
                '{' => from = Some(line.len()),
                '}' => spans.extend(from.take().map(|start| start..line.len())),
                c => line.push(c),
            }
        }
        (line, spans)
    }

    /// The game's words may fill in a command, never choose one (the crate
    /// review of 2026-10-01, MO-F-3).
    #[test]
    fn the_games_words_fill_in_a_command_and_never_choose_it() {
        let chosen = |written: &str| {
            let (line, spans) = marked(written);
            chosen_by_the_game(&line, &spans, '.')
        };
        for fills in [
            "say {hello there}",
            "weapon {tackle}",
            "say {hi, all; of you}",
            ".sc 401 {Baelor}",
            ".multi 1,say {hello}",
            "whisper {Baelor} {what, now}",
            "plain",
        ] {
            assert!(!chosen(fills), "{fills}");
        }
        for chooses in [
            "{.agent level takeover}",
            "{ look}",
            ".{hunt stop}",
            ". {hunt stop}",
            ".multi 1,say {hi,.agent level takeover}",
            ".multi 1,say {hi;.all stand}",
            ".multi 1,{stand}",
            ".multi 1, .{go2 bank}",
            ".foreach in backpack; {sell}",
            ".foreach in {bag | .all stand}",
        ] {
            assert!(chosen(chooses), "{chooses}");
        }
    }
}
