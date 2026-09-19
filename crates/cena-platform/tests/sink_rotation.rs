//! Part numbering across a roll, and what happens when a roll fails.
//!
//! The `.bytes` file is the capture format M2's golden corpus is cut from and
//! the one criterion 7 replays, so its claim is narrow and absolute: the parts
//! of a session, concatenated in order, are the wire verbatim.
//!
//! A gap in the numbering breaks that claim in the one way a reader cannot
//! recover from -- it is indistinguishable from a part that was written and
//! later deleted. Review finding PL-8 found `roll()` spending a part number
//! before the file it names exists.

use cena_platform::{Redactions, SessionSink};
use std::path::{Path, PathBuf};

/// A directory of this test's own, under the OS temp dir.
///
/// Not `tempfile`: the workspace does not depend on it, and a dependency added
/// for one test is the kind of thing `plan/05` Rule -1 exists to refuse.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-sink-rotation-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The `.bytes` files in `dir`, sorted.
///
/// Returns empty rather than panicking on an unreadable directory: this is a
/// HELPER, not a `#[test]`, so `clippy.toml`'s `allow-panic-in-tests` does not
/// reach it (the exemption is scoped to test functions, which is correct --
/// a helper is ordinary code). Every caller asserts on the contents, and an
/// empty vector fails those assertions with the directory listing in the
/// message, so nothing is swallowed.
fn parts_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            Path::new(n)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("bytes"))
        })
        .collect();
    names.sort();
    names
}

/// Rolling produces consecutive parts, starting at 000.
#[test]
fn parts_are_numbered_consecutively_from_zero() {
    let dir = scratch("consecutive");
    // Roll after every write, so three writes are three parts.
    let mut sink = SessionSink::create_with_rotation(&dir, "Tester", "stamp", Redactions::new(), 1)
        .expect("create");

    for i in 0..3u8 {
        sink.wire(true, format!("line {i}\n").as_bytes())
            .expect("write");
    }
    drop(sink);

    let parts = parts_in(&dir);
    assert_eq!(
        parts,
        vec![
            "Tester-stamp-000.bytes",
            "Tester-stamp-001.bytes",
            "Tester-stamp-002.bytes",
            "Tester-stamp-003.bytes",
        ],
        "every part is numbered and none is skipped"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **A roll that cannot create its file does not consume the part number.**
///
/// The failure is forced by putting a DIRECTORY where the next part's file
/// would go: `File::create` on an existing directory fails on every platform,
/// and it fails at exactly the call whose ordering is under test, without
/// needing permissions, a full disk, or a platform-specific error.
///
/// The old code did `self.part += 1` before `File::create(&next)?`, so this
/// sequence produced `-000`, a failed `-001`, and then `-002` -- a hole where
/// nothing was ever written. The sink is still usable after the failure,
/// because the old writer is still open; it just has not rolled.
#[test]
fn a_failed_roll_does_not_burn_a_part_number() {
    let dir = scratch("failed-roll");
    let mut sink = SessionSink::create_with_rotation(&dir, "Tester", "stamp", Redactions::new(), 2)
        .expect("create");

    // Block part 001 with a directory of the same name.
    let blocked = dir.join("Tester-stamp-001.bytes");
    std::fs::create_dir_all(&blocked).expect("block the next part");

    // Two writes reach the threshold and attempt the roll, which fails.
    sink.wire(true, b"one\n").expect("the first write lands");
    let rolled = sink.wire(true, b"two\n");
    assert!(
        rolled.is_err(),
        "creating a part over an existing directory must fail, or this test \
         proves nothing"
    );

    // Unblock, and write enough to trigger the NEXT roll.
    std::fs::remove_dir(&blocked).expect("unblock");
    sink.wire(true, b"three\n").expect("write");
    sink.wire(true, b"four\n").expect("write");
    drop(sink);

    let parts = parts_in(&dir);
    assert!(
        parts.contains(&"Tester-stamp-001.bytes".to_owned()),
        "the part number the FAILED roll named must be reused by the next \
         successful one. A hole here is indistinguishable from a deleted part, \
         in a format whose whole claim is that it is the wire verbatim. \
         Found: {parts:?}"
    );
    assert!(
        !parts.contains(&"Tester-stamp-002.bytes".to_owned()),
        "002 means 001 was skipped -- the defect PL-8 reported. Found: {parts:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
