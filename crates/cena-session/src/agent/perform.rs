//! Doing an act the level, or the player, allowed, and telling the player
//! so. Moved out of `agent.rs` at its cap.

use super::{Act, Admitted, takeover};
use crate::SessionHandle;
use crate::lifecycle::Generation;
use crate::notice::{Body, Notice, NoticeKind};

impl SessionHandle {
    /// Do an act the level, or the player, allowed on connection
    /// `generation`, as the approval numbered `approval` when it was one. The
    /// player is told what the agent did and why: the audit trail, in their
    /// stream and their log.
    pub(super) fn perform(
        &self,
        act: &Act,
        because: &str,
        approval: Option<u64>,
        generation: Generation,
    ) -> Result<Admitted, String> {
        match act {
            Act::TellPlayer { text } => {
                let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
                if let Some(first) = lines.first_mut() {
                    *first = format!("Agent: {first}");
                }
                lines.push(format!("  (because: {because})"));
                self.say(Notice {
                    kind: NoticeKind::Info,
                    body: Body::Lines(lines),
                });
                Ok(Admitted::Told)
            }
            Act::Perform { line } => {
                let report = crate::operation::start(self, line, approval)?;
                self.say(Notice::line(
                    NoticeKind::Info,
                    format!(
                        "Agent: `{line}` (operation {}), because: {because}",
                        report.id
                    ),
                ));
                Ok(Admitted::Operation(report))
            }
            Act::Command { line } => {
                let report = crate::operation::send(self, line, approval, generation);
                self.say(Notice::line(
                    NoticeKind::Info,
                    format!(
                        "Agent: sent `{line}` (operation {}), because: {because}",
                        report.id
                    ),
                ));
                Ok(Admitted::Operation(report))
            }
            Act::TakeOver => {
                let report = takeover::take_over(self, approval)?;
                self.say(Notice::line(
                    NoticeKind::Warn,
                    format!(
                        "Agent: taking the character over (operation {}), because: {because}. {}agent stop takes it back.",
                        report.id,
                        self.symbol()
                    ),
                ));
                Ok(Admitted::Operation(report))
            }
            Act::Control { operation, control } => {
                let report = crate::operation::steer(self, *operation, *control)?;
                self.say(Notice::line(
                    NoticeKind::Info,
                    format!(
                        "Agent: {} operation {operation} (`{}`), because: {because}",
                        control.word(),
                        report.line
                    ),
                ));
                Ok(Admitted::Operation(report))
            }
        }
    }

    /// The player's command symbol, for the words that name a command.
    pub(super) fn symbol(&self) -> char {
        self.command_symbol()
            .unwrap_or(crate::command::claimant::DEFAULT_SYMBOL)
    }
}
