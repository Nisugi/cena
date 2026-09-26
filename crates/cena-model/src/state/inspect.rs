//! What `appraise` and `measure` say about the one thing they are aimed at:
//! the two replies bigshot reads before a sacrifice and a briar raise.
//!
//! Stateless readers over the reply's text, for the behavior that sent the
//! command and holds its reply lines. Nothing here is folded into
//! [`GameState`](super::GameState): the answer is about one command, and the
//! behavior that asked is the one that acts on it.
//!
//! # `appraise #id`, and which line is read
//!
//! bigshot's `cmd_sacrifice` (`reference/scripts/scripts/bigshot.lic:6611-6622`):
//!
//! ```ruby
//! status = Lich::Util.issue_command("appraise ##{target.id}", /^The .+? is \w+ in size/,
//!                                   include_end: false, quiet: true, silent: true).last
//! if status =~ /enticingly frail/
//! ```
//!
//! `issue_command` gathers the lines from the one matching the start pattern
//! up to the prompt, leaving the prompt out (`lib/util/util.rb:202-213`), and
//! bigshot tests the **last** of them. So "enticingly frail" is read off the
//! reply's final line, which is the size line only when the reply is one
//! line. The other scripts that read the same reply read it the same way and
//! name its other verdicts: `susceptible to manipulation`, and a soul
//! `stalwart and formidable`, `firmly bound` or `indomitable`
//! (`reference/lich_repo_mirror/lib/sacrifice_decide.lic:41-52`,
//! `osacombat.lic:3482-3493`). No committed fixture carries an `appraise`
//! reply: the shape is theirs, UNVERIFIED here.
//!
//! One deviation: blank lines are skipped when finding the last line. A reply
//! that ended in a blank line would make bigshot's `.last` blank and its
//! sacrifice never fire; whether Lich's line feed ever hands `issue_command`
//! a blank line is UNVERIFIED.
//!
//! # `measure #id`
//!
//! bigshot's `cmd_briar` (`bigshot.lic:5650-5673`) tests every reply line for
//! `/to be about (\d+) percent\./i` and raises the weapon at 100. That is one
//! line's question, so [`measured_percent`] takes one line. No committed
//! fixture carries this reply either.

/// The line `appraise` opens with: `/^The .+? is \w+ in size/`
/// (`bigshot.lic:6617`).
///
/// Anchored at the line's start, as bigshot's pattern is; `\w` is Ruby's,
/// ASCII letters, digits and `_`.
#[must_use]
pub fn is_appraise_size(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("The ") else {
        return false;
    };
    rest.match_indices(" in size").any(|(at, _)| {
        rest.get(..at)
            .and_then(|head| head.rsplit_once(" is "))
            .is_some_and(|(name, word)| {
                !name.is_empty()
                    && !word.is_empty()
                    && word.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
    })
}

/// Whether an appraised creature reads **enticingly frail**, bigshot's test
/// for `sacrifice` (`bigshot.lic:6619`), over the reply's lines before the
/// prompt.
///
/// `None`: no size line, so this is not an `appraise` reply. Otherwise
/// whether the last non-blank line from the size line on says it.
#[must_use]
pub fn appraised_frail<'a>(reply: impl IntoIterator<Item = &'a str>) -> Option<bool> {
    let mut lines = reply.into_iter().skip_while(|line| !is_appraise_size(line));
    let first = lines.next()?;
    let last = lines
        .filter(|line| !line.trim().is_empty())
        .last()
        .unwrap_or(first);
    Some(last.contains("enticingly frail"))
}

/// The charge `measure` reports: the `N` of `to be about N percent.`, any
/// case (`bigshot.lic:5666`). bigshot raises the weapon at 100.
#[must_use]
pub fn measured_percent(line: &str) -> Option<u32> {
    const LEAD: &str = "to be about ";
    const TAIL: &str = " percent.";
    let lower = line.to_ascii_lowercase();
    lower.match_indices(LEAD).find_map(|(at, _)| {
        let rest = lower.get(at + LEAD.len()..)?;
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if !rest.get(digits..)?.starts_with(TAIL) {
            return None;
        }
        // No digits parses to nothing: `\d+` wants one or more.
        rest.get(..digits)?.parse().ok()
    })
}
