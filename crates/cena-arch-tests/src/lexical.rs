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
//! - `tokens` over substring needles: `include ! (...)`, a multi-line
//!   `#[allow(` whose lint sat on the next line, and `use std::{fs, io}` each
//!   walked past a needle that assumed one spelling on one line (review
//!   findings 6, 10 and 12).

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
/// This is deliberately not a Rust parser, but it **is** a Rust lexer for the
/// four things that decide where a comment starts: ordinary literals, raw
/// literals, char literals and block comments -- because getting any one of
/// them wrong did not fail safe.
///
/// # Why the "vanishingly unlikely" gap was closed
///
/// This used to skip string tracking, on the reasoning that the cost was a
/// false *negative* on a needle hidden inside a literal that also opens a
/// block comment. That reasoning was wrong about the blast radius. A block
/// comment latches across lines, so a single `/*` inside a literal blanks
/// **every remaining line of the file** -- for the `static` allowlist, the
/// `static mut` ban, the game-name flag, the include ban and the facade scan
/// at once. One unremarkable line silently disarms five rules over the rest
/// of a file. That is failing open, not safe.
///
/// Literals are COPIED, not blanked: the needle scans legitimately look
/// inside them -- a game name in a string is the case Rule 3.4 exists for.
/// What must not happen is a `/*` in one opening a comment. [`skeleton_lines`]
/// is the same pass with literal contents blanked, for the scans that must
/// NOT look inside them (brace depth, item structure, tokens).
///
/// # Char literals, and literals that span lines (review finding 8)
///
/// This doc used to say *"Char literals are not tracked: a lone `"` in one
/// cannot open a comment"*. It can. `let q = '"';` opened an ordinary string
/// at the quote inside the char literal; that string then closed at the FIRST
/// quote of the next literal, `"**/*.rs"`, leaving `**/*.rs"` read as code --
/// and the `/*` in it opened a block comment that ran to end of file. VERIFIED
/// against the previous lexer by `lexer_gaps.rs`'s
/// `a_quote_in_a_char_literal_does_not_open_a_string`.
///
/// Ordinary literals also used to be closed at end of line, whatever they
/// said. A multi-line `"...` therefore ended early and its second line was
/// read as code, where a `/*` inside it did the same damage. The string state
/// now outlives the line, exactly as the raw-literal state always did.
///
/// A char literal is told from a lifetime by shape: `'x'` and `'\n'` close
/// within a few characters, `'a` in `&'a str` does not close at all.
pub fn code_lines(text: &str) -> Vec<String> {
    lex_lines(text).into_iter().map(|(code, _)| code).collect()
}

/// [`code_lines`] with every literal's CONTENTS blanked to spaces, delimiters
/// kept. Same length per line, so a column in one is a column in the other.
///
/// For the scans that must not see inside a literal: brace counting (a `'{'`
/// or `"{"` is not a scope), item structure, and the token scans that ask
/// "is this identifier code".
pub fn skeleton_lines(text: &str) -> Vec<String> {
    lex_lines(text).into_iter().map(|(_, skel)| skel).collect()
}

/// The lexer's state between characters -- and between lines, which is the
/// point: every variant except `Code` may span a newline.
#[derive(Clone, Copy)]
enum Mode {
    Code,
    /// Inside `/* */`, at this nesting depth (Rust block comments nest).
    Block(usize),
    /// Inside an ordinary (or byte, or C) string literal.
    Str,
    /// Inside a raw literal closed by `"` plus this many hashes.
    Raw(usize),
}

/// What a run of characters is, for the two outputs of [`lex_lines`].
#[derive(Clone, Copy, PartialEq)]
enum Class {
    /// Structure: copied to both outputs.
    Code,
    /// A literal's contents: copied to `code`, blanked in the skeleton.
    Literal,
    /// A comment: blanked in both.
    Comment,
}

/// One `(code, skeleton)` pair per line; see [`code_lines`].
fn lex_lines(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut mode = Mode::Code;
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut code = String::new();
        let mut skel = String::new();
        let mut emit = |run: &[char], class: Class| {
            for c in run {
                code.push(if class == Class::Comment { ' ' } else { *c });
                skel.push(if class == Class::Code { *c } else { ' ' });
            }
        };
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            let (width, class) = match mode {
                Mode::Block(depth) => {
                    if c == '*' && next == Some('/') {
                        mode = if depth == 1 {
                            Mode::Code
                        } else {
                            Mode::Block(depth - 1)
                        };
                        (2, Class::Comment)
                    } else if c == '/' && next == Some('*') {
                        mode = Mode::Block(depth + 1);
                        (2, Class::Comment)
                    } else {
                        (1, Class::Comment)
                    }
                }
                Mode::Raw(hashes) => {
                    if c == '"' && count_hashes(&chars, i + 1) >= hashes {
                        mode = Mode::Code;
                        (1 + hashes, Class::Code)
                    } else {
                        (1, Class::Literal)
                    }
                }
                Mode::Str => {
                    if c == '\\' {
                        // An escape -- including a trailing `\` continuing the
                        // literal onto the next line, where the mode survives.
                        (if next.is_some() { 2 } else { 1 }, Class::Literal)
                    } else if c == '"' {
                        mode = Mode::Code;
                        (1, Class::Code)
                    } else {
                        (1, Class::Literal)
                    }
                }
                Mode::Code => {
                    if c == '/' && next == Some('*') {
                        mode = Mode::Block(1);
                        (2, Class::Comment)
                    } else if c == '/' && next == Some('/') {
                        break; // a line comment: the rest of the line is prose
                    } else if let Some(hashes) = raw_literal_start(&chars, i) {
                        mode = Mode::Raw(hashes);
                        (2 + hashes, Class::Code)
                    } else if c == '"' {
                        mode = Mode::Str;
                        (1, Class::Code)
                    } else if let Some(close) =
                        (c == '\'').then(|| char_literal_end(&chars, i)).flatten()
                    {
                        // Quotes are structure, the body is a literal. The
                        // opening quote and body are emitted here; the step
                        // below emits the closing quote.
                        emit(&chars[i..=i], Class::Code);
                        emit(&chars[i + 1..close], Class::Literal);
                        i = close;
                        (1, Class::Code)
                    } else {
                        (1, Class::Code)
                    }
                }
            };
            let end = (i + width).min(chars.len());
            emit(&chars[i..end], class);
            i = end;
        }
        out.push((code, skel));
    }
    out
}

/// The index of the closing quote if a char literal starts at `at`, or `None`
/// for a lifetime or label.
///
/// `'x'` closes two characters on; an escape (`'\n'`, `'\''`, `'\u{1F600}'`)
/// closes at the first quote after the escaped character, within a short
/// bound. A lifetime (`'a`, `'static`) is a quote followed by an identifier
/// that never closes -- `'a'` is a char, `'ab` is a lifetime, which is Rust's
/// own rule.
fn char_literal_end(chars: &[char], at: usize) -> Option<usize> {
    let first = *chars.get(at + 1)?;
    if first == '\\' {
        // Skip the escaped character itself, so `'\''` does not close early.
        let from = at + 3;
        let bound = (at + 14).min(chars.len());
        return (from..bound).find(|&j| chars[j] == '\'');
    }
    (first != '\'' && chars.get(at + 2) == Some(&'\'')).then_some(at + 2)
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
    let lines = lex_lines(text);
    let mut out = Vec::with_capacity(lines.len());
    for idx in 0..lines.len() {
        // Join this line with those that follow until the item terminates, so
        // a construct split across lines is one string anchored at its first
        // line. Bounded: a declaration is not 40 lines long, and the bound
        // keeps this O(n * k) rather than O(n^2). Termination is read off the
        // SKELETON, so a `;` inside a literal does not end the item early.
        let mut joined = String::new();
        for (code, skel) in lines.iter().skip(idx).take(40) {
            joined.push(' ');
            joined.push_str(code.trim());
            if skel.contains(';') || skel.contains('{') {
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
///
/// Braces are counted on the SKELETON. This doc's predecessor in `code_lines`
/// said the `'{'` problem "belongs to `items`, which handles it there"; it did
/// not -- a `'{'` or a `"{"` counted as a scope, so a test module's depth
/// never returned and everything after it was marked test code (review
/// finding 8).
pub fn items(text: &str) -> Vec<Item> {
    let lines = lex_lines(text);
    let mut out = Vec::new();
    let mut pending_cfg_test = false;
    let mut test_mod_depth: Option<usize> = None;
    let mut depth: usize = 0;

    for (idx, (raw, skel)) in lines.iter().enumerate() {
        let code = collapse_whitespace(raw);
        let opens = skel.matches('{').count();
        let closes = skel.matches('}').count();

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

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

/// What a [`Token`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// An identifier or keyword: `fn`, `static`, `SessionHandle`, `r#type`.
    Ident,
    /// `'a`, `'static`, a loop label. Distinct from `Ident`, so the lifetime
    /// `'static` is never the keyword `static`.
    Lifetime,
    /// A string, byte-string, raw-string or char literal, whole.
    Literal,
    /// A number, suffix included.
    Number,
    /// Any other single character.
    Punct,
}

/// One token of comment-free Rust.
#[derive(Clone, Debug)]
pub struct Token {
    /// What it is.
    pub kind: TokenKind,
    /// Its text. For a literal, the literal as written, quotes and all.
    pub text: String,
    /// 1-indexed source line it starts on.
    pub line: usize,
}

impl Token {
    /// Whether this is the identifier or punctuation `text`.
    pub fn is(&self, text: &str) -> bool {
        self.kind != TokenKind::Literal && self.text == text
    }

    /// A string literal's contents between its quotes, escapes left as
    /// written; `None` for anything that is not a string literal.
    pub fn string_value(&self) -> Option<&str> {
        if self.kind != TokenKind::Literal {
            return None;
        }
        let open = self.text.find('"')?;
        let hashes = self.text[..open].matches('#').count();
        let body = &self.text[open + 1..];
        let close = body.len().checked_sub(1 + hashes)?;
        body.get(..close)
    }
}

/// Every token of `text`, comments removed, literals kept whole.
///
/// Built on the skeleton so that nothing inside a literal can be mistaken for
/// code, and read back from the code lines so a literal's own text is still
/// available -- a `#[path = "..."]` value is inside one.
pub fn tokens(text: &str) -> Vec<Token> {
    let lines = lex_lines(text);
    let mut code: Vec<char> = Vec::new();
    let mut skel: Vec<char> = Vec::new();
    let mut line_of: Vec<usize> = Vec::new();
    for (n, (c, s)) in lines.iter().enumerate() {
        code.extend(c.chars());
        skel.extend(s.chars());
        code.push('\n');
        skel.push('\n');
        line_of.extend(std::iter::repeat_n(n + 1, c.chars().count() + 1));
    }
    let ident_start = |c: char| c.is_alphabetic() || c == '_';
    let ident_char = |c: char| c.is_alphanumeric() || c == '_';

    let mut out = Vec::new();
    let mut i = 0;
    while i < skel.len() {
        let c = skel[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let kind = if ident_start(c) {
            while i < skel.len() && ident_char(skel[i]) {
                i += 1;
            }
            // `r"..."`, `b"..."`, `br#"..."#`, `b'x'`, `c"..."`: a prefix,
            // not an identifier, when a quote or a raw hash follows.
            let prefix: String = skel[start..i].iter().collect();
            let quoted = matches!(skel.get(i), Some('"' | '#' | '\''));
            if quoted && matches!(prefix.as_str(), "r" | "b" | "br" | "c" | "cr") {
                i = literal_end(&skel, i);
                TokenKind::Literal
            } else if prefix == "r" && skel.get(i) == Some(&'#') {
                // `r#type`, a raw identifier.
                i += 1;
                while i < skel.len() && ident_char(skel[i]) {
                    i += 1;
                }
                TokenKind::Ident
            } else {
                TokenKind::Ident
            }
        } else if c.is_ascii_digit() {
            while i < skel.len() && ident_char(skel[i]) {
                i += 1;
            }
            TokenKind::Number
        } else if c == '"' {
            i = literal_end(&skel, i);
            TokenKind::Literal
        } else if c == '\'' && skel.get(i + 1).is_some_and(|n| ident_start(*n)) {
            // In the skeleton a char literal's body is blank, so a quote
            // followed by an identifier character is always a lifetime.
            i += 1;
            while i < skel.len() && ident_char(skel[i]) {
                i += 1;
            }
            TokenKind::Lifetime
        } else if c == '\'' {
            i = literal_end(&skel, i);
            TokenKind::Literal
        } else {
            i += 1;
            TokenKind::Punct
        };
        let source = if kind == TokenKind::Literal {
            &code
        } else {
            &skel
        };
        out.push(Token {
            kind,
            text: source[start..i].iter().collect(),
            line: line_of[start],
        });
    }
    out
}

/// One past the end of the literal whose opening delimiter (or raw hashes) is
/// at `at`, in a skeleton -- where the body is blank, so the first matching
/// delimiter is the close.
fn literal_end(skel: &[char], at: usize) -> usize {
    let hashes = count_hashes(skel, at);
    let open = at + hashes;
    let Some(&quote) = skel.get(open) else {
        return open;
    };
    let mut j = open + 1;
    while j < skel.len() {
        if skel[j] == quote && count_hashes(skel, j + 1) >= hashes {
            return j + 1 + hashes;
        }
        j += 1;
    }
    skel.len()
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
///
/// `br"..."` and `cr"..."` are raw too: a `b` or `c` that itself starts a
/// token is a prefix, not an identifier the `r` belongs to. Before this, a
/// `br#"..."#` fell through to the ordinary-literal path, which closed it at
/// the first inner quote.
fn raw_literal_start(bytes: &[char], at: usize) -> Option<usize> {
    if bytes.get(at) != Some(&'r') {
        return None;
    }
    let ident = |c: &char| c.is_alphanumeric() || *c == '_';
    if let Some(prev) = at.checked_sub(1).and_then(|p| bytes.get(p))
        && ident(prev)
    {
        let prefix = matches!(prev, 'b' | 'c');
        let before = at.checked_sub(2).and_then(|p| bytes.get(p));
        if !prefix || before.is_some_and(ident) {
            return None;
        }
    }
    let hashes = count_hashes(bytes, at + 1);
    (bytes.get(at + 1 + hashes) == Some(&'"')).then_some(hashes)
}
