//! No string literal in this crate carries a collapsed line continuation.
//!
//! # The defect this guards (review finding 14)
//!
//! A `\` continuation in a Rust string joins two source lines and eats the
//! second line's indent. When the backslash and newline are lost -- an editing
//! tool that rewrote the file, a paste -- the indent is NOT lost: it lands in
//! the string. `live.rs`'s keepalive warning printed "The              session
//! continues", and its release-build TLS warning, the one `cena-arch-tests`
//! insists exists, printed four such gaps. The code compiled, every test
//! passed, and the operator read the damage.
//!
//! It happened more than once in one session, so it is checked rather than
//! remembered: a run of NINE or more spaces between two non-space characters
//! on a line that is inside a string literal. Nine because a continuation's
//! indent is at least that deep in this codebase's style (`eprintln!(` inside
//! a function body), and because no string here aligns text with that much
//! space on purpose -- if one ever must, it can say so here.
//!
//! Comments are skipped: tables in comments align columns deliberately.
//!
//! **Scope is this crate.** The same scan belongs in `cena-arch-tests` for the
//! workspace; that crate was out of scope for the fix that added this.

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// A line inside a string literal carrying a gap that only a lost `\` makes.
fn collapsed(line: &str) -> bool {
    let code = line.trim_start();
    if code.starts_with("//") || !line.contains('"') {
        return false;
    }
    let bytes = line.as_bytes();
    let mut run = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b' ' {
            run += 1;
            continue;
        }
        if run >= 9 && i > run && bytes[i - run - 1] != b' ' {
            // Inside a string: an odd number of quotes before the gap.
            let quotes = line[..i - run].matches('"').count();
            if quotes % 2 == 1 {
                return true;
            }
        }
        run = 0;
    }
    false
}

#[test]
fn the_detector_recognises_the_real_damage() {
    // Verbatim shape of the keepalive warning as it was found.
    assert!(collapsed(
        r#"            "[socket] WARNING: TCP keepalive could not be set ({e}). The              session continues""#
    ));
    // And not a continuation done properly, nor an aligned comment.
    assert!(!collapsed(r#"            "[socket] WARNING: the \"#));
    assert!(!collapsed("// | a |          | b |"));
    assert!(!collapsed(r#"    let x = f("a");          // aligned"#));
}

#[test]
fn no_string_in_this_crate_has_a_collapsed_continuation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    rust_files(&root.join("tests"), &mut files);
    assert!(files.len() > 20, "the scan found {} files", files.len());

    let mut found = Vec::new();
    for file in &files {
        // This file quotes the damage on purpose, as its detector's fixture.
        if file.ends_with("no_collapsed_continuations.rs") {
            continue;
        }
        let text = std::fs::read_to_string(file).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            if collapsed(line) {
                found.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "a `\\` continuation lost its backslash and newline, and its indent \
         is now inside the string:\n{}",
        found.join("\n")
    );
}
