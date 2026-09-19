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
    // only: criterion 7 (`plan/12:551`) says the replay runs "with no
    // network", and a behavior test that proves criterion 4's stop latency
    // needs a live session to stop -- which means a `ReplaySource`. This
    // table filters to intra-workspace edges and does not distinguish dev
    // from normal (see `harness::crate_dependency_names`), so the edge must
    // be recorded here to be legal. It is downward under `plan/12:72-86`
    // (behavior -> session -> platform) and it is absent from the shipped
    // graph: `cargo tree -p cena-behavior -e normal` does not contain it.
    ("cena-behavior", &["cena-platform", "cena-session"]),
    ("cena-ui", &["cena-model"]),
    // AMENDED for Milestone 1 Step 2, the live run (author's call,
    // 2026-09-18). The row was `&["cena-behavior", "cena-session",
    // "cena-ui"]`.
    //
    // **This is the edge this table's own doc comment warns about** -- a
    // direct edge from the binary to a layer three below it -- and it is taken
    // knowingly rather than by drift. The doc above says "If the author wants
    // the looser reading, this row is where to change it." This is that
    // change, for one named reason:
    //
    // `cena_platform::eaccess::authenticate` produces a `LiveSource`, and
    // `Session::new` consumes a `ByteSource`. Something must hold both ends,
    // and the binary is the only layer that can: the session cannot, because
    // then it would own a login protocol it does not otherwise speak.
    //
    // The alternative considered and declined was `Session::connect(creds)`,
    // which keeps this row at three crates. It was declined because it moves
    // EAccess *up* into `cena-session` to avoid an edge pointing *down* --
    // trading a real layering violation for a cosmetic one.
    //
    // NOTE the scope of what this permits: `cena` may now name
    // `cena-platform`. It still may not name `cena-protocol` or `cena-model`,
    // and the set equality below is what keeps that true.
    (
        "cena",
        &["cena-behavior", "cena-platform", "cena-session", "cena-ui"],
    ),
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
        // Filtered by MEMBERSHIP, not by name prefix.
        //
        // This was `starts_with("cena")`, which goes blind the moment the
        // `cena` -> `hydra` rename `CLAUDE.md` describes is half done: a
        // `hydra-session` edge would simply not be seen, and the table would
        // report agreement it had not checked. The member set is the thing
        // actually being asked about, and it is already built above
        // (review AR-2).
        let actual: BTreeSet<String> = crate_dependency_names(krate)
            .into_iter()
            .filter(|k| members.contains(k))
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

/// Everything `cena-ui` is allowed to depend on.
///
/// **An allowlist, because the deny-list was not a rule.** It named ten
/// toolkits, so `tauri`, `slint`, `gtk4`, `cursive` and `dioxus` all passed --
/// and `tauri` is specifically the one not ruled out for this project, which
/// makes it the likeliest to arrive. A deny-list enforces a rule only against
/// the futures whoever wrote it happened to imagine (review AR-7).
///
/// Inverting is free here and nowhere else: this crate's whole dependency set
/// is one entry, so the allowlist is shorter than the list it replaces and
/// cannot go stale by omission. Adding a dependency now means saying so here,
/// which is the point -- Rule 1.3 is about what `cena-ui` may name, and a list
/// of what it may name states that rule directly.
const CENA_UI_MAY_DEPEND_ON: &[&str] = &["cena-model"];

#[test]
fn cena_ui_depends_on_no_ui_toolkit() {
    let names = crate_dependency_names("cena-ui");
    let unexpected: Vec<&String> = names
        .iter()
        .filter(|n| !CENA_UI_MAY_DEPEND_ON.contains(&n.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "cena-ui must stay toolkit-free: its types are Cena's own, never \
         ratatui's or egui's or the browser's (plan/05 Rule 1.3, :258-264).\n\n\
         This is an ALLOWLIST. If one of these is genuinely right, add it to \
         CENA_UI_MAY_DEPEND_ON and say why -- do not widen it to make a build \
         pass. A UI toolkit here is the coupling Rule 1.3 exists to prevent, \
         and it arrives looking like an ordinary dependency.\n\n\
         Unexpected: {unexpected:?}\n\
         Allowed: {CENA_UI_MAY_DEPEND_ON:?}"
    );
}

/// Edges justified as **dev-only** are absent from the shipped graph.
///
/// `ALLOWED_EDGES` records `cena-behavior -> cena-platform` with the reason
/// that it is "a DEV-dependency only ... absent from the shipped graph:
/// `cargo tree -p cena-behavior -e normal` does not contain it".
///
/// That is a claim about the **normal** edges, and it was checked against a
/// listing built with `--edges normal,build,dev`, which merges all three and
/// cannot tell them apart. Moving the entry to `[dependencies]` -- and with it
/// opening a `LiveSource` from behavior code, a real socket in a behavior --
/// left the suite green (review AR-2).
///
/// The justification is worth keeping precisely because it is narrow: a test
/// crate reaching for `AnsweringSource` is fine, and the same edge in the
/// shipped graph would put transport concerns inside behavior logic.
#[test]
fn a_dev_only_edge_stays_out_of_the_shipped_graph() {
    // (dependent, dependency) pairs whose entry in ALLOWED_EDGES is justified
    // as dev-only. Each is asserted ABSENT from `--edges normal`.
    const DEV_ONLY: &[(&str, &str)] = &[("cena-behavior", "cena-platform")];

    for (dependent, dependency) in DEV_ONLY {
        let shipped = cena_arch_tests::harness::shipped_dependency_names(dependent);
        assert!(
            !shipped.contains(*dependency),
            "`{dependent} -> {dependency}` is recorded in ALLOWED_EDGES as a \
             DEV-dependency only, and it is now in the SHIPPED graph.\n\n\
             Either move it back to [dev-dependencies], or -- if the edge is \
             genuinely wanted at runtime -- rewrite its justification in \
             ALLOWED_EDGES, because the one it carries is now false. A \
             behavior that can open a transport directly is the coupling the \
             layer graph exists to prevent.\n\n\
             Shipped dependencies of {dependent}: {shipped:?}"
        );
        // ...and still present in the merged graph, or the test above passes
        // because the dependency was simply deleted.
        let all = cena_arch_tests::harness::crate_dependency_names(dependent);
        assert!(
            all.contains(*dependency),
            "`{dependent} -> {dependency}` is gone entirely. If that is \
             intended, drop it from ALLOWED_EDGES and from DEV_ONLY here -- \
             leaving it listed makes this test pass by checking nothing."
        );
    }
}
