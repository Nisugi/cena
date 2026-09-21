//! Architecture tests: layering and state.
//!
//! One test per rule, each naming the rule it enforces and the line of
//! `plan/05` or `plan/12` it comes from. The companion file
//! `tests/file_rules.rs` holds the file-shape rules (caps, facades,
//! `include!`) and the Rule 9.3 ratchet; the split is by rule section, under
//! `plan/05:352-353` -- move code down, do not raise the cap.
//!
//! Shared mechanism lives in `cena_arch_tests::harness` (workspace geometry,
//! dependency resolution, source discovery) and `cena_arch_tests::lexical`
//! (comment stripping, span joining, item classification). Read those modules'
//! docs for why each exists. These files hold only the rules.
//!
//! # What these tests do NOT claim
//!
//! Every ban here is a **lexical scan or a manifest query with named limits**,
//! not a proof. Four adversarial passes established which evasions are closed
//! and which are documented-but-open; each open one is named in the doc
//! comment of the test that does not cover it, and the test's *name* says what
//! it actually enforces. `plan/05` §0 cuts both ways: a rule that is not
//! enforced is a wish, and a test claiming more than it enforces is worse than
//! one claiming less, because the reader stops looking.

use cena_arch_tests::harness::{relative, scannable_sources, workspace_root};
use cena_arch_tests::lexical::{items, scan_spans};

// ---------------------------------------------------------------------------
// Rule 5.2 — No process globals. None. (plan/05:400-408)
//
// "Enforced by: architecture test bans `static mut`, and `lazy_static`/
// `OnceLock` of mutable game state." (plan/05:407-408)
//
// This is the rule multi-session depends on. Lich's entire character model is
// process-global (`@@loot`, `@@right_hand` at lib/common/gameobj.rb:25-42;
// `XMLData` referenced 575 times), which is *why* Lich runs one OS process per
// character (plan/05:401-406). Cena's headline feature is the absence of that.
//
// Vellum has no equivalent test — it is single-session and never needed one.
// This is a test Cena needs that the reference does not have.
//
// # This is an ALLOWLIST, and that is the whole design
//
// The needle-based version of this rule was defeated by the ordinary way to
// give a registry an API:
//
// ```text
// pub struct Registry { inner: Mutex<Vec<SessionHandle>> }
// static ACTIVE_CHARACTER_SESSIONS: Registry = Registry::new();
// ```
//
// VERIFIED: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
// and all 11 tests green on a genuinely mutable process-global session
// registry, proven mutable by a passing test that drove `sessions().len()`
// from 0 to 2 through a shared reference. Lich's exact failure mode, through
// every gate.
//
// No needle can close that: whether `static X: T` is mutable is a
// type-resolution question, and `T` can be defined in another crate. The
// severity gradient runs the wrong way — the better-engineered the global, the
// more invisible it is to a scan.
//
// So the test is inverted. Every `static` item in the workspace must appear in
// `ALLOWED_STATICS` with a justification. A new `static` is then a reviewed
// table entry regardless of its type, which is strictly more enforcing than
// any needle and needs no dependency. The workspace has zero statics today, so
// the table starts empty and costs nothing.
//
// `static mut` keeps its own test below, because the message it gives is
// specific and `static mut` is never allowlistable.
// ---------------------------------------------------------------------------

/// A `static` item permitted to exist, with why.
///
/// `plan/05:408`: "Immutable config and interned tables are fine." The
/// justification field is where that judgement is recorded and reviewed —
/// `KNOWN_WIRE_TAGS` (`plan/13` §4a) was the expected first entry and is now
/// the actual one. A `Mutex` of game state is not fine, and the reviewer of
/// the diff that adds it is the enforcement.
///
/// An earlier draft of this comment sized that table at "~130 entries",
/// inherited from CLAUDE.md. It is **116** in the reference, measured twice;
/// the entry below carries the command. Corrected here because a number
/// nobody measured is exactly what `plan/05` §-2 forbids, and a stale one in
/// the enforcer's own documentation is worse than in prose.
struct AllowedStatic {
    /// Path relative to the workspace root, forward slashes.
    path: &'static str,
    /// The static's identifier.
    name: &'static str,
    /// Why this static is not a process global.
    justification: &'static str,
}

/// The reviewed statics.
const ALLOWED_STATICS: &[AllowedStatic] = &[
    AllowedStatic {
        path: "crates/cena-model/src/state/combat/defs.rs",
        name: "DEFS",
        justification: "A OnceLock<Defs> holding the 954 combat definition rows from three \
                        include_str! TSVs -- 946 of them compiled regexes -- built on first use \
                        and never mutated. The same argument as bounty.rs's MATCHERS, gameobj.rs's \
                        TABLE, armaments.rs's TABLES and creature.rs's BESTIARY, and the same \
                        caveat: it IS process-wide state, made safe by holding no session handle \
                        and being a pure function of compile-time strings. Compiling ~950 regexes \
                        per classification, or per session, is not a tradeoff worth making: a \
                        combat consumer asks every family of every line of every prompt-bounded \
                        chunk during a fight. Lich holds the same tables as frozen module \
                        constants, one TABLE per def file. Also folded into this one \
                        static, rather than owning statics of their own: parser.rb's \
                        SELF_IN_PATTERN and SWING_WEAPON_PATTERN, two regexes the attack \
                        classifier reads.",
    },
    AllowedStatic {
        path: "crates/cena-model/src/state/menu.rs",
        name: "DICT",
        justification: "A OnceLock<MenuCommands> holding the 1,106 context-menu command rows \
                        from one include_str! TSV, built on first use and never mutated. The \
                        same argument as defs.rs's DEFS and creature.rs's BESTIARY, and the same \
                        honest caveat: it IS process-wide state, made safe by holding no session \
                        handle and being a pure function of compile-time strings. The dictionary \
                        is the game's own, keyed by coordinate, and a `<menu>` response carries \
                        NO labels -- MEASURED over 425 `<mi>` in the corpus, zero carry a label \
                        or command -- so every frontend of every session resolves against the \
                        identical table and none of them may mutate it. Holding it per session \
                        would parse the same 1,106 rows N times for N characters and produce N \
                        identical maps. Deliberately NOT included is anything about a menu that \
                        was actually received: a ResolvedItem is returned to its caller and the \
                        exist id it substitutes belongs to the caller, never to this table.",
    },
    AllowedStatic {
        path: "crates/cena-model/src/spells.rs",
        name: "TABLE",
        justification: "A OnceLock<BTreeMap<u16, Spell>> holding the 514 spells cut from Lich's \
                        data/effect-list.xml by tools/extract_spells.rb, parsed from one \
                        include_str! TSV on first use and never mutated. The same argument as \
                        creature.rs's BESTIARY, armaments.rs's TABLES and gameobj.rs's TABLE, and \
                        the same honest caveat: it IS process-wide state, made safe by holding no \
                        session handle and being a pure function of compile-time strings. The \
                        source file lists 515 <spell> elements and 514 distinct numbers -- 9052 \
                        appears twice, byte-identical -- and the first wins, which is also \
                        spell.rb:160's rule. Smaller than the bestiary, but asked on every spell \
                        up and down message, so parsing per query would be a per-line cost. NOT \
                        included: anything about which spells are ACTIVE, which is per-session \
                        and lives in Effects and in the character model.",
    },
    AllowedStatic {
        path: "crates/cena-model/src/state/creature.rs",
        name: "BESTIARY",
        justification: "A OnceLock<Bestiary> holding the 627 creature templates, joined from four \
                        include_str! TSVs on first use and never mutated. The same argument as \
                        armaments.rs's TABLES and gameobj.rs's TABLE, and the same honest caveat: \
                        it IS process-wide state, made safe by holding no session handle and \
                        being a pure function of compile-time strings. This is the largest of \
                        them -- 627 creatures, 1,394 room UID spans, 1,603 attacks and 3,862 \
                        message lines across 580 KB of TSV -- and the join is four passes with a \
                        BTreeMap lookup per row, so parsing it per query is not a tradeoff worth \
                        making: a hunting consumer asks `what lives in this room` on every room \
                        change. Lich holds the same data in a class variable behind an @@loaded \
                        flag (creature.rb:11-12, :81-122). Deliberately NOT included is any \
                        live per-creature state: CreatureInstance's damage, stun estimates and \
                        room roster stay unported precisely because they would be mutable \
                        process-wide state, which this rule exists to forbid.",
    },
    AllowedStatic {
        path: "crates/cena-model/src/state/societies/membership.rs",
        name: "STANDING",
        justification: "A OnceLock<Option<Regex>> holding ONE compiled pattern: the society \
                        standing line (parser.rb:38), which is the only membership line needing \
                        captures -- it reads a society name and an optional rank out of the \
                        middle of the text. It is built on first use, never mutated, holds no \
                        session handle and is a pure function of a string literal in the same \
                        file, so concurrent readers cannot observe different values. It is an \
                        Option rather than an expect() because a pattern that failed to compile \
                        should make classification return None, not abort the process. The other \
                        six membership patterns were regexes in the first draft and are now \
                        plain string prefixes compared with starts_with: every one of them is a \
                        literal with no regex syntax in it, so compiling them bought nothing and \
                        cost six more statics to justify. Only this one earns the machinery.",
    },
    AllowedStatic {
        path: "crates/cena-model/src/state/armaments.rs",
        name: "TABLES",
        justification: "A OnceLock<Tables> holding the weapon, armor, shield and alias tables,                         parsed from four include_str! TSVs on first use and never mutated. The                         third of these in the crate and the argument does not change: process-                         wide state, made safe by holding no session handle and being a pure                         function of compile-time strings. 96 weapons, 18 armor sub-groups, 4                         shields and 706 aliases, parsed once rather than per lookup -- and a                         loot filter or a damage estimate reads them per item, so per-call                         parsing is not a tradeoff worth making. Lich holds the same data in                         class variables behind Lich::Util.deep_freeze                         (armaments/weapon_stats.rb:55).",
    },
    AllowedStatic {
        path: "crates/cena-model/src/state/gameobj.rs",
        name: "TABLE",
        justification: "A OnceLock<Table> holding the compiled gameobj classification patterns,                         built from an include_str! of data/gameobj-data.tsv on first use and                         never mutated after. Same argument as bounty.rs's MATCHERS below, and                         the same caveat: it IS process-wide state, made safe by holding no                         session handle and being a pure function of a compile-time string. The                         table is ~100 regexes over a 135 KB TSV, one of which is a 40 KB                         alternation of creature names, so compiling it per object -- which is                         what a non-static would mean for a loot filter walking a room -- is not                         a tradeoff worth making. Lich reaches the same conclusion with class                         variables plus a memo cache (gameobj.rb:42, :260); the cache is NOT                         ported, because keying it by object identity would be per-session state                         in a crate that holds none.",
    },
    AllowedStatic {
        path: "crates/cena-model/src/state/bounty.rs",
        name: "MATCHERS",
        justification: "A OnceLock<Vec<(TaskKind, Regex)>> holding the 22 compiled bounty task                         patterns, built on first use and never mutated after. It is process-wide                         state and this entry does not pretend otherwise -- what makes it safe is                         that it holds no handle to anything a session owns and its contents are                         a pure function of string literals in the same file, so two sessions                         reading it concurrently cannot observe different values or interfere.                         The alternative is compiling 22 regexes per bounty check, per session;                         `regex` documents compilation as the expensive step and matching as the                         cheap one. `crit.rs` faces the same tradeoff for ~2,395 patterns and                         resolves it with an owned table threaded through the model, which is the                         better shape -- this should move to it when a second consumer needs the                         patterns, and until then a table with one reader does not earn the                         plumbing (Rule -1).",
    },
    AllowedStatic {
        path: "crates/cena-protocol/src/tags.rs",
        name: "KNOWN_WIRE_TAGS",
        justification: "An interned table of wire element names, which plan/05:408 names as fine: \
                    `&[&str]` of string literals, immutable, with no interior mutability and no \
                    handle to anything a session owns. It is read through is_known(), a \
                    binary_search over the slice. Ported from \
                    reference/VellumFE/src/parser/text.rs:174-192 per plan/13 section 4a. \
                    Measured at 116 entries there (sed -n '175,193p' | grep -oE '\"[^\"]+\"' | \
                    wc -l), NOT the ~130 an earlier draft of this comment claimed. The table \
                    in Cena holds 126: the reference's 116 plus ten added since, from the \
                    gated corpus replay and the Saga reconciliation. Measured by \
                    grep -cE '^    .[^.]+.,$' crates/cena-protocol/src/tags.rs -- this said \
                    121, which was true when written. A count copied into a second place is \
                    a count that drifts, so the command is here and not only the number.",
    },
];

#[test]
fn every_static_is_allowlisted() {
    let sources = scannable_sources();
    let mut violations = Vec::new();

    for (path, text) in &sources {
        let rel = relative(path);
        for item in items(text) {
            // Anchored on `static` as a standalone token in a line that
            // declares an item. `item.code` is whitespace-collapsed, so a
            // declaration split by rustfmt -- or held apart by
            // `#[rustfmt::skip]`, which survives `cargo fmt --check` -- is
            // still one string here.
            let tokens: Vec<&str> = item.code.split_whitespace().collect();
            let Some(pos) = tokens.iter().position(|t| *t == "static") else {
                continue;
            };
            // `&'static T` in a signature is not a static item. The keyword
            // there is part of the lifetime token `'static`, which does not
            // match `== "static"`, so this is already excluded -- but a
            // `static` appearing after `fn` is a signature, not an item.
            if tokens[..pos].contains(&"fn") {
                continue;
            }
            // `static NAME:` or `static mut NAME:`; the name is the first
            // token after the keyword that is not `mut`.
            let name = tokens
                .iter()
                .skip(pos + 1)
                .find(|t| **t != "mut")
                .map_or("<unnamed>", |t| t.trim_end_matches(':'));
            let allowed = ALLOWED_STATICS
                .iter()
                .any(|a| a.path == rel && a.name == name);
            if !allowed {
                violations.push(format!("{rel}:{}: static {name}", item.line));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "No process globals. None. (plan/05 Rule 5.2, :400-408.) Every \
         `static` must be listed in ALLOWED_STATICS with a justification.\n\n\
         This is an allowlist and not a needle because a needle was VERIFIED \
         defeated by the ordinary shape of a registry with an API: \
         `struct Registry {{ inner: Mutex<..> }}` plus `static R: Registry = \
         Registry::new();` passed fmt, clippy and all 11 tests while being a \
         genuinely mutable process global.\n\n\
         If this static is immutable config or an interned table \
         (plan/05:408, e.g. KNOWN_WIRE_TAGS), add it to ALLOWED_STATICS and \
         say so. If it holds mutable game state, it belongs in a session, not \
         in the process.\n{}",
        violations.join("\n")
    );
}

#[test]
fn allowlisted_statics_exist_and_are_justified() {
    // A stale allowlist entry is worse than none: it reads as a reviewed
    // decision about a file that has since moved.
    for entry in ALLOWED_STATICS {
        assert!(
            entry.justification.len() > 30,
            "the ALLOWED_STATICS entry for {}::{} needs a real justification, \
             not {:?} (plan/05:408 -- 'immutable config and interned tables \
             are fine' is a judgement someone has to make in writing)",
            entry.path,
            entry.name,
            entry.justification
        );
        let target = workspace_root().join(entry.path);
        assert!(
            target.is_file(),
            "ALLOWED_STATICS names {}, which does not exist; a stale entry \
             permits a static nobody reviewed in a file nobody has",
            entry.path
        );
    }
}

#[test]
fn no_static_mut_anywhere() {
    // Matched against whitespace-collapsed item spans, not raw lines.
    // VERIFIED that the raw-line needle is defeated by
    //
    //     #[rustfmt::skip]
    //     pub static
    //         mut CURRENT_STREAM_BUFFER: u64 = 0;
    //
    // which compiles and passes `cargo fmt --check` -- so relying on rustfmt
    // to rejoin the tokens is relying on the author's cooperation.
    let hits = scan_spans(&scannable_sources(), &["static mut "]);
    assert!(
        hits.is_empty(),
        "`static mut` is banned (plan/05 Rule 5.2, :400-408): process-global \
         mutable state is what forces one OS process per character.\n{}",
        hits.join("\n")
    );
}
