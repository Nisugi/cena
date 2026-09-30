//! `;scripts check <script>`: which lines of one of the player's scripts
//! will not work under Hydra, and why, before running it (`plan/46` §1; the
//! checker is `cena_agent::scripts::checker`).
//!
//! The script is found as Lich finds one ([`super::find_script`]) and read
//! by the runner's own Ruby, against the runner's own names. What is said is
//! the verdict and each thing found, once, at the first line it is on, with
//! how many more lines have it; a thing the script does itself (a name
//! defined nowhere, a method Ruby 4.0 dropped) is marked as the script's.

use std::path::PathBuf;
use std::sync::Arc;

use cena_agent::scripts::checker::{self, Checked, Finding, Verdict};
use cena_agent::scripts::runner;
use cena_session::{Notice, NoticeKind, SessionHandle};

use super::Shared;

/// How many things one check says, the first by line.
pub(super) const SAID: usize = 15;

/// Check the script `word` names, and say what was found.
pub(super) fn check(word: &str, shared: &Arc<Shared>, told: &SessionHandle) {
    let word = word.trim().to_ascii_lowercase();
    if word.is_empty() {
        told.say(
            Notice::line(
                NoticeKind::Error,
                "Scripts: check which? scripts check <script>",
            )
            .answering(),
        );
        return;
    }
    let Some(path) = super::find_script(&shared.dir.join("scripts"), &word) else {
        told.say(
            Notice::line(
                NoticeKind::Error,
                format!(
                    "Scripts: no script named {word} in {}.",
                    shared.dir.join("scripts").display()
                ),
            )
            .answering(),
        );
        return;
    };
    let (shared, told) = (Arc::clone(shared), told.clone());
    tokio::spawn(async move {
        let said = match run(&shared, path).await {
            Ok(checked) => said(&checked),
            Err(why) => Notice::line(NoticeKind::Error, format!("Scripts: {why}")),
        };
        told.say(said.answering());
    });
}

async fn run(shared: &Shared, path: PathBuf) -> Result<Checked, String> {
    let ruby = runner::find_ruby().ok_or(super::NO_RUBY)?;
    let dir = super::unpacked(shared)?;
    checker::check(&ruby, &dir, &shared.dir.join("lich"), &[path])
        .await?
        .pop()
        .ok_or_else(|| "the checker said nothing".to_owned())
}

/// What a check says: the verdict, then what was found.
pub(super) fn said(checked: &Checked) -> Notice {
    let mut lines = vec![format!(
        "{} ({} lines): {}",
        checked.name,
        checked.lines,
        verdict(checked.verdict)
    )];
    if checked.builtin {
        lines.push(format!(
            "Hydra has {} built in: its own runs in this one's place, typed or started by a script.",
            checked.name
        ));
    }
    let mut seen: Vec<(&Finding, usize)> = Vec::new();
    for finding in &checked.findings {
        match seen
            .iter_mut()
            .find(|(first, _)| first.what == finding.what)
        {
            Some((_, more)) => *more += 1,
            None => seen.push((finding, 0)),
        }
    }
    for (finding, more) in seen.iter().take(SAID) {
        let also = match more {
            0 => String::new(),
            1 => " (and 1 more line)".to_owned(),
            more => format!(" (and {more} more lines)"),
        };
        let whose = if finding.hydra {
            ""
        } else {
            " -- the script's own"
        };
        lines.push(format!(
            "  line {}{also}  {}: {}{whose}",
            finding.line, finding.what, finding.why
        ));
    }
    if seen.len() > SAID {
        lines.push(format!("  and {} more things.", seen.len() - SAID));
    }
    let kind = match checked.verdict {
        Verdict::Runs => NoticeKind::Info,
        Verdict::Stops => NoticeKind::Error,
        Verdict::Markup | Verdict::Windows | Verdict::Differs => NoticeKind::Warn,
    };
    Notice::table(kind, lines)
}

fn verdict(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Runs => "nothing found that Hydra does not answer, as far as the checker can see",
        Verdict::Stops => "stops where a line below raises under Hydra, if it gets there",
        Verdict::Markup => "runs, but reads the game's markup, which Hydra does not give scripts",
        Verdict::Windows => {
            "runs, and opens Lich's windows: through the gtk3 gem, which the runner loads for now"
        }
        Verdict::Differs => "runs, and does not do all it did under Lich",
    }
}
