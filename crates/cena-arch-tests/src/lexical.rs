//! Lexical analysis of Rust source, for the tests whose mechanism is a scan.
//!
//! Mechanism only; it states no rule.
//!
//! Split out of `harness.rs` under `plan/05:352-353` -- move code down, do not
//! raise the cap. The split is by role: `harness` answers "which bytes are
//! this workspace's source", this module answers "what does a line of it say".
//!
//! # Every function here exists because a simpler one was defeated
//!
//! - `code_lines` over a raw scan: a test that fires on a comment *stating*
//!   the rule gets fixed by deleting the comment.
//! - `item_spans` over `code_lines`: `cargo fmt` wraps a long declaration onto
//!   three lines, and `#[rustfmt::skip]` holds one apart through
//!   `cargo fmt --check`.
//! - `items`' `#[cfg(test)]` tracking: without it the facade rule fires on a
//!   crate root that legitimately has a test module -- a false positive on
//!   correct code, which is the failure direction that gets a rule deleted.
//! - `declares_behavior`'s token test over a prefix allowlist: eight
//!   enumerated spellings missed `pub const fn` and three others.

use crate::harness::relative;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Lexical normalization
// ---------------------------------------------------------------------------

/// `text` with comments blanked out, one entry per source line, 1-indexed by
/// position.
///
/// Returns the *code* of each line with `//` tails and `/* */` spans replaced
/// by spaces, preserving line numbering so a hit still reports the right line.
///
/// Comment-blindness is `plan/05:212-215`'s named defect in Vellum's approach,
/// and the direction of the failure is what makes it matter: a test that fires
/// on a comment *stating the rule* gets fixed by deleting the comment, or by
/// concluding the test is noise. That is how enforcement dies. Both were
/// demonstrated: a block comment naming `GemStone`, and a `cena-ui/Cargo.toml`
/// comment reading "we deliberately avoid ratatui".
///
/// This is deliberately not a Rust lexer, but it **does** model ordinary and
/// raw string literals, because not doing so did not fail safe.
///
/// # Why the "vanishingly unlikely" gap was closed
///
/// This used to skip string tracking, on the reasoning that the cost was a
/// false *negative* on a needle hidden inside a literal that also opens a
/// block comment. That reasoning was wrong about the blast radius. `in_block`
/// latches across lines, so a single `/*` inside a literal blanks **every
/// remaining line of the file** -- for the `static` allowlist, the `static
/// mut` ban, the game-name flag, the include ban and the facade scan at once.
/// One unremarkable line silently disarms five rules over the rest of a file.
/// That is failing open, not safe.
///
/// It stops being hypothetical at M2: a golden-corpus walker calling
/// `glob("**/*.xml")` is exactly that shape, and M2's first work is the
/// corpus (review AR-6, demonstrated). Measured before the fix:
///
/// ```text
/// $ grep -rnE '"[^"]*/\*' crates/ --include=*.rs | wc -l
/// 0
/// ```
///
/// Zero today, which is why this was latent rather than broken.
///
/// Literals are COPIED, not blanked: the needle scans legitimately look
/// inside them -- a game name in a string is the case Rule 3.4 exists for.
/// What must not happen is a `/*` in one opening a comment.
///
/// Escapes are handled for ordinary literals (`\"` does not close) and raw
/// literals match by hash count (`r#"..."#`). Char literals are not tracked:
/// a lone `"` in one cannot open a *comment*, and the `'{'` counting problem
/// it would also solve belongs to `items`, which handles it there.
pub fn code_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;
    // `Some(n)` while inside a raw literal closed by `"` plus n hashes. A raw
    // literal may span lines, so this outlives the loop body like `in_block`.
    let mut in_raw: Option<usize> = None;
    for line in text.lines() {
        let bytes: Vec<char> = line.chars().collect();
        let mut code = String::new();
        let mut i = 0;
        while i < bytes.len() {
            if let Some(hashes) = in_raw {
                // Only `"` plus the same hash count ends it, and nothing
                // inside starts a comment.
                if bytes[i] == '"' && count_hashes(&bytes, i + 1) >= hashes {
                    in_raw = None;
                    code.push('"');
                    for _ in 0..hashes {
                        code.push('#');
                    }
                    i += 1 + hashes;
                } else {
                    code.push(bytes[i]);
                    i += 1;
                }
            } else if in_block {
                if bytes[i] == '*' && bytes.get(i + 1) == Some(&'/') {
                    in_block = false;
                    code.push(' ');
                    code.push(' ');
                    i += 2;
                } else {
                    code.push(' ');
                    i += 1;
                }
            } else if bytes[i] == '/' && bytes.get(i + 1) == Some(&'*') {
                in_block = true;
                code.push(' ');
                code.push(' ');
                i += 2;
            } else if bytes[i] == '/' && bytes.get(i + 1) == Some(&'/') {
                break; // line comment: rest of the line is prose
            } else if let Some(hashes) = raw_literal_start(&bytes, i) {
                in_raw = Some(hashes);
                code.push('r');
                for _ in 0..hashes {
                    code.push('#');
                }
                code.push('"');
                i += 2 + hashes;
            } else if bytes[i] == '"' {
                // An ordinary literal, copied to its unescaped close.
                code.push('"');
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == '\\' {
                        code.push(bytes[i]);
                        if let Some(next) = bytes.get(i + 1) {
                            code.push(*next);
                        }
                        i += 2;
                        continue;
                    }
                    let ch = bytes[i];
                    code.push(ch);
                    i += 1;
                    if ch == '"' {
                        break;
                    }
                }
            } else {
                code.push(bytes[i]);
                i += 1;
            }
        }
        out.push(code);
    }
    out
}

/// `code_lines`, then each line's whitespace runs collapsed to one space.
///
/// Needles are matched against this rather than the raw line wherever a
/// construct can be split by formatting. `#[rustfmt::skip]` above
///
/// ```text
/// pub static
///     mut CURRENT_STREAM_BUFFER: u64 = 0;
/// ```
///
/// survives `cargo fmt --check`, so relying on rustfmt to rejoin the tokens is
/// relying on the attacker's cooperation. VERIFIED: that declaration compiled
/// with `cargo fmt --check` clean and the `static mut` ban green.
pub fn item_spans(text: &str) -> Vec<String> {
    let lines = code_lines(text);
    let mut out = Vec::with_capacity(lines.len());
    for (idx, _) in lines.iter().enumerate() {
        // Join this line with those that follow until the item terminates, so
        // a construct split across lines is one string anchored at its first
        // line. Bounded: a declaration is not 40 lines long, and the bound
        // keeps this O(n * k) rather than O(n^2).
        let mut joined = String::new();
        for follow in lines.iter().skip(idx).take(40) {
            joined.push(' ');
            joined.push_str(follow.trim());
            if follow.contains(';') || follow.contains('{') {
                break;
            }
        }
        out.push(collapse_whitespace(&joined));
    }
    out
}

/// Whitespace runs collapsed to a single space, trimmed.
pub fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Scan `sources` for any of `needles`, ignoring comments.
pub fn scan_lines(sources: &[(PathBuf, String)], needles: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for (path, text) in sources {
        for (idx, code) in code_lines(text).into_iter().enumerate() {
            if needles.iter().any(|n| code.contains(n)) {
                hits.push(format!("{}:{}: {}", relative(path), idx + 1, code.trim()));
            }
        }
    }
    hits
}

/// Scan `sources` for any of `needles` against whitespace-collapsed item
/// spans, so a construct split across lines is still found.
pub fn scan_spans(sources: &[(PathBuf, String)], needles: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for (path, text) in sources {
        // A span starting on line N overlaps the spans starting on N+1, N+2,
        // ... so one violation would be reported once per line it spans.
        // Report only the first line of each run: that is the declaration.
        let mut previous_matched = false;
        for (idx, span) in item_spans(text).into_iter().enumerate() {
            let matched = needles.iter().any(|n| span.contains(n));
            if matched && !previous_matched {
                hits.push(format!("{}:{}: {}", relative(path), idx + 1, span.trim()));
            }
            previous_matched = matched;
        }
    }
    hits
}

// ---------------------------------------------------------------------------
// Item-level structure
// ---------------------------------------------------------------------------

/// A source line classified for the facade scan.
pub struct Item {
    /// 1-indexed line number.
    pub line: usize,
    /// The whitespace-collapsed code of the line.
    pub code: String,
    /// Whether this line is inside a `#[cfg(test)]` module.
    pub in_test_module: bool,
}

/// Every non-blank code line of `text`, with `#[cfg(test)]` module bodies
/// marked.
///
/// The `cfg(test)` tracking is not a nicety. Every Rust crate root
/// legitimately carries a `#[cfg(test)] mod tests`, and a facade rule that
/// fires on one is a **false positive on correct code** — VERIFIED on a
/// `lib.rs` containing nothing but `pub mod room; pub use room::RoomTitle;`
/// and a three-line test, which broke the build. `plan/05` §0's "a rule that
/// is not enforced is a wish" has a mirror: a rule that fires on compliant
/// code gets deleted, and then it is not enforced either.
pub fn items(text: &str) -> Vec<Item> {
    let lines = code_lines(text);
    let mut out = Vec::new();
    let mut pending_cfg_test = false;
    let mut test_mod_depth: Option<usize> = None;
    let mut depth: usize = 0;

    for (idx, raw) in lines.iter().enumerate() {
        let code = collapse_whitespace(raw);
        let opens = raw.matches('{').count();
        let closes = raw.matches('}').count();

        let in_test_module = test_mod_depth.is_some();

        if !in_test_module {
            if code.contains("#[cfg(test)]") {
                pending_cfg_test = true;
            } else if pending_cfg_test && (code.starts_with("mod ") || code.starts_with("pub mod "))
            {
                // The `#[cfg(test)] mod tests {` body starts here.
                test_mod_depth = Some(depth);
                pending_cfg_test = false;
            } else if !code.is_empty() && !code.starts_with("#[") {
                pending_cfg_test = false;
            }
        }

        out.push(Item {
            line: idx + 1,
            code,
            in_test_module: in_test_module || test_mod_depth.is_some(),
        });

        // Saturating: unbalanced braces in a partial file must not panic a
        // test whose job is to report on that file.
        depth = depth.saturating_add(opens).saturating_sub(closes);
        if let Some(entry_depth) = test_mod_depth
            && depth <= entry_depth
        {
            test_mod_depth = None;
        }
    }
    out
}

/// Whether a whitespace-collapsed code line declares a function or an `impl`
/// block at item level.
///
/// A **token** test, not a prefix allowlist. The allowlist it replaced
/// enumerated eight spellings (`fn `, `pub fn `, `async fn `, ...) and was
/// VERIFIED to miss `pub const fn`, `pub extern "Rust" fn`, `pub(crate) const
/// fn` and a bare `const fn` — five working functions in a facade, green. It
/// also misses `pub unsafe fn`, `pub(in path) fn` and `default fn`. Any
/// allowlist of spellings has the same shape of hole; a token test does not,
/// because `fn` and `impl` are keywords that cannot be spelled another way.
pub fn declares_behavior(code: &str) -> bool {
    // A declaration, not a use: `fn` inside a type position (`Box<dyn Fn()>`,
    // a `fn(u8) -> u8` pointer type) is not an item. Requiring the token to be
    // followed by an identifier-or-`<` and preceded only by modifiers is more
    // machinery than the rule needs; anchoring on the keyword as a standalone
    // token, in a line that is not a type alias or a field, is enough.
    if code.starts_with("type ") || code.starts_with("pub type ") {
        return false;
    }
    code.split_whitespace()
        .any(|token| token == "fn" || token == "impl" || token.starts_with("impl<"))
}

/// How many consecutive `#` start at `from`.
fn count_hashes(bytes: &[char], from: usize) -> usize {
    if from >= bytes.len() {
        return 0;
    }
    bytes[from..].iter().take_while(|c| **c == '#').count()
}

/// Hash count if a raw string literal starts at `at`.
///
/// Returns `None` when this `r` is part of an identifier -- `for`, `char`,
/// `parser` -- which the PREVIOUS character decides. Without that check the
/// `r` ending an identifier before a string would open a literal that never
/// closes, blanking the rest of the file in the other direction.
fn raw_literal_start(bytes: &[char], at: usize) -> Option<usize> {
    if bytes.get(at) != Some(&'r') {
        return None;
    }
    if let Some(prev) = at.checked_sub(1).and_then(|p| bytes.get(p))
        && (prev.is_alphanumeric() || *prev == '_')
    {
        return None;
    }
    let hashes = count_hashes(bytes, at + 1);
    (bytes.get(at + 1 + hashes) == Some(&'"')).then_some(hashes)
}
