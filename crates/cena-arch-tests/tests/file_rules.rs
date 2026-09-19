//! Architecture tests: file shape, and the ratchet on the ratchet.
//!
//! The companion to `tests/architecture.rs`, which holds layering and state.
//! Split by rule section under `plan/05:352-353` -- move code down, do not
//! raise the cap. This file carries Rule 4.1 (caps), Rule 4.4 (facades), the
//! `include!` ban that keeps files inside the scan at all, and Rule 2.1 (no
//! raw wire text in `cena-protocol`'s public API). Rule 9.3 and the
//! enforcer-integrity tests moved to `tests/ratchet.rs` when this file went
//! red a fourth time.
//!
//! Read `tests/architecture.rs`'s module header first: its "what these tests
//! do NOT claim" paragraph governs all three files.

use cena_arch_tests::harness::{relative, scannable_sources, workspace_root, workspace_sources};
use cena_arch_tests::lexical::{collapse_whitespace, declares_behavior, items, scan_lines};
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
// bans that by name.
//
// THE NUMBER, AND THE ONE TIME IT WAS RAISED (2026-09-18).
//
// It started at 400, and that figure was INVENTED -- this comment used to say
// so: "400 is not measured from Cena, there is no Cena code yet to measure. It
// is a starting cap chosen to be tightened." It was a guess made before there
// was anything to calibrate against, and plan/05 Rule 4.1 mandates *a* cap
// without ever naming one.
//
// By Milestone 2 there was something to calibrate against, and the author
// raised it to 800:
//
//   AUTHOR: "where did the 400 line limit come from? Is that best practices or
//            invented? I feel like 1000 lines is better? Or I guess 400 is
//            easier for an llm to ingest"
//
// Invented, and the author's instinct is backed by their own shipped code:
// VellumFE's caps are 3600 / 2100 / 1700 / 1100 / 800 / 700
// (reference/VellumFE/tests/architecture.rs:261-268). 800 is below all but one
// of them.
//
// What 400 actually cost: FIVE splits in one milestone, several of which were
// forced by the line count rather than by the code wanting to split --
// actor/ending.rs exists because `shutdown` and `transition` had nowhere else
// to go, not because they are a coherent module. A cap should make seams be
// chosen DELIBERATELY; one set too low makes them be chosen by arithmetic.
//
// What 400 bought, and why 800 rather than 1100+: a file that fits in one
// reading is a file whose stale header claims get noticed. Three of those
// survived several edits in actor.rs this milestone precisely because the file
// was read in excerpts.
//
// **The ratchet is unchanged and still only moves down** (plan/05:352-353).
// This was a one-time recalibration of a guessed starting value, made at a
// milestone boundary and by the author -- not a cap raised reactively because
// a file tripped, which is the thing Rule 4.1 forbids and which was declined
// five times tonight in favour of splitting.
// ---------------------------------------------------------------------------

const DEFAULT_MAX_LINES: usize = 800;

/// A cap exception. `justification` is required by the type, which is how
/// `plan/05:364-365` ("allowlist requiring a justifying comment") stops being
/// a convention. Vellum's table is a bare `&[(&str, usize)]` with no such
/// field (`reference/VellumFE/tests/architecture.rs:261-268`).
struct CapException {
    path: &'static str,
    cap: usize,
    justification: &'static str,
}

const CAP_EXCEPTIONS: &[CapException] = &[
    // NOTE: this file's own exception was REMOVED when the default rose to 800.
    // It had been 650, and at 800 it is no longer an exception at all -- which
    // `cap_exceptions_are_justified` reported as an error rather than letting it
    // sit as dead weight. That is the ratchet working on its own enforcer: an
    // exception that no longer excepts anything is a claim nobody checks.
    //
    // Its history is worth keeping, because it is the strongest evidence for the
    // rule: this file went red FOUR times and split four times rather than
    // raising its cap once -- into src/harness.rs, src/lexical.rs,
    // src/plan_rules.rs, tests/architecture.rs and tests/ratchet.rs.
    CapException {
        path: "crates/cena-model/data/crit_tables.tsv",
        cap: 2400,
        justification: "This is DATA, not code, and it is the only file in the workspace for \
                        which 'move code down' has no meaning: there is nothing to move, because \
                        there are no functions in it. It is the 2,394 critical-hit table entries \
                        ported from Lich per plan/13 section 4a, one entry per line plus a \
                        header, and plan/13:125 specifies this exact shape for this exact row -- \
                        'static data; ships as data files'. Splitting it would produce N files \
                        with no seam to choose, since the rows are a flat sorted sequence keyed \
                        by (type, location, rank). It is scanned at all only because \
                        SOURCE_EXTENSIONS was deliberately extended to cover .tsv, which is the \
                        amendment no_source_file_is_included_from_outside_the_scan asked for in \
                        its own failure message; being scanned is the point, since that is what \
                        makes include_str! of it unable to smuggle anything past the static mut \
                        ban or the game-name ban. The cap is 2400 rather than unbounded so that \
                        a regeneration which doubled the file still goes red. Generated Rust was \
                        measured and rejected: cargo fmt expands 200 entries written one per \
                        line from 203 lines to 4,223, so the const-array form would be ~50,500 \
                        lines across ~127 files. The next move, if Lich grows past 2,400 \
                        entries, is to raise this number in the same commit that regenerates the \
                        file and updates GOLDEN_DIGEST in crates/cena-model/tests/crit_parity.rs \
                        -- all three change together or the parity test goes red, which is the \
                        ratchet working.",
    },
];

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
//
// # AMENDED when the crit tables landed: `include_str!` of a SCANNED file
//
// That day came. `plan/13` section 4a names the crit tables as a port target
// and `plan/13:125` says they "ship as data files". A const array is not an
// option: `cargo fmt` was VERIFIED to expand 200 entries written one per line
// from 203 lines to 4,223 (21x), which extrapolates to ~50,500 lines and ~127
// files at the 400-line default cap. Moving code down does not mean 127 files
// of generated Rust nobody reads.
//
// So the amendment this comment asked for in advance was taken, exactly as
// written: `harness::SOURCE_EXTENSIONS` gained `tsv`, and the ban narrowed
// from three macros to two.
//
// **The narrowing is sound because the ban was never about `include_str!`.**
// The stated harm is a file spliced into a crate from OUTSIDE EVERY SCAN.
// `include_str!` of a scanned `.tsv` has neither half of that: the file is
// walked by `crate_sources`, so the line cap, the `static mut` ban and the
// game-name ban all read it -- and its contents become a `&str`, never items,
// so there is nothing for those bans to miss. The 906-line `pub static mut`
// that defeated four rules is not expressible through a string literal.
//
// `include!` and `include_bytes!` stay banned. `include!` splices code, which
// is the original harm. `include_bytes!` is banned because bytes are NOT
// scanned as text: `collect_sources` reads with `read_to_string`, so a
// non-UTF-8 payload is a file the walk cannot read -- the out-of-scan hole
// again, by another route.
// ---------------------------------------------------------------------------

#[test]
fn no_source_file_is_included_from_outside_the_scan() {
    let hits = scan_lines(&scannable_sources(), &["include!(", "include_bytes!("]);
    assert!(
        hits.is_empty(),
        "`include!` splices a file into a crate without that file being a \
         module, which puts it outside every scan in this suite. VERIFIED: a \
         906-line `.in` file with `pub static mut` and an `if game == \
         \"GemStone\"` branch compiled in with four bans green.\n\n\
         If generated code is genuinely needed (plan/13 §4a), lift this ban \
         deliberately and extend harness::SOURCE_EXTENSIONS so the generated \
         file is scanned, rather than routing around the scan.\n\n\
         `include_str!` of a file the walk already collects is NOT banned -- \
         see this test's comment for the crit tables, the case that amendment \
         was written for. `include_bytes!` remains banned: collect_sources \
         reads with read_to_string, so a non-UTF-8 payload is a file no scan \
         can see.\n{}",
        hits.join("\n")
    );
}

// ---------------------------------------------------------------------------
// Rule 2.1 — Nothing above `cena-protocol` ever sees a raw byte or an
// unparsed string. (plan/05:270-274)
//
// "Enforced by: `cena-protocol` exposes no `String`-of-wire-text type in its
// public API; architecture test." (plan/05:273-274)
//
// This rule was DEFERRED until `cena-protocol` had a public API to name. It
// now does, so the deferral is spent and the test is written -- Rule 9.3
// (:515-517) asks for the test when the rule is adopted, and "the types exist
// now" was the stated unblocking condition.
//
// # Rule 2.2 mandates the one escape 2.1 forbids
//
// The deferral entry said to write the two as a cross-referenced pair, and
// that is what RAW_TEXT_ESCAPES is. Rule 2.2 (:276-283) REQUIRES an unmodelled
// tag to reach the user as text, which is by definition raw wire text in the
// public API. So the test cannot ban the shape outright; it bans every
// instance except the ones 2.2 compels, each named with its reason.
//
// # What this enforces, and what it does not
//
// It scans `cena-protocol`'s public items for fields whose NAME says they
// carry unparsed wire text (`raw`, `xml`, `markup`, `inner_xml`). It is a
// lexical scan over field names, so it cannot see that a field called
// `value` holds markup -- which is exactly Vellum's violation
// (`src/parser.rs:128-131`, `Component { id, value }` where `value` is the raw
// inner XML). That specific case is covered by a golden instead
// (`room_components_arrive_parsed_rather_than_as_markup`), which asserts a
// component body contains no `<`.
//
// So: this test catches a field that ANNOUNCES itself as raw, and the golden
// catches the one that does not. Neither is a proof; both name what they do.
// ---------------------------------------------------------------------------

/// A public field in `cena-protocol` that may carry raw wire text, and the
/// rule that compels it.
///
/// Two of the three are Rule 2.2's. The third, `Structural`, is the author's
/// drop-nothing rule, which is the same shape of obligation from a different
/// direction: 2.2 says an unmodelled tag must reach the user, drop-nothing
/// says an unremarkable one must too.
const RAW_TEXT_ESCAPES: &[(&str, &str)] = &[
    (
        "UnknownTag",
        "Rule 2.2 (:276-283) mandates it: an unmodelled tag reaches the user as text and a log. \
         The raw bytes ARE the diagnostic -- a reader has to see what the game actually sent.",
    ),
    (
        "MalformedTag",
        "A tag with no closing `>`. Same rule: it renders and is logged rather than being \
         smuggled into prose, which is the silent desync the reference has at \
         reference/VellumFE/src/parser.rs:733-736.",
    ),
    (
        "Structural",
        "Compelled by the author's drop-nothing rule of 2026-09-18 rather than by 2.2:          \"Cena shouldn't drop anything that comes in.\" Three arms in cena-protocol          returned without emitting -- close_tag's `_ if tags::is_known(name)` and          markup_tag's two `_ => {}` -- so 133 tag/form combinations produced NO frame and          were invisible to every consumer (measured by          crates/cena-protocol/tests/every_tag_is_observable.rs before the fix: 123 close          forms plus 10 self-closing). `raw` is the same diagnostic as UnknownTag's and for          the same reason -- without it a consumer knows a tag arrived but not which bytes,          and the rule is that nothing is unrecoverable. It is NOT a licence to stop          modelling: a tag that gains a real handler moves out of Structural, and the          enforcement test counts these so the move is visible in a diff.",
    ),
];

#[test]
fn wire_text_reaches_the_public_api_only_through_rule_2_2() {
    // Field DECLARATIONS, not construction sites: `raw: String` declares the
    // hole, while `raw: tag.to_owned()` merely fills one that already exists
    // and is already allowlisted. A first draft matched both and reported four
    // violations that were all just the two escapes being built.
    let needles = [
        "raw: String",
        "xml: String",
        "markup: String",
        "inner_xml: String",
        "raw_text: String",
        "raw: Vec<u8>",
        "bytes: Vec<u8>",
    ];
    let mut violations = Vec::new();

    for (path, text) in workspace_sources() {
        let rel = relative(&path);
        if !rel.starts_with("crates/cena-protocol/src/") {
            continue;
        }
        for item in items(&text) {
            if item.in_test_module {
                continue;
            }
            let code = collapse_whitespace(&item.code);
            if !needles.iter().any(|n| code.contains(n)) {
                continue;
            }
            // Allowed only inside a variant that Rule 2.2 compels.
            let excused = RAW_TEXT_ESCAPES
                .iter()
                .any(|(variant, _)| code.contains(variant));
            if !excused {
                violations.push(format!("{rel}:{}: {code}", item.line));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Nothing above cena-protocol ever sees a raw byte or an unparsed \
         string (plan/05 Rule 2.1, :270-274). A public field named `raw`, \
         `xml` or `markup` hands wire text to the layer above and makes it \
         re-parse.\n\n\
         The ONLY exceptions are the ones Rule 2.2 (:276-283) mandates -- \
         Frame::UnknownTag and Frame::MalformedTag -- because 2.2 requires an \
         unmodelled tag to reach the user as text. Those are listed in \
         RAW_TEXT_ESCAPES with their reasons. If a new escape is genuinely \
         compelled, add it there and say which rule compels it.\n\n\
         NOTE this scan reads field NAMES. A field called `value` that holds \
         markup is invisible to it -- that is Vellum's own violation at \
         src/parser.rs:128-131, and it is covered by the golden \
         `room_components_arrive_parsed_rather_than_as_markup` instead.\n{}",
        violations.join("\n")
    );
}

#[test]
fn every_raw_text_escape_names_the_rule_that_compels_it() {
    // A bare allowlist would let a future escape be added with no argument.
    // Rule 2.1 is only meaningful if each hole in it is justified in writing.
    for (variant, reason) in RAW_TEXT_ESCAPES {
        assert!(
            reason.len() > 80 && reason.contains(':'),
            "the RAW_TEXT_ESCAPES entry for {variant} must cite the rule that \
             compels it (plan/05 §-2), not {reason:?}"
        );
    }
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
