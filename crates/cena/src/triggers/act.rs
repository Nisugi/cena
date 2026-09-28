//! A trigger's send, sent (`plan/45` Stage 5): the session decided it and
//! paced it ([`Event::Act`]); here, where the character's command table is,
//! it goes as if the player had typed it -- to Hydra's `;` commands first,
//! otherwise to the game as [`Origin::Trigger`], which never counts as the
//! player being there. What the game or Hydra refuses is said, naming the
//! trigger. Each character's triggers task ([`super::follow`]) hears the
//! sends and sends them here.
//!
//! [`Event::Act`]: cena_session::Event::Act

use cena_session::command::claimant::Claimed;
use cena_session::{Gate, Notice, NoticeKind, Origin, Sent, SessionHandle};

/// Send `trigger`'s `line`, as typed.
pub(super) async fn send(handle: &SessionHandle, trigger: &str, line: &str) {
    let said = match handle.typed(line) {
        Some(Claimed::Unknown) => Some(format!(
            "Trigger `{trigger}`: Hydra has no command `{}`.",
            line.trim()
        )),
        Some(_) => None,
        None => match handle.send_now(line, Origin::Trigger, Gate::None).await {
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
                Event::Sent { line, origin } => sent.push((line, origin)),
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

        send(&handle, "stun", "stand").await;
        send(&handle, "sort", ";sorter on").await;
        send(&handle, "odd", ";frobnicate").await;
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
}
