//! Architecture tests: file shape, and the ratchet on the ratchet.
//!
//! The companion to `tests/architecture.rs`, which holds layering and state.
//! Split by rule section under `plan/05:352-353` -- move code down, do not
//! raise the cap. This file carries Rule 4.1 (caps), Rule 4.4 (facades), the
//! `include!` ban that keeps files inside the scan at all, and Rule 9.3, which
//! asserts that every rule `plan/05` tags for an architecture test has one.
//!
//! Read `tests/architecture.rs`'s module header first: its "what these tests
//! do NOT claim" paragraph governs both files.

use cena_arch_tests::harness::{
    lint_keys, relative, scannable_sources, workspace_root, workspace_sources,
};
use cena_arch_tests::lexical::{
    code_lines, collapse_whitespace, declares_behavior, items, scan_lines,
};
use cena_arch_tests::plan_rules::{
    architecture_test_paragraph_count, architecture_test_tagged_rules,
};
use std::collections::BTreeSet;
use std::fs;

// ---------------------------------------------------------------------------
// Rule 4.1 — Per-file line caps, from day one. (plan/05:346-365)
//
// "No crate graph can express 'this file holds only the struct and its
// dispatcher.' Only a cap can." (plan/05:348-351)
//
// Vellum's caps were never once raised (plan/06:122) but arrived at ~250K
// lines, after a 9,227-line `impl` had to be hand-split (plan/06:120-127).
// "Start with the ratchet in place and you never need the refactor."
//
// NOTE the deliberate design difference: this is a *default* cap over every
// file, not Vellum's closed table of six named files
// (reference/VellumFE/tests/architecture.rs:261-268). A closed table only
// covers files someone remembered to add; a default covers the file written
// tomorrow. The table below is the ALLOWLIST of exceptions, and plan/05:364
// requires each entry to carry a justifying comment — which the
// `justification` field makes structural rather than conventional.
//
// Vellum's numbers (2100/3600/800) are deliberately NOT copied: plan/05:356-359
// bans that by name. 400 is not measured from Cena — there is no Cena code yet
// to measure. It is a starting cap chosen to be tightened, and the ratchet only
// ever moves down (plan/05:352-353).
// ---------------------------------------------------------------------------

const DEFAULT_MAX_LINES: usize = 400;

/// A cap exception. `justification` is required by the type, which is how
/// `plan/05:364-365` ("allowlist requiring a justifying comment") stops being
/// a convention. Vellum's table is a bare `&[(&str, usize)]` with no such
/// field (`reference/VellumFE/tests/architecture.rs:261-268`).
struct CapException {
    path: &'static str,
    cap: usize,
    justification: &'static str,
}

const CAP_EXCEPTIONS: &[CapException] = &[CapException {
    path: "crates/cena-arch-tests/tests/file_rules.rs",
    cap: 650,
    justification: "The enforcer is not exempt from its own ratchet, and it went red three                     times. At one file it was 953 lines against the 400 default; the response                     was plan/05:352-353 -- move code down -- giving src/harness.rs,                     src/lexical.rs and src/plan_rules.rs. It went red again at 910 and split by                     rule section into tests/architecture.rs (layering and state, which now fits                     the default cap with no exception) and this file. It went red a third time                     when the lint-drift test was added, and that test moved here rather than                     the cap moving up. What remains is the rule citation and, for each test,                     the VERIFIED evasion that determined its shape -- which plan/05 section -2                     requires be written next to the code it governs, and which is the reason                     these tests are not the decoration plan/05 section 0 warns about. The next                     split, if this grows, is Rule 9.3 and the enforcer-integrity tests into                     tests/ratchet.rs. Compare reference/VellumFE/tests/architecture.rs for the                     closed-table design this deliberately does not copy.",
}];

#[test]
fn no_source_file_exceeds_its_line_cap() {
    let mut violations = Vec::new();
    for (path, text) in workspace_sources() {
        let rel = relative(&path);
        let cap = CAP_EXCEPTIONS
            .iter()
            .find(|e| e.path == rel)
            .map_or(DEFAULT_MAX_LINES, |e| e.cap);
        let lines = text.lines().count();
        if lines > cap {
            violations.push(format!("{rel}: {lines} lines, cap {cap}"));
        }
    }
    assert!(
        violations.is_empty(),
        "Move code down, do not raise the cap (plan/05 Rule 4.1, :352-353).\n{}",
        violations.join("\n")
    );
}

#[test]
fn cap_exceptions_are_justified() {
    // A length floor was VERIFIED insufficient: a 47-character `"aaaa..."`
    // literal raised a cap from 400 to 4000 with the suite green. plan/05:364
    // asks for a justifying *comment*, and Rule E.1 (:19-33) says a finding
    // carries its proof inline. So the shape of a justification is asserted:
    // enough distinct words to be prose, and a citation.
    for exception in CAP_EXCEPTIONS {
        let words: BTreeSet<&str> = exception
            .justification
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();
        assert!(
            words.len() >= 12,
            "the cap exception for {} needs prose explaining why the file is \
             long and what the next split is, not {:?} (plan/05:364). \
             Distinct words over two characters: {}",
            exception.path,
            exception.justification,
            words.len()
        );
        assert!(
            exception.justification.contains("plan/")
                || exception.justification.contains("reference/"),
            "the cap exception for {} must cite the rule or the measurement it \
             rests on (plan/05 Rule E.1, :19-33): a justification without a \
             citation is the speculation §-2 forbids",
            exception.path
        );
        assert!(
            exception.cap > DEFAULT_MAX_LINES,
            "cap exception for {} is not an exception: {} <= default {}",
            exception.path,
            exception.cap,
            DEFAULT_MAX_LINES
        );
        let target = workspace_root().join(exception.path);
        assert!(
            target.is_file(),
            "cap exception names {}, which does not exist; a stale exception \
             silently raises no cap but hides that the file moved",
            exception.path
        );
    }
}

// ---------------------------------------------------------------------------
// Rule 4.4 — Facade modules stay facades. (plan/05:385-388)
//
// "A `mod.rs`/`lib.rs` re-exports and wires; it does not implement."
// "Enforced by: architecture test — facade files have a low line cap and may
// not define behavior." (plan/05:388)
//
// Vellum approximates the "may not define behavior" half with a line count
// alone (`config_root_stays_a_facade`,
// reference/VellumFE/tests/architecture.rs:232-249).
//
// Two corrections, both VERIFIED against the previous draft:
//
// 1. **The prefix allowlist was defeated by `const fn`.** Eight spellings were
//    enumerated (`fn `, `pub fn `, `async fn `, ...) and five working
//    functions -- `pub const fn`, `pub extern "Rust" fn`, `pub(crate) const
//    fn`, a bare `const fn` -- passed green in a `cena-ui/src/lib.rs`. Any
//    allowlist of spellings has that hole; `harness::declares_behavior` is a
//    token test instead, because `fn` and `impl` are keywords that cannot be
//    spelled another way.
//
// 2. **It fired on a textbook-correct facade.** A `lib.rs` containing nothing
//    but `pub mod room; pub use room::RoomTitle;` and a `#[cfg(test)] mod
//    tests` broke the build. That is a false positive on compliant code, and
//    it is the more dangerous direction: it gets "fixed" by deleting the test
//    module, or by deleting this rule. Every Rust crate root legitimately
//    carries a test module, so `harness::items` tracks `#[cfg(test)]` module
//    bodies and this scan skips them.
//
// Known gap, recorded not fixed: a macro-generated `impl` is invisible to a
// scan. `syn` is the upgrade if a macro ever generates items here.
// ---------------------------------------------------------------------------

const FACADE_MAX_LINES: usize = 120;

#[test]
fn facade_files_stay_facades() {
    let mut violations = Vec::new();
    for (path, text) in workspace_sources() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name != "lib.rs" && name != "mod.rs" {
            continue;
        }
        let rel = relative(&path);
        let lines = text.lines().count();
        if lines > FACADE_MAX_LINES {
            violations.push(format!(
                "{rel}: {lines} lines, facade cap {FACADE_MAX_LINES}"
            ));
        }
        for item in items(&text) {
            if item.in_test_module {
                continue;
            }
            if declares_behavior(&item.code) {
                violations.push(format!(
                    "{rel}:{}: facade defines behavior: {}",
                    item.line, item.code
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "A lib.rs/mod.rs re-exports and wires; it does not implement \
         (plan/05 Rule 4.4, :385-388). A `#[cfg(test)] mod tests` is exempt: \
         every crate root legitimately has one.\n{}",
        violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// `include!` — the file-discovery escape hatch, banned outright.
//
// `harness::crate_sources` now walks the whole crate directory, which closes
// the `#[path = "../gen/globals.rs"]` evasion. `include!` is the other half:
// it accepts any path, including one outside the crate and one with an
// extension the walk does not collect.
//
// VERIFIED: `include!("parser_body.in")` in `cena-protocol/src/lib.rs`, with a
// 906-line `parser_body.in` holding `pub static mut CURRENT_STREAM_BUFFER` and
// `if game == "GemStone"`, compiled into the crate with the line cap, the
// `static mut` ban and the game-name ban all green simultaneously. One line
// defeated four rules.
//
// Banning it is cheaper than chasing it and loses nothing: `include!` has no
// legitimate M1 use. When generated code arrives (`plan/13` §4a names
// `KNOWN_WIRE_TAGS` and the 61-variant `ParsedElement` as ports, which is
// exactly where a build script would generate a table), the honest move is to
// lift this ban with a written reason and extend `SOURCE_EXTENSIONS`, not to
// route around it.
// ---------------------------------------------------------------------------

#[test]
fn no_source_file_is_included_from_outside_the_scan() {
    let hits = scan_lines(
        &scannable_sources(),
        &["include!(", "include_str!(", "include_bytes!("],
    );
    assert!(
        hits.is_empty(),
        "`include!` splices a file into a crate without that file being a \
         module, which puts it outside every scan in this suite. VERIFIED: a \
         906-line `.in` file with `pub static mut` and an `if game == \
         \"GemStone\"` branch compiled in with four bans green.\n\n\
         If generated code is genuinely needed (plan/13 §4a), lift this ban \
         deliberately and extend harness::SOURCE_EXTENSIONS so the generated \
         file is scanned, rather than routing around the scan.\n{}",
        hits.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Rule 3.4 — Game-specific code lives under a game namespace. (plan/05:332-336)
//
// "Enforced by: architecture test bans game-name identifiers outside the game
// modules." (plan/05:336)
//
// This is the DragonRealms-deferral guard: CLAUDE.md and plan/12 §9d forbid a
// `GameAdapter` abstraction, so the only thing stopping `if game == ...` from
// spreading through shared modules is this test.
//
// The rule's words are "shared modules", not "cena-model". The first draft
// scanned `cena-model` only, which left the banned branch legal in the six
// other crates — including `cena-session`, where an M1 multi-session dispatch
// branch would most plausibly appear, and `cena-protocol`, where a GS-vs-DR
// wire divergence would. It now scans the workspace; the `/src/gemstone/` and
// `/src/dragonrealms/` exclusions already work workspace-wide because
// `relative()` emits full paths.
//
// `GS4` is a needle (the game is GemStone IV, CLAUDE.md:3). Bare `DR` is NOT:
// it would match `DRY`, `ADDR`, `DROP` and every hex literal.
//
// # THIS TEST FLAGS; IT DOES NOT PROVE. The name says `flags`, not `bans`.
//
// VERIFIED defeated by `concat!("Gem", "Stone")`, and equally by
// `"Gem\u{53}tone"`, `"GEMSTONE".to_lowercase()`, or a const assembled from
// bytes. No lexical scan can close that class: the set of expressions
// evaluating to "GemStone" is unbounded. Stripping `concat!` arguments would
// close one spelling and leave the rest, which is `plan/05` §-1's "simplest
// thing that works" applied to the wrong problem — the work does not reduce
// the residue.
//
// So this is a speed bump against **drift**, not a proof against intent, and
// the test is named for what it does. The real enforcement for a deliberate
// DragonRealms branch is review plus `plan/12` §9d; this catches the ordinary
// case where someone types the literal without thinking about it.
//
// FLAG: plan/05:335 justifies the rule with "that is what the `GameAdapter`
// trait is for", but CLAUDE.md bans `GameAdapter` and plan/05:533 itself lists
// it as over-engineered. The rule stands; its stated rationale is stale and
// should be amended per plan/05 §10.
// ---------------------------------------------------------------------------

#[test]
fn game_names_outside_game_modules_are_flagged() {
    let sources = scannable_sources();
    let needles = &[
        "GemStone",
        "Gemstone",
        "gemstone",
        "GS4",
        "Gs4",
        "gs4",
        "DragonRealms",
        "Dragonrealms",
        "dragonrealms",
    ];
    let hits: Vec<String> = scan_lines(&sources, needles)
        .into_iter()
        .filter(|hit| !hit.contains("/src/gemstone/") && !hit.contains("/src/dragonrealms/"))
        .collect();
    assert!(
        hits.is_empty(),
        "game-specific names belong under <crate>/src/<game>/, not in shared \
         modules behind an `if game == ...` branch \
         (plan/05 Rule 3.4, :332-336).\n\n\
         Note this test FLAGS the literal spelling; it does not prove absence. \
         `concat!(\"Gem\", \"Stone\")` was VERIFIED to pass it. Deliberate \
         evasion is review's job (plan/12 §9d); this catches drift.\n{}",
        hits.join("\n")
    );
}

// ---------------------------------------------------------------------------
// plan/12 §5.5 — "A panic kills one session, not the process ... Requires
// `panic = \"unwind\"` — `panic = \"abort\"` defeats it, and that must be
// asserted in CI."
//
// A manifest line is a wish until something checks it (plan/05 §0).
//
// VERIFIED defeated by TOML single quotes: `[profile.dist] inherits =
// "release"` / `panic = 'abort'` built with `cargo build --profile dist` and
// left this test green. TOML basic and literal strings are interchangeable, so
// quotes are normalized before matching, which covers every profile name at
// once -- `dist` is the realistic case, since release binaries are commonly
// cut from a named profile.
//
// Known gap, recorded not fixed: `CARGO_PROFILE_RELEASE_PANIC=abort` or
// `RUSTFLAGS=-Cpanic=abort` in the CI environment defeats this, and no
// manifest test can see it. The CI workflow sets neither.
//
// Member manifests are not read: Cargo ignores `[profile.*]` outside the
// workspace root with a warning, so a profile there is inert.
// ---------------------------------------------------------------------------

#[test]
fn no_profile_aborts_on_panic() {
    let manifest = fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("root Cargo.toml must be readable");
    // Strip `#` comments first. Without this, the comment in the root manifest
    // that *states* this rule breaks it -- the same failure that made
    // `cena_ui_depends_on_no_ui_toolkit` fire on a comment reading "we
    // deliberately avoid ratatui". A test that punishes documenting the rule
    // gets fixed by deleting the documentation.
    let offenders: Vec<String> = manifest
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .filter(|line| {
            // Normalize both whitespace and TOML's two string quotings.
            line.replace([' ', '\t', '\'', '"'], "")
                .contains("panic=abort")
        })
        .map(str::to_owned)
        .collect();
    assert!(
        offenders.is_empty(),
        "panic=abort defeats per-session panic isolation (plan/12 §5.5): a \
         panic must kill one session, not the process. This applies to every \
         profile, not just [profile.release] -- a [profile.dist] with \
         `inherits = \"release\"` was VERIFIED to build and ship abort.\n{}",
        offenders.join("\n")
    );
}

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
        "2.1",
        "No raw wire text above cena-protocol (:270-274). Must name real types in \
         cena-protocol's public API, which does not exist yet. When writing it: Rule 2.2 \
         (:276-283) MANDATES the one escape 2.1 forbids -- Frame::Unknown carrying raw text to \
         the UI. The allowlist entry is not optional; write the two as a cross-referenced pair.",
    ),
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
    // Both test files, because a test can be moved between them. Naming one
    // would make a move look like a deletion, and a reader who "fixed" that by
    // narrowing the scan would reopen the hole this test exists to close.
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for name in ["architecture.rs", "file_rules.rs"] {
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
        "COVERED_RULES names test function(s) that do not exist in this file: \
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
