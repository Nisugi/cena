//! Every committed fixture is scrubbed. Asserted, not assumed.
//!
//! The corpus findings recommend making the scrubber "a committed, tested tool
//! rather than a manual step -- fixtures get re-cut, and a manual redaction
//! pass will eventually miss a `druby://`. Given 20.5% of the corpus carries
//! host addresses, hand-redaction is not a safe default."
//!
//! This is that test. It reads every file under `tests/fixtures/` and asserts
//! the four hazards named in the findings §4 are absent:
//!
//! 1. `druby://` host URIs -- the author's real link-local IPv6 addresses.
//! 2. The player names that were pseudonymised out.
//! 3. Third-party speech -- whispers, group OOC, private channels.
//! 4. `<vellumImg>`, which is not a privacy hazard but a **correctness** one:
//!    a tag the game never sends (`reference/VellumFE/src/core/inline_image.rs:5`).
//!
//! # This test can go RED (plan/05 §0)
//!
//! VERIFIED by writing each hazard in turn into a scratch file under
//! `tests/fixtures/` and running `cargo test -p cena-protocol`. Each failure
//! named the file and the pattern. The `no_fixture_is_empty` and
//! `the_scan_actually_found_files` tests below close the way this test would
//! otherwise pass vacuously -- a renamed directory makes a scanner that
//! iterates nothing report success.

use cena_protocol::scrub::Scrubber;
use std::fs;
use std::path::{Path, PathBuf};

/// Names pseudonymised when these fixtures were cut, in `real -> pseudonym`
/// order.
///
/// The real name is here because the test's job is to prove it is **absent**
/// from the fixtures, which cannot be done without knowing it. `Inochi`
/// appeared in `<component id='room players'>` in
/// `GSIV-Nisugi/2025/11/xml/2025-11-29_22-03-30.xml:271`.
///
/// This file is source, not a fixture, so it is exempt from its own scan --
/// see `fixture_files`, which reads only `tests/fixtures/`.
const PSEUDONYMS: &[(&str, &str)] = &[
    ("Inochi", "Alderin"),
    // The eleven players in `room_populated.xml`'s roster, from
    // `GSIV-Nisugi/2026/09/xml/2026-09-01_15-13-56.xml:240`. Kept in step with
    // `examples/cut_fixtures.rs`; this list is what proves they are absent.
    ("Fulmen", "Aldric"),
    ("Khadzim", "Bresnik"),
    ("Vortalis", "Cerdwyn"),
    ("Gidion", "Dorlan"),
    ("Komoki", "Eshvarr"),
    ("Eliaku", "Faldrin"),
    ("Jabalia", "Gwenlyn"),
    ("Attalynx", "Halvorn"),
    ("Jinxt", "Ithriel"),
    ("Richland", "Jorvath"),
    ("Berean", "Kelmond"),
];

/// Substrings that must not appear in any committed fixture.
fn forbidden() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = vec![
        ("host URI (corpus findings §4)", "druby://".to_owned()),
        (
            "client-injected tag, never on the wire",
            "<vellumImg".to_owned(),
        ),
        ("third-party speech", "whispers to the group".to_owned()),
        ("third-party speech", "whispers,".to_owned()),
        ("private channel", "pushStream id='society'".to_owned()),
        ("private channel", "pushStream id='bounty'".to_owned()),
    ];
    for (real, _) in PSEUDONYMS {
        out.push(("un-pseudonymised player name", (*real).to_owned()));
    }
    out
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn fixture_files() -> Vec<PathBuf> {
    let dir = fixture_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    out.sort();
    out
}

#[test]
fn the_scan_actually_found_files() {
    // Without this, renaming the directory turns every assertion below into a
    // loop over nothing and the suite stays green while the fixtures are
    // unchecked.
    let files = fixture_files();
    assert!(
        files.len() >= 4,
        "expected at least the four M1 fixtures under {}, found {}. An empty \
         scan makes every other test in this file vacuous.",
        fixture_dir().display(),
        files.len()
    );
}

#[test]
fn no_fixture_contains_a_redaction_hazard() {
    let mut violations = Vec::new();
    for path in fixture_files() {
        let Ok(text) = fs::read_to_string(&path) else {
            violations.push(format!("{}: not valid UTF-8", path.display()));
            continue;
        };
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        for (why, needle) in forbidden() {
            for (i, line) in text.lines().enumerate() {
                if line.contains(&needle) {
                    violations.push(format!("{name}:{}: {needle:?} -- {why}", i + 1));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "a committed fixture carries data that must not be committed \
         (corpus findings §4). Re-cut it through \
         `cena_protocol::scrub::Scrubber`, do not edit it by hand.\n{}",
        violations.join("\n")
    );
}

#[test]
fn every_fixture_is_already_a_scrub_fixed_point() {
    // Stronger than the needle scan: running the scrubber again must change
    // nothing. This catches a fixture that was cut before a scrubber rule
    // existed and so predates its own redaction.
    let mut scrubber = Scrubber::new();
    for (real, pseudonym) in PSEUDONYMS {
        scrubber.pseudonymise(real, pseudonym);
    }
    let mut violations = Vec::new();
    for path in fixture_files() {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if scrubber.scrub(&text) != text {
            violations.push(path.display().to_string());
        }
    }
    assert!(
        violations.is_empty(),
        "re-scrubbing these fixtures changed them, so they were not produced \
         by the current rules:\n{}",
        violations.join("\n")
    );
}

#[test]
fn no_fixture_has_carriage_returns_or_is_empty() {
    // Corpus findings, determinism item 4: a golden carrying `\r` makes every
    // diff unreadable. An empty fixture passes every other assertion here.
    let mut violations = Vec::new();
    for path in fixture_files() {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if text.contains('\r') {
            violations.push(format!("{name}: contains a carriage return"));
        }
        if text.trim().is_empty() {
            violations.push(format!("{name}: is empty"));
        }
        if !text.ends_with('\n') {
            violations.push(format!("{name}: does not end in a newline"));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn every_fixture_stays_under_the_size_budget() {
    // Corpus findings: "keep each committed fixture under ~10 KB". A golden
    // that grows into a log is no longer a golden anybody reads.
    const BUDGET: usize = 10 * 1024;
    let mut violations = Vec::new();
    for path in fixture_files() {
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        let size = usize::try_from(meta.len()).unwrap_or(usize::MAX);
        if size > BUDGET {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            violations.push(format!("{name}: {size} bytes, budget {BUDGET}"));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}
