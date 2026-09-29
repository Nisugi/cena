//! `;to <character> <command>` and `;all <command>`: a line typed on one
//! character and sent on another, or on every one -- a **relay**
//! (`plan/47` step 5).
//!
//! The author: *"in my head you have one command input period. sending a
//! command in it sends a command on that character. if you want to send a
//! command to another session then you preface it with a command. like the
//! borg trio of scripts you would `;queen <character> <command>` to send it to
//! that character and that character send it essentially. or `;queen wall
//! <command>` to send it to all characters connected to the borg. Our command
//! wouldn't be ;queen, and what ever it is they'd be able to alias it to
//! something different when we get the alias system in."*
//!
//! Two words, kept apart, so a character named All is never a broadcast.
//! The line goes on each character as if typed there: its own command line
//! first (`;to Baelor ;go2 bank` walks Baelor), then the game. A character is
//! named in full, or by the start of its name when only one fits; one whose
//! name is on two games is named with its game, `;to GSF:Baelor look`, and
//! the name alone is refused rather than sent to either (the crate review of 2026-09-28, R6).
//!
//! **`;all` can leave characters out, or name only some** (the author,
//! 2026-09-29: *"should probably have an include/exclude list"*):
//! `;all -Dicate,Maravel stand` sends on everyone but those two, and
//! `;all +Nisugi,Dicate stand` on those two alone. Each name is picked as
//! `;to` picks one; one that picks nobody refuses the whole line, so a typo
//! never sends on the wrong characters.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use cena_session::{Notice, NoticeKind, Outcome, SessionHandle};

use crate::commands::{Commands, Took};

/// The characters running now: what a relay may send on. A closure, so the
/// session table stays the binary's.
pub(crate) type Characters =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Vec<Running>> + Send>> + Send + Sync>;

/// A character running now.
#[derive(Clone)]
pub(crate) struct Running {
    /// The game it is on, by its code (`GS3`).
    pub(crate) game: String,
    /// Its name.
    pub(crate) name: String,
    /// Its session.
    pub(crate) handle: SessionHandle,
}

impl Running {
    /// How the player tells it apart among `running`: its name, with its
    /// game when another there has the same name.
    fn label(&self, running: &[Running]) -> String {
        let twice = running
            .iter()
            .filter(|other| other.name.eq_ignore_ascii_case(&self.name))
            .count()
            > 1;
        if twice {
            format!("{}:{}", self.game, self.name)
        } else {
            self.name.clone()
        }
    }
}

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
    /// `;all [-names|+names] <line>`: `line` on every running character, or
    /// all but some, or only some.
    All {
        /// Who, of those running.
        who: Who,
        /// What to send on each.
        line: String,
    },
}

/// Which running characters an `;all` is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Who {
    /// Every one.
    Everyone,
    /// Every one but these, as typed.
    Except(Vec<String>),
    /// Only these, as typed.
    Only(Vec<String>),
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
        let (first, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let names = |list: &str| -> Vec<String> {
            list.split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect()
        };
        let (who, line) = match (first.strip_prefix('-'), first.strip_prefix('+')) {
            (Some(list), _) => (Who::Except(names(list)), after.trim()),
            (_, Some(list)) => (Who::Only(names(list)), after.trim()),
            _ => (Who::Everyone, rest),
        };
        if matches!(&who, Who::Except(list) | Who::Only(list) if list.is_empty()) {
            return Some(Err(
                "All: name characters after - or +, as `all -Dicate,Maravel stand`.",
            ));
        }
        if line.is_empty() {
            return Some(Err(
                "All: say what every character is to do, as `all stand`.",
            ));
        }
        return Some(Ok(Relay::All {
            who,
            line: line.to_owned(),
        }));
    }
    None
}

/// The character `typed` picks among `running`: `GAME:Name` on that game,
/// or a name on whichever game has it. Its whole name, whatever the case,
/// or else the one whose name starts so.
///
/// # Errors
///
/// What to tell the player: nobody fits, or more than one does -- a whole
/// name on two games among them, which went to the first found
/// (the crate review of 2026-09-28, R6).
pub(crate) fn pick<'a>(typed: &str, running: &'a [Running]) -> Result<&'a Running, String> {
    let (game, name) = match typed.split_once(':') {
        Some((game, name)) => (Some(game.trim()), name.trim()),
        None => (None, typed),
    };
    let on_game = || {
        running
            .iter()
            .filter(move |one| game.is_none_or(|game| one.game.eq_ignore_ascii_case(game)))
    };
    let exact: Vec<&Running> = on_game()
        .filter(|one| one.name.eq_ignore_ascii_case(name))
        .collect();
    let lower = name.to_lowercase();
    let fits = if exact.is_empty() {
        on_game()
            .filter(|one| one.name.to_lowercase().starts_with(&lower))
            .collect()
    } else {
        exact
    };
    let labels = |fits: &[&Running]| {
        fits.iter()
            .map(|one| one.label(running))
            .collect::<Vec<_>>()
    };
    match fits.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("To: no character named {typed} is running here.")),
        [first, rest @ ..]
            if rest
                .iter()
                .all(|one| one.name.eq_ignore_ascii_case(&first.name)) =>
        {
            Err(format!(
                "To: {} is on more than one game; say which, as {}.",
                first.name,
                labels(&fits).join(" or ")
            ))
        }
        many => Err(format!(
            "To: {typed} could be {}; say more of the name.",
            labels(many).join(" or ")
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
async fn relay_on(told: &SessionHandle, relay: &Relay, running: &[Running]) {
    let (targets, line) = match relay {
        Relay::To { name, line } => match pick(name, running) {
            Ok(one) => (vec![one.clone()], line),
            Err(why) => {
                told.say(Notice::line(NoticeKind::Error, why));
                return;
            }
        },
        Relay::All { who, line } => match chosen(who, running) {
            Ok(targets) => (targets, line),
            Err(why) => {
                told.say(Notice::line(NoticeKind::Error, why));
                return;
            }
        },
    };
    if targets.is_empty() {
        told.say(Notice::line(
            NoticeKind::Warn,
            format!("All: {line} -- nobody is left to send it on."),
        ));
        return;
    }
    if matches!(relay, Relay::All { .. }) {
        let names: Vec<String> = targets.iter().map(|one| one.label(running)).collect();
        told.say(Notice::line(
            NoticeKind::Info,
            format!("All: {line} -- on {}.", names.join(", ")),
        ));
    }
    let mut sending = tokio::task::JoinSet::new();
    for target in targets {
        let (name, target) = (target.label(running), target.handle);
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

/// The characters `who` names among `running`, each name picked as `;to`
/// picks one.
///
/// # Errors
///
/// A name that picks nobody, or more than one, said as `;to` says it: the
/// whole line is refused.
pub(crate) fn chosen(who: &Who, running: &[Running]) -> Result<Vec<Running>, String> {
    // By place in `running`: two characters are two entries, whatever else
    // they share.
    let picked = |names: &[String]| -> Result<Vec<usize>, String> {
        names
            .iter()
            .map(|name| {
                let one = pick(name, running).map_err(|why| why.replacen("To:", "All:", 1))?;
                Ok(running
                    .iter()
                    .position(|each| std::ptr::eq(each, one))
                    .unwrap_or(usize::MAX))
            })
            .collect()
    };
    let keep = |keep: &dyn Fn(usize) -> bool| -> Vec<Running> {
        running
            .iter()
            .enumerate()
            .filter(|(at, _)| keep(*at))
            .map(|(_, one)| one.clone())
            .collect()
    };
    Ok(match who {
        Who::Everyone => running.to_vec(),
        Who::Except(names) => {
            let out = picked(names)?;
            keep(&|at| !out.contains(&at))
        }
        Who::Only(names) => {
            let only = picked(names)?;
            keep(&|at| only.contains(&at))
        }
    })
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
