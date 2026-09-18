//! Parsing `plan/05` for its `*Enforced by:* ... architecture test` tags.
//!
//! Mechanism only; the rule this serves (Rule 9.3, `plan/05:515-517`) and the
//! covered/deferred tables live beside their test.
//!
//! Split out of the test file under `plan/05:352-353` — move code down, do not
//! raise the cap.
//!
//! # Why this is not a `grep`
//!
//! Three successive drafts of this parser were defeated, each by the
//! *next-nearest* real shape rather than by anything exotic:
//!
//! 1. A line-local `contains("architecture test")` filter **missed** Rule 2.1
//!    (whose tag wraps across `plan/05:273-274`) and **falsely counted**
//!    `plan/05:515`, Rule 9.3's own self-quoting title. The two errors
//!    cancelled into a green count of 8.
//! 2. Anchoring on `line.strip_prefix("**Rule ")` made any rule written
//!    `### Rule N.M`, `#### Rule N.M`, ` **Rule N.M` or `**N.M — ...**`
//!    invisible — and worse, an invisible heading does not start a block, so
//!    the new rule's tag is **absorbed into the preceding rule's block**. If
//!    that neighbour is already covered, a newly adopted unenforced rule is
//!    green. VERIFIED across five formattings.
//! 3. The same absorption corrupts the *reported* set even when the test does
//!    go red: appending a tagged rule at end-of-file reported `["9.4"]`.
//!
//! The answer to all three is: normalize the heading before matching, and
//! anchor the tag on a **line** inside the block rather than on a substring of
//! the joined block. `plan/05` currently writes numbered rules as `**Rule N.M`
//! (VERIFIED: 35 of them) and the E./0. rules as `### Rule ` (13), so both
//! forms are already live in the document this parses.

use std::collections::BTreeSet;

/// A rule heading's number, if `line` is one, in any of the heading forms
/// `plan/05` uses or plausibly will.
///
/// Strips Markdown heading and emphasis markers before matching, so
/// `### Rule 3.6`, `**Rule 3.6`, `- **Rule 3.6` and a leading-space variant all
/// parse. Requires a dotted number, which is what distinguishes a rule heading
/// from prose beginning "Rule of three".
fn heading_number(line: &str) -> Option<String> {
    let normalized = line
        .trim_start()
        .trim_start_matches(['#', '-', '*', '>', ' '])
        .trim_start();
    let rest = normalized.strip_prefix("Rule ")?;
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if number.contains('.') {
        Some(number)
    } else {
        None
    }
}

/// Whether `line` *starts* this rule's enforcement paragraph.
///
/// Anchored on the line, not on a substring of the joined block. That is what
/// excludes Rule 9.3's title, which quotes the tag it is about
/// (`plan/05:515`) — VERIFIED that 9.3 carries no `*Enforced by:*` paragraph
/// of its own. A `block.contains("Enforced by")` test counts 9.3 as tagged and
/// then demands a test for the meta-rule.
fn starts_enforcement(line: &str) -> bool {
    let normalized = line.trim_start().trim_start_matches(['*', ' ']);
    normalized.starts_with("Enforced by:")
}

/// Rule numbers whose `*Enforced by:*` paragraph names an architecture test,
/// paired with that paragraph, for the counting cross-check.
fn tagged_paragraphs(plan: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = plan.lines().collect();
    let starts: Vec<(usize, String)> = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| heading_number(line).map(|n| (idx, n)))
        .collect();

    let mut out = Vec::new();
    for (i, (start, number)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map_or(lines.len(), |(next, _)| *next);
        let block = &lines[*start..end];
        for (offset, line) in block.iter().enumerate() {
            if !starts_enforcement(line) {
                continue;
            }
            // Join to the end of the paragraph so a tag that hard-wraps --
            // as Rule 2.1's does at plan/05:273-274 -- is one string.
            let mut paragraph = String::new();
            for follow in block.iter().skip(offset) {
                if paragraph.is_empty() || !follow.trim().is_empty() {
                    paragraph.push_str(follow.trim());
                    paragraph.push(' ');
                } else {
                    break;
                }
            }
            if paragraph.contains("architecture test") {
                out.push((number.clone(), paragraph));
            }
        }
    }
    out
}

/// Rule numbers whose enforcement paragraph names an architecture test.
pub fn architecture_test_tagged_rules(plan: &str) -> BTreeSet<String> {
    tagged_paragraphs(plan)
        .into_iter()
        .map(|(number, _)| number)
        .collect()
}

/// How many enforcement paragraphs in the whole document name an architecture
/// test, counted **without** reference to rule blocks.
///
/// The cross-check against `architecture_test_tagged_rules().len()`. Heading
/// absorption is a silent failure: a tag that belongs to an unparsed heading
/// is credited to the preceding rule, so the set stays plausible and the count
/// stays right. Counting paragraphs independently of blocks is what catches
/// it — if a paragraph exists that no block claimed, or two paragraphs are
/// credited to one rule, the two numbers disagree.
pub fn architecture_test_paragraph_count(plan: &str) -> usize {
    let lines: Vec<&str> = plan.lines().collect();
    let mut count = 0;
    for (idx, line) in lines.iter().enumerate() {
        if !starts_enforcement(line) {
            continue;
        }
        let mut paragraph = String::new();
        for follow in lines.iter().skip(idx) {
            if paragraph.is_empty() || !follow.trim().is_empty() {
                paragraph.push_str(follow.trim());
                paragraph.push(' ');
            } else {
                break;
            }
        }
        if paragraph.contains("architecture test") {
            count += 1;
        }
    }
    count
}
