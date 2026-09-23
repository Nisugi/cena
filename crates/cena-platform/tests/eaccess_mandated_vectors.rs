//! **The unit tests `plan/10` §11.5 mandates, four of which did not exist.**
//!
//! §11.5 lists seven vectors "the spike should carry". MEASURED 2026-09-19 by
//! grepping for each: `PROBLEM` and `KEY=abc` were covered in
//! `eaccess_wire.rs`; the other five were not, and the most important of them is
//! the one §3.3 calls out explicitly:
//!
//! > *"Use this as Cena's unit-test vector."* (`plan/10:264`)
//!
//! A known-answer vector is the only test that catches a **silently wrong**
//! hash. Every other test here asserts a shape; this one asserts the arithmetic,
//! and a broken XOR produces a well-shaped wrong answer that a shape test passes.
//! That is the difference between a test that can fail and a test that cannot --
//! which is `plan/19` pattern D, and the review's finding A4/PL-6.
//!
//! # PL-3: the K key is NOT trimmed, and that is the fix
//!
//! `handshake.rs` trimmed ASCII whitespace off the hash key before hashing,
//! citing Vellum (`network.rs:760`, `hash_key.trim()`) as a working
//! implementation. Vellum does do that. **It is still wrong**, and `plan/10`
//! measured why:
//!
//! > `| K | 32 | 32 random bytes, **not printable ASCII** (contained DEL) |`
//! > (`plan/10:1734`)
//!
//! The key is 32 bytes of RANDOM BINARY with no terminator (`:1749` settles
//! this: "`K` has no trailing newline"). So a leading `0x09`, `0x0A`, `0x0D` or
//! `0x20` is an ordinary key byte, not framing -- and trimming it shifts every
//! XOR index by one. The consequence is not a visible error: a wrong password is
//! sent, the server answers `PASSWORD`, and that is a **fatal** stop plus one
//! bad-password strike against the account.
//!
//! Probability is 4/256 per key that the first byte is one of those, so roughly
//! **one login in 64**. That is not exotic; it is a coin-flip that has not come
//! up yet.
//!
//! ## What the PL-3 tests below can and cannot catch
//!
//! They call [`hash_password`] with a whole key, so they pin the ARITHMETIC:
//! a hash that trimmed its own input would fail them. They **cannot** see the
//! call site -- the trim PL-3 removed lived in `handshake.rs`, before the hash
//! was called, and putting it back there left every test in this file green
//! (review finding 4, measured by doing exactly that).
//!
//! The call site is pinned by
//! `src/eaccess/handshake_tests.rs::the_a_request_carries_the_hash_of_the_whole_key_as_sent`,
//! which drives the real conversation over a scripted server with a key whose
//! first byte is `0x20`, and reads back the `A` line that was written. That is
//! a unit test rather than one here because the conversation is crate-private:
//! exporting it for a test would widen the public API to reach a function
//! whose only other caller is `authenticate`.

use cena_platform::eaccess::{hash_password, redact_char_code, resolve_char_code};

// ---------------------------------------------------------------------------
// The mandated known-answer vector -- `plan/10` §3.3 / §11.5
// ---------------------------------------------------------------------------

#[test]
fn the_mandated_hash_vector_matches_byte_for_byte() {
    // `plan/10:258-264`, verbatim. The ONLY test here that can catch a hash
    // that is wrong rather than merely misshapen.
    let hashed = hash_password(b"P@ssw0rd!", b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345")
        .expect("the vector must hash");

    assert_eq!(
        hashed,
        vec![145, 130, 48, 55, 50, 118, 53, 44, 104],
        "the hash does not match `plan/10` §3.3's vector. Every byte of this is \
         the password going to the auth server; a wrong one is a bad-password \
         strike against the account."
    );
}

#[test]
fn a_password_longer_than_the_key_is_refused_before_any_write() {
    // §11.5, and §3.3 item 3. Refusing beats truncating: a truncated hash is a
    // WRONG password, which the server counts against the account.
    let key = [0x41u8; 32];
    let too_long = vec![b'x'; 33];
    let result = hash_password(&too_long, &key);

    assert!(
        result.is_err(),
        "a password longer than the key was hashed anyway, which sends a \
         truncated -- therefore wrong -- credential"
    );
}

// ---------------------------------------------------------------------------
// PL-3 -- the key is binary, so nothing may be trimmed off it
// ---------------------------------------------------------------------------

#[test]
fn a_key_beginning_with_a_whitespace_byte_is_used_whole() {
    // **The defect.** `0x20` is a perfectly ordinary byte in 32 bytes of random
    // binary. Trimming it shifts every XOR index by one.
    //
    // The two hashes below differ in EVERY byte, which is what makes this
    // catchable: the assertion is not "it did not trim" but "the answer is the
    // one the untrimmed key produces".
    let mut key = [0u8; 32];
    key[0] = b' ';
    for (i, slot) in key.iter_mut().enumerate().skip(1) {
        *slot = u8::try_from(i).unwrap_or(0).wrapping_add(0x40);
    }

    let hashed = hash_password(b"secret", &key).expect("must hash");

    // What the UNTRIMMED key gives, computed from the same formula the
    // protocol specifies: ((p - 32) ^ k) + 32.
    let expected: Vec<u8> = b"secret"
        .iter()
        .zip(key.iter())
        .map(|(&p, &k)| u8::try_from(((i32::from(p) - 32) ^ i32::from(k)) + 32).unwrap_or(0))
        .collect();

    assert_eq!(
        hashed, expected,
        "a leading whitespace BYTE was treated as framing and trimmed. The K \
         response is 32 bytes of random binary with no terminator (`plan/10` \
         :1734, :1749) -- 0x20 is data. Trimming shifts every index, sends a \
         wrong password, and costs a bad-password strike."
    );
}

#[test]
fn a_key_beginning_with_a_newline_byte_is_used_whole() {
    // `0x0A` is the one most likely to look like framing to a reader, and the
    // one a `.trim()` ported from Ruby removes without comment.
    let mut key = [0x55u8; 32];
    key[0] = b'\n';
    let hashed = hash_password(b"pw", &key).expect("must hash");

    let first = u8::try_from(((i32::from(b'p') - 32) ^ i32::from(b'\n')) + 32).unwrap_or(0);
    assert_eq!(
        hashed[0], first,
        "the key's leading 0x0A was trimmed; it is a key byte, not a terminator"
    );
}

#[test]
fn a_key_ending_with_a_whitespace_byte_is_used_whole() {
    // The tail matters too: a 32-byte key whose LAST byte is 0x20 is only
    // visible when the password is long enough to reach it.
    let mut key = [0x33u8; 32];
    key[31] = b' ';
    let password = vec![b'a'; 32];
    let hashed = hash_password(&password, &key).expect("must hash");

    let last = u8::try_from(((i32::from(b'a') - 32) ^ i32::from(b' ')) + 32).unwrap_or(0);
    assert_eq!(
        hashed[31], last,
        "the key's trailing 0x20 was trimmed, so a 32-byte password hashed \
         against a 31-byte key"
    );
}

// ---------------------------------------------------------------------------
// The remaining §11.5 vectors
// ---------------------------------------------------------------------------

#[test]
fn a_character_name_with_a_caret_resolves() {
    // §4.6's `[^\t\n]` class. A name containing `^` is legal and a naive
    // character class would reject it.
    let line = "C\t1\t1\t0\t0\tW_ACCT_1\tFoo^Bar\n";
    assert_eq!(
        resolve_char_code(line, "Foo^Bar"),
        Some("W_ACCT_1"),
        "a name containing `^` did not resolve"
    );
}

#[test]
fn a_character_named_new_resolves_to_its_code_not_the_generator() {
    // §4.6. `New` is also the character-generator keyword, so a lookup that
    // matched loosely would send the player to character creation instead of
    // into the game.
    let line = "C\t1\t1\t0\t0\tW_ACCT_7\tNew\n";
    assert_eq!(
        resolve_char_code(line, "New"),
        Some("W_ACCT_7"),
        "a character legitimately named `New` did not resolve to its own code"
    );
}

#[test]
fn an_empty_account_parses_to_zero_characters_rather_than_erroring() {
    // §11.5: `"C\t0\t0\t0\t0\n"`. An account with no characters is a real state,
    // not a protocol error.
    assert_eq!(resolve_char_code("C\t0\t0\t0\t0\n", "Anyone"), None);
}

// ---------------------------------------------------------------------------
// PL-4 -- a character code carries the account name
// ---------------------------------------------------------------------------

#[test]
fn a_character_code_does_not_print_the_account() {
    // `W_<ACCOUNT>_<SLOT>` (`plan/10:472`). The `resolve_char` progress line
    // printed the raw code on EVERY login, so the account went to stderr and
    // into the scrollback -- the same exposure `redact` closed for the `A`
    // response and left open here.
    assert_eq!(redact_char_code("W_SOMEACCT_000"), "W_<ACCOUNT>_000");
}

#[test]
fn an_account_name_containing_an_underscore_still_redacts_whole() {
    // Why the slot is taken from the END. Splitting on `_` and keeping index 2
    // would leave half the account in the output.
    assert_eq!(redact_char_code("W_SOME_ACCT_007"), "W_<ACCOUNT>_007");
}

#[test]
fn an_unrecognised_code_is_returned_unchanged_rather_than_blanked() {
    // An unrecognised code is a DIAGNOSTIC. Blanking it would destroy exactly
    // what a reader needs when the shape is the thing that went wrong.
    assert_eq!(redact_char_code("0"), "0");
    assert_eq!(redact_char_code("NEW_TO_GAME"), "NEW_TO_GAME");
    assert_eq!(redact_char_code(""), "");
}
