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
/// # A function parameter is NOT an owning field, and this used to miss that
///
/// This doc claimed *"the leading whitespace of a struct body"* distinguished a
/// field from a parameter. It does not: `scan_lines` **trims** the line before
/// the filter sees it, so an indented parameter and an indented field are the
/// same string. MEASURED -- `travel/desk.rs:201` is
/// `        handle: SessionHandle,`, a parameter of `fn walk`, and it counted
/// as an owner.
///
/// That is a false positive on the highest-value test in the suite, and a
/// false positive is not harmless here: an arch test that cries wolf gets
/// suppressed, and this is the one guarding the defect that cost Vellum 49.8%
/// of its countdowns.
///
/// So the enclosing item is checked. `struct`/`union` bodies own fields;
/// `fn` signatures do not.
///
/// `scan_lines` strips comments, so prose naming a field does not register.
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
        .filter(|hit| in_a_struct_body(&sources, hit))
        .filter(|hit| !is_test_code(hit))
        // **This test file names its own needles**, and so would any other
        // arch test. Without this the suite counts itself and every needle has
        // at least two hits -- which is how the first version of this file
        // failed, reporting three owners where there is one.
        .filter(|hit| !hit.starts_with("crates/cena-arch-tests/"))
        .collect()
}

/// Whether a hit is in test code rather than in the shipped build.
///
/// # The rule is about production, and its own words say so
///
/// *"A second stored handle is a second way to reach a connection, and they go
/// stale independently."* That is a hazard about two long-lived owners in a
/// running client. A test harness that holds a handle for the duration of one
/// test -- `cena-behavior/tests/travel_desk.rs`'s `Playing`, which exists to
/// drive a desk and then drop -- is not that: it is constructed, used and
/// dropped inside a function whose whole job is to exercise the one real
/// owner.
///
/// Vellum's defect is the measure. A duplicate `server_time_offset` survived
/// ten months because both fields **shipped** and only one was maintained. A
/// field in a `tests/` directory does not ship.
///
/// **Narrowly scoped deliberately.** This skips `tests/` directories and
/// `#[cfg(test)]` is not consulted, because an inline test module sits inside a
/// production file and excluding by path is the honest, checkable line. A
/// duplicate field in `src/` still fails however it is annotated.
fn is_test_code(hit: &str) -> bool {
    hit.contains("/tests/") || hit.contains("/benches/")
}

/// Whether a hit's line sits inside a `struct`/`union` body rather than a `fn`
/// signature.
///
/// Walks **backwards** to the nearest enclosing item keyword. Crude, like the
/// rest of this file and for the same stated reason: a duplicate field is a
/// lexical fact, so a lexical test catches it. A parser would be a better tool
/// and a worse fit for a rule whose whole value is that it cannot be argued
/// with.
fn in_a_struct_body(sources: &[(std::path::PathBuf, String)], hit: &str) -> bool {
    let mut parts = hit.splitn(3, ':');
    let (Some(path), Some(line)) = (parts.next(), parts.next()) else {
        return true;
    };
    let Ok(line) = line.parse::<usize>() else {
        return true;
    };
    let Some((_, text)) = sources
        .iter()
        .find(|(candidate, _)| cena_arch_tests::harness::relative(candidate) == path)
    else {
        // Unknown file: keep the hit. A detector that drops what it cannot
        // classify would let a real duplicate through silently.
        return true;
    };
    let lines: Vec<&str> = text.lines().collect();
    for above in lines[..line.saturating_sub(1).min(lines.len())]
        .iter()
        .rev()
    {
        let code = above.trim_start();
        if code.starts_with("struct ") || code.starts_with("union ") {
            return true;
        }
        if code.starts_with("fn ")
            || code.starts_with("pub fn ")
            || code.starts_with("pub(crate) fn ")
            || code.starts_with("pub(super) fn ")
            || code.starts_with("async fn ")
            || code.starts_with("pub async fn ")
        {
            return false;
        }
    }
    true
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
