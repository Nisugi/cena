//! The lexer the whole suite rests on, attacked on its own terms.
//!
//! Every rule in this crate is a needle scan over [`code_lines`]. If that
//! function can be made to blank a file's contents, every one of those rules
//! passes by seeing nothing -- and passes *silently*, which is the failure
//! `plan/05` Rule 0 names: a rule that is not enforced is a wish.
//!
//! These are the cases review finding AR-6 demonstrated. Each was verified to
//! fail against the previous lexer and pass against this one.

use cena_arch_tests::lexical::code_lines;

/// The banned declaration, assembled rather than spelled.
///
/// **This file is scanned by the very rules it tests.** Spelling the two
/// tokens together in a fixture makes `no_static_mut_anywhere` and
/// `every_static_is_allowlisted` fire on the test that proves they can see it
/// -- which is exactly what happened on this file's first run.
///
/// That is not those rules misbehaving. They are lexical by design, and a
/// lexical scan cannot tell a declaration from a string that looks like one.
/// `is_game_data` in `file_rules.rs` records the same tension for `.tsv` rows,
/// and resolves it the same way: the scanned side adapts. No banned token
/// appears in this source; the fixtures build one at runtime.
fn banned_decl(name: &str) -> String {
    format!("pub {} {} {name}: u64 = 0;\n", "static", "mut")
}

/// The same two tokens as a bare needle, for asserting a line survived.
fn banned_needle(name: &str) -> String {
    format!("{} {} {name}", "static", "mut")
}

/// **A `/*` inside a string literal must not open a block comment.**
///
/// This is the one that mattered. `in_block` latches across lines, so a single
/// such literal blanked every remaining line of the file -- for the `static`
/// allowlist, the mutable-global ban, the game-name flag, the include ban and
/// the facade scan simultaneously.
///
/// The shape is not exotic. `glob("**/*.xml")` is what M2's golden-corpus
/// walker will be written with, and M2's first work is the corpus.
#[test]
fn a_glob_pattern_in_a_literal_does_not_blank_the_rest_of_the_file() {
    let source = format!(
        "let files = glob(\"**/*.xml\");\n{}fn ordinary() {{}}\n",
        banned_decl("SNEAKY")
    );
    let lines = code_lines(&source);

    assert!(
        lines.iter().any(|l| l.contains(&banned_needle("SNEAKY"))),
        "the banned declaration on the line AFTER a glob literal was blanked, \
         so the scan for it would pass by seeing nothing. Lines: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("fn ordinary")),
        "everything after the glob literal was blanked: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("*.xml")),
        "the literal's own content must survive -- the needle scans \
         legitimately look inside literals: {lines:?}"
    );
}

/// A real block comment is still stripped.
///
/// The fix must not buy literal-safety by giving up comment-blindness, which
/// exists so a test cannot fire on a comment *stating* the rule -- the failure
/// mode that gets tests deleted as noise.
#[test]
fn a_real_block_comment_is_still_stripped() {
    let source = concat!(
        "let a = 1; /* mentions the game by name deliberately\n",
        "   and keeps mentioning it */ let b = 2;\n",
        "let c = 3;\n"
    );
    let lines = code_lines(source);
    let joined = lines.join("\n");

    assert!(
        !joined.contains("mentions the game"),
        "a block comment must be invisible to the scans, or a rule fires on \
         prose that states it: {lines:?}"
    );
    assert!(
        joined.contains("let a = 1;") && joined.contains("let b = 2;"),
        "code either side of the comment must survive: {lines:?}"
    );
    assert!(
        joined.contains("let c = 3;"),
        "the comment must CLOSE -- a latched block comment blanks the rest of \
         the file: {lines:?}"
    );
}

/// An escaped quote does not end a literal early.
///
/// If it did, the lexer would leave the literal mid-way and read what follows
/// as code -- the same class of desync in the other direction.
#[test]
fn an_escaped_quote_does_not_end_a_literal() {
    let source = format!(
        "let msg = \"she said \\\" /* not a comment \\\" ok\";\n{}",
        banned_decl("AFTER")
    );
    let lines = code_lines(&source);
    assert!(
        lines.iter().any(|l| l.contains(&banned_needle("AFTER"))),
        "an escaped quote desynced the lexer and blanked what followed: \
         {lines:?}"
    );
}

/// A raw literal is closed by its hash count, not by the first quote.
#[test]
fn a_raw_literal_is_closed_by_its_hash_count() {
    let source = format!(
        "let re = r#\"a \" quote and /* a comment opener\"#;\n{}",
        banned_decl("AFTER_RAW")
    );
    let lines = code_lines(&source);
    assert!(
        lines
            .iter()
            .any(|l| l.contains(&banned_needle("AFTER_RAW"))),
        "a raw literal containing a bare quote and a `/*` blanked the line \
         after it: {lines:?}"
    );
}

/// An `r` that ends an identifier does not open a raw literal.
///
/// The guard against this matters as much as the feature: `let parser = "x";`
/// has an `r` immediately before a quote, and reading it as `r"` would open a
/// literal that never closes -- blanking the rest of the file exactly as the
/// original defect did.
#[test]
fn an_identifier_ending_in_r_does_not_open_a_raw_literal() {
    let source = format!("let parser = \"value\";\n{}", banned_decl("AFTER_IDENT"));
    let lines = code_lines(&source);
    assert!(
        lines
            .iter()
            .any(|l| l.contains(&banned_needle("AFTER_IDENT"))),
        "the `r` ending `parser` was read as a raw-literal prefix, so the \
         literal never closed and the rest of the file was blanked: {lines:?}"
    );
}

/// A `//` inside a literal does not truncate the line.
#[test]
fn a_url_in_a_literal_is_not_a_line_comment() {
    let source = format!(
        "let url = \"https://example.invalid/path\";\n{}",
        banned_decl("AFTER_URL")
    );
    let lines = code_lines(&source);
    let joined = lines.join("\n");
    assert!(
        joined.contains("example.invalid"),
        "the `//` inside a URL literal truncated the line: {lines:?}"
    );
    assert!(
        joined.contains(&banned_needle("AFTER_URL")),
        "the line after a URL literal was lost: {lines:?}"
    );
}

// ---------------------------------------------------------------------------
// Review finding 8. Each fixture below was run against the PREVIOUS lexer
// (`git show 2ad1209:crates/cena-arch-tests/src/lexical.rs`, compiled in a
// scratch binary) and produced the failure its assertion names: the banned
// line blanked, the comment text visible, or a line after a test module still
// marked as test code. All five pass against the current one.
// ---------------------------------------------------------------------------

/// **A `"` inside a char literal must not open a string.**
///
/// The previous lexer did not track char literals, and its doc said that was
/// safe because "a lone `"` in one cannot open a *comment*". It opens a
/// string, which closes at the first quote of the NEXT literal -- leaving that
/// literal's body read as code. Here the body is a glob, so its `/*` opened a
/// block comment that ran to end of file.
#[test]
fn a_quote_in_a_char_literal_does_not_open_a_string() {
    let source = format!(
        "let q = '\"'; let g = \"**/*.rs\";\n{}",
        banned_decl("AFTER_CHAR")
    );
    let lines = code_lines(&source);
    assert!(
        lines
            .iter()
            .any(|l| l.contains(&banned_needle("AFTER_CHAR"))),
        "a quote in a char literal desynced the lexer and blanked the rest of \
         the file: {lines:?}"
    );
}

/// An ordinary literal that spans lines stays a literal on its second line.
///
/// The previous lexer closed every ordinary literal at end of line, so the
/// second line of a multi-line string was read as code -- and a `/*` there
/// blanked everything after it.
#[test]
fn a_multi_line_string_does_not_end_at_the_newline() {
    let source = format!(
        "let s = \"one\n/* two\n\";\n{}",
        banned_decl("AFTER_MULTILINE")
    );
    let lines = code_lines(&source);
    assert!(
        lines
            .iter()
            .any(|l| l.contains(&banned_needle("AFTER_MULTILINE"))),
        "the second line of a string literal was read as code: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("/* two")),
        "the literal's second line is literal text and must be copied: {lines:?}"
    );
}

/// A brace in a char literal is not a scope.
///
/// `items` counted braces on the raw line, so a `'{'` inside a test module
/// left its depth one too high: the module never closed, and every item after
/// it was marked test code -- exempt from the facade rule. The doc on
/// `code_lines` claimed `items` handled this. It did not.
#[test]
fn a_brace_in_a_char_literal_does_not_open_a_scope() {
    let source = "#[cfg(test)]\nmod tests {\n    const C: char = '{';\n}\npub fn after() {}\n";
    let after = cena_arch_tests::lexical::items(source)
        .into_iter()
        .find(|item| item.code.contains("after"))
        .expect("the fixture's last item");
    assert!(
        !after.in_test_module,
        "`after` sits outside the test module, and a `'{{'` in a char literal \
         kept the module open past its closing brace"
    );
}

/// Block comments nest in Rust, so the first `*/` does not end them.
#[test]
fn a_nested_block_comment_closes_at_its_own_end() {
    let lines = code_lines("/* outer /* inner */ still comment */ let x = 1;\n");
    assert!(
        !lines[0].contains("still comment"),
        "the inner `*/` closed the outer comment, exposing prose to the needle \
         scans: {lines:?}"
    );
    assert!(lines[0].contains("let x = 1;"), "{lines:?}");
}

/// `br#"..."#` is a raw literal: its inner quote does not close it.
#[test]
fn a_byte_raw_literal_is_raw() {
    let source = format!(
        "let b = br#\"a \" /* x\"#;\n{}",
        banned_decl("AFTER_BYTE_RAW")
    );
    let lines = code_lines(&source);
    assert!(
        lines
            .iter()
            .any(|l| l.contains(&banned_needle("AFTER_BYTE_RAW"))),
        "`br#\"` was read as an ordinary literal, closed at its inner quote, \
         and the `/*` after it blanked the file: {lines:?}"
    );
}

/// Lifetimes are not char literals, and a char that looks like one is.
#[test]
fn lifetimes_and_chars_are_told_apart() {
    let source = format!(
        "fn f<'a>(x: &'a str) -> char {{ let _ = x; 'a' }}\n{}",
        banned_decl("AFTER_LIFETIME")
    );
    let lines = code_lines(&source);
    assert!(
        lines
            .iter()
            .any(|l| l.contains(&banned_needle("AFTER_LIFETIME"))),
        "{lines:?}"
    );
    let tokens = cena_arch_tests::lexical::tokens(&source);
    let lifetimes = tokens
        .iter()
        .filter(|t| t.kind == cena_arch_tests::lexical::TokenKind::Lifetime)
        .count();
    assert_eq!(lifetimes, 2, "`'a` twice as a lifetime: {tokens:?}");
}
