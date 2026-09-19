//! `plan/05` §-2: **a citation that resolves to nothing manufactures a false
//! negative.**
//!
//! `CLAUDE.md` records the incident this exists to prevent. A doc cited
//! `research/Wrayth protocol.txt`, which does not exist -- the file is under
//! `reference/wiki_clean/`. An agent searched the dead path, got zero hits,
//! and concluded the wiki does not document `styleIfClosed`. It documents it
//! five times.
//!
//! > **A citation that resolves to nothing does not fail loudly; it
//! > manufactures a false negative.**
//!
//! Two adversarial reviews then found citation rot in every crate -- roughly
//! sixty stale `path:line` references, several pointing at real files whose
//! content had moved. The review's own summary called a mechanical check
//! "the cheapest single improvement this review suggests", because fixing
//! sixty citations by hand fixes them once, and this fixes them forever.
//!
//! # What it checks, and what it deliberately cannot
//!
//! **Checks:** every path-rooted `path:line` citation in a comment names a
//! file that exists and has at least that many lines.
//!
//! **Does not check** that the cited line still says what the citing comment
//! claims. That needs a human. What it catches is the cheaper and more common
//! rot: a file renamed, deleted, or shortened past the line being cited.
//!
//! **Does not check bare filenames** like `wire.rs:160`. Those resolve only
//! against the reader's context, and guessing which `wire.rs` was meant would
//! produce false failures -- the direction `plan/05` warns gets tests deleted
//! as noise. Path-rooted citations are the ones a machine can settle.
//!
//! **Skips `reference/`**, which is gitignored: those clones are not present
//! on every machine, so asserting on them would fail CI for the wrong reason.

use cena_arch_tests::harness::{workspace_root, workspace_sources};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A citation found in a source file.
#[derive(Debug)]
struct Citation {
    /// Where the citation was written.
    in_file: String,
    /// The path it names, workspace-relative.
    target: String,
    /// The first line number it names.
    line: usize,
    /// The citation as written, for the failure message.
    raw: String,
}

/// Directory prefixes whose citations are checked.
///
/// `reference/` is excluded because it is gitignored -- see the module doc.
const CHECKED_ROOTS: &[&str] = &["crates/", "plan/", "tools/"];

/// Is `c` a character that can appear in a cited path?
fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/')
}

/// Every `path:line` citation in `text`, however it is delimited.
///
/// Scans raw text rather than `code_lines`, because citations live in comments
/// and `code_lines` strips exactly those. Backticks are optional: the citation
/// is recognised by shape, so `` `plan/05:352` ``, `(plan/05:352)` and a bare
/// `plan/05:352` are all found.
fn citations_in(in_file: &str, text: &str) -> Vec<Citation> {
    let mut found = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        // Anchor on a root prefix, so only path-rooted citations are taken.
        let rest: String = bytes[i..].iter().take(16).collect();
        let Some(root) = CHECKED_ROOTS.iter().find(|r| rest.starts_with(**r)) else {
            i += 1;
            continue;
        };
        // A prefix must start a word, or `crates/` matches inside a longer
        // token and the path is truncated.
        if i > 0 && is_path_char(bytes[i - 1]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && is_path_char(bytes[i]) {
            i += 1;
        }
        let path: String = bytes[start..i].iter().collect();
        // A citation is a path followed by `:` and a digit. Anything else is
        // an ordinary path mention, which this test says nothing about.
        if bytes.get(i) != Some(&':') || !bytes.get(i + 1).is_some_and(char::is_ascii_digit) {
            continue;
        }
        i += 1;
        let num_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let digits: String = bytes[num_start..i].iter().collect();
        let Ok(line) = digits.parse::<usize>() else {
            continue;
        };
        let _ = root;
        found.push(Citation {
            in_file: in_file.to_owned(),
            target: path.clone(),
            line,
            raw: format!("{path}:{digits}"),
        });
    }
    found
}

/// Resolve a cited path to a file on disk.
///
/// `plan/05` is shorthand: the plan files carry descriptive names
/// (`05-engineering-rules.md`) and every citation in the tree uses the number
/// alone. Resolving by numeric prefix is what makes the shorthand checkable
/// instead of exempt, and it is unambiguous because the numbers are unique.
fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let direct = root.join(target);
    if direct.is_file() {
        return Some(direct);
    }
    let rest = target.strip_prefix("plan/")?;
    // `plan/05` or `plan/12` -- a bare number naming one plan document.
    if !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    std::fs::read_dir(root.join("plan"))
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&format!("{rest}-")))
        })
}

/// Check every citation in `documents`, returning the broken ones and a count.
fn broken_citations(root: &Path, documents: &[(String, String)]) -> (Vec<String>, usize) {
    let mut broken = Vec::new();
    let mut checked = 0usize;
    // Line counts are read once per target: the same file is cited many times.
    let mut lines_of: BTreeMap<String, Option<usize>> = BTreeMap::new();

    for (in_file, text) in documents {
        for c in citations_in(in_file, text) {
            checked += 1;
            let count = lines_of.entry(c.target.clone()).or_insert_with(|| {
                resolve(root, &c.target)
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .map(|t| t.lines().count())
            });
            match count {
                None => broken.push(format!("  {}: `{}` -- no such file", c.in_file, c.raw)),
                Some(n) if c.line > *n => broken.push(format!(
                    "  {}: `{}` -- {} has only {} lines",
                    c.in_file, c.raw, c.target, n
                )),
                Some(_) => {}
            }
        }
    }
    (broken, checked)
}

/// The shared failure message: why this matters, not just what failed.
fn report(kind: &str, broken: &[String], checked: usize) -> String {
    format!(
        "{} of {checked} path-rooted citations in {kind} do not resolve.\n\n\
         `plan/05` section -2: a citation that resolves to nothing does not \
         fail loudly, it manufactures a false negative -- a reader searches \
         the dead path, finds nothing, and concludes the thing is not \
         documented. That happened here once already (`CLAUDE.md`, the \
         `styleIfClosed` incident).\n\n\
         Fix the citation, or delete it if what it pointed at is gone.\n\n{}",
        broken.len(),
        broken.join("\n")
    )
}

#[test]
fn every_path_rooted_citation_resolves() {
    let root = workspace_root();
    let documents: Vec<(String, String)> = workspace_sources()
        .into_iter()
        .map(|(path, text)| {
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            (rel, text)
        })
        // This file names citation shapes as examples; scanning itself would
        // report them as broken. The same self-reference `scannable_sources`
        // exists for.
        .filter(|(rel, _)| !rel.ends_with("tests/citations.rs"))
        .collect();

    let (broken, checked) = broken_citations(&root, &documents);

    assert!(
        checked > 50,
        "only {checked} citations were found, which means the scanner stopped \
         matching rather than that the tree stopped citing. A citation test \
         that finds nothing passes by seeing nothing."
    );
    assert!(
        broken.is_empty(),
        "{}",
        report("source comments", &broken, checked)
    );
}

/// The same check over `plan/`, `research/`, `inventory/` and `CLAUDE.md`.
///
/// **This is where the incident actually happened.** `CLAUDE.md` cited
/// `research/Wrayth protocol.txt`, a path that does not exist, and an agent
/// reasoned from its silence. The prose documents cite each other and the
/// source tree constantly, and nothing checked any of it.
///
/// `plan/12` is authoritative, so a dead citation there is the most expensive
/// kind: the review found `plan/12` section 6.4 cited from two files and
/// absent from the document.
#[test]
fn every_citation_in_the_plan_documents_resolves() {
    let root = workspace_root();
    let mut documents = Vec::new();
    for dir in ["plan", "research", "inventory"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                documents.push((
                    format!("{dir}/{}", entry.file_name().to_string_lossy()),
                    text,
                ));
            }
        }
    }
    if let Ok(text) = std::fs::read_to_string(root.join("CLAUDE.md")) {
        documents.push(("CLAUDE.md".to_owned(), text));
    }

    assert!(
        documents.len() > 5,
        "only {} prose documents were read; the walk is not finding them, so \
         this test would pass by seeing nothing",
        documents.len()
    );

    let (broken, checked) = broken_citations(&root, &documents);
    assert!(
        broken.is_empty(),
        "{}",
        report("the plan documents", &broken, checked)
    );
}
