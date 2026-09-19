//! Shared harness for the architecture tests.
//!
//! Moved out of `tests/architecture.rs` because that file exceeded its own
//! Rule 4.1 line cap. `plan/05:352-353`: move code down, do not raise the cap.
//! The split is by role -- harness here, one test per rule there -- so a test
//! and the rule it cites stay adjacent.
//!
//! Everything here is mechanism: filesystem walking, comment stripping,
//! dependency resolution, path normalization. It states no rule. The rules and
//! their citations live beside their tests.
//!
//! # Two lessons are load-bearing here
//!
//! **Ask Cargo, do not re-implement Cargo.** `crate_dependency_names` shells
//! out to `cargo tree` rather than parsing manifests. The hand parser it
//! replaced matched three literal section headers and was therefore blind to
//! `[target.'cfg(windows)'.dependencies]`, to `[dependencies.sess]` sub-table
//! form, and to renames (`sess = { package = "cena-session" }`, whose key is
//! `sess`). All three were VERIFIED to hide a live `cena-ui -> cena-session`
//! edge with the whole suite green.
//!
//! **Discover files by walking, not by an opt-in list.** `crate_sources`
//! walks the whole crate directory. The three blessed subdirectories it
//! replaced (`src`, `tests`, `benches`) were escaped by a `#[path]` module and
//! by `include!` of a non-`.rs` file, each of which compiled a 900-line
//! `pub static mut` into the crate with every ban green.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Workspace geometry
// ---------------------------------------------------------------------------

/// The workspace root. `CARGO_MANIFEST_DIR` is this crate's directory, which
/// in a workspace is NOT the root — the trap Vellum never hit, being one
/// crate (`plan/05:200-203`).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(Path::parent) // workspace root
        .expect("cena-arch-tests must live at <root>/crates/cena-arch-tests")
        .to_path_buf()
}

/// Every double-quoted string inside `text`, in order.
///
/// Used for the `members` array. A hand parser rather than a `toml`
/// dev-dependency (rule of three, `plan/05` §-1) — but it must be robust to
/// legal TOML the naive version breaks on: a single-line array, and a trailing
/// `#` comment on a member line. Both were demonstrated to produce four
/// misdiagnosed failures pointing at a missing `src/` directory.
fn quoted_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let mut rest = line;
        while let Some(open) = rest.find('"') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('"') else { break };
            out.push(after[..close].to_owned());
            rest = &after[close + 1..];
        }
    }
    out
}

/// Member crate names, parsed from the root manifest's `members` array.
///
/// Read from the manifest rather than hardcoded so a new crate is scanned the
/// day it is added. Vellum's `core_is_android_safe` shows the failure mode of
/// hardcoding: when `parser.rs` became `parser/`, the scan silently stopped
/// covering it (`reference/VellumFE/tests/architecture.rs:208-219`).
pub fn member_crates() -> Vec<String> {
    let manifest = fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("root Cargo.toml must be readable");
    let members = manifest
        .split_once("members = [")
        .expect("root Cargo.toml must declare workspace members")
        .1
        .split_once(']')
        .expect("members array must be closed")
        .0;
    let crates: Vec<String> = quoted_strings(members)
        .into_iter()
        .filter_map(|m| m.strip_prefix("crates/").map(str::to_owned))
        .collect();
    assert!(
        !crates.is_empty(),
        "parsed zero members from the root manifest; the parser has drifted \
         from the manifest format and every scan below is silently vacuous"
    );
    crates
}

// ---------------------------------------------------------------------------
// Dependency edges — resolved by Cargo, not re-parsed
// ---------------------------------------------------------------------------

/// The direct dependency package names of one crate, as Cargo resolves them,
/// across **every** target platform and including dev- and build-dependencies.
///
/// Shells out to `cargo tree` rather than reading the manifest. That is not
/// laziness; three separate manifest shapes were VERIFIED to hide a forbidden
/// `cena-ui -> cena-session` edge from the hand parser while the suite stayed
/// green and `cargo tree` reported the edge:
///
/// 1. `[target.'cfg(windows)'.dependencies]` — not one of the three literal
///    section names the parser matched.
/// 2. `[dependencies.sess]` with `package = "cena-session"` — the parser took
///    the key before `=`, recording `sess`, which starts with no `cena`.
/// 3. `[target.'cfg(unix)'.dependencies]` — invisible even to a fixed parser
///    running on Windows unless the *resolver* is asked about other targets.
///
/// `--target all` is what closes (3), and it matters for this project
/// specifically: development is on Windows (`CLAUDE.md`) and CI runs Linux, so
/// without it each platform is blind to the other's forbidden edges — and
/// `plan/12:63` binds Android and iOS builds, which is exactly where a
/// `cfg`-gated edge would land without anyone intending to evade anything.
///
/// `--depth 1` keeps this to *direct* edges. The transitive closure is a
/// different rule; `plan/12:78-86` is a statement about who may name whom.
pub fn crate_dependency_names(krate: &str) -> BTreeSet<String> {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace_root())
        .args([
            "tree",
            "-p",
            krate,
            "--depth",
            "1",
            "--prefix",
            "depth",
            "--edges",
            "normal,build,dev",
            "--target",
            "all",
        ])
        .output()
        .unwrap_or_else(|e| panic!("`cargo tree -p {krate}` must run: {e}"));
    assert!(
        output.status.success(),
        "`cargo tree -p {krate}` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);

    // `--prefix depth` prints the depth as a leading integer with no
    // separator: `0cena-ui v0.1.0 (path)`, `1cena-model v0.1.0 (path)`.
    // Depth 0 is the crate itself; depth 1 are its direct dependencies.
    let mut names = BTreeSet::new();
    let mut saw_root = false;
    for line in text.lines() {
        let rest = line.trim_end();
        let Some(digits_end) = rest.find(|c: char| !c.is_ascii_digit()) else {
            continue;
        };
        if digits_end == 0 {
            continue;
        }
        let depth: usize = rest[..digits_end]
            .parse()
            .unwrap_or_else(|e| panic!("cargo tree depth prefix in {rest:?}: {e}"));
        // The name runs to the ` v<version>` that cargo always prints.
        let name = rest[digits_end..]
            .split(' ')
            .next()
            .unwrap_or_default()
            .to_owned();
        if name.is_empty() {
            continue;
        }
        if depth == 0 {
            saw_root = true;
        } else if depth == 1 {
            names.insert(name);
        }
    }
    assert!(
        saw_root,
        "`cargo tree -p {krate}` printed no depth-0 root; the output format \
         has changed and every edge assertion below is silently vacuous. \
         Output was:\n{text}"
    );
    names
}

// ---------------------------------------------------------------------------
// Source discovery
// ---------------------------------------------------------------------------

/// Directory names never scanned, because nothing in them is crate source.
///
/// **`target` is skipped only at a crate root**, not at any depth. Skipping it
/// anywhere meant `crates/<crate>/src/target/` -- a perfectly ordinary module
/// name for a combat client -- would be invisible to every architecture test in
/// this workspace: no line cap, no facade rule, no layering check. A rule that
/// is not enforced is a wish (`plan/05` §0), and a rule that silently stops
/// applying to a directory because of its NAME is worse than one that was never
/// written.
///
/// MEASURED 2026-09-19: no such directory exists today, so this closes a latent
/// hole rather than a live one. It is closed now because the name is a likely
/// one here and the failure would be silent.
const SKIPPED_DIRS: &[&str] = &[".git"];

/// Skipped at a crate root only: Cargo's build directory.
const SKIPPED_AT_ROOT: &[&str] = &["target"];

/// Every source file a crate owns, anywhere under the crate directory, as
/// (path, contents).
///
/// **Walks the whole crate directory**, not an opt-in list of subdirectories.
/// The previous version scanned `src/`, `tests/` and `benches/`, and two
/// ordinary pieces of Rust escaped all three while compiling into the crate:
///
/// ```text
/// #[path = "../gen/globals.rs"] pub mod globals;   // outside src/
/// include!("parser_body.in");                      // not a .rs file
/// ```
///
/// Each was VERIFIED to carry a 900-line body with `pub static mut` and an
/// `if game == "GemStone"` branch past the cap, the `static mut` ban and the
/// game-name ban simultaneously, with all 11 tests green.
///
/// A file under `crates/<c>/` that is not scanned is the hole, so there is no
/// opt-in: directories are opted *out*, and only `target/` and `.git/` are.
/// Extensions are opted out the same way — `.rs` plus any extension a scanned
/// file names in an `include!`, which is why `.in` and `.toml`-adjacent
/// generated files cannot hide.
///
/// `src/` must still exist: a scan over a nonexistent path finds nothing and
/// passes green, which is how a renamed crate silently drops out of
/// enforcement.
fn crate_sources(krate: &str) -> Vec<(PathBuf, String)> {
    let root = workspace_root().join("crates").join(krate);
    let src = root.join("src");
    assert!(
        src.is_dir(),
        "{} has no src/ directory; a scan over a missing path passes vacuously",
        src.display()
    );
    let mut out = Vec::new();
    collect_sources(&root, &mut out);
    assert!(
        out.iter().any(|(p, _)| p.starts_with(&src)),
        "{} contains no source files; scan would be vacuous",
        src.display()
    );
    out
}

/// Extensions collected by `collect_sources`.
///
/// `.rs` is the obvious one. The rest are the extensions an `include!` target
/// plausibly carries — `include!` accepts any path, and the compiled result is
/// crate source whatever it is called.
///
/// # What makes this list sufficient, stated accurately
///
/// This used to say `no_included_files_escape_the_scan` "asserts that every
/// path named by an `include!` is one of the files this walk actually
/// collected". **No test of that name exists**, and the real one
/// (`no_source_file_is_included_from_outside_the_scan`) does something
/// different and stronger: it bans `include!` and `include_bytes!` outright,
/// so there is no included path to check (review AR-5).
///
/// That inversion is why this list does not have to be exhaustive. A list of
/// extensions is a guess about what someone might include; a ban is not a
/// guess. The list matters only for `include_str!` of a file the walk already
/// collects — the crit tables — which is why `.tsv` is here.
///
/// A doc describing a test that does not exist is the citation rot `plan/05`
/// §−2 warns about, in its most expensive form: it describes a guarantee the
/// suite does not make, and a reader trusts it.
///
/// `.tsv` was added when the crit tables landed (`plan/13` §4a, whose port
/// table says at `plan/13:125` that they "ship as data files"). That is the
/// amendment `no_source_file_is_included_from_outside_the_scan` asks for in
/// its own failure message: extend this list so the data file is scanned,
/// rather than routing around the scan. `crates/cena-model/data/crit_tables.tsv`
/// is therefore covered by every rule in this suite -- the `static mut` ban,
/// the game-name ban and the line cap all read it -- which is what makes
/// `include_str!` of it unable to smuggle anything past them.
const SOURCE_EXTENSIONS: &[&str] = &["rs", "in", "inc", "tsv"];

fn collect_sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    collect_sources_inner(dir, out, true);
}

fn collect_sources_inner(dir: &Path, out: &mut Vec<(PathBuf, String)>, at_root: bool) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry must be readable").path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if SKIPPED_DIRS.contains(&name.as_ref())
                || (at_root && SKIPPED_AT_ROOT.contains(&name.as_ref()))
            {
                continue;
            }
            collect_sources_inner(&path, out, false);
        } else if path
            .extension()
            .is_some_and(|ext| SOURCE_EXTENSIONS.iter().any(|e| ext == *e))
        {
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            out.push((path, text));
        }
    }
}

/// All workspace source, every member crate.
///
/// Includes this file. That is deliberate: the enforcer is not exempt from the
/// caps or the facade rule, and `CAP_EXCEPTIONS` carries this file's entry with
/// a written justification rather than a silent skip.
pub fn workspace_sources() -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = member_crates()
        .iter()
        .flat_map(|c| crate_sources(c))
        .collect();
    // A crate directory nested inside another would otherwise be collected
    // twice and report every violation twice.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// The architecture test files' own paths, relative to the workspace root.
const RULE_FILES: &[&str] = &[
    "crates/cena-arch-tests/tests/architecture.rs",
    "crates/cena-arch-tests/tests/file_rules.rs",
];

/// Workspace source minus the two files that state the rules.
///
/// Used only by the tests whose mechanism is a *needle*, because those files
/// necessarily contain every needle they ban: the literals `"static mut "`,
/// `"GemStone"` and `panic="abort"` all appear there as search terms. The
/// comment-stripping in `code_lines` cannot help — they are string literals,
/// not comments.
///
/// This exemption is deliberately as narrow as it can be. It is two files,
/// named by a constant, and it does NOT apply to
/// `no_source_file_exceeds_its_line_cap` or `facade_files_stay_facades`, which
/// continue to cover them. The alternative -- teaching `code_lines` to skip
/// string literals -- would mean a real `static mut FOO` inside an
/// `include_str!`-style literal elsewhere stops being found, which trades a
/// self-reference nuisance for a hole in the ban.
///
/// The cost is stated plainly: a genuine `static mut` written in one of those
/// two files would not be caught by them. That is acceptable because they are
/// the one place in the workspace whose entire contents are a review artifact.
pub fn scannable_sources() -> Vec<(PathBuf, String)> {
    workspace_sources()
        .into_iter()
        .filter(|(path, _)| !RULE_FILES.contains(&relative(path).as_str()))
        .collect()
}

/// Path relative to the workspace root, with forward slashes.
///
/// Windows backslashes would make every path assertion below
/// platform-specific.
pub fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---------------------------------------------------------------------------
// Manifest lint tables
// ---------------------------------------------------------------------------

/// Lint names declared in any `[<prefix>rust]` / `[<prefix>clippy]` table.
pub fn lint_keys(manifest: &str, prefix: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut in_lints = false;
    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_lints = line.starts_with(prefix);
            continue;
        }
        if !in_lints || line.is_empty() {
            continue;
        }
        if let Some(key) = line.split('=').next() {
            let key = key.trim();
            // `workspace = true` is the inheritance marker, not a lint.
            if !key.is_empty() && key != "workspace" {
                keys.insert(key.to_owned());
            }
        }
    }
    keys
}
