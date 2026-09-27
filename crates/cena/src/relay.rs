//! `;to <character> <command>` and `;all <command>`: a line typed on one
//! character and sent on another, or on every one -- a **relay**
//! (`plan/47` step 5).
//!
//! The author: *"in my head you have one command input period. sending a
//! command in it sends a command on that character. if you want to send a
//! command to another session then you preface it with a command. like the
//! borg trio of scripts you would ;queen <character> <command> to send it to
//! that character and that character send it essentially. or ;queen wall
//! <command> to send it to all characters connected to the borg. Our command
//! wouldn't be ;queen, and what ever it is they'd be able to alias it to
//! something different when we get the alias system in."*
//!
//! Two words, kept apart, so a character named All is never a broadcast.
//! The line goes on each character as if typed there: its own command line
//! first (`;to Baelor ;go2 bank` walks Baelor), then the game. A character is
//! named in full, or by the start of its name when only one fits.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use cena_session::{Notice, NoticeKind, Outcome, SessionHandle};

use crate::commands::{Commands, Took};

/// The characters running now, each by name with its handle: what a relay
/// may send on. A closure, so the session table stays the binary's.
pub(crate) type Characters = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Vec<(String, SessionHandle)>> + Send>> + Send + Sync,
>;

/// How long a relayed line waits for its answer, as a typed one does.
const DEADLINE: Duration = Duration::from_secs(10);

/// A relay, as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Relay {
    /// `;to <name> <line>`: `line` on the character `name` picks.
    To {
        /// The character, in full or its start.
        name: String,
        /// What to send on it.
        line: String,
    },
    /// `;all <line>`: `line` on every running character.
    All(String),
}

/// The relay a line (without its symbol) asks for: `None` when it is not
/// `to` or `all`; `Err` with how to use it when it is, but says too little.
pub(crate) fn parse(line: &str) -> Option<Result<Relay, &'static str>> {
    let line = line.trim_start();
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    if word.eq_ignore_ascii_case("to") {
        let Some((name, line)) = rest.split_once(char::is_whitespace) else {
            return Some(Err(
                "To: name a character and a command, as `to Baelor look`.",
            ));
        };
        return Some(Ok(Relay::To {
            name: name.to_owned(),
            line: line.trim().to_owned(),
        }));
    }
    if word.eq_ignore_ascii_case("all") {
        if rest.is_empty() {
            return Some(Err(
                "All: say what every character is to do, as `all stand`.",
            ));
        }
        return Some(Ok(Relay::All(rest.to_owned())));
    }
    None
}

/// The character `name` picks among `running`: its whole name, whatever the
/// case, or else the one whose name starts so.
///
/// # Errors
///
/// What to tell the player: nobody fits, or more than one does.
pub(crate) fn pick<'a>(
    name: &str,
    running: &'a [(String, SessionHandle)],
) -> Result<&'a (String, SessionHandle), String> {
    if let Some(exact) = running
        .iter()
        .find(|(character, _)| character.eq_ignore_ascii_case(name))
    {
        return Ok(exact);
    }
    let lower = name.to_lowercase();
    let fits: Vec<&(String, SessionHandle)> = running
        .iter()
        .filter(|(character, _)| character.to_lowercase().starts_with(&lower))
        .collect();
    match fits.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("To: no character named {name} is running here.")),
        many => Err(format!(
            "To: {name} could be {}; say more of the name.",
            many.iter()
                .map(|(character, _)| character.as_str())
                .collect::<Vec<_>>()
                .join(" or ")
        )),
    }
}

/// Register `;to` and `;all` on `handle`'s command line, sending on the
/// characters `characters` lists.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, characters: Characters) {
    let told = handle.clone();
    commands.relay(Arc::new(move |line: &str| {
        let relay = match parse(line)? {
            Ok(relay) => relay,
            Err(how) => {
                told.say(Notice::line(NoticeKind::Error, how));
                return Some(Took::Done);
            }
        };
        let (told, characters) = (told.clone(), Arc::clone(&characters));
        Some(Took::Started(tokio::spawn(async move {
            relay_on(&told, &relay, &characters().await).await;
        })))
    }));
}

/// Send `relay` on the characters it names among `running`, saying on `told`
/// -- the character it was typed on -- whatever did not go.
async fn relay_on(told: &SessionHandle, relay: &Relay, running: &[(String, SessionHandle)]) {
    let (targets, line) = match relay {
        Relay::To { name, line } => match pick(name, running) {
            Ok(one) => (vec![one.clone()], line),
            Err(why) => {
                told.say(Notice::line(NoticeKind::Error, why));
                return;
            }
        },
        Relay::All(line) => (running.to_vec(), line),
    };
    if matches!(relay, Relay::All(_)) {
        let names: Vec<&str> = targets.iter().map(|(name, _)| name.as_str()).collect();
        told.say(Notice::line(
            NoticeKind::Info,
            format!("All: {line} -- on {}.", names.join(", ")),
        ));
    }
    let mut sending = tokio::task::JoinSet::new();
    for (name, target) in targets {
        let line = line.clone();
        sending.spawn(async move {
            let outcome = target
                .send_manual_at(target.generation(), &line, DEADLINE)
                .await;
            (name, outcome)
        });
    }
    while let Some(Ok((name, outcome))) = sending.join_next().await {
        if let Some(why) = unsent(&outcome) {
            told.say(Notice::line(NoticeKind::Warn, format!("To {name}: {why}")));
        }
    }
}

/// Why a relayed line may not have gone, or `None` when it did.
fn unsent(outcome: &Outcome) -> Option<&'static str> {
    match outcome {
        Outcome::Handled | Outcome::Confirmed(_) => None,
        Outcome::Refused(_) => Some("not sent; the session refused it."),
        Outcome::Timeout => Some("no answer yet; it may have reached the game."),
        Outcome::Interrupted | Outcome::Dead | Outcome::Disconnected => {
            Some("the connection was interrupted; it may not have reached the game.")
        }
    }
}

#[cfg(test)]
mod tests;
