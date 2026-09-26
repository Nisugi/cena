//! `;sorter`: show this character's container looks one line per category
//! (`plan/30` §4, M6e).
//!
//! The join only. The sorting is `cena_ui`'s, over the lines the web pump
//! assembles, and the switch is [`cena_web::Sessions::sort_containers`];
//! this is the word on Hydra's command line that flips it. It sends nothing
//! to the game.
//!
//! The words are `VellumFE`'s `.sorter`
//! (`src/core/app_core/commands.rs:2455-2487`): `on`, `off`, and nothing at
//! all to toggle, answered as `VellumFE` answers, `Container-look sorting
//! on.` `status` is Hydra's own, because there is no settings window to look
//! in; `VellumFE`'s `edit` opens its GUI editor, which Hydra has not got.
//!
//! **Off until asked, for the session's life, not saved.** Off is
//! `VellumFE`'s default (`src/config/settings.rs:860`, `enabled: false`).
//! `VellumFE` saves the toggle to its config file; Hydra does not yet,
//! because nothing writes a character's settings file
//! (`cena_session::settings_store`) and the first thing to do so is the
//! author's call.

use std::sync::Arc;

use cena_session::command::claimant::Claimed;
use cena_session::{Notice, NoticeKind, SessionHandle, SessionId};

use crate::commands::Commands;

/// The words `;sorter` knows.
const USAGE: &str = "sorter [on|off|status]";

/// One `;sorter` command, parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Bare `;sorter`: flip it.
    Toggle,
    /// `;sorter on` or `;sorter off`.
    Set(bool),
    /// `;sorter status`: say which, change nothing.
    Status,
}

/// Parse a command line, without its symbol. `None` when the word is not
/// `sorter`; `Some(Err)` holding the rest of the line when it is not
/// understood.
pub(crate) fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("sorter") {
        return None;
    }
    let rest: Vec<&str> = words.collect();
    let command = match rest.as_slice() {
        [] => Ok(Command::Toggle),
        [word] if word.eq_ignore_ascii_case("on") => Ok(Command::Set(true)),
        [word] if word.eq_ignore_ascii_case("off") => Ok(Command::Set(false)),
        [word] if word.eq_ignore_ascii_case("status") => Ok(Command::Status),
        _ => Err(rest.join(" ")),
    };
    Some(command)
}

/// Do `parsed` to session `id`'s page on `web`, and say what became of it.
fn answer(
    parsed: Result<Command, String>,
    web: Option<&cena_web::Sessions>,
    id: SessionId,
) -> Notice {
    let Some(web) = web else {
        return Notice::line(
            NoticeKind::Warn,
            "Sorter: container looks are sorted in the browser, and this run has no --web.",
        );
    };
    let Some(now) = web.sorts_containers(id) else {
        return Notice::line(
            NoticeKind::Warn,
            "Sorter: this character has no page to sort.",
        );
    };
    let state = |on: bool| if on { "on" } else { "off" };
    let on = match parsed {
        Ok(Command::Toggle) => !now,
        Ok(Command::Set(on)) => on,
        Ok(Command::Status) => now,
        Err(rest) => {
            return Notice::line(
                NoticeKind::Error,
                format!(
                    "Sorter: `{rest}` is not a word it knows. Usage: {USAGE} (currently {}).",
                    state(now)
                ),
            );
        }
    };
    if on != now && !web.sort_containers(id, on) {
        return Notice::line(
            NoticeKind::Warn,
            "Sorter: this character has no page to sort.",
        );
    }
    Notice::line(
        NoticeKind::Info,
        format!("Container-look sorting {}.", state(on)),
    )
}

/// Register `;sorter` on `handle`'s command line, flipping its page on `web`
/// (`None` when this run has no `--web`, and so no page).
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, web: Option<cena_web::Sessions>) {
    let told = handle.clone();
    let id = handle.session();
    commands.sorter(Arc::new(move |line: &str| {
        let parsed = parse(line)?;
        told.say(answer(parsed, web.as_ref(), id));
        Some(Claimed::Done)
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_platform::AnsweringSource;
    use cena_session::{Event, Outcome, Session};
    use std::time::Duration;

    const DEADLINE: Duration = Duration::from_secs(5);

    #[test]
    fn its_words_are_vellumfes_and_status() {
        assert_eq!(parse("sorter"), Some(Ok(Command::Toggle)));
        assert_eq!(parse("SORTER On"), Some(Ok(Command::Set(true))));
        assert_eq!(parse("sorter off"), Some(Ok(Command::Set(false))));
        assert_eq!(parse("sorter status"), Some(Ok(Command::Status)));
        assert_eq!(parse("sorter edit"), Some(Err("edit".to_owned())));
        assert_eq!(parse("sorter on now"), Some(Err("on now".to_owned())));
        assert_eq!(parse("sort on"), None);
        assert_eq!(parse("loot"), None);
    }

    /// What the player was told, as text.
    fn told(events: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
        let mut said = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let Event::Notice(notice) = event {
                said.extend(notice.lines().iter().cloned());
            }
        }
        said
    }

    /// Typed on the command line, `;sorter` flips the session's page, says
    /// so, and sends the game nothing.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn the_command_flips_the_page_and_sends_nothing() {
        let (source, transcript) = AnsweringSource::new(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        let observer = session.observer();
        let (_, mut events) = session.subscribe();
        let generation = handle.generation();
        let commands = Commands::install(&handle);
        let server = cena_web::WebServer::bind(observer, handle.clone())
            .await
            .unwrap();
        let web = server.sessions();
        open(&handle, &commands, Some(web.clone()));
        tokio::spawn(session.into_actor().run());
        let id = handle.session();

        let mut typed = async |line: &str| {
            let outcome = handle.send_manual_at(generation, line, DEADLINE).await;
            assert_eq!(outcome, Outcome::Handled, "{line}");
            told(&mut events)
        };
        assert_eq!(web.sorts_containers(id), Some(false), "off until asked");
        assert_eq!(typed(";sorter on").await, ["Container-look sorting on."]);
        assert_eq!(web.sorts_containers(id), Some(true));
        assert_eq!(
            typed(";sorter status").await,
            ["Container-look sorting on."]
        );
        assert_eq!(web.sorts_containers(id), Some(true));
        assert_eq!(typed(";sorter").await, ["Container-look sorting off."]);
        assert_eq!(web.sorts_containers(id), Some(false));
        let refused = typed(";sorter edit").await;
        assert!(
            refused.len() == 1
                && refused[0].contains("`edit`")
                && refused[0].contains("currently off"),
            "{refused:?}"
        );
        assert_eq!(web.sorts_containers(id), Some(false));
        assert!(transcript.lines().is_empty(), "{:?}", transcript.lines());
    }

    /// With no `--web` there is no page, and it says so rather than
    /// pretending to have sorted something.
    #[test]
    fn with_no_page_it_says_so() {
        let notice = answer(Ok(Command::Set(true)), None, SessionId::FIRST);
        assert_eq!(notice.kind, NoticeKind::Warn);
        assert!(notice.lines()[0].contains("no --web"), "{notice:?}");
    }
}
