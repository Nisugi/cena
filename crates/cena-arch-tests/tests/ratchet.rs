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

use cena_arch_tests::caps::{
    EXCEPTION_PREFIX, cap_exceptions, caps_file, exception_cap_drift, is_split_parent_entry,
    scan_usize, split_parents, suggested_split_parent_cap,
};
use cena_arch_tests::harness::{lint_keys, workspace_root, workspace_sources};
use cena_arch_tests::lexical::scan_lines;
use cena_arch_tests::plan_rules::{
    architecture_test_paragraph_count, architecture_test_tagged_rules,
};
use cena_arch_tests::structure::live_test_names;
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
    ("4.3", "roundtime_has_a_single_owning_field"),
    ("4.4", "facade_files_stay_facades"),
    ("5.2", "every_static_is_allowlisted"),
];

/// Rules tagged in plan/05 whose test cannot be written yet, with the reason,
/// the unblocking condition, and a tripwire. Machine-readable, so a deferral is
/// a declaration the tests read rather than prose nobody checks.
///
/// `plan/05` Rule 0.5's corollary (:168-170): "an architecture test that
/// enforces a rule protecting against a problem we do not have is also
/// over-engineering."
///
/// Each row is `(rule, reason, tripwire)`.
///
/// # The tripwire, and the defect it closes
///
/// A deferral used to be validated by `reason.len() > 120` alone, so **a
/// deferral whose unblocking condition had already come true looked identical
/// to a live one**. Rule 4.3 sat deferred on *"none of Cena's four values has a
/// field name yet"* long after `roundtime_ends` and `game_time` were written,
/// and nothing detected it -- review finding AR-1, reported HIGH.
///
/// The third column is a source needle naming the thing whose **absence** is
/// the reason for the deferral. [`a_spent_deferral_fails`] fails once it
/// appears, so a deferral expires on its own stated terms rather than when
/// somebody happens to re-read it.
const DEFERRED_RULES: &[(&str, &str, &str)] = &[(
    "2.3",
    "The read path cannot write (:285-298). The seam now EXISTS -- `SessionHandle` \
         (command/handle.rs:171) and `subscribe` (actor/handle.rs:141) -- so the blocker is no \
         longer a missing type. What is missing is the residue itself: plan/05:287-288 notes \
         most of the rule is already structural (`&` cannot send), leaving only 'an owned \
         sender smuggled into a state type'. No state type today holds one, so the test would \
         assert over an empty set and pass whatever the code did. It unblocks when a read-path \
         type could plausibly own a sender.",
    // Deliberately empty: this deferral is blocked on a SHAPE not
    // existing, not on a name, so there is nothing to needle for. Recorded
    // as a decision rather than an oversight -- `a_spent_deferral_fails`
    // requires the column and this explains the value.
    "",
)];

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
        .chain(DEFERRED_RULES.iter().map(|(r, _, _)| r))
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
        "the tests directory should hold at least the three rule files; found \
         {names:?}. A scan that finds nothing reports every rule as deleted."
    );
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for name in &names {
        let path = workspace_root()
            .join("crates/cena-arch-tests/tests")
            .join(name);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
        declared.extend(live_test_names(&text));
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
    for (rule, reason, _tripwire) in DEFERRED_RULES {
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

/// The split-parent cap FLOOR -- the ratchet for the nine per-file caps.
///
/// `caps.baseline` is the enforced value; this is the value it may not exceed
/// without an edit here. See `the_split_parent_caps_only_turn_down`.
const CAPS_FLOOR: &str = "caps.floor";

/// The committed baseline the live caps are measured against.
fn baseline_caps() -> std::collections::BTreeMap<String, usize> {
    caps_file(CAPS_BASELINE)
}

/// The committed floor the baseline is measured against.
fn floor_caps() -> std::collections::BTreeMap<String, usize> {
    caps_file(CAPS_FLOOR)
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
/// **Rule 4.4's ratchet.** A split-parent cap may fall silently; raising one
/// must appear in `caps.floor`'s diff.
///
/// # The hole this closes
///
/// `the_cap_ratchet_only_turns_down` guards `DEFAULT_MAX_LINES` and
/// `MAX_CAP_EXCEPTIONS` against `caps.baseline`. The nine split-parent caps had
/// no such guard: `split_parents_stay_facades` reads each cap **out of
/// `caps.baseline`** and asserts only `lines > cap`, so the file that records
/// the limit is the same file a raiser edits.
///
/// VERIFIED 2026-09-19 -- raising all nine caps at once left the whole suite
/// GREEN. Rule 4.4 had precisely the defect Rule 4.1's ratchet exists to
/// prevent, which is `plan/05` §0's own lesson: a rule that is not enforced is
/// a wish. The author found it by asking where `state.rs`'s 500 came from.
#[test]
fn the_split_parent_caps_only_turn_down() {
    let baseline = baseline_caps();
    let floor = floor_caps();

    let mut raised = Vec::new();
    let mut missing = Vec::new();
    let mut checked = 0usize;
    for (name, cap) in &baseline {
        if !is_split_parent_entry(name) {
            continue; // a scalar or an exception cap, guarded elsewhere
        }
        checked += 1;
        match floor.get(name) {
            Some(limit) if cap > limit => {
                raised.push(format!("{name}: baseline {cap}, floor {limit}"));
            }
            Some(_) => {}
            // A NEW split parent is not a violation -- a file gains a submodule
            // and needs a cap. But it must be recorded, or the next raise has
            // nothing to be measured against.
            None => missing.push(name.clone()),
        }
    }

    // The guard against the guard: a floor emptied of paths would make every
    // comparison above vacuous, exactly as `split_parents_stay_facades`
    // protects itself.
    assert!(
        checked >= 9,
        "only {checked} split parents were compared against caps.floor; there \
         were 9 when this rule was written. A parent missing from caps.baseline \
         is a parent nothing is watching."
    );
    assert!(
        missing.is_empty(),
        "these split parents have a cap in caps.baseline and no entry in \
         caps.floor, so their caps can be raised without any test failing. Add \
         them to caps.floor at their current value:\n{}",
        missing.join("\n")
    );
    assert!(
        raised.is_empty(),
        "RULE 4.4: a split-parent cap went UP. Lowering one is silent and \
         encouraged; raising one has to be visible.\n\n{}\n\n\
         `plan/05:385-388`: \"if one trips, move code down into a submodule \
         instead of raising the cap.\"\n\n\
         If the increase is genuinely right, edit caps.floor in this same commit \
         and say why in caps.baseline -- with a measurement, as the 2026-09-19 \
         re-baselining did.",
        raised.join("\n")
    );
}

/// **Every live cap is at or below its committed baseline**: the default, the
/// size of the exception table, and each exception's own cap.
///
/// The mechanism is the one [`the_split_parent_caps_only_turn_down`]
/// describes -- an increase must be an edit to `caps.baseline` in the same
/// commit -- applied to the constants in `tests/file_rules.rs`.
///
/// # Each exception's cap is ratcheted too (review finding 3)
///
/// This used to compare only `DEFAULT_MAX_LINES` and the COUNT of
/// `CapException` entries. The caps inside them -- 4200, 1800, 1600, 1300,
/// 2400 -- were recorded nowhere else, so raising `crit_tables.tsv` from 2400
/// to 24000 changed neither number this test read and passed green: the
/// exception table was a set of five raisable caps behind one fixed count.
/// `an_exception_cap_raised_in_place_fails` is that edit, fed through the
/// same functions, and it asserts the two old checks still pass it.
///
/// Each exception now has an `exception:<path>` line in `caps.baseline`. A
/// raise must edit it; a new exception must add one; a deleted exception must
/// remove it, so a stale line cannot pre-authorise the next one.
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
    let live = cap_exceptions(&file_rules);

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
        live.len() <= baseline_exceptions,
        "RULE 4.1: the exception table may only SHRINK. There are now {} \
         entries in CAP_EXCEPTIONS and caps.baseline allows \
         {baseline_exceptions}.\n\n\
         A growing exception table is a cap that is failing -- each entry is a \
         file that was allowed to keep growing instead of being split. If the \
         new exception is justified, raise MAX_CAP_EXCEPTIONS in caps.baseline \
         and say why.",
        live.len()
    );

    let drift = exception_cap_drift(&live, &baseline);
    assert!(
        drift.is_empty(),
        "RULE 4.1: every CapException's cap is ratcheted by an \
         `{EXCEPTION_PREFIX}<path>` line in caps.baseline, and these disagree. \
         A cap may fall silently; raising one, adding one, or removing one \
         must edit caps.baseline in the same commit.\n{}",
        drift.join("\n")
    );
}

/// Finding 3's mutation, run through the real functions against the real
/// file: raise one exception's cap in place and nothing else.
#[test]
fn an_exception_cap_raised_in_place_fails() {
    let path = workspace_root().join("crates/cena-arch-tests/tests/file_rules.rs");
    let original = fs::read_to_string(&path).expect("file_rules.rs");
    let live = cap_exceptions(&original);
    let (victim, cap) = live.first().expect("at least one CapException").clone();
    let from = format!("cap: {cap},");
    let to = format!("cap: {},", cap * 10);
    assert_eq!(
        original.matches(&from).count(),
        1,
        "fixture edit must be unique"
    );
    let mutated = original.replace(&from, &to);

    // What the OLD test compared: the default and the number of entries. The
    // mutation changes neither, which is why it passed.
    let old_count = |t: &str| {
        t.lines()
            .filter(|l| l.trim_start().starts_with("CapException {"))
            .count()
    };
    let prefix = "const DEFAULT_MAX_LINES: usize = ";
    assert_eq!(scan_usize(&mutated, prefix), scan_usize(&original, prefix));
    assert_eq!(old_count(&mutated), old_count(&original));

    let baseline = baseline_caps();
    assert!(
        exception_cap_drift(&live, &baseline).is_empty(),
        "unmutated must pass"
    );
    let drift = exception_cap_drift(&cap_exceptions(&mutated), &baseline);
    assert!(
        drift
            .iter()
            .any(|d| d.contains(&victim) && d.contains("ABOVE")),
        "raising {victim} from {cap} to {} went unnoticed: {drift:?}",
        cap * 10
    );
}

/// The token parse counts what the old line count could not see.
#[test]
fn an_unformatted_exception_entry_is_still_counted() {
    let text = "#[rustfmt::skip]\nconst X: &[CapException] = &[CapException{ path: \"a/b.tsv\", \
                cap: 1_200, justification: \"j\" }];\nstruct CapException { path: &'static str }\n";
    assert_eq!(cap_exceptions(text), vec![("a/b.tsv".to_owned(), 1200)]);
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

/// **Rule 4.4's other half: split parents stay facades.**
///
/// `plan/05:385-388` cites Vellum's `split_parents_stay_facades` BY NAME, and
/// Cena implemented only the `lib.rs`/`mod.rs` half (`file_rules.rs`'s
/// `facade_files_stay_facades`, which skips every other filename).
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
///
/// # Split parents are DISCOVERED (review finding 4)
///
/// MEASURED at 2026-09-19, nine files owned child modules, and they were
/// written into `caps.baseline` by hand. Nothing discovered the next one. By
/// 2026-09-23 there were twenty-three, and the fourteen unlisted ones included
/// the three largest split parents in the workspace -- `state/character.rs` at
/// 797 lines, `travel/drive.rs` at 750, `travel.rs` at 661 -- each governed by
/// the 800 default and nothing else. That is the hand-maintained-list failure
/// `harness.rs` names for source discovery, one layer up.
///
/// Now every `name.rs` with a sibling `name/` of `.rs` files is a split parent,
/// and it FAILS until `caps.baseline` and `caps.floor` record a cap for it --
/// by the rule written there: measured size + 50, rounded up to the next 50,
/// never above the default. An entry for a file that is no longer a split
/// parent fails too, so the table cannot rot in either direction.
#[test]
fn split_parents_stay_facades() {
    let baseline = baseline_caps();
    let default = baseline["DEFAULT_MAX_LINES"];
    let discovered = split_parents(&workspace_sources());

    // The guard against the guard. Discovery that found nothing would pass
    // vacuously, which is the dead-ratchet shape this file already records
    // for `tags.rs`.
    assert!(
        discovered.len() >= 9,
        "only {} split parents were discovered; there were 9 when this rule \
         was written. The walk has stopped seeing submodule directories.",
        discovered.len()
    );

    let mut violations = Vec::new();
    for (name, lines) in &discovered {
        match baseline.get(name) {
            Some(cap) if lines > cap => {
                violations.push(format!("{name}: {lines} lines, cap {cap}"));
            }
            Some(_) => {}
            None => violations.push(format!(
                "{name}: a split parent ({lines} lines) with no cap. Add \
                 `{name} {}` to caps.baseline AND caps.floor.",
                suggested_split_parent_cap(*lines, default)
            )),
        }
    }
    for name in baseline.keys().filter(|n| is_split_parent_entry(n)) {
        if !discovered.contains_key(name) {
            violations.push(format!(
                "{name}: in caps.baseline, but not a split parent -- renamed, \
                 removed, or its submodules folded back. Remove the entry in \
                 the same commit, or this rule silently stops covering the file \
                 it meant."
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "RULE 4.4: a split parent grew past its cap, or has none. **Move the \
         new code into its submodule** -- that is what the parent was split \
         for.\n\n\
         `plan/05:385-388` cites Vellum's `split_parents_stay_facades`, whose \
         own comment is the instruction: \"if one trips, move code down into a \
         submodule instead of raising the cap.\"\n\n\
         If the increase is genuinely right, edit caps.baseline and caps.floor \
         in this same commit and say why.\n\n{}",
        violations.join("\n")
    );
}

// A deferral whose unblocking condition has come true must FAIL.
///
/// # The hole this closes
///
/// `every_deferral_states_its_unblocking_condition` checks that a reason is
/// long enough to be auditable. That is necessary and not sufficient: a
/// deferral whose condition has already been met has a long reason too, and
/// looks identical.
///
/// Rule 4.3 was deferred on *"none of Cena's four values ... has a field name
/// yet. Write each in the same commit as its field"*. The fields were written;
/// the tests were not; the deferral stayed green for as long as nobody re-read
/// it. Review finding AR-1, reported HIGH -- and the review was right that the
/// missing piece is *"a machine-checkable tripwire needle ... that fails the
/// deferral once it appears in source"*.
///
/// # Why an empty needle is allowed
///
/// Not every deferral is blocked on a NAME. Rule 2.3 is blocked on a shape --
/// a state type that could own a command sender -- and there is nothing to
/// grep for. An empty needle is that case, stated explicitly so it reads as a
/// decision. The reason string still has to explain itself, which the test
/// above enforces.
#[test]
fn a_spent_deferral_fails() {
    let sources = workspace_sources();
    for (rule, _reason, tripwire) in DEFERRED_RULES {
        if tripwire.is_empty() {
            continue;
        }
        let hits: Vec<String> = scan_lines(&sources, &[tripwire])
            .into_iter()
            // This file NAMES every tripwire, so without excluding it the test
            // fires on its own table.
            .filter(|hit| !hit.starts_with("crates/cena-arch-tests/"))
            .collect();
        assert!(
            hits.is_empty(),
            "Rule {rule} is deferred because {tripwire:?} does not exist -- and it \
             now does. The deferral is SPENT: write the test and move the rule to \
             COVERED_RULES. Found:\n{}",
            hits.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------
// Review finding 5: what `every_covered_rule_names_a_test_that_exists` counts
// as a LIVE test. The mechanism is `cena_arch_tests::structure::live_test_names`,
// whose doc records the history. Each fixture below was run through the
// previous, attributes-directly-above version in a scratch binary, and every
// one was reported live. They are asserted not live here.
// ---------------------------------------------------------------------------

/// Each way of compiling a test out while its `#[test]` stays in place.
#[test]
fn a_test_compiled_out_by_its_surroundings_is_not_live() {
    let fixtures = [
        (
            "an inner cfg at file top",
            "#![cfg(any())]\n#[test]\nfn a() {}\n",
        ),
        (
            "a cfg on the enclosing module",
            "#[cfg(any())]\nmod off {\n    #[test]\n    fn b() {}\n}\n",
        ),
        (
            "ignore applied through cfg_attr",
            "#[cfg_attr(all(), ignore)]\n#[test]\nfn c() {}\n",
        ),
        (
            "the same cfg_attr split over lines",
            "#[cfg_attr(\n    all(),\n    ignore\n)]\n#[test]\nfn d() {}\n",
        ),
        (
            "an inner cfg inside a module",
            "mod m {\n    #![cfg(any())]\n    #[test]\n    fn e() {}\n}\n",
        ),
    ];
    for (label, fixture) in fixtures {
        let live = live_test_names(fixture);
        assert!(live.is_empty(), "{label}: counted as live: {live:?}");
    }
}

/// The negative control: without it, a `live_test_names` that returned
/// nothing would pass the test above.
#[test]
fn an_ordinary_test_is_live() {
    let fixture = "#[cfg(test)]\nfn helper() {}\n\n/// Doc.\n#[test]\n#[should_panic]\nfn plain() {}\n\
                   mod inner {\n    #[test]\n    fn nested() {}\n}\n";
    let live: Vec<String> = live_test_names(fixture).into_iter().collect();
    assert_eq!(live, ["nested", "plain"]);
}
