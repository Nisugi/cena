//! `plan/12` §5.2: what a reconnect forgets, and what it keeps.
//!
//! The fixtures are the shapes the live server actually sent, from Cena's own
//! captures of 2026-09-18 — the indicator burst, a real roundtime, a real
//! effect id, and the vitals the login burst carries.
//!
//! # Every test here sets the fact before clearing it
//!
//! A test that asserted `roundtime_ends == None` on a `GameState::default()`
//! would pass without `invalidate_for_reconnect` existing at all. So each
//! assertion is preceded by an assertion that the fact was **known**, which is
//! what makes the clearing the thing under test rather than the default.

use cena_model::{GameState, Room};
use cena_protocol::Frame;

/// Drive a burst through the real parser, as a live session would.
fn state_from(wire: &str) -> GameState {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

/// A session that knows everything, so that clearing it proves something.
fn a_fully_known_session() -> GameState {
    let mut state = state_from(concat!(
        "<nav rm='7503251'/>",
        "<compass><dir value=\"n\"/><dir value=\"e\"/></compass>",
        "<roundTime value='1789775824'/>",
        r#"<indicator id="IconSTANDING" visible="y"/>"#,
        r#"<indicator id="IconSTUNNED" visible="n"/>"#,
        "<left>a brass lantern</left><right>a steel broadsword</right>",
        "<progressBar id='health' value='97'/>",
        "<progressBar id='mana' value='42'/>",
        "<prompt time=\"1789775821\">R&gt;</prompt>\n",
    ));
    // An effect, in the shape the incant-515 capture carried.
    state.effects.insert(
        "515".to_owned(),
        cena_model::Effect {
            category: "Buffs".to_owned(),
            text: "Rapid Fire".to_owned(),
            ends_at: Some(1_789_776_000),
            percent: 100,
        },
    );
    // An unmodelled tag, criterion 8's evidence.
    state.apply(&Frame::UnknownTag {
        name: "somethingNew".to_owned(),
        raw: "<somethingNew/>".to_owned(),
    });
    state
}

/// The facts a new connection has not been told return to `Unknown`.
///
/// # This doc used to name the wrong rule, and two wrong facts
///
/// It read: *"the burst carries no `nav rm`, no `compass`, no `prompt`, no
/// hands, no `roundTime`, no `indicator`..."* -- and **hands and `indicator`
/// are both in the burst**. `indicator` was corrected in `reconnect.rs`
/// (review MO-12) and not here; hands were MEASURED 2026-09-20, arriving with
/// real contents (`plan/15` §2a.4a.3a).
///
/// The rule was wrong too, not just the facts:
///
/// > **AUTHOR, 2026-09-20:** *"The login burst is all the stuff needed to
/// > populate the ui on login. It doesn't mean delete stuff."*
///
/// The burst is a UI population message. What this test actually asserts is
/// `plan/12` §5.2's rule -- a fact that **could have changed while
/// disconnected** is not believed on the strength of the old connection. The
/// room can change (a character can be moved), spells tick down, the clock
/// drifts. Hands are cleared too, and that one is merely harmless rather than
/// necessary; see `reconnect.rs`.
#[test]
fn facts_the_burst_omits_return_to_unknown() {
    let mut state = a_fully_known_session();

    // GUARD: without this, the assertions below pass on a default state and
    // the method under test could be empty.
    assert_eq!(state.room.id.as_deref(), Some("7503251"));
    assert!(
        state.room.exits.as_ref().is_some_and(|e| !e.is_empty()),
        "exits were observed"
    );
    assert!(state.prompt.is_some(), "a prompt was observed");
    assert!(state.left_hand.is_some() && state.right_hand.is_some());
    assert_eq!(state.roundtime_ends, Some(1_789_775_824));
    assert!(state.status.standing(), "IconSTANDING was reported y");
    assert!(
        state.status.is_known("stunned"),
        "IconSTUNNED was reported n"
    );
    assert_eq!(state.effects.len(), 1);
    assert!(state.game_time_now().is_some(), "the clock was calibrated");

    state.invalidate_for_reconnect();

    assert_eq!(state.room, Room::default(), "the room goes whole");
    assert_eq!(state.prompt, None);
    assert_eq!(state.left_hand, None);
    assert_eq!(state.right_hand, None);
    // **RETAINED**, and this assertion was flipped deliberately. It read
    // `None`, encoding §5.2's "roundtime after reconnect is Unknown, never 0"
    // literally -- but clearing it produced the opposite of Unknown: a review
    // reproduced `in_roundtime() == Some(false)` with 20 real seconds left,
    // because `clock.rs`'s `is_some_and` maps a cleared end to "not in
    // roundtime" the moment a prompt restores the clock.
    //
    // `roundtime_ends` is an ABSOLUTE SERVER EPOCH, so unlike the room or the
    // hands it cannot have gone stale while the socket was down. Keeping it is
    // keeping a true fact; what §5.2 forbids is asserting "not in roundtime" on
    // no evidence, which the clock's own invalidation below still prevents.
    assert_eq!(
        state.roundtime_ends,
        Some(1_789_775_824),
        "an absolute server epoch survives a reconnect -- clearing it made          `in_roundtime` report `Some(false)` during a live roundtime"
    );
    assert!(state.effects.is_empty());
}

/// **Unknown is not `false`**, and `StatusInfo` is where that distinction is
/// easiest to get wrong.
///
/// `get` collapses "reported inactive" with "never reported"; only `is_known`
/// separates them. So asserting `!state.status.standing()` would pass on an
/// implementation that wrote `false` into every key rather than removing it —
/// which is a confident belief about a connection that has ended, exactly what
/// `plan/12` §5.2 forbids.
#[test]
fn cleared_indicators_are_unknown_rather_than_reported_false() {
    let mut state = a_fully_known_session();
    assert!(state.status.is_known("standing"), "guard: it was reported");

    state.invalidate_for_reconnect();

    assert!(!state.status.standing(), "it no longer reads true");
    assert!(
        !state.status.is_known("standing"),
        "...and it must be UNKNOWN, not known-false. A new generation has been \
         told nothing; writing `false` would be a belief nobody reported."
    );
}

/// The clock is per-connection, and **both halves** of it go.
///
/// This is the subtle one. `game_time_now` extrapolates from the local instant
/// the reading arrived, so a clock carried across a reconnect reports a server
/// time as far in the future as the outage was long — and `in_roundtime`
/// compares against it. A stale clock is worse than no clock.
#[test]
fn the_clock_does_not_survive_a_reconnect() {
    let mut state = a_fully_known_session();
    assert!(state.game_time_now().is_some(), "guard: it was calibrated");

    state.invalidate_for_reconnect();

    assert_eq!(state.game_time_now(), None, "the server clock is unknown");
    assert_eq!(
        state.in_roundtime(),
        None,
        "with no clock this is Unknown, NOT false -- a caller that wants to \
         treat unknown as 'go ahead' has to write that decision down"
    );
    assert_eq!(state.roundtime_remaining(), None);
}

/// Vitals survive, because the burst re-sends them unprompted.
///
/// MEASURED: ten `progressBar`s in every login burst. Clearing them would open
/// a window where health reads `Unknown` for no reason — the burst is about to
/// confirm them anyway.
#[test]
fn vitals_survive_because_the_burst_re_sends_them() {
    let mut state = a_fully_known_session();
    assert_eq!(state.vitals.get("health"), Some(&97));

    state.invalidate_for_reconnect();

    assert_eq!(
        state.vitals.get("health"),
        Some(&97),
        "the login burst carries ten progressBars (MEASURED, 7/7 logins), so \
         these are refreshed rather than unobserved"
    );
    assert_eq!(state.vitals.get("mana"), Some(&42));
}

/// Unknown tags survive: they are a fact about the protocol, not the socket.
///
/// Criterion 8's evidence, and it is most useful *across* a reconnect — a tag
/// that appeared before a drop is exactly what a reader needs when working out
/// why the session dropped.
#[test]
fn unknown_tags_survive_because_they_belong_to_the_session() {
    let mut state = a_fully_known_session();
    assert_eq!(state.unknown_tags.len(), 1, "guard: one was recorded");

    state.invalidate_for_reconnect();

    assert_eq!(
        state.unknown_tags.len(),
        1,
        "criterion 8's evidence belongs to the SESSION, not to one connection"
    );
    assert_eq!(state.unknown_tags[0].name, "somethingNew");
}

/// Invalidating twice is the same as once, and invalidating a fresh state is
/// harmless.
///
/// The supervisor calls this between generations; a reconnect that fails and
/// retries calls it again. It must not depend on having something to clear.
#[test]
fn invalidation_is_idempotent() {
    let mut once = a_fully_known_session();
    once.invalidate_for_reconnect();
    let mut twice = a_fully_known_session();
    twice.invalidate_for_reconnect();
    twice.invalidate_for_reconnect();
    assert_eq!(once, twice);

    let mut fresh = GameState::default();
    fresh.invalidate_for_reconnect();
    assert_eq!(
        fresh,
        GameState::default(),
        "clearing nothing changes nothing"
    );
}

// ---------------------------------------------------------------------------
// M3 step 9: the character model is invalidated PER GROUP.
// ---------------------------------------------------------------------------

/// **Stats survive a reconnect, because a reconnect does not change them.**
///
/// This is the step-9 correction. `invalidate_for_reconnect` did
/// `*character = Character::default()`, which was right when the struct held
/// only the four dialogs and became wrong the moment M3 added `stats`.
///
/// The change was INVISIBLE to the whole suite when it was made -- every test
/// here stayed green either way -- which is why this exists. Guard-before-
/// assert, so it cannot pass against a default.
#[test]
fn stats_survive_a_reconnect() {
    let mut state = GameState::default();
    let (line, bolded) = cena_model::StatLine::classify_with_bold(
        "   Strength (STR):   115 (32)    ...  115 (32)",
        &[],
    )
    .expect("the stat line should classify");
    state.character.stats.insert(
        line.kind,
        cena_model::InfoReport::merge_into(&line, bolded, cena_model::Stat::default()),
    );

    // Guard: the fact is known before anything clears it.
    assert!(
        state
            .character
            .stats
            .contains_key(&cena_model::StatKind::Strength),
        "guard: Strength must be known first"
    );

    state.invalidate_for_reconnect();

    let strength = state.character.stats.get(&cena_model::StatKind::Strength);
    assert!(
        strength.is_some(),
        "a reconnect does not change a character's Strength -- it was taught \
         by an `info` a person typed, and the burst was never going to \
         volunteer it"
    );
    assert_eq!(
        strength.and_then(|s| s.ascended).map(|v| v.value),
        Some(115)
    );
}

/// Race and profession survive too, for the same reason.
#[test]
fn identity_survives_a_reconnect() {
    let mut state = GameState::default();
    state.character.identity.race = Some("Half-Elf".to_owned());
    state.character.identity.profession = Some("Ranger".to_owned());

    assert!(state.character.identity.race.is_some(), "guard");

    state.invalidate_for_reconnect();

    assert_eq!(state.character.identity.race.as_deref(), Some("Half-Elf"));
    assert_eq!(
        state.character.identity.profession.as_deref(),
        Some("Ranger")
    );
}

/// The four dialogs still go, because the burst's silence about THEM means
/// the fact is unobserved.
///
/// The other half of the split: without this, "keep the character" would be
/// indistinguishable from "keep everything", and a stale `stance` would
/// survive a generation.
#[test]
fn the_dialog_facts_are_still_invalidated() {
    let mut state = GameState::default();
    state.character.stance = Some("offensive".to_owned());
    state.character.stance_percent = Some(0);
    state.character.encumbrance = Some("Heavy".to_owned());
    state
        .character
        .injuries
        .insert("head".to_owned(), cena_model::Injury { wound: 2, scar: 0 });
    state.character.experience.level = Some("Level 100".to_owned());

    // Guard: all five are known.
    assert!(state.character.stance.is_some(), "guard");
    assert!(state.character.encumbrance.is_some(), "guard");
    assert!(!state.character.injuries.is_empty(), "guard");
    assert!(state.character.experience.level.is_some(), "guard");

    state.invalidate_for_reconnect();

    assert_eq!(state.character.stance, None);
    assert_eq!(state.character.stance_percent, None);
    assert_eq!(state.character.encumbrance, None);
    assert!(state.character.injuries.is_empty());
    assert_eq!(state.character.experience.level, None);
}

/// **`shrouded` is cleared, and the reason is the effect list beside it.**
///
/// It is a spell, and `effects` is invalidated on the same path. A surviving
/// `shrouded = true` would make the new session refuse every `info` identity
/// on the strength of an effect nobody has re-observed -- a stale belief
/// silently suppressing good data, which is worse than the lie it guards
/// against.
#[test]
fn the_shroud_does_not_survive_its_own_effect_list() {
    let mut state = GameState::default();
    state.character.shrouded = true;
    assert!(state.character.shrouded, "guard");

    state.invalidate_for_reconnect();

    assert!(
        !state.character.shrouded,
        "the effect that sets this is itself invalidated; keeping it would \
         suppress the first good `info` of the new generation"
    );
}

/// **What persists and what survives a reconnect are the same set.**
///
/// Two layers make the same judgement independently: `CharacterSnapshot`
/// chooses what to write to disk, and `Character::invalidate_for_reconnect`
/// chooses what to keep across a generation. They must agree, because the
/// question is the same one -- *"was this taught by a command, or volunteered
/// by the connection?"*
///
/// `plan/12` §8's step 9 note says the `expr`-derived level must not survive.
/// It does not, on either path: cleared here, and absent from the snapshot.
/// This asserts the agreement rather than leaving it to coincidence, because
/// the two decisions live in different files and a later field could satisfy
/// one and not the other.
#[test]
fn persistence_and_reconnect_agree_on_what_a_command_taught() {
    let mut state = GameState::default();
    state.character.identity.race = Some("Half-Elf".to_owned());
    state.character.experience.level = Some("Level 100".to_owned());
    state
        .character
        .stats
        .insert(cena_model::StatKind::Strength, cena_model::Stat::default());

    state.invalidate_for_reconnect();

    // Survives a reconnect AND is persisted: taught by `info`.
    assert!(state.character.identity.race.is_some());
    assert!(!state.character.stats.is_empty());

    // Survives neither: `<dialogData id='expr'>` is pushed by the connection,
    // changes continuously, and `info`'s own level is explicitly not to be
    // trusted (`infomon/parser.rb:246`).
    assert_eq!(state.character.experience.level, None);

    // The persistence half of the same judgement: the snapshot has no
    // experience field at all, so the question cannot even be asked of it.
    // Asserted through `Group`, which is the vocabulary of what gets synced --
    // there is no `Group::Experience`, and adding one would be the moment to
    // revisit this test rather than to quietly widen it.
    //
    // Checked this way rather than by serialising, because `serde_json` is not
    // a `cena-model` dependency and adding one so a test can grep a string
    // would be a test reshaping the crate graph.
    assert!(
        cena_model::state::character::snapshot::Group::ALL
            .iter()
            .all(|g| !format!("{g:?}").eq_ignore_ascii_case("experience")),
        "a persisted `expr` level would be stale the moment it was written"
    );
}

/// **What the login burst actually carries, from a real burst.**
///
/// The claim this file and `reconnect.rs` both rested on -- that the burst
/// omits the hands -- was false, and neither could have caught it: the old
/// `login_burst.xml` fixture is a partial cut with no hands, no indicators and
/// no vitals in it at all. A fixture that cannot exhibit the fact cannot
/// refute a claim about it.
///
/// `login_burst_full.xml` is cut from `<app>` onward, so it can. What it shows
/// (MEASURED, and matching a second independent capture):
///
/// * hands arrive with **real contents**, not placeholders
/// * all ten indicators arrive in one bulk declaration
/// * `<spell>` arrives
///
/// # Why this test exists rather than a comment
///
/// > **AUTHOR, 2026-09-20:** *"The login burst is all the stuff needed to
/// > populate the ui on login. It doesn't mean delete stuff."*
///
/// The burst is a UI population message, so what is in it was never the right
/// basis for deciding what to forget. But the wrong rule was derived from two
/// wrong facts, and facts are the part a test can pin.
#[test]
fn the_login_burst_carries_the_hands() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/login_burst_full.xml");
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(&bytes) {
        state.apply(&frame);
    }

    assert!(
        state.left_hand.is_some(),
        "the burst carries the left hand -- the old table said it did not"
    );
    assert!(state.right_hand.is_some(), "and the right");
    assert!(
        state.status.is_known("standing"),
        "and all ten indicators, in one bulk declaration"
    );
}
