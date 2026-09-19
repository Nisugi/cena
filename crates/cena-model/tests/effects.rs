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

    assert_eq!(state.vitals.get("mana"), Some(&93), "the vital landed");
    assert!(
        !state.vitals.contains_key("515"),
        "the effect must NOT be a vital -- both are progressBars and only the \
         enclosing dialog id tells them apart"
    );
    assert!(state.effects.get("515").is_some());
}
