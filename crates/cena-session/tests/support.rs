//! Shared scaffolding for the session tests. Not a test file itself.
//!
//! # Where the wire bytes come from
//!
//! Two sources, deliberately:
//!
//! - [`room_fixture`] reads `crates/cena-protocol/tests/fixtures/room.xml`,
//!   which is **real, scrubbed wire traffic** cut from the corpus and covered
//!   by `cena-protocol`'s `fixtures_are_scrubbed.rs`. It is read across the
//!   crate boundary rather than copied so that there is exactly one fixture
//!   directory in the workspace and exactly one scrubbing scan over it. A
//!   second copy here would be a second place for an unscrubbed byte to live,
//!   and nothing would scan it.
//! - The `wire!`-style literals in the tests themselves are **synthetic**:
//!   hand-written bytes that isolate one behaviour (a tag split at a chosen
//!   offset, an unknown tag, a window with no terminator). Corpus data cannot
//!   isolate those, because it contains whatever it contains.

use std::path::PathBuf;

/// The committed room fixture, as raw bytes.
///
/// # Errors
///
/// A message naming the path, if the fixture is missing.
///
/// It returns an error rather than panicking because this is a plain
/// test-target module with no `#[test]` in it, so `clippy.toml`'s
/// `allow-panic-in-tests` does not reach it -- and routing around that with a
/// per-file `#[allow]` is exactly the discipline that file's comment says it
/// exists to avoid ("nothing can be forgotten at the top of a new test file").
///
/// **Every caller must `.expect()` this rather than defaulting to empty.** A
/// test that reads no bytes and asserts over an empty frame stream passes
/// vacuously, which is worse than failing.
pub fn room_fixture() -> Result<Vec<u8>, String> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cena-protocol/tests/fixtures/room.xml");
    std::fs::read(&path).map_err(|e| {
        format!(
            "the room fixture is a build input for the session tests and must \
             be readable at {}: {e}",
            path.display()
        )
    })
}
