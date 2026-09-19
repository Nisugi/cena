//! Rule 4.3 — **one owning field per shared value** (`plan/05:376-383`).
//!
//! # Why this is the highest-value test in the reference suite
//!
//! `VellumFE` carried a **duplicate `server_time_offset` field that nothing
//! assigned for ten months**. Every countdown read the stale copy: 49.8% of
//! 6,373 measured countdowns were wrong, and the bug was invisible because both
//! fields existed, both compiled, and only one was maintained.
//!
//! The mechanism is deliberately crude -- a needle for a literal field
//! declaration -- and that is the point. A duplicate field is a *lexical* fact,
//! so a lexical test catches it, where no type or trait could.
//!
//! # Why these tests did not exist until now, and what that cost
//!
//! `ratchet.rs`'s `DEFERRED_RULES` deferred Rule 4.3 because *"none of Cena's
//! four values -- clock offset, roundtime, current-room id, active-session
//! handle -- has a field name yet. Write each in the same commit as its
//! field."*
//!
//! **The fields were written and the tests were not.** `roundtime_ends` and
//! `game_time` have existed in `cena-model/src/state.rs` since the model
//! landed, so the deferral's own unblocking condition had come true and nothing
//! detected it -- a deferral was validated only by the length of its reason
//! string (review finding AR-1, reported HIGH).
//!
//! That is the failure mode `plan/05` Rule 0 names: a rule that is not enforced
//! is a wish. The deferral was the wish; this file is the rule.
//!
//! # The two assertions, and why the second one matters
//!
//! Each test asserts `hits.len() == 1` **and the owning path**. Ported from
//! `reference/VellumFE/tests/architecture.rs:337-358`, whose comment explains
//! the second: the count alone passes if the field is *relocated* to a crate
//! that has no business owning it, because one hit is still one hit. The path
//! assertion is what makes a silent move fail.

use cena_arch_tests::harness::workspace_sources;
use cena_arch_tests::lexical::scan_lines;

/// Declarations of `needle`, as `path:line: code`.
///
/// Matching includes the trailing colon and the leading whitespace of a struct
/// body, so a function parameter of the same name -- which shares the
/// `name: Type` shape -- is not counted. `scan_lines` strips comments, so prose
/// naming a field does not register either.
fn owning_fields(needle: &str) -> Vec<String> {
    let sources = workspace_sources();
    scan_lines(&sources, &[needle])
        .into_iter()
        .filter(|hit| {
            // `scan_lines` emits `path:line: <trimmed code>`. Split after the
            // LINE NUMBER -- `splitn(3, ':')` -- not on `": "`, which lands
            // inside the needle's own trailing colon.
            let code = hit.splitn(3, ':').nth(2).unwrap_or_default().trim();
            // The code must START with the declaration, so a match buried
            // mid-line is not an owning field.
            code.starts_with(needle)
        })
        // **This test file names its own needles**, and so would any other
        // arch test. Without this the suite counts itself and every needle has
        // at least two hits -- which is how the first version of this file
        // failed, reporting three owners where there is one.
        .filter(|hit| !hit.starts_with("crates/cena-arch-tests/"))
        .collect()
}

#[test]
fn roundtime_has_a_single_owning_field() {
    // **The value Vellum's bug was actually about.** A second `roundtime_ends`
    // on `SessionActor`, or a cached copy in a behavior, is the exact shape
    // that made half of Vellum's countdowns wrong -- and a gated send reading
    // the stale one fires EARLY, which `plan/19` §1a records as the roundtime
    // defect that "acts" rather than merely displaying wrong.
    let hits = owning_fields("pub roundtime_ends:");
    assert_eq!(
        hits.len(),
        1,
        "exactly one struct may own a roundtime_ends field (GameState). \
         A second copy is the Vellum bug: both compile, one is maintained, and \
         a gated send reading the stale one fires early. Found:\n{}",
        hits.join("\n")
    );
    assert!(
        hits[0].contains("cena-model/src/state.rs"),
        "roundtime must stay on GameState in cena-model -- a copy anywhere \
         above it is a second source of truth for the same fact. Found at {}",
        hits[0]
    );
}

#[test]
fn the_server_clock_has_a_single_owning_field() {
    // Cena's equivalent of Vellum's `server_time_offset`, and private rather
    // than `pub` -- `state.rs:119` -- because it is read through
    // `game_time_now()`. The needle has no `pub ` prefix for that reason.
    let hits = owning_fields("game_time:");
    assert_eq!(
        hits.len(),
        1,
        "exactly one struct may own a game_time field (GameState). This is the \
         value whose duplicate cost Vellum 49.8% of 6,373 countdowns. \
         Found:\n{}",
        hits.join("\n")
    );
    assert!(
        hits[0].contains("cena-model/src/state.rs"),
        "the server clock must stay on GameState. Found at {}",
        hits[0]
    );
}

#[test]
fn the_session_handle_has_a_single_owning_field() {
    // The fourth value in the deferral. A second stored `SessionHandle` is how
    // a command reaches a connection its holder has already been told is gone
    // -- the shape review finding SE-1 turned out to be.
    //
    // The needle is the TYPED declaration, so the many `handle: &SessionHandle`
    // parameters in behavior code do not count: a borrow is not ownership.
    let hits = owning_fields("handle: SessionHandle,");
    assert_eq!(
        hits.len(),
        1,
        "exactly one struct may own a SessionHandle field. A second stored \
         handle is a second way to reach a connection, and they go stale \
         independently. Found:\n{}",
        hits.join("\n")
    );
    assert!(
        hits[0].contains("cena-session/src/actor/handle.rs"),
        "the owning handle must stay in cena-session. Found at {}",
        hits[0]
    );
}

// THE FOURTH VALUE, `current-room id`, HAS NO TEST, and the reason is recorded
// rather than left as a gap.
//
// Its field is `pub id: Option<String>` on `Room` (`state/room.rs:77`) -- a
// name too generic to needle. `pub id:` matches `RoomItem::id`, and every other
// struct that will ever have one; the test would fail on unrelated code and be
// deleted or weakened, which is worse than not having it.
//
// What makes this acceptable is that the room is already structurally
// single-owner: `GameState::apply` replaces the WHOLE `Room` on
// `Frame::RoomId`, so a second copy could not stay consistent for a single
// frame and would be found immediately rather than after ten months. The
// countdown bug Rule 4.3 exists for is specifically a value that DRIFTS
// silently, and this one cannot.
//
// If `Room` ever gains a distinctively-named id field, write the test then.
