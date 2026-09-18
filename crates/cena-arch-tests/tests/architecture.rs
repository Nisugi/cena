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

use cena_arch_tests::harness::{
    crate_dependency_names, member_crates, relative, scannable_sources, workspace_root,
};
use cena_arch_tests::lexical::{items, scan_spans};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Rule 1.1 / 1.3 — the dependency graph IS the architecture. (plan/12 §2,
// plan/05:225-246)
//
// `cargo` enforces only the acyclic half. VERIFIED that a forbidden edge which
// happens not to close a loop compiles clean: adding `cena-session.workspace =
// true` to `crates/cena-ui/Cargo.toml` violates `plan/12:81` and `plan/05:232`
// and neither the compiler nor the rest of this suite noticed.
//
// That edge is not hypothetical — `cena-ui` reaching into `cena-session` is
// the natural shortcut the moment a frontend wants live session state, and
// Rule 1.3 (`plan/05:258-264`) exists because that boundary erodes.
//
// Edges come from `cargo tree`, not from reading manifests. Three manifest
// shapes were VERIFIED to hide exactly this edge from a hand parser while the
// suite stayed green: a `[target.'cfg(windows)'.dependencies]` table, a
// `[dependencies.sess]` sub-table, and a rename `sess = { package =
// "cena-session" }`. See `harness::crate_dependency_names`.
//
// The table is asserted as a SET EQUALITY, not a subset: a missing edge is as
// much a drift from plan/12 §2 as an extra one, and equality is what makes the
// table a statement of the architecture rather than a floor under it.
// ---------------------------------------------------------------------------

/// The allowed intra-workspace edges, from `plan/12:78-86` (authoritative) and
/// `plan/05:226-235`.
///
/// **This table is deliberately stricter than `plan/12:81`.** That line says
/// `cena` depends on "everything"; the row below names three crates. The other
/// four arrive transitively, so the binary can still reach them — but because
/// this is a set equality, adding `cena-model.workspace = true` to
/// `crates/cena/Cargo.toml` goes red. That is intended: a direct edge from the
/// binary to a layer three below it is the shortcut the graph exists to
/// prevent. Read `plan/12:81` as "may reach everything", not "may name
/// everything". If the author wants the looser reading, this row is where to
/// change it.
///
/// `cena-arch-tests` has no row in `plan/12` §2. `plan/05` Rule 1.2
/// (`:254-256`) requires every crate to have a stated reason and a place in the
/// graph, recorded in that document, so this is a **known gap in the scaffold,
/// enforced by review and not by this test** — the member-set equality below
/// forces the crate to appear *here*, but nothing forces `plan/12` §2 to gain a
/// row. Flagged for the author rather than fixed, because editing `plan/12` is
/// an amendment, not a scaffold change.
///
/// `cena-agent`, `cena-tui`, `cena-gui` and `cena-web` are in `plan/12` §2 but
/// not in this workspace — `plan/12` §7.1 puts "TUI, GUI, full web" in the Out
/// column for M1. Their rows are recorded here, commented, so the day a
/// frontend crate is added its edge set is already written down; in particular
/// `plan/05:246` forbids a frontend depending on another frontend, which is
/// also an acyclic edge no compiler will catch.
///
/// ```text
/// cena-agent           (cena-session)
/// cena-tui/gui/web     (cena-ui, cena-session)   -- and never each other
/// ```
const ALLOWED_EDGES: &[(&str, &[&str])] = &[
    ("cena-platform", &[]),
    ("cena-protocol", &["cena-platform"]),
    ("cena-model", &["cena-protocol"]),
    ("cena-session", &["cena-model"]),
    ("cena-behavior", &["cena-session"]),
    ("cena-ui", &["cena-model"]),
    ("cena", &["cena-behavior", "cena-session", "cena-ui"]),
    ("cena-arch-tests", &[]),
];

#[test]
fn crate_dependency_edges_match_the_plan() {
    let expected: BTreeMap<&str, BTreeSet<String>> = ALLOWED_EDGES
        .iter()
        .map(|(k, deps)| (*k, deps.iter().map(|d| (*d).to_owned()).collect()))
        .collect();

    let members = member_crates();
    let member_set: BTreeSet<&str> = members.iter().map(String::as_str).collect();
    let table_set: BTreeSet<&str> = expected.keys().copied().collect();
    assert_eq!(
        member_set, table_set,
        "the workspace members and the ALLOWED_EDGES table have diverged. A \
         member missing from the table is an unenforced crate; a table row \
         with no member is a stale rule. Add the crate to plan/12 §2 and to \
         this table in the same commit (plan/05 Rule 1.2, :254-256)."
    );

    let mut violations = Vec::new();
    for krate in &members {
        // Every direct dependency Cargo resolves, on every target, including
        // dev and build. Filtered to intra-workspace edges: an external crate
        // is Rule 1.3's business (below), not the layering graph's.
        let actual: BTreeSet<String> = crate_dependency_names(krate)
            .into_iter()
            .filter(|k| k.starts_with("cena"))
            .collect();
        let Some(allowed) = expected.get(krate.as_str()) else {
            continue; // already reported by the set-equality assert above
        };
        if &actual != allowed {
            let extra: Vec<&String> = actual.difference(allowed).collect();
            let missing: Vec<&String> = allowed.difference(&actual).collect();
            violations.push(format!(
                "{krate}: forbidden edges {extra:?}, missing expected edges {missing:?}"
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "Dependencies point one way, downward (plan/05 Rule 1.1, :249-252; \
         graph at plan/12:78-86). `cargo` rejects only edges that close a \
         cycle; a forbidden acyclic edge such as cena-ui -> cena-session \
         compiles clean, which is what this test exists for \
         (plan/06:133-135).\n{}",
        violations.join("\n")
    );
}

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
/// `KNOWN_WIRE_TAGS` (`plan/13` §4a, ~130 entries) is the expected first
/// entry, and it is fine. A `Mutex` of game state is not, and the reviewer of
/// the diff that adds it is the enforcement.
struct AllowedStatic {
    /// Path relative to the workspace root, forward slashes.
    path: &'static str,
    /// The static's identifier.
    name: &'static str,
    /// Why this static is not a process global.
    justification: &'static str,
}

/// Empty, deliberately. The workspace has no statics.
const ALLOWED_STATICS: &[AllowedStatic] = &[];

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

// ---------------------------------------------------------------------------
// Rule 1.3 — `cena-ui` holds the input vocabulary. (plan/05:258-264)
//
// "Enforced by: `cena-ui` does not depend on any UI toolkit; architecture test
// bans toolkit paths in `cena-ui`." (plan/05:263-264)
//
// Checked against resolved dependencies, not the source. plan/05:208-209 names
// the exact gap in Vellum's source scan: "it catches `arboard::` but not a
// re-export". A dependency list cannot be bypassed by a re-export.
//
// Resolved by `cargo tree --target all`, not by reading the manifest. A
// `[target.'cfg(unix)'.dependencies] crossterm = "0.28"` was VERIFIED to pass
// the manifest parser -- and would have been invisible to a *fixed* manifest
// parser too, running on this project's Windows dev machine.
//
// Matched against dependency *names*, not manifest text. A raw
// `manifest.contains("ratatui")` meant the comment "the tui frontend adapts
// these types to ratatui; cena-ui itself must not" broke the build — the exact
// failure mode this file criticizes Vellum for.
//
// Direct dependencies only, not the transitive closure: at M1 cena-ui's
// dependency list is one line. Revisit if cena-ui ever takes a dependency that
// could pull a toolkit in transitively — then the closure, not the direct
// list, is what matters. `cargo tree` without `--depth 1` is that upgrade.
// ---------------------------------------------------------------------------

#[test]
fn cena_ui_depends_on_no_ui_toolkit() {
    let names = crate_dependency_names("cena-ui");
    let toolkits = [
        "ratatui",
        "crossterm",
        "termion",
        "egui",
        "eframe",
        "iced",
        "winit",
        "web-sys",
        "wasm-bindgen",
        "arboard",
    ];
    let found: Vec<&str> = toolkits
        .iter()
        .copied()
        .filter(|t| names.contains(*t))
        .collect();
    assert!(
        found.is_empty(),
        "cena-ui must stay toolkit-free: its types are Cena's own, never \
         ratatui's or egui's or the browser's (plan/05 Rule 1.3, :258-264). \
         Found: {found:?}"
    );
}
