//! What the player's Lich shows, shown as the character's text
//! (`plan/51` §7, step 3).
//!
//! The author, 2026-09-28 (§6, question 1), of a character running Lich:
//! Lich's standard output is its text, so Lich's squelches hold and its
//! scripts' messages show, as in any frontend. So from the chunk a Lich
//! takes the copy of (`EventPublisher::wire`), what a viewer is shown --
//! [`Event::Line`] and [`Event::Prompt`] -- comes from what Lich writes,
//! parsed by a parser of its own and put together by the model's own
//! [`Unfinished`](cena_model::line::Unfinished)
//! ([`LichText`](crate::script::lich::LichText)).
//!
//! The game's own parse is everything else, as it was: the model and the
//! panels read from it, the player log, a script runner's lines
//! ([`Event::Heard`]), and what the triggers do beyond the line -- a flag,
//! attention, a send -- which acts once, on the game's line. What the
//! triggers make a line look like, and `;sorter`, are what a frontend does
//! with what it is given, so they answer Lich's lines instead.
//!
//! # A quiet command's report
//!
//! A frontend leaves a quiet command's report out between
//! [`Event::Quiet`]s, which bracket the game's lines. Lich's copy of them
//! comes later, after the window has closed, so while Lich shows the text
//! the session leaves the report out itself, and publishes no `Quiet`: each
//! line of main in the window is expected back, and left out when Lich
//! passes it on, in order, among Lich's own lines (`script/lich/text.rs`,
//! `Echoes`). What Lich hid of it is passed over, and what it hid wholly is
//! let go once Lich has shown enough else. Lich's own quiet commands hide
//! their prompt as well as their lines
//! (`reference/lich-5/lib/util/util.rb:177-178`), so the prompts cannot be
//! counted to find where the report falls in what Lich wrote.
//!
//! The login a Lich started late is handed is left out the same way: the
//! character's text showed what it tells once already.

use std::sync::Arc;

use cena_model::line::Line;
use cena_platform::ByteSource;

use super::{Event, SessionActor};
use crate::script::lich::{LichText, Showing};

/// A quiet command's window, while one is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QuietWindow {
    /// Viewers were told ([`Event::Quiet`]), and leave its report out.
    Told,
    /// The player's Lich shows the text, and the report is left out of
    /// what it shows here.
    Kept,
}

/// The next chunk the player's Lich wrote; never, with none showing.
pub(super) async fn next(text: &mut Option<LichText>) -> Option<Vec<u8>> {
    match text {
        Some(text) => text.next().await,
        None => std::future::pending().await,
    }
}

/// Whether `stream` is main's: the wire writes it with no id.
pub(super) fn is_main(stream: &str) -> bool {
    stream.is_empty() || stream == "main"
}

impl<S: ByteSource> SessionActor<S> {
    /// Take what a Lich shows, if some waits: a new Lich's, or what the last
    /// connection's actor showed. What this actor held belonged to a Lich
    /// that is gone.
    ///
    /// And a Lich attached since the last turn is handed a login built from
    /// the model as it stands (`GameState::login`), before the next chunk is
    /// read, so it misses nothing and is told nothing twice; what it shows of
    /// that login is left out of the character's text, which showed it once.
    pub(super) fn take_lich_text(&mut self) {
        if let Some(text) = self.events.lich_text().take() {
            self.lich_text = Some(text);
        }
        if self.events.lich_waits() {
            // Parked with the Lich, under its lock: here now if not before.
            if let Some(text) = self.events.lich_text().take() {
                self.lich_text = Some(text);
            }
            let login = self.state.login();
            if let Some(text) = self.lich_text.as_mut() {
                text.expect_login(login.as_bytes());
            }
            self.events.begin_lich(login.as_bytes());
        }
    }

    /// Show what the player's Lich wrote: each line it finishes, as a line
    /// the game finished would be, and each prompt, in order; but what it
    /// was expected to pass on and not to show.
    pub(super) fn show_lichs(&mut self, chunk: &[u8]) {
        let Some(text) = self.lich_text.as_mut() else {
            return;
        };
        for showing in text.read(chunk) {
            if self
                .lich_text
                .as_mut()
                .is_some_and(|text| text.echoes(&showing))
            {
                continue;
            }
            match showing {
                Showing::Line(line) => self.show_lichs_line(Arc::new(line)),
                Showing::Prompt(prompt) => {
                    let _ = self.events.send(Event::Prompt(prompt));
                }
            }
        }
    }

    /// A line the game finished while Lich shows the text, of a quiet
    /// command's report: to be left out when Lich passes it on.
    pub(super) fn expect_quiet(&mut self, line: &Line) {
        if self.quiet_window.is_some()
            && is_main(&line.stream)
            && let Some(text) = self.lich_text.as_mut()
        {
            text.expect_quiet(line);
        }
    }
}
