//! Hydra's one question to the game: `health`, when the nerves are confused.
//!
//! The model works a nerve rank out from what happened -- damage makes a
//! wound, a herb heals it and leaves a scar -- and only when that does not
//! explain the rank is it confused (`cena_model`'s `character/nerves.rs`,
//! `plan/55` §4a). Then the session sends `health` once, as
//! [`Origin::Hydra`](crate::command::Origin::Hydra): after the login, never
//! while dead, never preempting a behavior. The six nerve lines in its reply
//! are not shown, as Lich returns `nil` for them; the rest shows, as Lich's
//! does. A `health` the player types shows everything.

use tokio::sync::oneshot;

use cena_model::state::character::nerves::is_nerve_line;
use cena_platform::ByteSource;

use super::SessionActor;
use crate::command::{CommandId, Envelope, Gate, Origin, Outcome};

impl<S: ByteSource> SessionActor<S> {
    /// At each prompt: let go of an answered `health`, then send one if the
    /// nerves are confused and the character can be asked.
    pub(super) fn ask_nerves(&mut self) {
        if let Some(answer) = &mut self.nerve_health
            && !matches!(answer.try_recv(), Err(oneshot::error::TryRecvError::Empty))
        {
            self.nerve_health = None;
        }
        if self.nerve_health.is_some()
            || !self.lifecycle.behaviors_may_run()
            || self.state.status.dead()
            || !self.state.character.take_nerve_question()
        {
            return;
        }
        let (reply, answer) = oneshot::channel::<Outcome>();
        self.nerve_health = Some(answer);
        self.admit(Envelope {
            id: CommandId(0),
            line: "health".to_owned(),
            origin: Origin::Hydra,
            reply,
            generation: self.generation,
            // The report ends at its prompt, like any report.
            matcher: crate::queue::any_frame,
            answers: None,
            quiet: false,
            gate: Gate::None,
            revocable: None,
        });
    }

    /// Whether `text` is left out of what viewers are shown: a nerve line in
    /// the reply to Hydra's own `health`.
    pub(super) fn hides(&self, text: &str) -> bool {
        self.nerve_health.is_some() && is_nerve_line(text)
    }
}
