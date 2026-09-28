//! The script checker (`plan/46` §1; `bridges/ruby/hydra/check.rb`): which
//! lines of a Lich script will not work under Hydra, and why, found without
//! running it.
//!
//! **Ruby reads the script, against the runner itself.** The checker is one
//! of the runner's files, run with the player's Ruby: it parses the script
//! with Ruby's own parser (Prism, as the runner will read it), loads Lich's
//! engine and Hydra's edges exactly as a runner does, and judges each name
//! the script uses by whether that runner defines it. So what it reports
//! cannot drift from what the runner answers, and nothing in Rust lists
//! Lich's names. What it cannot see into -- a method of an unknown receiver,
//! a gem's constants -- it does not judge: [`Verdict::Runs`] is nothing
//! found, not everything proven.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;

/// The checker, under the folder [`super::runner::unpack`] writes.
pub const CHECKER: &str = "hydra/check.rb";

/// A script, checked.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Checked {
    /// Its name: the file's, without `.lic`.
    pub name: String,
    /// Hydra has it built in (`go2`): Hydra's own runs in its place, typed
    /// or started by a script (`bridges/ruby/hydra/builtins.rb`).
    pub builtin: bool,
    /// How many lines it has.
    pub lines: u64,
    /// The worst of its findings.
    pub verdict: Verdict,
    /// What was found, by line.
    pub findings: Vec<Finding>,
}

/// A script's verdict: the worst kind found in it, or that none was.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Nothing found.
    Runs,
    /// A line raises under Hydra: the script stops there, if it gets there.
    Stops,
    /// A line reads the game's markup, which Hydra does not give scripts.
    Markup,
    /// A line opens one of Lich's windows: through the gtk3 gem while the
    /// runner loads it (`runner::Start::windows`), and the script stops
    /// there where it cannot.
    Windows,
    /// It runs, and does not do what it did under Lich.
    Differs,
}

/// One thing found.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Finding {
    /// Its line, from 1.
    pub line: u64,
    /// How bad: a [`Verdict`] other than `Runs`.
    pub kind: Verdict,
    /// What: the name, the call or the pattern.
    pub what: String,
    /// Why, in words.
    pub why: String,
    /// Whether it is Hydra's to answer, rather than the script's own: a name
    /// defined nowhere, a method Ruby 4.0 dropped, a library not installed.
    pub hydra: bool,
}

/// Check `scripts` with the checker in `dir` (written by
/// [`super::runner::unpack`]), run by `ruby`; `data` is where a runner keeps
/// its data, which Lich's engine opens as it loads.
///
/// # Errors
///
/// Ruby did not run, or the checker failed: why, in words.
pub async fn check(
    ruby: &Path,
    dir: &Path,
    data: &Path,
    scripts: &[PathBuf],
) -> Result<Vec<Checked>, String> {
    std::fs::create_dir_all(data).map_err(|e| format!("{}: {e}", data.display()))?;
    let output = tokio::process::Command::new(ruby)
        .arg(dir.join(CHECKER))
        .args(scripts)
        .env("HYDRA_DATA", data)
        .current_dir(data)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("Ruby did not start: {e}"))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "the checker failed: {}",
            said.lines().last().unwrap_or("no reason given")
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("the checker's answer: {e}"))
}
