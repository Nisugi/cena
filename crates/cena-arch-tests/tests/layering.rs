//! Architecture test: the layering rule, and the table that IS the architecture.
//!
//! Split out of `tests/architecture.rs` under Rule 4.1 (`plan/05:352-353`) --
//! **move code down, do not raise the cap.** That file hit 411 lines against
//! the 400 default when the Milestone 1 Step 2 edge amendments were written
//! into `ALLOWED_EDGES`, and the response was the one the rule prescribes: the
//! layering rule moved to its own file rather than the cap moving up. Its
//! module header had already named the split axis -- "the split is by rule
//! section".
//!
//! Read `tests/architecture.rs`'s module header first: its "what these tests
//! do NOT claim" paragraph governs this file too.

use cena_arch_tests::harness::{crate_dependency_names, member_crates};
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
    // AMENDED for Milestone 1 Step 2, the session actor slice. The row was
    // `&["cena-model"]`. Two edges added, both downward under `plan/12:72-86`:
    //
    // `cena-protocol` -- the session drives `Parser` directly. `Parser` is
    // stateful and explicitly ONE PER SESSION
    // (`crates/cena-protocol/src/parser.rs:113-120`), so the session owns that
    // value, and the actor matches on `Frame` to build `GameState`.
    // `cena-model -> cena-protocol` already exists
    // (`crates/cena-model/Cargo.toml:8`), so this is the same direction one
    // layer further; it closes no cycle and skips no layer upward. Routing it
    // through `cena-model` was the alternative and was rejected: re-exporting
    // `Parser` and 51 `Frame` variants (measured: python over the enum body in crates/cena-protocol/src/frame.rs, counting 4-space-indented variant heads) is a pass-through facade with one
    // caller, which Rule -1 (`plan/05` §-1) forbids.
    //
    // `cena-platform` -- the session OWNS THE SOCKET. `plan/12` §9c moves
    // reconnect to Milestone 2, so a connection-manager crate between them
    // would today be a trait with one implementor.
    //
    // `plan/12` §2's prose does not spell either row out; this is the
    // amendment that table's own doc calls for, taken deliberately rather
    // than by drift.
    (
        "cena-session",
        &["cena-model", "cena-platform", "cena-protocol"],
    ),
    // AMENDED for Milestone 1 Step 2. `cena-platform` is a DEV-dependency
    // only: criterion 7 (`plan/12:465`) says the replay runs "with no
    // network", and a behavior test that proves criterion 4's stop latency
    // needs a live session to stop -- which means a `ReplaySource`. This
    // table filters to intra-workspace edges and does not distinguish dev
    // from normal (see `harness::crate_dependency_names`), so the edge must
    // be recorded here to be legal. It is downward under `plan/12:72-86`
    // (behavior -> session -> platform) and it is absent from the shipped
    // graph: `cargo tree -p cena-behavior -e normal` does not contain it.
    ("cena-behavior", &["cena-platform", "cena-session"]),
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
