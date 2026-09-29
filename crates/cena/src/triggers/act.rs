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
//! [`Event::Act`]: cena_session::Event::Act

use cena_session::command::claimant::Claimed;
use cena_session::{Gate, Generation, Notice, NoticeKind, Origin, Sent, SessionHandle};

/// Send `trigger`'s `line`, as typed, if the connection `generation` that
/// set it off is still the session's.
pub(super) async fn send(
    handle: &SessionHandle,
    generation: Generation,
    trigger: &str,
    line: &str,
) {
    if handle.generation() != generation {
        return;
    }
    let said = match handle.typed(line) {
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
        send(&handle, now, "stun", "stand").await;
        send(&handle, now, "sort", ";sorter on").await;
        send(&handle, now, "odd", ";frobnicate").await;
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
                .any(|notice| notice.contains("Trigger `odd`") && notice.contains(";frobnicate")),
            "{notices:?}"
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
        send(&handle, then, "stun", "stand").await;
        send(&handle, then, "sort", ";sorter on").await;
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
        send(&handle, handle.generation(), "stun", "stand").await;
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

        send(&handle, handle.generation(), "stun", "stand").await;
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
}
