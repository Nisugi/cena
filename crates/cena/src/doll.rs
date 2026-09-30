//! `;doll`: the injury doll's one command (`plan/55` step 6).
//!
//! `;doll import <folder>` brings a player's `VellumFE` dolls into Hydra's
//! `dolls` folder, each picture's calibration written into its copy
//! ([`cena_gui::import_dolls`]). `<folder>` is `VellumFE`'s own folder or
//! its dolls folder itself. Once for every character: the dolls folder is
//! Hydra's, not a character's. The copying is done off the typing task.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use cena_session::SessionHandle;
use cena_session::command::claimant::Claimed;
use cena_session::notice::{Notice, NoticeKind};

use crate::commands::Commands;

/// What `;doll help` says.
const HELP: &str = "doll import <folder>   your VellumFE dolls into Hydra's dolls folder, each \
                    picture's calibration written into it. <folder> is VellumFE's own folder or \
                    its dolls folder. Pick a picture on the Injuries widget's page.";

/// What a line of `;doll` asks.
#[derive(Debug, PartialEq, Eq)]
enum Asked {
    Help,
    Import(PathBuf),
}

/// `line`, without its symbol, as `;doll`: `None` when it is not one, an
/// error when it says what it cannot.
fn parse(line: &str) -> Option<Result<Asked, String>> {
    let line = line.trim();
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    if !word.eq_ignore_ascii_case("doll") {
        return None;
    }
    let rest = rest.trim();
    let (verb, folder) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    Some(match verb.to_ascii_lowercase().as_str() {
        "" | "help" => Ok(Asked::Help),
        "import" if folder.trim().is_empty() => {
            Err("Doll: say where your VellumFE dolls are, as `doll import <folder>`.".to_owned())
        }
        "import" => Ok(Asked::Import(PathBuf::from(
            folder.trim().trim_matches('"'),
        ))),
        other => Err(format!("Doll: there is no `doll {other}`. {HELP}")),
    })
}

/// Register `;doll` on a character's command line.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands) {
    let told = handle.clone();
    commands.doll(Arc::new(move |line: &str| {
        let asked = match parse(line)? {
            Ok(asked) => asked,
            Err(why) => {
                told.say(Notice::line(NoticeKind::Error, why).answering());
                return Some(Claimed::Done);
            }
        };
        let folder = match asked {
            Asked::Help => {
                told.say(Notice::line(NoticeKind::Info, HELP).answering());
                return Some(Claimed::Done);
            }
            Asked::Import(folder) => folder,
        };
        let told = told.clone();
        tokio::spawn(async move {
            let into = cena_session::character_store::data_dir().join("dolls");
            let brought =
                tokio::task::spawn_blocking(move || cena_gui::import_dolls(&folder, &into)).await;
            let said = match brought {
                Ok(Ok(brought)) => Notice::line(NoticeKind::Info, said(&brought)).answering(),
                Ok(Err(why)) => Notice::line(NoticeKind::Error, format!("Doll: {why}")).answering(),
                Err(_) => Notice::line(NoticeKind::Error, "Doll: the import stopped.".to_owned())
                    .answering(),
            };
            told.say(said);
        });
        Some(Claimed::Done)
    }));
}

/// What an import says it brought.
fn said(brought: &cena_gui::DollsImported) -> String {
    let mut said = format!(
        "Doll: from {}, {} picture(s) and {} overlay(s); {} calibration(s) written into their \
         pictures.",
        brought.from.display(),
        brought.pictures,
        brought.overlays,
        brought.calibrated
    );
    if !brought.kept.is_empty() {
        let _ = write!(
            said,
            " Already in Hydra's folder, left as they were: {}.",
            brought.kept.join(", ")
        );
    }
    said
}

#[cfg(test)]
mod tests {
    use super::{Asked, parse};
    use std::path::PathBuf;

    #[test]
    fn doll_import_takes_a_folder_quoted_or_not() {
        assert_eq!(parse("look"), None);
        assert_eq!(parse("doll"), Some(Ok(Asked::Help)));
        assert_eq!(
            parse("doll import \"C:/Users/me/.vellum-fe\""),
            Some(Ok(Asked::Import(PathBuf::from("C:/Users/me/.vellum-fe"))))
        );
        assert!(matches!(parse("doll import"), Some(Err(_))));
        assert!(matches!(parse("Doll fly"), Some(Err(_))));
    }
}
