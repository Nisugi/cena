//! Every member crate is governed by the workspace lints.
//!
//! `unsafe_code`, `unwrap_used`, `expect_used` and `panic` are denied in the
//! root manifest, and they reach a crate **only** through
//! `[lints] workspace = true`. A new crate that omits the stanza gets all four
//! past `cargo clippy --workspace --all-targets -- -D warnings`, silently: the
//! build is green, and the rules simply do not apply there.
//!
//! `ratchet.rs`'s `the_enforcer_inherits_every_workspace_lint_it_does_not_name`
//! reads the root manifest and **this crate's**, which is a different question
//! -- it is about the enforcer's own deviations. Nothing covered the other
//! seven (review AR-4).
//!
//! All eight are correct today, which is what makes this the right moment:
//! a test written while the tree is green is a ratchet, and one written after
//! a crate has drifted is a cleanup.

use cena_arch_tests::harness::{member_crates, workspace_root};

/// The crate that cannot inherit, and why that is not an exception being
/// carved for convenience.
///
/// `cena-arch-tests` must **flip** `unwrap_used`, `expect_used` and `panic`
/// to `allow`: it is a test harness whose whole job is to read the filesystem
/// and assert, and the root manifest denies exactly those. Inheriting and then
/// overriding is not expressible -- `workspace = true` merges, so a crate
/// cannot both take the set and contradict three of it.
///
/// The cost is that its manifest restates the rest by hand, and a hand-written
/// copy drifts. That is precisely what `ratchet.rs`'s enforcer test exists to
/// catch, so the gap is covered rather than merely accepted.
const CANNOT_INHERIT: &str = "cena-arch-tests";

#[test]
fn every_member_crate_inherits_the_workspace_lints() {
    let root = workspace_root();
    let members = member_crates();
    assert!(
        members.len() > 5,
        "only {} member crates were found; the walk is not reading the root \
         manifest, and a test that finds nothing passes by seeing nothing",
        members.len()
    );

    let mut missing = Vec::new();
    let mut checked = 0usize;
    for name in &members {
        if name == CANNOT_INHERIT {
            continue;
        }
        let path = root.join("crates").join(name).join("Cargo.toml");
        let Ok(manifest) = std::fs::read_to_string(&path) else {
            missing.push(format!("  {name}: {} is unreadable", path.display()));
            continue;
        };
        checked += 1;
        // The stanza is `[lints]` followed by `workspace = true`. Matching the
        // pair rather than either alone: `[lints.clippy]` with its own entries
        // is a crate opting OUT while looking like it opted in.
        let inherits = manifest.split("[lints]").skip(1).any(|after| {
            after
                .lines()
                .take(3)
                .any(|l| l.trim() == "workspace = true")
        });
        if !inherits {
            missing.push(format!(
                "  {name}: no `[lints]` + `workspace = true` in {}",
                path.display()
            ));
        }
    }

    assert!(
        checked >= members.len() - 1,
        "only {checked} of {} manifests were actually read",
        members.len()
    );
    assert!(
        missing.is_empty(),
        "these crates do not inherit the workspace lints, so `unsafe_code`, \
         `unwrap_used`, `expect_used` and `panic` do not apply to them -- and \
         nothing about the build says so.\n\n\
         `plan/12` §5.5 wants a panic to kill one session rather than the \
         process, and `plan/06` §1.5 calls the parser never panicking \
         \"Non-negotiable\". Neither holds in a crate the lints do not reach.\n\n\
         Add `[lints]` + `workspace = true`. If a crate genuinely must \
         deviate, it becomes a second CANNOT_INHERIT with its reason written \
         out, the way `{CANNOT_INHERIT}` is.\n\n{}",
        missing.join("\n")
    );
}

/// A per-site `#[allow]` does not reopen what the workspace denied.
///
/// The root manifest says per-file `#![allow]` attributes "were rejected: a
/// forgotten attribute breaks the build rather than degrading quietly". The
/// per-SITE form is the same escape one scope down, and nothing banned it.
///
/// `clippy.toml`'s `allow-{unwrap,expect,panic}-in-tests` already covers the
/// legitimate case, which is why this can be a flat ban rather than a judgement
/// call: in test code the lints do not fire, and in production code there is no
/// good reason to silence them one line at a time.
#[test]
fn no_site_reopens_a_denied_lint_by_attribute() {
    let hits = reopened_lints(&cena_arch_tests::harness::workspace_sources());
    assert!(
        hits.is_empty(),
        "a per-site `#[allow]` or `#[expect]` reopens a lint the workspace \
         denied, one site at a time and without the manifest recording it. The \
         root Cargo.toml rejected the per-file form for the same reason.\n\n\
         `clippy.toml` already scopes these away from `#[test]` functions and \
         `#[cfg(test)]` modules, so a test HELPER that needs one belongs in a \
         `#[cfg(test)] mod` (or should return a Result) -- not behind an \
         attribute that silences the objection beside it.\n{}",
        hits.join("\n")
    );
}

/// The lints the workspace denies that a site could reopen, and the group
/// that contains the three clippy ones.
///
/// `clippy::restriction` is here because `#[allow(clippy::restriction)]`
/// silences `unwrap_used`, `expect_used` and `panic` together without naming
/// any of them.
const DENIED: &[&str] = &[
    "clippy::unwrap_used",
    "clippy::expect_used",
    "clippy::panic",
    "clippy::restriction",
    "unsafe_code",
];

/// Every attribute in `sources` that allows or expects a [`DENIED`] lint.
///
/// # Parsed, not needled (review finding 6)
///
/// The needles were `allow(clippy::unwrap_used` and three siblings, matched
/// one line at a time. Four ordinary spellings walked past them:
///
/// - `#[allow(` on one line and the lint on the next -- which is how rustfmt
///   lays out an attribute with a `reason`, and exactly how
///   `crates/cena-session/tests/player_log_writer.rs` line 34 reopens
///   `expect_used`, with this test green;
/// - `#[expect(clippy::unwrap_used)]` -- `expect` silences a lint as surely
///   as `allow` does, it only also warns when there is nothing to silence;
/// - `#[allow(dead_code, clippy::unwrap_used)]` -- the lint is not first;
/// - `#[allow(clippy::restriction)]` -- the group, which names none of them.
///
/// Attributes are now read whole from tokens, and every `allow(..)` or
/// `expect(..)` list inside one -- including inside a `cfg_attr` -- is split
/// into its lints and compared exactly.
fn reopened_lints(sources: &[(std::path::PathBuf, String)]) -> Vec<String> {
    let mut hits = Vec::new();
    for (path, text) in sources {
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        for attr in cena_arch_tests::structure::outline(text).attributes {
            let lints: Vec<String> = ["allow", "expect"]
                .iter()
                .flat_map(|head| attr.lists(head))
                .filter(|lint| DENIED.contains(&lint.as_str()))
                .collect();
            if !lints.is_empty() {
                hits.push(format!(
                    "{}:{}: {}",
                    cena_arch_tests::harness::relative(path),
                    attr.line,
                    lints.join(", ")
                ));
            }
        }
    }
    hits
}

/// Finding 6's four spellings, each of which passed the old needles.
#[test]
fn every_spelling_of_a_reopened_lint_is_found() {
    let old_needles = [
        "allow(clippy::unwrap_used",
        "allow(clippy::expect_used",
        "allow(clippy::panic",
        "allow(unsafe_code",
    ];
    let fixtures = [
        "#[allow(\n    clippy::expect_used,\n    reason = \"a helper\"\n)]\nfn f() {}\n",
        "#[expect(clippy::unwrap_used)]\nfn f() {}\n",
        "#[allow(dead_code, clippy::unwrap_used)]\nfn f() {}\n",
        "#[allow(clippy::restriction)]\nfn f() {}\n",
        "#[cfg_attr(test, allow(clippy::panic))]\nfn f() {}\n",
        "#![allow(unsafe_code)]\n",
    ];
    let path = cena_arch_tests::harness::workspace_root().join("crates/cena-fixture/src/f.rs");
    for fixture in fixtures {
        let hits = reopened_lints(&[(path.clone(), fixture.to_owned())]);
        assert_eq!(hits.len(), 1, "not found: {fixture:?}");
    }
    // The first four defeat the old needles line by line; asserted so the
    // fixtures cannot drift into a shape the old test would also have caught.
    for fixture in &fixtures[..4] {
        assert!(
            !fixture
                .lines()
                .any(|l| old_needles.iter().any(|n| l.contains(n))),
            "{fixture:?} no longer defeats the old needles"
        );
    }
    // Negative control: an unrelated allow, and a denied lint named in prose.
    let clean =
        "#[allow(clippy::too_many_arguments, reason = \"clippy::unwrap_used\")]\nfn f() {}\n";
    assert!(reopened_lints(&[(path, clean.to_owned())]).is_empty());
}

// The unpinned-TLS weakening announces itself in a release build.
///
/// `live.rs` says the `danger_accept_invalid_certs` call "is the one thing in
/// this module that should not survive to a release build". That was a note,
/// and `plan/05` Rule 0 calls an unenforced rule a wish (review PL-10).
///
/// # Why this is a needle and not a stronger mechanism
///
/// A `compile_error!` under `not(debug_assertions)` would be airtight and
/// wrong: pinning is not built -- `plan/12` §7.1 puts the credential ladder
/// Out for M1 -- so refusing to build would force the guard to be deleted
/// instead, and a deleted guard is worse than a loud one.
///
/// So the mechanism is a runtime warning on release builds, and this test is
/// what stops that warning being quietly removed. It asserts the anchor
/// comment, the `cfg!` and the eaccess host all still sit in the same file, so
/// deleting any of them fails here rather than restoring the silence.
///
/// **What it cannot check** is whether the warning's text is still accurate,
/// or whether pinning has since been built and made it unnecessary. When
/// pinning lands, this test should be deleted in the same commit -- and its
/// failure message says so.
#[test]
fn release_builds_announce_the_unpinned_tls() {
    let source = std::fs::read_to_string(workspace_root().join("crates/cena-platform/src/live.rs"))
        .unwrap_or_else(|e| panic!("live.rs must be readable: {e}"));

    for needle in [
        "ARCH-TEST ANCHOR: `release_builds_announce_the_unpinned_tls`",
        "if !cfg!(debug_assertions)",
        "UNPINNED",
    ] {
        assert!(
            source.contains(needle),
            "`{needle}` is gone from live.rs. The eaccess TLS handshake does \
             not verify certificates or hostnames and the account password \
             crosses it, so a release build must say so out loud until \
             plan/10 section 9.2's pin exists.\n\n\
             If pinning HAS been built, delete this test in the same commit \
             as the warning -- that is the outcome it is waiting for, not a \
             reason to weaken it."
        );
    }

    // The weakening itself must still be the thing being warned about. If the
    // call is gone, the warning is stale and this test is misleading.
    assert!(
        source.contains("danger_accept_invalid_certs"),
        "live.rs no longer accepts invalid certificates, so the release \
         warning it carries is stale. Delete both, together."
    );
}
