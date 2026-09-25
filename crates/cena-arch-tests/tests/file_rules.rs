//! Architecture tests: file shape, and the ratchet on the ratchet.
//!
//! The companion to `tests/architecture.rs`, which holds layering and state.
//! Split by rule section under `plan/05:352-353` -- move code down, do not
//! raise the cap. This file carries Rule 4.1 (caps), Rule 4.4 (facades),
//! Rule 3.4 (game names) and the `panic = "abort"` guard. The `include!` ban
//! is in `tests/include_ban.rs` and Rule 2.1 in `tests/raw_text_escapes.rs`.
//! Rule 9.3 and the enforcer-integrity tests moved to `tests/ratchet.rs` when
//! this file went red a fourth time.
//!
//! Read `tests/architecture.rs`'s module header first: its "what these tests
//! do NOT claim" paragraph governs all three files.

use cena_arch_tests::harness::{relative, scannable_sources, workspace_root, workspace_sources};
use cena_arch_tests::lexical::{declares_behavior, items, scan_lines};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

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
    // The four large creature tables, for exactly the reason crit_tables.tsv
    // states below: they are DATA, one fact per line, with no seam to split on.
    // The other three -- creatures.tsv (628 lines), creature_abilities.tsv (80)
    // and creature_info.tsv (51) -- need no exception, which is the ratchet
    // working: only the files that genuinely exceed the cap carry a
    // justification, and the cap still binds the ones that do not.
    //
    // RAISED 2026-09-24, when the port took everything a template says (the
    // author: "The reason it was all there was multiple reasons, one of which
    // is a comprehensive beastiary"). This regeneration went red here, which is
    // what the ~10% headroom was for; caps.baseline records the measurement.
    //
    // Each cap is set just above the current row count so that a regeneration
    // which grew the bestiary substantially still goes red and gets looked at.
    CapException {
        path: "crates/cena-model/data/creature_messages.tsv",
        cap: 7800,
        justification: "7,125 message lines of twelve kinds -- attacks, triggers, casting \
                        preparations, death, flee, arrival, decay and the rest -- for 627 \
                        creatures, one per row, generated by tools/extract_creatures.rb from \
                        Lich's bestiary. Data \
                        with no functions in it, so 'move code down' has nothing to move -- the \
                        same argument crit_tables.tsv makes below, and the same shape plan/13 \
                        section 4a specifies for creature templates. A split would have to be \
                        alphabetical by creature id, which is not a seam but an arbitrary cut \
                        through one sorted sequence. Scanned deliberately (SOURCE_EXTENSIONS \
                        covers .tsv) so that include_str! of it cannot smuggle a game name or a \
                        static mut past the other bans; the cap is 7800 rather than unbounded so \
                        a regeneration that added ~10% still goes red.",
    },
    CapException {
        path: "crates/cena-model/data/creature_attacks.tsv",
        cap: 3500,
        justification: "3,174 attacks across 627 creatures in six categories (1,603 physical, \
                        641 maneuvers, 378 warding, 247 offensive, 153 special, 152 bolt), one \
                        per row, with the attack or casting strength as a scalar or a lo..hi \
                        range. Data with no functions in \
                        it, so Rule 4.1's 'move code down' (plan/05:352-353) has nothing to \
                        move, and plan/13 section 4a specifies this shape for creature \
                        templates: static data shipping as data files. MEASURED: 3,174 rows, \
                        fourteen of which carry an unparsed raw value because the source is \
                        malformed there, a count tests/creatures.rs asserts -- so a silent \
                        change to this file fails that test as well as this one. Scanned \
                        deliberately (SOURCE_EXTENSIONS covers .tsv) so include_str! of it \
                        cannot smuggle anything past the other bans.",
    },
    CapException {
        path: "crates/cena-model/data/creature_lists.tsv",
        cap: 3650,
        justification: "3,308 list entries across 627 creatures, one per row: otherclass, \
                        equipment (1,068), immunities, defensive spells and abilities, special \
                        defenses and notes, and treasure's armaments and other drops -- the \
                        template's plain lists, one table because they are one shape (a \
                        creature, a list name, an entry). Data with no functions in it, so Rule \
                        4.1's 'move code down' has nothing to move, and plan/13 section 4a \
                        specifies static data shipping as data files. Not carried before \
                        2026-09-24. Scanned deliberately (SOURCE_EXTENSIONS covers .tsv).",
    },
    CapException {
        path: "crates/cena-model/data/creature_areas.tsv",
        cap: 1600,
        justification: "1,394 room UID spans saying where each creature is found, one span per \
                        row. Data with no functions in it, so Rule 4.1's 'move code down' \
                        (plan/05:352-353) has nothing to move, and plan/13 section 4a specifies \
                        static data shipping as data files. MEASURED: 1,394 spans across 584 of \
                        the 627 templates -- the remainder name an area with no measured rooms, \
                        which _creature_template.rb calls out as wiki presence without room \
                        data. Read by creature::in_room on every room change. Scanned \
                        deliberately (SOURCE_EXTENSIONS covers .tsv).",
    },
    CapException {
        path: "crates/cena-model/data/menu_commands.tsv",
        cap: 1300,
        justification: "DATA, not code: one row per line, a flat sequence keyed by coordinate \
                        with no seam to split on, and plan/13:125 specifies this exact shape -- \
                        'static data; ships as data files'. The 1,106 rows of the game's own \
                        cmdlist1.xml, which is the only thing that turns a context menu of bare \
                        coordinates into labels: the wire carries none, measured over 425 items \
                        in the corpus. Scanned deliberately (SOURCE_EXTENSIONS covers .tsv) so \
                        include_str! of it cannot smuggle a game name past the bans. The cap is \
                        1300 rather than unbounded because this file demonstrably grows -- it \
                        gained 514 rows when regenerated from the live client's copy rather than \
                        the 2003 one the reference ships.",
    },
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
        // **The file must still NEED the exception.**
        //
        // `cap > DEFAULT` says the entry raises the cap; it says nothing about
        // whether anything still requires it. A file split back under the
        // default kept its raised cap, so the next several hundred lines of
        // growth were invisible to the ratchet -- an exception outliving its
        // reason, the same shape as the spent deferral AR-1 found.
        //
        // The floor is the DEFAULT, not the file's current length: a file at
        // 810 lines legitimately holds a cap well above 810, and requiring the
        // cap to track the length would make every edit a cap edit.
        let lines = std::fs::read_to_string(&target)
            .map(|t| t.lines().count())
            .unwrap_or_default();
        assert!(
            lines > DEFAULT_MAX_LINES,
            "cap exception for {} is spent: the file is {lines} lines, under \
             the default cap of {DEFAULT_MAX_LINES}. It no longer needs an \
             exception, and keeping one hides {} lines of growth from the \
             ratchet. Delete the entry.",
            exception.path,
            exception.cap - DEFAULT_MAX_LINES
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
// The `include!` ban, escaping `#[path]`, and what `include_str!` may embed
// moved to `tests/include_ban.rs` when hardening them (review finding 10)
// would have put this file past its own cap.
// ---------------------------------------------------------------------------

#[test]
fn game_names_outside_game_modules_are_flagged() {
    let hits = game_name_hits(&scannable_sources());
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

/// The game's names, in every case this workspace has written them.
///
/// `GEMSTONE` and `DRAGONREALMS` were missing (review finding 13): the
/// all-caps spelling of a constant, `const GEMSTONE: &str`, was the one form
/// the list did not have.
const GAME_NAMES: &[&str] = &[
    "GemStone",
    "Gemstone",
    "gemstone",
    "GEMSTONE",
    "GS4",
    "Gs4",
    "gs4",
    "DragonRealms",
    "Dragonrealms",
    "dragonrealms",
    "DRAGONREALMS",
];

/// `EAccess` instance codes (`plan/10`): each names one game as surely as its
/// title does, and a default of `"GST"` in shared code is a `GemStone` default.
///
/// Matched as WHOLE WORDS, because three capitals are common in other words;
/// `GST` must not fire on `GSTREAMER`. Bare `DR` and `GS` are not here for the
/// reason `DR` never was: they match `DRY`, `ADDR` and every hex literal.
const INSTANCE_CODES: &[&str] = &["GS3", "GST", "GSX", "GSF", "DRX", "DRF", "DRT"];

/// Every Rule 3.4 hit in `sources`.
///
/// # Why instance codes exempt test code and names do not
///
/// A test of the login protocol must SEND an instance code -- `EAccess` takes it
/// as a parameter, and `creds("GS3")` is how a handshake test says which
/// instance it is scripting. That is a protocol input, not a branch. A game's
/// NAME in a test has no such necessity, and the rule has always read tests.
/// So the codes skip `tests/` directories, `*_tests.rs` files and
/// `#[cfg(test)]` modules; the names skip nothing new.
fn game_name_hits(sources: &[(PathBuf, String)]) -> Vec<String> {
    let exempt = |hit: &String| {
        hit.contains("/src/gemstone/")
            || hit.contains("/src/dragonrealms/")
            || is_module_plumbing(hit)
            || is_game_data(hit)
    };
    let mut hits: Vec<String> = scan_lines(sources, GAME_NAMES)
        .into_iter()
        .filter(|hit| !exempt(hit))
        .collect();
    for (path, text) in sources {
        let rel = relative(path);
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let test_file = rel.contains("/tests/") || stem.ends_with("_tests");
        if test_file || path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        for item in items(text).into_iter().filter(|i| !i.in_test_module) {
            let hit = format!("{rel}:{}: {}", item.line, item.code);
            if INSTANCE_CODES.iter().any(|c| has_word(&item.code, c)) && !exempt(&hit) {
                hits.push(hit);
            }
        }
    }
    hits
}

/// Whether `word` occurs in `text` with no identifier character either side.
fn has_word(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
}

/// Finding 13's mutations: each passed the previous needle list.
#[test]
fn an_all_caps_name_or_an_instance_code_is_flagged() {
    let shared = workspace_root().join("crates/cena-fixture/src/lib.rs");
    let cases = [
        "pub const GEMSTONE: &str = \"x\";\n",
        "let g = if g.is_empty() { \"GST\".to_owned() } else { g };\n",
        "match code { \"DRX\" => dr(), _ => gs() }\n",
    ];
    // The list as it stood before this finding.
    let old = [
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
    for case in cases {
        let hits = game_name_hits(&[(shared.clone(), case.to_owned())]);
        assert_eq!(hits.len(), 1, "missed: {case:?}");
        assert!(
            !old.iter().any(|n| case.contains(n)),
            "{case:?} no longer defeats the old list"
        );
    }
    // Whole words only, and a test of the protocol may name its instance.
    let clean = "const GSTREAMER: u8 = 1;\n";
    assert!(game_name_hits(&[(shared, clean.to_owned())]).is_empty());
    let test_file = workspace_root().join("crates/cena-fixture/tests/login.rs");
    let in_test = "let r = run(creds(\"GS3\"));\n";
    assert!(game_name_hits(&[(test_file, in_test.to_owned())]).is_empty());
}
/// Whether a flagged line is a row of **game data** rather than Rust code.
///
/// # Why data is different from code
///
/// Rule 3.4 bans game-specific code *"in shared modules behind an
/// `if game == ...` branch"* (`plan/05:332-336`). **A data file cannot
/// branch.** A `.tsv` row is a fact about the game, and the whole point of
/// shipping the crit tables and the gameobj patterns as data is that they are
/// ported rather than transcribed into Rust (`plan/13:125`).
///
/// FOUND 2026-09-19 by `cena-model/data/gameobj-data.tsv`, a transcription of
/// Lich's `gameobj-data.xml`. Two of its 113 rows name in-game ITEMS --
/// "scintillating mote of gemstone dust", "ancient crumbling gemstone" -- and
/// the needle list cannot tell an item name from the game's title. The
/// pre-existing `crit_tables.tsv` simply never happened to contain the word,
/// which is why this only surfaced now.
///
/// # Why this is not a path exemption
///
/// `.tsv` is scanned **deliberately**: `harness.rs`'s `SOURCE_EXTENSIONS`
/// comment records that `include_str!` of a data file would otherwise smuggle
/// content past every rule in this suite, and
/// `no_source_file_is_included_from_outside_the_scan` enforces that the list
/// stays sufficient. Excluding `data/` by path would reopen exactly that hole
/// for the `static mut` ban and the line cap as well.
///
/// So this narrows only the needles that **describe code shape** -- game names
/// -- and only for extensions that cannot contain code. Every other rule still
/// reads every byte of the file.
fn is_game_data(hit: &str) -> bool {
    // `scan_lines` formats a hit as `path:line: <code>`; the path is everything
    // before the first `:` that a line number follows.
    let path = hit.split(':').next().unwrap_or_default();
    // Case-insensitive: a `.TSV` that slipped past this would be exempt from
    // the game-name needles while still being compiled in by `include_str!`.
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("tsv"))
}

/// Whether a flagged line is the `mod` / `use` plumbing that REACHES a game
/// namespace, rather than game-specific code in a shared module.
///
/// # Why this exists: the rule forbade its own remedy
///
/// FOUND 2026-09-19 by the first code to actually use Rule 3.4. The rule says
/// game-specific things live in `<crate>/src/<game>/`; this test exempts that
/// directory by path -- but `"gemstone"` is also a needle, so
/// `pub(crate) mod gemstone;` in `lib.rs` was flagged, as was every
/// `use crate::gemstone::...` that reached it. **A namespace that cannot be
/// declared or imported is a namespace that cannot be used**, so the test as
/// written made Rule 3.4 unfollowable: the only passing options left were to
/// keep game-specific data in a shared module, or to hide the literal behind
/// `concat!` -- the two things the rule exists to prevent.
///
/// # Why this stays narrow
///
/// A `mod`/`use` line names a *path*; it cannot express an `if game == ...`
/// branch, which is what Rule 3.4 targets (`plan/05:332-336`). Exempting the
/// plumbing therefore removes no enforcement -- the branch itself is still
/// flagged, and so is every other mention in code.
///
/// It deliberately does NOT exempt a line merely *containing* `use` or `mod`.
/// The match is anchored at the start of the trimmed line and requires the
/// statement to end in `;`, so `dispatch(use_gemstone_rules())` is still
/// flagged, and so is a `mod gemstone { ... }` opening an inline module.
fn is_module_plumbing(hit: &str) -> bool {
    const PLUMBING: &[&str] = &["mod ", "pub mod ", "pub(crate) mod ", "use ", "pub use "];

    // `scan_lines` formats a hit as `path:line: <trimmed code>`, so the code is
    // whatever follows the LAST `": "` -- the path may contain one too.
    let Some((_, code)) = hit.rsplit_once(": ") else {
        return false;
    };
    PLUMBING.iter().any(|p| code.starts_with(p)) && code.ends_with(';')
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
// `RUSTFLAGS=-Cpanic=abort` in the CI environment defeats this, and so does a
// `config.toml` in a PARENT directory or in `$CARGO_HOME` -- Cargo merges
// config from every ancestor of the working directory and from the user's
// home, and none of those is in this repository for a test to read. The CI
// workflow sets neither variable.
//
// Member manifests are not read: Cargo ignores `[profile.*]` outside the
// workspace root with a warning, so a profile there is inert.
//
// **The workspace's own `.cargo/config.toml` IS read (review finding 9).**
// This test read only the root `Cargo.toml`, and Cargo takes `[profile.*]` --
// and `[build] rustflags` -- from `.cargo/config.toml` (or the legacy
// `.cargo/config`) with the same authority. A committed
// `[profile.release] panic = "abort"` there shipped abort with this green.
// ---------------------------------------------------------------------------

/// The files under `root` that can set a profile's `panic` strategy.
fn panic_config_files(root: &std::path::Path) -> Vec<PathBuf> {
    ["Cargo.toml", ".cargo/config.toml", ".cargo/config"]
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.is_file())
        .collect()
}

/// The lines of a TOML file that select `panic = "abort"`, however quoted.
///
/// Strips `#` comments first. Without this, the comment in the root manifest
/// that *states* this rule breaks it -- the same failure that made
/// `cena_ui_depends_on_no_ui_toolkit` fire on a comment reading "we
/// deliberately avoid ratatui". A test that punishes documenting the rule gets
/// fixed by deleting the documentation. Also catches `-C panic=abort` in a
/// `rustflags` array, which normalizes to the same string.
fn abort_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .filter(|line| {
            // Normalize both whitespace and TOML's two string quotings.
            line.replace([' ', '\t', '\'', '"'], "")
                .contains("panic=abort")
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn no_profile_aborts_on_panic() {
    let root = workspace_root();
    let files = panic_config_files(&root);
    assert!(
        files.iter().any(|f| f.ends_with("Cargo.toml")),
        "the root Cargo.toml must be readable"
    );
    let mut offenders = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("config file must be readable");
        for line in abort_lines(&text) {
            offenders.push(format!("{}: {line}", relative(file)));
        }
    }
    assert!(
        offenders.is_empty(),
        "panic=abort defeats per-session panic isolation (plan/12 §5.5): a \
         panic must kill one session, not the process. This applies to every \
         profile, not just [profile.release] -- a [profile.dist] with \
         `inherits = \"release\"` was VERIFIED to build and ship abort.\n{}",
        offenders.join("\n")
    );
}

/// Finding 9's mutation: the abort lives in `.cargo/config.toml`, which the
/// previous test never opened.
#[test]
fn an_abort_in_cargo_config_is_found() {
    let dir = std::env::temp_dir().join(format!("cena-arch-panic-{}", std::process::id()));
    fs::create_dir_all(dir.join(".cargo")).expect("temp dir");
    fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = []\n").expect("manifest");
    fs::write(
        dir.join(".cargo/config.toml"),
        "[profile.release]\npanic = 'abort'\n\n[build]\nrustflags = [\"-C\", \"panic=abort\"]\n",
    )
    .expect("config");

    let files = panic_config_files(&dir);
    let manifest = fs::read_to_string(dir.join("Cargo.toml")).expect("manifest");
    // The old test read the manifest alone, and it is clean.
    assert!(abort_lines(&manifest).is_empty());
    let found: usize = files
        .iter()
        .map(|f| abort_lines(&fs::read_to_string(f).expect("read")).len())
        .sum();
    fs::remove_dir_all(&dir).expect("clean up");
    assert_eq!(found, 2, "the profile line and the rustflags line");
}
