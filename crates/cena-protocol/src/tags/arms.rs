//! Reading the dispatch match arms out of the parser source, for the test
//! that keeps `KNOWN_WIRE_TAGS` and the dispatcher in step.
//!
//! Split out of `tags.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap.
//!
//! **This is a lexical scan, and that is the whole design.** The failure it
//! guards is someone editing the table and not the dispatcher, or the
//! reverse: a hand edit to a string literal, which is exactly what a literal
//! scan sees. A name built by a macro or assembled from pieces is invisible to
//! it -- the same known limit the architecture tests record about themselves.
//! `syn` is the documented upgrade if a macro ever generates these arms.

/// Read a file next to this one at test time.
///
/// `include_str!` is banned workspace-wide (`cena-arch-tests`: it splices
/// a file into a crate without that file being a module, which puts it
/// outside every scan in that suite). Reading at runtime keeps both files
/// ordinary modules that the scans still see.
pub(super) fn include_str_of_sibling(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(relative);
    // **NOT `unwrap_or_default()`.** A missing or renamed file returned `""`,
    // from which `match_arm_names` finds zero names, and a cross-check over
    // zero names passes. The test whose whole purpose is to notice that the
    // table and the dispatcher have drifted apart would then be reporting
    // "they agree" about a file it never read. That is the dead-ratchet shape
    // `table_tests.rs` warns about, in the ratchet itself (review PR-6).
    //
    // A test helper may panic (`clippy.toml` scopes the lints away from
    // tests), and a panic naming the path is the only answer that cannot be
    // mistaken for a pass.
    match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => panic!(
            "the tag/arm cross-check could not read {}: {e}. A source file was              renamed or moved without updating `sources` in `table_tests.rs`;              reading it as empty would make this test pass by seeing nothing.",
            path.display()
        ),
    }
}

/// Every `"name"` appearing in a match-arm position in `source`.
///
/// A match arm here is a line whose content is a run of `"name"` literals
/// separated by `|`, ending in `=>` or `|`. That shape is what
/// `dispatch.rs` and `thin.rs` use throughout and it does not match a
/// string literal inside an expression, which would produce false
/// positives such as `attribute(tag, "id")`.
pub(super) fn match_arm_names(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with("//") || line.starts_with("///") {
            continue;
        }
        let Some(body) = line.strip_suffix("=> {").or_else(|| {
            line.strip_suffix("=>")
                .or_else(|| line.strip_suffix('|'))
                .or_else(|| line.split(" => ").next().filter(|_| line.contains(" => ")))
        }) else {
            continue;
        };
        // Every token must be a quoted literal or a `|`, or this is not a
        // match arm made of names.
        let tokens: Vec<&str> = body.split('|').map(str::trim).collect();
        if tokens.is_empty() || !tokens.iter().all(|t| is_quoted_name(t)) {
            continue;
        }
        for token in tokens {
            out.push(token.trim_matches('"').to_owned());
        }
    }
    out
}

/// A `"bareName"` literal: quoted, non-empty, alphanumeric inside.
fn is_quoted_name(token: &str) -> bool {
    let Some(inner) = token
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return false;
    };
    !inner.is_empty() && inner.chars().all(|c| c.is_ascii_alphanumeric())
}
