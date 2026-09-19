//! The ratchet on the ratchet: Rule 9.3, and the enforcer's own integrity.
//!
//! Split from `tests/file_rules.rs` under Rule 4.1 (`plan/05:352-353`) --
//! move code down, do not raise the cap. That file's cap exception named this
//! split in advance: "The next split, if this grows, is Rule 9.3 and the
//! enforcer-integrity tests into tests/ratchet.rs." It grew, when Rule 2.1's
//! deferral was spent and its test written, and this is that split.
//!
//! Read `tests/architecture.rs`'s module header first: its "what these tests
//! do NOT claim" paragraph governs all three files.

use cena_arch_tests::harness::{lint_keys, workspace_root};
use cena_arch_tests::lexical::{code_lines, collapse_whitespace};
use cena_arch_tests::plan_rules::{
    architecture_test_paragraph_count, architecture_test_tagged_rules,
};
use std::collections::BTreeSet;
use std::fs;

// ---------------------------------------------------------------------------
// Rule 9.3 — every rule tagged "Enforced by: architecture test" has that test
// written when the rule is adopted, not later. (plan/05:515-517)
//
// The ratchet on the ratchet, and it has been defeated in both directions.
//
// **Adoption side.** Anchoring on `line.strip_prefix("**Rule ")` made a rule
// written `### Rule N.M` invisible -- and an invisible heading does not start a
// block, so its `*Enforced by:*` paragraph is absorbed into the *preceding*
// rule's block and vanishes if that neighbour is already covered. VERIFIED
// green across five formattings. `plan/05` itself writes 13 of its rules as
// `### Rule ` and 35 as `**Rule `, so both forms are live in the document this
// parses. `plan_rules::heading_number` normalizes the markers; the count
// cross-check below catches absorption directly, which is the failure that
// also produced a corrupt *reported* set (`["9.4"]` for a rule appended at
// end-of-file).
//
// **Withdrawal side.** `COVERED_RULES` was a hand-maintained list of rule
// numbers with nothing connecting it to a test. VERIFIED: deleting
// `fn game_names_stay_inside_game_modules` entirely -- Rule 3.4's only
// enforcement -- left `"3.4"` in the list, its tag in plan/05, and the suite
// green at 10 tests. So each entry now names its test, and
// `every_covered_rule_names_a_test_that_exists` parses this file for that
// function.
//
// NOTE, against the reviews: the true tagged set is EIGHT rules, not seven.
// Two adversarial reviews concluded "real set is 7: {1.3, 2.3, 3.4, 4.1, 4.3,
// 4.4, 5.2}", dropping Rule 2.1. Their diagnosis of the *mechanism* was right
// and is implemented; their conclusion about the set was not. Rule 2.1's tag
// genuinely exists at plan/05:273-274 -- it just wraps. A third review
// independently VERIFIED this and agreed.
// ---------------------------------------------------------------------------

/// Rules tagged in plan/05 that have a test in this file, each naming it.
///
/// The test name is not documentation; `every_covered_rule_names_a_test_that_exists`
/// asserts the function is present in this file. Without that link, deleting a
/// test leaves its rule "covered" and the suite green — VERIFIED.
const COVERED_RULES: &[(&str, &str)] = &[
    ("1.3", "cena_ui_depends_on_no_ui_toolkit"),
    (
        "2.1",
        "wire_text_reaches_the_public_api_only_through_rule_2_2",
    ),
    ("3.4", "game_names_outside_game_modules_are_flagged"),
    ("4.1", "no_source_file_exceeds_its_line_cap"),
    ("4.4", "facade_files_stay_facades"),
    ("5.2", "every_static_is_allowlisted"),
];

/// Rules tagged in plan/05 whose test cannot be written yet, with the reason
/// and the unblocking condition. Machine-readable, so a deferral is a
/// declaration the test reads rather than prose nobody checks.
///
/// `plan/05` Rule 0.5's corollary (:168-170): "an architecture test that
/// enforces a rule protecting against a problem we do not have is also
/// over-engineering." Each of these needs a needle naming a type that does not
/// exist.
const DEFERRED_RULES: &[(&str, &str)] = &[
    (
        "2.3",
        "The read path cannot write (:285-298). Needs the read seam to exist. plan/05:287-288 \
         notes most of it is already structural (& cannot send); the residue is an owned sender \
         smuggled into a state type, so the test is 'the read module may not import the command \
         sink' and both must exist to be named.",
    ),
    (
        "4.3",
        "One owning field per shared value (:376-383). The highest-value test in the reference \
         suite -- it caught a duplicate field nothing assigned for ten months, inflating 49.8% \
         of 6,373 measured countdowns. Its mechanism is a needle for a literal field name \
         (reference/VellumFE/tests/architecture.rs:337-358) and none of Cena's four values -- \
         clock offset, roundtime, current-room id, active-session handle -- has a field name \
         yet. Write each in the same commit as its field, asserting both hits.len() == 1 AND \
         the owning path; the path assertion is what stops a silent relocation.",
    ),
];

#[test]
fn architecture_test_tags_in_plan_05_are_accounted_for() {
    let path = workspace_root().join("plan/05-engineering-rules.md");
    let plan = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "plan/05 is a build input for this test and must be present at {}: {e}. \
             If the plan documents moved or this is a sparse checkout, this test cannot run.",
            path.display()
        )
    });

    let tagged = architecture_test_tagged_rules(&plan);

    // Cross-check against a block-independent count. Heading absorption is
    // silent: a tag belonging to an unparsed heading is credited to the
    // preceding rule, which leaves the set plausible. If a tagged paragraph
    // exists that no rule block claimed, these two disagree.
    let paragraphs = architecture_test_paragraph_count(&plan);
    assert_eq!(
        tagged.len(),
        paragraphs,
        "plan/05 contains {paragraphs} enforcement paragraph(s) naming an \
         architecture test, but only {} were attributed to a rule heading: \
         {tagged:?}. A paragraph that no heading claimed means a rule heading \
         this parser does not recognize -- its tag has been absorbed into the \
         preceding rule and the set above is wrong. Fix the heading form or \
         plan_rules::heading_number, not this count.",
        tagged.len()
    );

    let accounted: BTreeSet<String> = COVERED_RULES
        .iter()
        .map(|(r, _)| r)
        .chain(DEFERRED_RULES.iter().map(|(r, _)| r))
        .map(|r| (*r).to_owned())
        .collect();

    let unenforced: Vec<&String> = tagged.difference(&accounted).collect();
    assert!(
        unenforced.is_empty(),
        "plan/05 tags rule(s) {unenforced:?} with \"Enforced by: architecture \
         test\" and this file neither covers nor defers them. Rule 9.3 \
         (:515-517) requires the test written when the rule is adopted, not \
         later -- add it to this file and to COVERED_RULES, or to \
         DEFERRED_RULES with the type it must name and the condition that \
         unblocks it.\nTagged in plan/05: {tagged:?}\nAccounted for here: {accounted:?}"
    );

    let stale: Vec<&String> = accounted.difference(&tagged).collect();
    assert!(
        stale.is_empty(),
        "this file claims to cover or defer rule(s) {stale:?}, which plan/05 \
         no longer tags with \"Enforced by: architecture test\". A test \
         enforcing a withdrawn rule is dead enforcement; remove it, or restore \
         the tag in plan/05.\nTagged in plan/05: {tagged:?}\nAccounted for here: {accounted:?}"
    );
}

#[test]
fn every_covered_rule_names_a_test_that_exists() {
    // The other half of Rule 9.3. VERIFIED that without this, deleting a test
    // outright leaves its rule "covered", its tag in plan/05, and the suite
    // green -- 11 tests became 10 and nothing said so.
    // EVERY test file, because a test can be moved between them. Naming a
    // subset would make a move look like a deletion, and a reader who "fixed"
    // that by narrowing the scan would reopen the hole this test exists to
    // close.
    //
    // AMENDED when tests/layering.rs was added: this list said
    // ["architecture.rs", "file_rules.rs"] and went red the moment Rule 1.3's
    // test moved into the third file, reporting the MOVE AS A DELETION -- the
    // exact false positive the paragraph above warns about, from the exact
    // cause it names. It is now discovered rather than enumerated, so the next
    // split cannot reproduce this.
    let dir = workspace_root().join("crates/cena-arch-tests/tests");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            // `extension()` rather than `ends_with(".rs")`: clippy's
            // case_sensitive_file_extension_comparisons fires on the string
            // form, and it is right to -- `FILE_RULES.RS` exists on a
            // case-insensitive filesystem, which is what this one is.
            (path.extension()? == "rs").then(|| path.file_name()?.to_str().map(str::to_owned))?
        })
        .collect();
    // Sorted so the reported set is stable run to run -- the same
    // determinism rule the replay tests rest on.
    names.sort();
    assert!(
        names.len() >= 3,
        "the tests directory should hold at least the three rule files; found          {names:?}. A scan that finds nothing reports every rule as deleted."
    );
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for name in &names {
        let path = workspace_root()
            .join("crates/cena-arch-tests/tests")
            .join(name);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
        declared.extend(code_lines(&text).iter().filter_map(|line| {
            let collapsed = collapse_whitespace(line);
            let rest = collapsed.strip_prefix("fn ")?;
            rest.split('(').next().map(str::to_owned)
        }));
    }

    let missing: Vec<&(&str, &str)> = COVERED_RULES
        .iter()
        .filter(|(_, test)| !declared.contains(*test))
        .collect();
    assert!(
        missing.is_empty(),
        "COVERED_RULES names test function(s) that do not exist in any rule file: \
         {missing:?}. Either the test was deleted -- in which case the rule is \
         no longer enforced and Rule 9.3 (:515-517) is violated silently -- or \
         it was renamed without updating the table.\nFound in this file: {declared:?}"
    );
}

#[test]
fn every_deferral_states_its_unblocking_condition() {
    // plan/05 §-2: a deferral without a written reason is a deferral nobody
    // can audit. This is what makes DEFERRED_RULES a declaration rather than a
    // place to park a rule.
    let covered: BTreeSet<&str> = COVERED_RULES.iter().map(|(r, _)| *r).collect();
    for (rule, reason) in DEFERRED_RULES {
        assert!(
            reason.len() > 120,
            "deferral of Rule {rule} needs the type it must name and the \
             condition that unblocks it, not {reason:?}"
        );
        assert!(
            !covered.contains(rule),
            "Rule {rule} is listed as both covered and deferred"
        );
    }
}

// ---------------------------------------------------------------------------
// The enforcer's own lints do not silently drift from the workspace's.
//
// `cena-arch-tests` cannot use `[lints] workspace = true`: Cargo forbids
// mixing the workspace set with overrides, and this crate must flip
// `unwrap_used` / `expect_used` / `panic` to `allow`. Cargo classifies it as a
// *library* target, so `clippy.toml`'s `allow-*-in-tests` does not reach it,
// and a harness whose every function reads the filesystem would otherwise
// carry a `Result` nobody reads -- which is `plan/05` §0's wish with ceremony.
//
// So it restates the set by hand. VERIFIED that nothing kept the two in sync:
// adding `print_stdout = "deny"` to the root `[workspace.lints.clippy]` and a
// `println!` to `harness.rs` -- the only violation in the workspace -- left
// `cargo clippy --workspace --all-targets -- -D warnings` at exit 0, while the
// same `println!` in `cena-model` failed the build. The exemption the
// manifest comment scopes to "three denies flipped" was in fact total and
// permanent for every lint added later.
//
// The enforcer crate silently ceasing to be enforced is the same
// ratchet-on-the-ratchet argument that justifies Rule 9.3's test, so it gets
// the same treatment: the two tables must agree except on a named set.
// ---------------------------------------------------------------------------

/// Lints `cena-arch-tests` deliberately does not inherit, and why.
const LINT_EXEMPTIONS: &[(&str, &str)] = &[
    (
        "unwrap_used",
        "a panic in a test harness IS the failure report; nothing here runs in a session",
    ),
    (
        "expect_used",
        "same as unwrap_used: the message is the assertion",
    ),
    (
        "panic",
        "plan/12 §5.5 bans a panic killing the process; this crate has no process to kill",
    ),
];

#[test]
fn the_enforcer_inherits_every_workspace_lint_it_does_not_name() {
    let root = fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("root Cargo.toml must be readable");
    let mine = fs::read_to_string(workspace_root().join("crates/cena-arch-tests/Cargo.toml"))
        .expect("cena-arch-tests Cargo.toml must be readable");

    let root_lints = lint_keys(&root, "[workspace.lints.");
    let my_lints = lint_keys(&mine, "[lints.");
    assert!(
        !root_lints.is_empty(),
        "parsed zero lints from the root manifest; the parser has drifted and \
         this test is silently vacuous"
    );

    let exempt: BTreeSet<&str> = LINT_EXEMPTIONS.iter().map(|(l, _)| *l).collect();
    let missing: Vec<&String> = root_lints
        .difference(&my_lints)
        .filter(|l| !exempt.contains(l.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "crates/cena-arch-tests/Cargo.toml restates the workspace lint set by \
         hand (Cargo forbids mixing `workspace = true` with overrides) and has \
         drifted: {missing:?} are denied for the workspace but absent here, so \
         the crate that enforces the architecture is itself unenforced.\n\n\
         Add each to that manifest's [lints.*], or -- if it is deliberately \
         not inherited -- to LINT_EXEMPTIONS with the reason.\n\
         Workspace: {root_lints:?}\ncena-arch-tests: {my_lints:?}"
    );

    for (lint, reason) in LINT_EXEMPTIONS {
        assert!(
            reason.len() > 30,
            "the exemption for `{lint}` needs a reason, not {reason:?}"
        );
        assert!(
            root_lints.contains(*lint),
            "LINT_EXEMPTIONS names `{lint}`, which the workspace no longer \
             denies; a stale exemption hides a lint that was never inherited"
        );
    }
}

// ---------------------------------------------------------------------------
// Rule 4.1's other half — the cap RATCHET. (plan/05:352-354)
//
// "the architecture test fails on a cap *increase* in the diff, not just on a
//  violation. Vellum's caps only ever went down."
//
// THIS DID NOT EXIST. The default cap went 400 -> 800 with the whole suite
// green, because nothing was checking. `plan/05` §0 is the rule this broke:
// "a rule that is not enforced is a wish", and Rule 4.1's second half was a wish
// for the whole of Milestone 1 and its tail. The ratchet file listed 4.1 as
// covered, which made it worse -- a claim nobody checked.
// ---------------------------------------------------------------------------

/// Where the committed baseline lives. Beside the crate, not under `tests/`,
/// because it is data the suite reads rather than a test.
const CAPS_BASELINE: &str = "caps.baseline";

/// Read `<name> <value>` pairs from the baseline, ignoring comments and blanks.
fn baseline_caps() -> std::collections::BTreeMap<String, usize> {
    let path = workspace_root()
        .join("crates")
        .join("cena-arch-tests")
        .join(CAPS_BASELINE);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} must exist and be readable: {e}. It is the cap ratchet's \
             baseline -- deleting it would silently disable Rule 4.1's \
             increase check, which is the exact failure the check exists to \
             prevent.",
            path.display()
        )
    });
    let mut caps = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line.split_once(' ').unwrap_or_else(|| {
            panic!("malformed baseline line {line:?}: expected `<name> <value>`")
        });
        let value = value.trim().parse().unwrap_or_else(|e| {
            panic!("malformed baseline value in {line:?}: {e}");
        });
        caps.insert(name.to_owned(), value);
    }
    caps
}

/// **A cap may go down silently. Raising one must appear in the diff.**
///
/// # Why a committed file and not a git diff
///
/// A test cannot read a diff -- it has no reliable base to diff against, and one
/// that shelled out to `git` would pass on a shallow clone, a worktree, or an
/// exported tarball, which is the kind of guard that is green exactly when it is
/// not working.
///
/// What it can do is make an increase **impossible to make quietly**: the live
/// constant must not exceed the committed baseline, so raising a cap means
/// editing `caps.baseline` in the same commit. That file exists for no other
/// purpose, so the increase lands in the diff where a reviewer cannot miss it,
/// with a place to write down why.
///
/// That is the honest reading of `plan/05:353`, and it is strictly stronger than
/// what the prose describes in one respect: it also catches an increase made by
/// someone who never looks at a diff at all.
#[test]
fn the_cap_ratchet_only_turns_down() {
    let baseline = baseline_caps();

    // The live values, read from the enforcer itself rather than restated here:
    // a copy would be a second source of truth and could drift from the file it
    // is supposed to be guarding.
    let file_rules = fs::read_to_string(
        workspace_root()
            .join("crates")
            .join("cena-arch-tests")
            .join("tests")
            .join("file_rules.rs"),
    )
    .expect("file_rules.rs must be readable");

    let live_default = scan_usize(&file_rules, "const DEFAULT_MAX_LINES: usize = ")
        .expect("DEFAULT_MAX_LINES must still be a plain `const ... = N;`");
    let live_exceptions = file_rules
        .lines()
        .filter(|line| line.trim_start().starts_with("CapException {"))
        .count();

    let baseline_default = baseline["DEFAULT_MAX_LINES"];
    assert!(
        live_default <= baseline_default,
        "RULE 4.1: a cap may only go DOWN. DEFAULT_MAX_LINES is {live_default}, \
         and caps.baseline records {baseline_default}.\n\n\
         `plan/05:352-354`: \"move code down, don't raise the cap ... Vellum's \
         caps only ever went down.\"\n\n\
         If the increase is deliberate, edit caps.baseline in this same commit \
         and say why there -- with a measurement, as the 400 -> 800 change did. \
         That is the whole mechanism: an increase has to be visible."
    );
    if live_default < baseline_default {
        // A turn DOWN is allowed and silent, but the baseline should follow so
        // the next increase is measured against the new floor.
        eprintln!(
            "note: DEFAULT_MAX_LINES ({live_default}) is below caps.baseline \
             ({baseline_default}). Lower the baseline to match, so the ratchet \
             holds at the new value."
        );
    }

    let baseline_exceptions = baseline["MAX_CAP_EXCEPTIONS"];
    assert!(
        live_exceptions <= baseline_exceptions,
        "RULE 4.1: the exception table may only SHRINK. There are now \
         {live_exceptions} entries in CAP_EXCEPTIONS and caps.baseline allows \
         {baseline_exceptions}.\n\n\
         A growing exception table is a cap that is failing -- each entry is a \
         file that was allowed to keep growing instead of being split. If the \
         new exception is justified, raise MAX_CAP_EXCEPTIONS in caps.baseline \
         and say why."
    );
}

/// The baseline itself must carry its reasoning, for the same reason a cap
/// exception must: a bare number is a claim nobody can check.
#[test]
fn the_caps_baseline_explains_itself() {
    let path = workspace_root()
        .join("crates")
        .join("cena-arch-tests")
        .join(CAPS_BASELINE);
    let text = fs::read_to_string(&path).expect("caps.baseline must be readable");

    assert!(
        text.contains("plan/05"),
        "caps.baseline must cite the rule it enforces, or a reader has no way \
         to tell it from an arbitrary config file"
    );
    let comment_lines = text
        .lines()
        .filter(|l| l.trim_start().starts_with('#'))
        .count();
    assert!(
        comment_lines >= 10,
        "caps.baseline has {comment_lines} comment lines. It is a file of bare \
         numbers whose entire value is the reasoning beside them -- a future \
         reader raising a cap needs to find out here why they should not."
    );
}

/// Pull `const NAME: usize = N;` out of source text.
fn scan_usize(text: &str, prefix: &str) -> Option<usize> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(prefix))
        .and_then(|rest| rest.trim_end_matches(';').trim().parse().ok())
}

/// **Rule 4.4's other half: split parents stay facades.**
///
/// `plan/05:385-388` cites Vellum's `split_parents_stay_facades` BY NAME, and
/// Cena implemented only the `lib.rs`/`mod.rs` half (`file_rules.rs`'s
/// `facade_files_stay_facades`, which skips every other filename). MEASURED at
/// 2026-09-19: **nine** files own child modules -- `state.rs`, `actor.rs`,
/// `supervisor.rs`, `parser.rs`, `frame.rs`, `text.rs`, `tags.rs`, `crit.rs`,
/// `probe.rs` -- and every one was governed by the 800-line default and nothing
/// else.
///
/// # It is a cap, not a no-behavior rule
///
/// The obvious reading of "stay facades" -- forbid `fn` and `impl`, as the
/// `lib.rs` half does -- is **wrong here**, and Vellum's own version says so. A
/// split parent legitimately holds type definitions and dispatchers;
/// `GameState::apply` belongs in `state.rs`. Vellum enforces a per-file line
/// cap and comments the intent: *"if one trips, move code down into a submodule
/// instead of raising the cap"*
/// (`reference/VellumFE/tests/architecture.rs:252-285`).
///
/// So this is the same ratchet as [`the_cap_ratchet_only_turns_down`], applied
/// per file: the baseline is editable, but only in the same commit, in a file
/// that exists for no other purpose.
#[test]
fn split_parents_stay_facades() {
    let baseline = baseline_caps();
    let root = workspace_root();
    let mut violations = Vec::new();
    let mut checked = 0usize;

    for (name, cap) in &baseline {
        // The split-parent entries are the ones whose name is a path.
        if !name.contains('/') {
            continue;
        }
        checked += 1;
        let path = root.join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            violations.push(format!(
                "{name}: listed in caps.baseline but not readable. A split                  parent that was renamed or removed must be removed from the                  baseline in the same commit, or this rule silently stops                  covering it."
            ));
            continue;
        };
        let lines = text.lines().count();
        if lines > *cap {
            violations.push(format!("{name}: {lines} lines, cap {cap}"));
        }
    }

    // The guard against the guard. A baseline whose path entries were all
    // deleted would pass vacuously, which is the dead-ratchet shape this file
    // already records for `tags.rs`.
    assert!(
        checked >= 9,
        "only {checked} split parents are covered; there were 9 when this rule          was written. A parent dropped from caps.baseline is a parent nothing          is watching."
    );

    assert!(
        violations.is_empty(),
        "RULE 4.4: a split parent grew past its cap. **Move the new code into          its submodule** -- that is what the parent was split for.

         `plan/05:385-388` cites Vellum's `split_parents_stay_facades`, whose          own comment is the instruction: \"if one trips, move code down into a          submodule instead of raising the cap.\"

         If the increase is genuinely right, edit caps.baseline in this same          commit and say why.

{}",
        violations.join("
")
    );
}
