//! Active effects, against the `incant 515` burst the author captured on
//! 2026-09-18.
//!
//! Split from `status_and_clock.rs` under `plan/05` Rule 4.1 when that file
//! passed the 400-line cap.
//!
//! **Every fixture here is real wire.** The ids, the two-category landing, the
//! `time='HH:MM:SS'` durations and the `clear='t'`-then-refill shape are all
//! from that capture -- which matters, because the model these test was
//! written against two references that disagreed until the capture settled it.

use cena_model::GameState;

/// The real `incant 515` burst, from the author's capture of 2026-09-18.
///
/// One action landed in **two** categories at once -- `Rapid Fire` in `Buffs`
/// and `Rapid Fire Recovery` in `Cooldowns` -- under different ids. That is
/// why effects live in one collection with the category as a field: a caller
/// asking "did 515 fire?" must not have to guess which dialog to look in.
#[test]
fn casting_a_spell_lands_in_buffs_and_cooldowns_together() {
    let burst = concat!(
        "<dialogData id='Cooldowns' clear='t'></dialogData><dialogData id='Cooldowns'>",
        r#"<progressBar id='37594784' value='100' text="Rapid Fire Recovery" left='22%' top='0' width='76%' height='15' time='00:02:00'/>"#,
        "</dialogData>",
        "<dialogData id='Buffs' clear='t'></dialogData><dialogData id='Buffs'>",
        r#"<progressBar id='515' value='100' text="Rapid Fire" left='22%' top='16' width='76%' height='15' time='00:01:59'/>"#,
        "</dialogData>\n",
    );

    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    // A prompt first: `ends_at` is derived from the server clock, and without
    // one there is no clock to derive against.
    for frame in parser.push_bytes(b"<prompt time=\"1789777252\">&gt;</prompt>\n") {
        state.apply(&frame);
    }
    for frame in parser.push_bytes(burst.as_bytes()) {
        state.apply(&frame);
    }

    let now = state.game_time_now().expect("a prompt arrived");
    assert_eq!(
        state.effects.active("515", now),
        Some(true),
        "the buff is up"
    );
    assert_eq!(
        state.effects.active("37594784", now),
        Some(true),
        "and the cooldown is running, from the SAME action"
    );
    assert_eq!(state.effects.remaining("515", now), Some(119));
    assert_eq!(
        state.effects.get("515").map(|e| e.text.as_str()),
        Some("Rapid Fire")
    );
    assert_eq!(
        state.effects.get("515").map(|e| e.category.as_str()),
        Some("Buffs")
    );

    // An id the game never mentioned is UNKNOWN, not inactive.
    assert_eq!(
        state.effects.active("999", now),
        None,
        "plan/12 §5.2: never reported is not the same as not active"
    );
}

/// **Presence is not liveness.** An effect whose timer has run out is still in
/// the dialog until the game bothers to re-send it.
///
/// Both references say so independently -- Lich's `active?` is `expiration >
/// Time.now`, and Vellum's comment notes "the protocol only re-sends effects
/// on change, so the `time` string goes stale". A model that answered from
/// presence would report an expired buff as up.
#[test]
fn an_expired_effect_is_still_listed_but_is_not_active() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(b"<prompt time=\"1789777252\">&gt;</prompt>\n") {
        state.apply(&frame);
    }
    let burst = concat!(
        "<dialogData id='Buffs'>",
        r#"<progressBar id='515' value='100' text="Rapid Fire" time='00:00:10'/>"#,
        "</dialogData>\n",
    );
    for frame in parser.push_bytes(burst.as_bytes()) {
        state.apply(&frame);
    }

    let at_cast = state.game_time_now().expect("clock");
    assert_eq!(state.effects.active("515", at_cast), Some(true));

    // Eleven seconds later the game has sent nothing new -- the effect is
    // still listed, and it is NOT active.
    let later = at_cast + 11;
    assert!(
        state.effects.get("515").is_some(),
        "the entry is still there: the game only re-sends on change"
    );
    assert_eq!(
        state.effects.active("515", later),
        Some(false),
        "but it has expired. Answering from presence would say it is up."
    );
    assert_eq!(state.effects.remaining("515", later), Some(0));
}

/// `clear='t'` empties one category and leaves the others alone.
///
/// MEASURED: the clear arrives as an EMPTY element immediately before the
/// populated one, and the four dialogs refill independently.
#[test]
fn a_category_refill_leaves_the_other_categories_alone() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(b"<prompt time=\"1789777252\">&gt;</prompt>\n") {
        state.apply(&frame);
    }
    let first = concat!(
        "<dialogData id='Buffs'><progressBar id='515' value='100' text=\"Rapid Fire\" time='00:01:59'/></dialogData>",
        "<dialogData id='Cooldowns'><progressBar id='37594784' value='100' text=\"Recovery\" time='00:02:00'/></dialogData>\n",
    );
    for frame in parser.push_bytes(first.as_bytes()) {
        state.apply(&frame);
    }
    assert_eq!(state.effects.len(), 2);

    // Refill Buffs with nothing -- the buff dropped off.
    for frame in parser.push_bytes(
        b"<dialogData id='Buffs' clear='t'></dialogData><dialogData id='Buffs'></dialogData>\n",
    ) {
        state.apply(&frame);
    }
    assert!(
        state.effects.get("515").is_none(),
        "the Buffs refill emptied it"
    );
    assert!(
        state.effects.get("37594784").is_some(),
        "a Buffs refill must say NOTHING about Cooldowns"
    );
}

/// Vitals and effects both arrive as `<progressBar>`; only the dialog id
/// separates them. An effect must not land in vitals.
#[test]
fn an_effect_bar_does_not_become_a_vital() {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(b"<prompt time=\"1789777252\">&gt;</prompt>\n") {
        state.apply(&frame);
    }
    let burst = concat!(
        "<dialogData id='minivitals'><progressBar id='mana' value='93' text='mana 380/405'/></dialogData>",
        "<dialogData id='Buffs'><progressBar id='515' value='100' text=\"Rapid Fire\" time='00:01:59'/></dialogData>\n",
    );
    for frame in parser.push_bytes(burst.as_bytes()) {
        state.apply(&frame);
    }

    assert_eq!(
        state.vitals.get("mana").map(|v| v.percent),
        Some(93),
        "the vital landed"
    );
    assert!(
        !state.vitals.contains_key("515"),
        "the effect must NOT be a vital -- both are progressBars and only the \
         enclosing dialog id tells them apart"
    );
    assert!(state.effects.get("515").is_some());
}

/// **A spell in two dialogs carries two ids, so keying by id does not collide.**
///
/// Review MO-2 reported that `Effects` keys by wire id across four independent
/// dialogs, so a spell listed in two of them would be re-tagged by whichever
/// refilled last and vanish from `in_category` for the other.
///
/// The author named the only real overlap — *"if it shows in both it would be
/// something buff + cooldown"* — and the wire settles it. MEASURED in
/// `2026-09-04_09-15-47.xml`:
///
/// ```text
/// Buffs      605       "Barkskin"
/// Cooldowns  19032922  "Barkskin"
/// ```
///
/// Same spell, two dialogs, **two different ids**: the buff's spell number and
/// the cooldown's own identifier. The game files them separately, so id alone
/// is a sufficient key.
///
/// Pinned because the argument runs the other way from the review's, and a
/// future reader deserves the evidence rather than the conclusion.
#[test]
fn a_buff_and_its_cooldown_are_separate_entries() {
    let mut state = cena_model::GameState::default();

    for frame in [
        prompt(1_789_775_821),
        buff_row("Buffs", "605", "Barkskin", 300),
        buff_row("Cooldowns", "19032922", "Barkskin", 300),
    ] {
        state.apply(&frame);
    }

    assert_eq!(
        state.effects.len(),
        2,
        "the buff and the cooldown are two facts and must both survive: {:?}",
        state.effects
    );
    assert_eq!(
        state.effects.get("605").map(|e| e.category.as_str()),
        Some("Buffs"),
        "the buff keeps its dialog"
    );
    assert_eq!(
        state.effects.get("19032922").map(|e| e.category.as_str()),
        Some("Cooldowns"),
        "and the cooldown keeps its own -- neither re-tags the other, because \
         the wire gave them different ids"
    );

    // A Cooldowns refill must not disturb the buff. This is MO-2's failure
    // mode, reached through the one overlap the author named.
    state.apply(&clear("Cooldowns"));
    state.apply(&buff_row("Cooldowns", "19032922", "Barkskin", 280));
    assert_eq!(
        state.effects.get("605").map(|e| e.category.as_str()),
        Some("Buffs"),
        "a Cooldowns refill must leave the Buffs entry alone: {:?}",
        state.effects
    );
}

/// A prompt, which teaches the server clock so `ends_at` can be stamped.
fn prompt(at: u32) -> cena_protocol::frame::Frame {
    cena_protocol::frame::Frame::Prompt {
        time: at.to_string(),
        text: ">".to_owned(),
    }
}

/// One `<progressBar>` inside a named effect dialog, as the wire sends it.
fn buff_row(dialog: &str, id: &str, text: &str, secs: u32) -> cena_protocol::frame::Frame {
    cena_protocol::frame::Frame::ProgressBar(cena_protocol::frame::ProgressBar {
        id: id.to_owned(),
        dialog: Some(dialog.to_owned()),
        text: text.to_owned(),
        percent: 100,
        amount: None,
        time_remaining_secs: Some(secs),
        attrs: Vec::new(),
    })
}

/// `<dialogData id=X clear='t'>`, the empty element that precedes a refill.
fn clear(dialog: &str) -> cena_protocol::frame::Frame {
    cena_protocol::frame::Frame::ClearDialogData {
        id: dialog.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// M3 step 10 — review MO-3: "gone" and "never seen" are different answers.
// ---------------------------------------------------------------------------

/// Drive wire text into a state that already has a server clock.
///
/// The prompt is not optional: `ends_at` is derived from the server clock, so
/// without one an effect's `time=` cannot become an absolute instant.
fn stated(wire: &str) -> (GameState, u32) {
    const NOW: u32 = 1_789_777_252;
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(
        b"<prompt time=\"1789777252\">&gt;</prompt>
",
    ) {
        state.apply(&frame);
    }
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    (state, NOW)
}

/// **The exact MO-3 scenario**, from the finding's own words:
///
/// > After `<dialogData id='Buffs' clear='t'>` plus a refill without 515,
/// > `active("515") == None` -- the same answer as never-observed.
///
/// The consequence it names is why this matters: *"A rebuff behavior following
/// §5.2 ('unknown -> error') can never learn 515 dropped; one reading `None` as
/// false recasts live spells in the window after a reconnect."* Two real
/// failures pulling opposite ways, which is why the answer must be
/// three-valued rather than defaulted either direction.
#[test]
fn an_effect_dropped_from_a_stated_list_is_known_gone() {
    let (state, now) = stated(concat!(
        "<dialogData id='Buffs' clear='t'></dialogData><dialogData id='Buffs'>",
        r#"<progressBar id='515' value='100' text="Rapid Fire" time='00:10:00'/>"#,
        "</dialogData>
",
    ));
    assert_eq!(
        state.effects.active_in("Buffs", "515", now),
        Some(true),
        "guard: 515 is listed and live"
    );

    // Clear, then refill WITHOUT it: the game has stated the complete list.
    let (state, now) = stated(concat!(
        "<dialogData id='Buffs' clear='t'></dialogData><dialogData id='Buffs'>",
        r#"<progressBar id='515' value='100' text="Rapid Fire" time='00:10:00'/>"#,
        "</dialogData>",
        "<dialogData id='Buffs' clear='t'></dialogData><dialogData id='Buffs'>",
        r#"<progressBar id='601' value='100' text="Natural Colors" time='00:10:00'/>"#,
        "</dialogData>
",
    ));

    assert_eq!(
        state.effects.active_in("Buffs", "515", now),
        Some(false),
        "the game just said 515 is not in Buffs -- knowledge, not a gap"
    );
    assert_eq!(
        state.effects.active_in("Buffs", "601", now),
        Some(true),
        "...and the one that IS listed is still live"
    );
    assert_eq!(
        state.effects.active("515", now),
        None,
        "the category-less accessor still cannot tell -- which is why          `active_in` exists rather than replacing it"
    );
}

/// An id in a category the game has never stated is still `None`.
///
/// The other half, and what stops this fix from becoming MO-4: it must not
/// answer `Some(false)` merely because an id is missing.
#[test]
fn an_unstated_category_answers_unknown() {
    let (state, now) = stated(
        "<dialogData id='Buffs' clear='t'></dialogData>
",
    );

    assert!(
        state.effects.saw_category("Buffs"),
        "guard: Buffs was stated"
    );
    assert!(!state.effects.saw_category("Cooldowns"));

    assert_eq!(
        state.effects.active_in("Buffs", "515", now),
        Some(false),
        "stated and absent"
    );
    assert_eq!(
        state.effects.active_in("Cooldowns", "515", now),
        None,
        "never stated -- a behavior must ask rather than assume"
    );
}

/// A plain refill, with no `clear='t'`, does NOT state a complete list.
///
/// MEASURED in a live capture: `Buffs` arrives 10 times with only 5 clears, so
/// half are incremental. An id missing from an incremental update says
/// nothing, and treating it as a statement would invent MO-3's opposite error.
#[test]
fn an_incremental_refill_states_nothing() {
    let (state, now) = stated(concat!(
        "<dialogData id='Buffs'>",
        r#"<progressBar id='601' value='100' text="Natural Colors" time='00:10:00'/>"#,
        "</dialogData>
",
    ));

    assert!(
        !state.effects.saw_category("Buffs"),
        "a refill without `clear='t'` is not a complete list"
    );
    assert_eq!(
        state.effects.active_in("Buffs", "515", now),
        None,
        "so an absent id remains unknown"
    );
}

/// `active_in` answers only for the category asked about.
///
/// The same spell can appear in two dialogs under different wire ids (the
/// module docs measure this), and the categories refill independently.
#[test]
fn a_category_answers_only_for_itself() {
    let (state, now) = stated(concat!(
        "<dialogData id='Buffs' clear='t'></dialogData><dialogData id='Buffs'>",
        r#"<progressBar id='515' value='100' text="Rapid Fire" time='00:10:00'/>"#,
        "</dialogData>
",
    ));

    assert_eq!(state.effects.active_in("Buffs", "515", now), Some(true));
    assert_eq!(
        state.effects.active_in("Cooldowns", "515", now),
        None,
        "515 is a Buffs id; Cooldowns has never been stated"
    );
}

/// An expired effect and an absent one both answer `Some(false)`.
///
/// Presence is not liveness -- the game leaves an expired effect in the dialog
/// until it bothers to re-send. Both paths to "not active" must agree, or a
/// caller has to know which one it is looking at.
#[test]
fn expired_and_absent_both_answer_false() {
    let (state, _) = stated(concat!(
        "<dialogData id='Buffs' clear='t'></dialogData><dialogData id='Buffs'>",
        r#"<progressBar id='515' value='0' text="Rapid Fire" time='00:00:01'/>"#,
        "</dialogData>
",
    ));

    let later = 1_789_777_252 + 600;
    assert_eq!(
        state.effects.active_in("Buffs", "515", later),
        Some(false),
        "listed but expired"
    );
    assert_eq!(
        state.effects.active_in("Buffs", "999", later),
        Some(false),
        "absent from a stated list -- same answer, as it must be"
    );
}

/// `clear` drops the observations with the effects.
///
/// Guard-before-clear. Keeping `observed` while emptying `by_id` would make
/// every id in the category answer a confident `Some(false)` from a list that
/// had been emptied rather than restated.
#[test]
fn clearing_effects_forgets_the_observations_too() {
    let (mut state, now) = stated(
        "<dialogData id='Buffs' clear='t'></dialogData>
",
    );
    assert!(state.effects.saw_category("Buffs"), "guard");

    state.effects.clear();

    assert!(!state.effects.saw_category("Buffs"));
    assert_eq!(
        state.effects.active_in("Buffs", "515", now),
        None,
        "forgetting the effects must forget that the list was ever stated"
    );
}

// ---------------------------------------------------------------------------
// Review: a timed effect with no clock is not an indefinite one.
// ---------------------------------------------------------------------------

/// **The login burst sends the effect dialogs before its first prompt**, and
/// the prompt is the only thing that teaches the clock. `ends_at` was
/// `game_time + secs`, so with no `game_time` it was stored `None` -- which
/// is how this model spells *indefinite*. A two-minute buff read as up
/// forever: `active()` `Some(true)` at any hour, `remaining()` `None`.
#[test]
fn an_effect_that_arrives_before_the_first_prompt_still_expires() {
    const PROMPT: u32 = 1_789_777_252;
    let mut state = GameState::default();
    state.apply(&buff_row("Buffs", "515", "Rapid Fire", 120));
    assert_eq!(
        state.effects.remaining("515", 0),
        Some(120),
        "before any clock: the duration the game stated, not `None`"
    );

    // The first prompt anchors it: the duration runs from THIS second.
    state.apply(&prompt(PROMPT));
    assert_eq!(
        state.effects.get("515").and_then(|e| e.ends_at),
        Some(PROMPT + 120)
    );
    assert_eq!(state.effects.active("515", PROMPT + 119), Some(true));
    assert_eq!(
        state.effects.active("515", PROMPT + 120),
        Some(false),
        "a timed buff must end"
    );
    assert_eq!(state.effects.remaining("515", PROMPT + 100), Some(20));
}

/// **Kept across a reconnect means the time LEFT is kept.** `ends_at` is an
/// absolute server second and the server's clock runs while the character is
/// offline, so keeping it unchanged charged the outage against the buff -- the
/// opposite of what `reconnect.rs` says it does (spells do not tick offline,
/// author 2026-09-20).
#[test]
fn a_reconnect_keeps_the_time_left_not_the_end_second() {
    const BEFORE: u32 = 1_789_777_252;
    const AFTER: u32 = BEFORE + 600; // a ten-minute outage
    let mut state = GameState::default();
    state.apply(&prompt(BEFORE));
    state.apply(&buff_row("Buffs", "515", "Rapid Fire", 120));
    state.apply(&prompt(BEFORE + 20)); // 100 seconds left at the drop

    state.invalidate_for_reconnect();
    assert_eq!(
        state.effects.remaining("515", 0),
        Some(100),
        "held as time left"
    );

    state.apply(&prompt(AFTER));
    assert_eq!(
        state.effects.active("515", AFTER + 99),
        Some(true),
        "the outage did not run the buff down"
    );
    assert_eq!(state.effects.active("515", AFTER + 100), Some(false));
}
