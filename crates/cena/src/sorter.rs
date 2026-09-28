//! `;sorter`: show this character's container looks one line per category
//! (`plan/30` §4, M6e).
//!
//! The join only. The sorting is the session's: with it on, a container look
//! is published sorted, to every viewer, and the model and the player log
//! keep it whole ([`SessionHandle::sort_containers`], `plan/45` §4a). This is
//! the word on Hydra's command line that flips it. It needs no page open, and
//! it sends nothing to the game.
//!
//! The words are `VellumFE`'s `.sorter`
//! (`src/core/app_core/commands.rs:2455-2487`): `on`, `off`, and nothing at
//! all to toggle, answered as `VellumFE` answers, `Container-look sorting
//! on.` `status` is Hydra's own, because there is no settings window to look
//! in; `VellumFE`'s `edit` opens its GUI editor, which Hydra has not got.
//!
//! **Off until asked, and saved.** Off is `VellumFE`'s default
//! (`src/config/settings.rs:860`, `enabled: false`), and `VellumFE` saves the
//! toggle to its config file. So does Hydra now, in the character's settings
//! file, the `sorter` section (author, 2026-09-27: *"sure and persistent"*,
//! `plan/50` §6 item 4), through the settings menu's own writer
//! ([`crate::general`]); a character starts as it was left.

use std::sync::Arc;

use cena_session::command::claimant::Claimed;
use cena_session::{Notice, NoticeKind, SessionHandle};

use crate::commands::Commands;
use crate::general::{GENERAL, Kept};

/// The `sorter` section of a character's settings file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Saved {
    /// Whether container looks are sorted; off when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) enabled: Option<bool>,
}

/// The name of [`Saved`]'s section.
pub(crate) const SECTION: &str = "sorter";

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

/// Do `parsed` to `handle`'s session, save it in `kept` when there is
/// somewhere to keep it, and say what became of it.
fn answer(parsed: Result<Command, String>, handle: &SessionHandle, kept: Option<&Kept>) -> Notice {
    let now = handle.sorts_containers();
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
    handle.sort_containers(on);
    let saved = match (parsed, kept) {
        (Ok(Command::Status), _) | (_, None) => Ok(String::new()),
        (_, Some(kept)) => kept.change(GENERAL, "sorter", Some(state(on))),
    };
    match saved {
        Ok(_) => Notice::line(
            NoticeKind::Info,
            format!("Container-look sorting {}.", state(on)),
        ),
        Err(why) => Notice::line(
            NoticeKind::Warn,
            format!(
                "Container-look sorting {}, for this session: {why}",
                state(on)
            ),
        ),
    }
}

/// Register `;sorter` on `handle`'s command line, the session sorting as
/// `kept`, the character's settings file, was left.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, kept: Option<Kept>) {
    if let Some(kept) = &kept {
        kept.take(handle);
    }
    let told = handle.clone();
    commands.sorter(Arc::new(move |line: &str| {
        let parsed = parse(line)?;
        told.say(answer(parsed, &told, kept.as_ref()));
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

    /// Typed on the command line, `;sorter` flips the session, says so, and
    /// sends the game nothing. No page is open: it needs none.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn the_command_flips_the_session_and_sends_nothing() {
        let (source, transcript) = AnsweringSource::new(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        let generation = handle.generation();
        let commands = Commands::install(&handle);
        open(&handle, &commands, None);
        tokio::spawn(session.into_actor().run());

        let mut typed = async |line: &str| {
            let outcome = handle.send_manual_at(generation, line, DEADLINE).await;
            assert_eq!(outcome, Outcome::Handled, "{line}");
            told(&mut events)
        };
        assert!(!handle.sorts_containers(), "off until asked");
        assert_eq!(typed(";sorter on").await, ["Container-look sorting on."]);
        assert!(handle.sorts_containers());
        assert_eq!(
            typed(";sorter status").await,
            ["Container-look sorting on."]
        );
        assert!(handle.sorts_containers());
        assert_eq!(typed(";sorter").await, ["Container-look sorting off."]);
        assert!(!handle.sorts_containers());
        let refused = typed(";sorter edit").await;
        assert!(
            refused.len() == 1
                && refused[0].contains("`edit`")
                && refused[0].contains("currently off"),
            "{refused:?}"
        );
        assert!(!handle.sorts_containers());
        assert!(transcript.lines().is_empty(), "{:?}", transcript.lines());
    }

    /// Saved: `;sorter on` is kept in the character's settings file, and
    /// the character starts sorting the next time, as it was left.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn the_sorter_is_saved_and_started_as_it_was_left() {
        let dir = std::env::temp_dir().join(format!("cena-sorter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let roster_name = format!("{}:Nisugi", cena_platform::DEFAULT_GAME_CODE);
        let kept = || Kept::of(&dir, &roster_name);

        let (source, _) = AnsweringSource::new(
            b"<prompt time=\"1\">&gt;</prompt>
",
        );
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        let generation = handle.generation();
        let commands = Commands::install(&handle);
        open(&handle, &commands, kept());
        assert!(!handle.sorts_containers(), "off until asked");
        tokio::spawn(session.into_actor().run());
        let outcome = handle
            .send_manual_at(generation, ";sorter on", DEADLINE)
            .await;
        assert_eq!(outcome, Outcome::Handled);
        assert_eq!(told(&mut events), ["Container-look sorting on."]);

        let (source, _) = AnsweringSource::new(b"");
        let again = Session::new(source).handle();
        open(&again, &Commands::install(&again), kept());
        assert!(again.sorts_containers(), "as it was left");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
