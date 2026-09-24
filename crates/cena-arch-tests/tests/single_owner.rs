//! Rule 4.3 — **one owning field per shared value** (`plan/05:376-383`).
//!
//! # Why this is the highest-value test in the reference suite
//!
//! `VellumFE` carried a **duplicate `server_time_offset` field that nothing
//! assigned for ten months**. Every countdown read the stale copy: 49.8% of
//! 6,373 measured countdowns were wrong, and the bug was invisible because both
//! fields existed, both compiled, and only one was maintained.
//!
//! The mechanism is deliberately crude -- a scan for a field declaration --
//! and that is the point. A duplicate field is a *lexical* fact, so a lexical
//! test catches it, where no type or trait could.
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
//! Each test asserts the count **and the owning path**. Ported from
//! `reference/VellumFE/tests/architecture.rs:337-358`, whose comment explains
//! the second: the count alone passes if the field is *relocated* to a crate
//! that has no business owning it, because one hit is still one hit. The path
//! assertion is what makes a silent move fail.
//!
//! # Fields are found by STRUCTURE, not by a needle (review findings 1 and 2)
//!
//! The first version matched a trimmed line that STARTED with a needle --
//! `handle: SessionHandle,`, `game_time:`, `pub roundtime_ends:` -- and then
//! walked upward to the nearest line starting `struct ` or `fn `. Three holes,
//! all live or one keystroke from it:
//!
//! 1. **Visibility defeated the needle.** `pub(crate) handle: SessionHandle,`
//!    does not start with `handle:`. MEASURED: `crates/cena-web/src/server.rs`
//!    line 31 is exactly that, a second stored handle, and the suite was
//!    green. The same hole let a `pub game_time` or a private
//!    `roundtime_ends` copy through.
//! 2. **A name is not the value.** The handle needle named the field; a
//!    `conn: SessionHandle` is the same second handle under another name.
//! 3. **`pub struct` was not a struct.** The walk-up knew `struct ` and
//!    `union ` only, so above `pub struct Copy { pub roundtime_ends: .. }` it
//!    kept going to the nearest `fn` and DROPPED the hit.
//!
//! `cena_arch_tests::structure::outline` answers "is this a field, of what"
//! with a scope stack, so all three close together: a field is a field
//! whatever its visibility, the handle is matched by TYPE, and the container is
//! whatever opened the scope.

use cena_arch_tests::harness::{relative, workspace_sources};
use cena_arch_tests::lexical::{TokenKind, tokens};
use cena_arch_tests::structure::{Field, outline};
use std::path::PathBuf;

/// Every field in `sources` matching `wanted`, as `path:line: Container.name: Type`.
///
/// Skips this crate -- whose fixtures below declare the very fields the rules
/// look for -- and test code (see [`is_test_code`]).
fn owning_fields(sources: &[(PathBuf, String)], wanted: impl Fn(&Field) -> bool) -> Vec<String> {
    let mut hits = Vec::new();
    for (path, text) in sources {
        let rel = relative(path);
        if rel.starts_with("crates/cena-arch-tests/") || is_test_code(&rel) {
            continue;
        }
        for field in outline(text).fields.iter().filter(|f| wanted(f)) {
            hits.push(format!(
                "{rel}:{}: {}.{}: {}",
                field.line, field.container, field.name, field.ty
            ));
        }
    }
    hits
}

/// Whether a path is test code rather than the shipped build.
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
fn is_test_code(rel: &str) -> bool {
    rel.contains("/tests/") || rel.contains("/benches/")
}

/// Whether a field's type STORES a `SessionHandle`, as opposed to borrowing
/// one.
///
/// `SessionHandle`, `Option<SessionHandle>`, `Vec<cena_session::SessionHandle>`
/// all store it; `&SessionHandle`, `&'a SessionHandle` and `&mut SessionHandle`
/// borrow it, and a borrow is not ownership -- which is why the many
/// `handle: &SessionHandle` parameters in behavior code never counted, and why
/// a struct holding a reference does not either.
fn stores_a_handle(field: &Field) -> bool {
    let toks = tokens(&field.ty);
    toks.iter().enumerate().any(|(k, t)| {
        if !(t.kind == TokenKind::Ident && t.text == "SessionHandle") {
            return false;
        }
        // Step back over a `path::` prefix, then an optional `mut` and
        // lifetime, to whatever precedes the type.
        let mut j = k;
        while j >= 3 && toks[j - 1].is(":") && toks[j - 2].is(":") {
            j -= 3;
        }
        if j >= 1 && toks[j - 1].is("mut") {
            j -= 1;
        }
        if j >= 1 && toks[j - 1].kind == TokenKind::Lifetime {
            j -= 1;
        }
        !(j >= 1 && toks[j - 1].is("&"))
    })
}

#[test]
fn roundtime_has_a_single_owning_field() {
    // **The value Vellum's bug was actually about.** A second `roundtime_ends`
    // on `SessionActor`, or a cached copy in a behavior, is the exact shape
    // that made half of Vellum's countdowns wrong -- and a gated send reading
    // the stale one fires EARLY, which `plan/19` §1a records as the roundtime
    // defect that "acts" rather than merely displaying wrong.
    let hits = owning_fields(&workspace_sources(), |f| f.name == "roundtime_ends");
    assert_eq!(
        hits.len(),
        1,
        "exactly one struct may own a roundtime_ends field (GameState), \
         whatever its visibility. A second copy is the Vellum bug: both \
         compile, one is maintained, and a gated send reading the stale one \
         fires early. Found:\n{}",
        hits.join("\n")
    );
    assert!(
        hits[0].contains("cena-model/src/state.rs") && hits[0].contains("GameState."),
        "roundtime must stay on GameState in cena-model -- a copy anywhere \
         above it is a second source of truth for the same fact. Found at {}",
        hits[0]
    );
}

#[test]
fn the_server_clock_has_a_single_owning_field() {
    // Cena's equivalent of Vellum's `server_time_offset`. Private on
    // GameState, read through `game_time_now()` -- and matched by NAME with
    // any visibility, so a `pub game_time` copy elsewhere counts too.
    let hits = owning_fields(&workspace_sources(), |f| f.name == "game_time");
    assert_eq!(
        hits.len(),
        1,
        "exactly one struct may own a game_time field (GameState). This is the \
         value whose duplicate cost Vellum 49.8% of 6,373 countdowns. \
         Found:\n{}",
        hits.join("\n")
    );
    assert!(
        hits[0].contains("cena-model/src/state.rs") && hits[0].contains("GameState."),
        "the server clock must stay on GameState. Found at {}",
        hits[0]
    );
}

/// The one struct that OWNS the session handle: it is built beside the actor.
const HANDLE_OWNER: (&str, &str) = ("crates/cena-session/src/actor/handle.rs", "Session");

/// Long-lived HOLDERS of a cloned `SessionHandle`, beyond the owner, each with
/// why it cannot go stale independently.
///
/// An allowlist, like `ALLOWED_STATICS`: a new holder is a reviewed entry, not
/// a needle miss.
const HANDLE_HOLDERS: &[(&str, &str, &str)] = &[
    (
        "crates/cena-web/src/server.rs",
        "Viewed",
        "The embedded frontend's manual-input surface, one per served session \
     since plan/29 step 5 (it was one `Shared` for the only session; the \
     reasoning below is unchanged by the move). Found by review finding 1 \
     (it evaded the old needle by being `pub(crate)`), and allowed rather than \
     reported, on the type's own terms: SessionHandle is documented as \
     'Cloneable, so a behavior and the manual-input surface hold the same \
     handle and their commands therefore go through the same queue' \
     (crates/cena-session/src/command/handle.rs:165-168), and every one of \
     its fields is a channel sender or an Arc-shared slot, cell or \
     publisher -- so a \
     clone cannot drift from the original, which is the hazard Rule 4.3 \
     exists for (plan/05:376-383). Sends are generation-pinned \
     (cena-web/src/socket.rs, send_manual_at), so a stale view is refused by \
     the session rather than delivered. M4 decision D2 gives cena-web this \
     edge (layering.rs ALLOWED_EDGES). `the_handle_premise_holds` checks the \
     'every field is shared' half mechanically.",
    ),
    (
        "crates/cena-host/src/table.rs",
        "Hosted",
        "The session table's entry for one running session (plan/29 step 3). \
         It is the handle `SupervisedSession::numbered` returned, kept for the \
         session's whole life in the table, and it cannot outlive the \
         connection it reaches: `Host::take` removes the entry and \
         `Hosted::stop` consumes it. The same premise as `Viewed` above holds \
         -- every field of a SessionHandle is a channel sender or an \
         Arc-shared slot, so a frontend's clone taken from this entry cannot \
         drift from it -- and sends from a frontend stay generation-pinned. \
         The cena-host row of layering.rs ALLOWED_EDGES gives it the edge.",
    ),
];

#[test]
fn the_session_handle_has_a_single_owning_field() {
    // The fourth value in the deferral. A second stored `SessionHandle` is how
    // a command reaches a connection its holder has already been told is gone
    // -- the shape review finding SE-1 turned out to be.
    //
    // Matched by TYPE: any field whose type stores a SessionHandle, whatever
    // the field is called and whatever its visibility.
    let hits = owning_fields(&workspace_sources(), stores_a_handle);
    let (owner_path, owner) = HANDLE_OWNER;
    let is = |hit: &str, path: &str, container: &str| {
        hit.starts_with(&format!("{path}:")) && hit.contains(&format!(" {container}."))
    };
    let owners: Vec<&String> = hits.iter().filter(|h| is(h, owner_path, owner)).collect();
    assert_eq!(
        owners.len(),
        1,
        "the owning SessionHandle field must stay on `{owner}` in \
         {owner_path}. Found:\n{}",
        hits.join("\n")
    );
    let unreviewed: Vec<&String> = hits
        .iter()
        .filter(|h| !is(h, owner_path, owner))
        .filter(|h| !HANDLE_HOLDERS.iter().any(|(p, c, _)| is(h, p, c)))
        .collect();
    assert!(
        unreviewed.is_empty(),
        "a struct stores a SessionHandle and is neither its owner nor a \
         reviewed holder. A second stored handle is a second way to reach a \
         connection. If this one is a clone that cannot go stale, add it to \
         HANDLE_HOLDERS and say why; otherwise borrow the handle.\n{}",
        unreviewed
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    for (path, container, why) in HANDLE_HOLDERS {
        assert!(
            hits.iter().any(|h| is(h, path, container)),
            "HANDLE_HOLDERS names {container} in {path}, which no longer holds \
             a handle: delete the entry, or it will excuse a future one unseen"
        );
        assert!(why.len() > 200 && why.contains(':'), "{container}: {why:?}");
    }
}

/// The premise `HANDLE_HOLDERS` rests on: a clone of `SessionHandle` shares
/// all of its state, so it cannot go stale on its own.
///
/// If `SessionHandle` gains a plain-value field -- a cached generation, a copy
/// of anything -- then two clones CAN disagree, the holder entry's argument is
/// false, and this fails before the allowance quietly outlives its reason.
#[test]
fn the_handle_premise_holds() {
    let path = cena_arch_tests::harness::workspace_root()
        .join("crates/cena-session/src/command/handle.rs");
    let text = std::fs::read_to_string(&path).expect("SessionHandle's file");
    let fields: Vec<Field> = outline(&text)
        .fields
        .into_iter()
        .filter(|f| f.container == "SessionHandle")
        .collect();
    assert!(
        fields.len() >= 3,
        "found {} SessionHandle fields; the struct moved and this premise \
         check is vacuous",
        fields.len()
    );
    // `EventPublisher` is itself only broadcast senders, `Arc`s, a
    // `GenerationCell` and the session's immutable id (observation.rs,
    // `struct EventPublisher`), so a clone of it is shared the same way. It
    // replaced a bare `broadcast::Sender` while this check was being written,
    // which is the case for naming it rather than widening the predicate.
    let shared = |ty: &str| {
        ty.contains("Sender<")
            || ty.ends_with("GenerationCell")
            || ty.ends_with("::Slot")
            || ty.ends_with("::EventPublisher")
    };
    let copied: Vec<String> = fields
        .iter()
        .filter(|f| !shared(&f.ty))
        .map(|f| format!("{}: {}", f.name, f.ty))
        .collect();
    assert!(
        copied.is_empty(),
        "SessionHandle gained a field that is not a channel sender or a \
         shared slot, so two clones can now disagree -- and HANDLE_HOLDERS' \
         justification is false. Either make it shared, or revisit every \
         holder: {copied:?}"
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

// ---------------------------------------------------------------------------
// Mutations. Each fixture is a shape the PREVIOUS version of this file passed
// green; the old needle's miss is asserted beside the new detector's hit, so
// the fixture cannot silently stop exercising the distinction.
// ---------------------------------------------------------------------------

/// Run the detector over one fixture as if it were a production file.
fn fixture_hits(text: &str, wanted: impl Fn(&Field) -> bool) -> Vec<String> {
    let path = cena_arch_tests::harness::workspace_root().join("crates/cena-fixture/src/f.rs");
    owning_fields(&[(path, text.to_owned())], wanted)
}

/// Finding 1: a second handle under `pub(crate)`, or under another name, is
/// still a second handle -- the `crates/cena-web/src/server.rs` shape.
#[test]
fn a_restricted_or_renamed_handle_field_is_found() {
    let fixture = "pub(crate) struct Shared {\n    pub(crate) handle: SessionHandle,\n}\n\
                   struct Other {\n    conn: Option<cena_session::SessionHandle>,\n}\n";
    // The old test: a trimmed line had to START with the needle.
    let old_needle = "handle: SessionHandle,";
    assert!(
        !fixture.lines().any(|l| l.trim().starts_with(old_needle)),
        "the fixture no longer defeats the old needle, so it proves nothing"
    );
    let hits = fixture_hits(fixture, stores_a_handle);
    assert_eq!(hits.len(), 2, "{hits:?}");
}

/// Finding 1, the clock and roundtime halves: visibility does not hide a copy.
#[test]
fn a_clock_copy_is_found_whatever_its_visibility() {
    let fixture =
        "struct Cache {\n    pub game_time: Option<u32>,\n    roundtime_ends: Option<u32>,\n}\n";
    assert!(
        !fixture
            .lines()
            .any(|l| l.trim().starts_with("pub roundtime_ends:")),
        "the fixture no longer defeats the old roundtime needle"
    );
    assert_eq!(fixture_hits(fixture, |f| f.name == "game_time").len(), 1);
    assert_eq!(
        fixture_hits(fixture, |f| f.name == "roundtime_ends").len(),
        1
    );
}

/// Finding 2: `pub struct` with a `fn` above it. The old walk-up did not know
/// `pub struct`, reached `fn f`, and dropped the hit.
#[test]
fn a_field_of_a_pub_struct_below_a_fn_is_found() {
    let fixture =
        "fn f() {}\n\npub struct Copy<T> where T: Clone {\n    pub roundtime_ends: Option<T>,\n}\n";
    let hits = fixture_hits(fixture, |f| f.name == "roundtime_ends");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].contains("Copy.roundtime_ends"), "{hits:?}");
}

/// The false positives the old walk-up existed to prevent must stay excluded:
/// a parameter, a borrow, and a construction site are not owning fields.
#[test]
fn parameters_borrows_and_literals_are_not_owners() {
    let fixture = "fn walk(\n    handle: SessionHandle,\n) {\n    let s = Shared { handle };\n}\n\
                   struct View<'a> {\n    handle: &'a SessionHandle,\n    other: &mut SessionHandle,\n}\n";
    let hits = fixture_hits(fixture, stores_a_handle);
    assert!(hits.is_empty(), "{hits:?}");
}
