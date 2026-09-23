//! `include!` and escaping `#[path]` -- the ways crate source can reach the
//! compiler without reaching the scans -- and what `include_str!` may embed.
//!
//! Split out of `tests/file_rules.rs` under the rule that file enforces
//! (`plan/05:352-353`): hardening this ban (review finding 10) would have put
//! that file past its own cap, and this is a whole rule with its own history,
//! which makes it the seam.
//!
//! Read `tests/architecture.rs`'s module header first: its "what these tests
//! do NOT claim" paragraph governs this file too.
//!
//! # The ban, and why it exists
//!
//! `harness::crate_sources` walks the whole crate directory, which closes the
//! `#[path = "../gen/globals.rs"]` evasion. `include!` is the other half: it
//! accepts any path, including one outside the crate and one with an
//! extension the walk does not collect.
//!
//! VERIFIED: `include!("parser_body.in")` in `cena-protocol/src/lib.rs`, with a
//! 906-line `parser_body.in` holding a `pub static mut` and a game-name
//! branch, compiled into the crate with the line cap, the `static mut` ban and
//! the game-name ban all green simultaneously. One line defeated four rules.
//!
//! Banning it is cheaper than chasing it and loses nothing: `include!` has no
//! legitimate use yet. When generated code arrives (`plan/13` §4a names
//! `KNOWN_WIRE_TAGS` and the 63-variant `ParsedElement` as ports, which is
//! exactly where a build script would generate a table), the honest move is to
//! lift this ban with a written reason and extend `SOURCE_EXTENSIONS`, not to
//! route around it.
//!
//! # AMENDED when the crit tables landed: `include_str!` of a SCANNED file
//!
//! `plan/13` section 4a names the crit tables as a port target and
//! `plan/13:125` says they "ship as data files". A const array is not an
//! option: `cargo fmt` was VERIFIED to expand 200 entries written one per line
//! from 203 lines to 4,223 (21x), which extrapolates to ~50,500 lines. So
//! `harness::SOURCE_EXTENSIONS` gained `tsv`, and the ban narrowed from three
//! macros to two.
//!
//! **The narrowing is sound because the ban was never about `include_str!`.**
//! The stated harm is a file spliced into a crate from OUTSIDE EVERY SCAN.
//! `include_str!` yields a `&str`, never items, so the 906-line `pub static
//! mut` that defeated four rules is not expressible through it.
//!
//! `include!` and `include_bytes!` stay banned. `include!` splices code, which
//! is the original harm. `include_bytes!` is banned because bytes are NOT
//! scanned as text: `collect_sources` reads with `read_to_string`, so a
//! non-UTF-8 payload is a file the walk cannot read.

use cena_arch_tests::harness::{SOURCE_EXTENSIONS, relative, scannable_sources, workspace_root};
use cena_arch_tests::lexical::{Token, tokens};
use cena_arch_tests::structure::outline;
use std::path::{Path, PathBuf};

/// Every `include!` or `include_bytes!` in `sources`, however it is spelled.
///
/// # Tokens, not needles (review finding 10)
///
/// The needles were `include!` and `include_bytes!` against the line text,
/// and three spellings of the same macro walked past them:
///
/// - `include ! ("body.in")` -- whitespace between the name and the `!`,
///   which rustfmt removes and `#[rustfmt::skip]` keeps;
/// - `use std::include as splice;` then `splice!("body.in")` -- a renamed
///   import, after which the banned name never appears at the call;
/// - the same through `pub use`, or a grouped `use std::{include, ..}`.
///
/// So the macro is found as the TOKEN `include` followed by the token `!`,
/// and any `use` statement naming either macro is itself a hit: there is no
/// reason to import one except to call it under another name.
fn included_code(sources: &[(PathBuf, String)]) -> Vec<String> {
    const BANNED: &[&str] = &["include", "include_bytes"];
    let mut hits = Vec::new();
    for (path, text) in rust_only(sources) {
        let toks = tokens(text);
        let mut in_use = false;
        for (k, t) in toks.iter().enumerate() {
            if t.is("use") {
                in_use = true;
            } else if t.is(";") {
                in_use = false;
            }
            let banned = BANNED.iter().any(|b| t.is(b));
            let called = toks.get(k + 1).is_some_and(|n| n.is("!"));
            if banned && (called || in_use) {
                let how = if called { "invoked" } else { "imported" };
                hits.push(format!("{}:{}: `{}` {how}", relative(path), t.line, t.text));
            }
        }
    }
    hits
}

/// Every `path = "..."` attribute in `sources` whose target escapes the
/// directory the walk follows.
///
/// `#[path]` itself is not banned -- `fallback_tests.rs` and
/// `weblogin/scrape_tests.rs` point at siblings inside `src/` that every scan
/// collects, and flagging them is the false positive that gets a test deleted.
/// What is banned is a target with a `..` component or an absolute path:
/// `#[path = "../../gen/x.rs"] mod x;` reaches the same place `include!` does
/// (review AR-5).
///
/// Read from ATTRIBUTES, not from lines beginning `#[path`, which is what
/// `#[cfg_attr(all(), path = "../gen/x.rs")] mod x;` walked past (review
/// finding 10). A `path =` whose value is not a string literal is reported
/// rather than assumed harmless.
fn escaping_paths(sources: &[(PathBuf, String)]) -> Vec<String> {
    let mut hits = Vec::new();
    for (path, text) in rust_only(sources) {
        for attr in outline(text).attributes {
            let assigned = attr
                .body
                .windows(2)
                .filter(|w| w[0].is("path") && w[1].is("="))
                .count();
            let values = attr.values("path");
            let escapes = |v: &String| v.contains("..") || v.starts_with('/') || v.contains(':');
            if values.len() < assigned || values.iter().any(escapes) {
                hits.push(format!(
                    "{}:{}: path = {values:?}",
                    relative(path),
                    attr.line
                ));
            }
        }
    }
    hits
}

/// Extensions `include_str!` may embed although the walk does not scan them,
/// each with why that is harmless.
///
/// # This replaced a claim that was false (review finding 10)
///
/// `harness.rs` said `SOURCE_EXTENSIONS` "matters only for `include_str!` of a
/// file the walk already collects -- the crit tables". MEASURED 2026-09-23:
/// 29 `include_str!` sites, 12 of them of files the walk does NOT collect --
/// `cena-web`'s `index.html`, `app.js`, `session.js` and `style.css`,
/// `cena-ui`'s `snapshot-v1.json`, and seven `.xml` wire fixtures in tests.
///
/// Scanning them was considered and declined. Every rule the scan feeds is a
/// rule about RUST: the line cap is about a file a reader must hold in their
/// head as code, the game-name flag is about an `if game ==` branch, the
/// `static` bans are about items. A stylesheet has none of those, and a wire
/// fixture is SUPPOSED to contain the game's own markup. Scanning them would
/// flag correct files -- the false positive that gets a rule deleted.
///
/// What makes them safe is the property the amendment above already relies
/// on: `include_str!` yields a `&str`, and no string can carry an item. So the
/// claim is now stated as what it is -- these extensions are INERT -- and a
/// new extension has to be added here, with its reason, rather than arriving
/// unexamined.
const INERT_EXTENSIONS: &[(&str, &str)] = &[
    (
        "html",
        "cena-web's page shell, served verbatim; markup a browser renders, never Rust",
    ),
    (
        "js",
        "cena-web's browser code, served verbatim; it runs in the page, not in the process",
    ),
    (
        "css",
        "cena-web's stylesheet, served verbatim; it can express nothing the scans ask about",
    ),
    (
        "json",
        "a serialized fixture a test compares against; the wire shape IS the assertion",
    ),
    (
        "xml",
        "real wire traffic cut into test fixtures; it must contain the game's own markup",
    ),
];

/// Every `include_str!` in `sources` whose target is neither scanned nor
/// named in [`INERT_EXTENSIONS`], or is not a string literal at all.
fn unexamined_embeds(sources: &[(PathBuf, String)]) -> Vec<String> {
    let mut hits = Vec::new();
    for (path, text) in rust_only(sources) {
        let toks: Vec<Token> = tokens(text);
        for (k, t) in toks.iter().enumerate() {
            let invoked = t.is("include_str") && toks.get(k + 1).is_some_and(|n| n.is("!"));
            if !invoked {
                continue;
            }
            let target = toks.get(k + 3).and_then(Token::string_value);
            let ext = target.and_then(|v| Path::new(v).extension()?.to_str());
            let known = ext.is_some_and(|e| {
                SOURCE_EXTENSIONS.contains(&e) || INERT_EXTENSIONS.iter().any(|(i, _)| *i == e)
            });
            if !known {
                hits.push(format!(
                    "{}:{}: include_str! of {target:?}",
                    relative(path),
                    t.line
                ));
            }
        }
    }
    hits
}

/// The `.rs` sources: the only files that can invoke a macro.
fn rust_only(sources: &[(PathBuf, String)]) -> impl Iterator<Item = &(PathBuf, String)> {
    sources
        .iter()
        .filter(|(p, _)| p.extension().is_some_and(|e| e == "rs"))
}

#[test]
fn no_source_file_is_included_from_outside_the_scan() {
    let sources = scannable_sources();
    let escaping = escaping_paths(&sources);
    assert!(
        escaping.is_empty(),
        "`#[path]` pointing OUTSIDE the source walk puts crate source where no \
         scan in this suite can see it -- the same hole `include!` opens, by \
         another route. A `#[path]` naming a sibling inside `src/` is fine and \
         is not flagged.\n{}",
        escaping.join("\n")
    );
    let hits = included_code(&sources);
    assert!(
        hits.is_empty(),
        "`include!` splices a file into a crate without that file being a \
         module, which puts it outside every scan in this suite. VERIFIED: a \
         906-line `.in` file with a `pub static mut` and a game-name branch \
         compiled in with four bans green.\n\n\
         If generated code is genuinely needed (plan/13 §4a), lift this ban \
         deliberately and extend harness::SOURCE_EXTENSIONS so the generated \
         file is scanned, rather than routing around the scan.\n\n\
         `include_str!` is NOT banned; see `embedded_text_is_scanned_or_inert`. \
         `include_bytes!` is: collect_sources reads with read_to_string, so a \
         non-UTF-8 payload is a file no scan can see.\n{}",
        hits.join("\n")
    );
}

#[test]
fn embedded_text_is_scanned_or_inert() {
    let hits = unexamined_embeds(&scannable_sources());
    assert!(
        hits.is_empty(),
        "`include_str!` embeds a file whose extension the walk does not scan \
         and INERT_EXTENSIONS does not name. Either extend \
         harness::SOURCE_EXTENSIONS so every rule reads it, or add the \
         extension to INERT_EXTENSIONS with why no rule needs to.\n{}",
        hits.join("\n")
    );
    for (ext, why) in INERT_EXTENSIONS {
        assert!(
            why.len() > 40,
            "INERT_EXTENSIONS entry {ext} needs a reason"
        );
        assert!(
            !SOURCE_EXTENSIONS.contains(ext),
            "{ext} is scanned; not inert"
        );
    }
}

// ---------------------------------------------------------------------------
// Mutations. Each fixture is a spelling the previous line-needle version
// passed; the old needle's miss is asserted beside the new detector's hit.
// ---------------------------------------------------------------------------

fn fixture(text: &str) -> Vec<(PathBuf, String)> {
    let path = workspace_root().join("crates/cena-fixture/src/lib.rs");
    vec![(path, text.to_owned())]
}

#[test]
fn every_spelling_of_include_is_found() {
    let cases = [
        "const X: &str = include ! (\"body.in\");\n",
        "use std::include as splice;\nsplice!(\"body.in\");\n",
        "pub use core::{include_bytes, include_str};\n",
        "fn f() { std::include!{\"body.in\"} }\n",
    ];
    for case in cases {
        assert!(
            !included_code(&fixture(case)).is_empty(),
            "missed: {case:?}"
        );
    }
    // The first three contain neither old needle anywhere.
    for case in &cases[..3] {
        assert!(!case.contains("include!") && !case.contains("include_bytes!"));
    }
    // Negative control: `include_str!` and an ordinary identifier are not hits.
    let clean = "const T: &str = include_str!(\"t.tsv\");\nfn includes(x: u8) -> u8 { x }\n";
    assert!(included_code(&fixture(clean)).is_empty());
}

#[test]
fn a_path_through_cfg_attr_is_read() {
    let escaping = "#[cfg_attr(all(), path = \"../gen/x.rs\")]\nmod x;\n";
    assert!(
        !escaping.lines().any(|l| l.contains("#[path")),
        "old needle"
    );
    assert_eq!(escaping_paths(&fixture(escaping)).len(), 1);
    let unreadable = "#[path = concat!(\"..\", \"/x.rs\")]\nmod x;\n";
    assert_eq!(escaping_paths(&fixture(unreadable)).len(), 1);
    let sibling = "#[path = \"fallback_tests.rs\"]\nmod tests;\n";
    assert!(escaping_paths(&fixture(sibling)).is_empty());
}

#[test]
fn an_embed_of_an_unexamined_extension_is_flagged() {
    let new_kind = "const W: &str = include_str!(\"../assets/app.wasm.txt\");\n";
    assert_eq!(unexamined_embeds(&fixture(new_kind)).len(), 1);
    let known = "const A: &str = include_str!(\"../assets/app.js\");\n";
    assert!(unexamined_embeds(&fixture(known)).is_empty());
}
