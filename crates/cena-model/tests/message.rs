//! Who said what.
//!
//! Every wire line here is **copied from `E:/Gemstone/dev/lich-5/logs`**, not
//! reconstructed — there is no Lich regex to reconstruct from, since Lich has
//! no speech classifier at all.

use cena_model::GameState;
use cena_model::state::message::Channel;
use cena_protocol::Parser;

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    for frame in parser.parse_line("<prompt time=\"1\">&gt;</prompt>") {
        state.apply(&frame);
    }
    state
}

/// Real wire, single-quoted preset.
const PUKK: &str = concat!(
    r#"<preset id='speech'><a exist="-10007833" noun="Pukk">Pukk</a> says</preset>, "#,
    r#""Bards have it easy in the arena.""#,
);

/// Real wire, **double-quoted** preset — the form that broke a first pass.
const CALVIX: &str = concat!(
    r#"<preset id="whisper"><a exist="-10807620" noun="Calvix">Calvix</a> whispers,</preset> "#,
    r#""Vellum is really cool""#,
);

#[test]
fn a_speech_line_yields_speaker_verb_and_body() {
    let state = state_after(&[PUKK]);
    let message = state.messages.all().first().expect("one message");
    assert_eq!(message.channel, Channel::Speech);
    assert_eq!(message.speaker_id.as_deref(), Some("-10007833"));
    assert_eq!(message.speaker.as_deref(), Some("Pukk"));
    assert_eq!(message.verb, "says");
    assert_eq!(message.body, "Bards have it easy in the arena.");
}

#[test]
fn a_double_quoted_preset_reads_the_same_as_a_single_quoted_one() {
    // **The bug a first measurement pass had.** Counting presets with a
    // single-quote-only pattern reported "192, all speech" and missed the
    // whisper lines entirely, which use double quotes. The wire uses both.
    let state = state_after(&[CALVIX]);
    let message = state.messages.all().first().expect("one message");
    assert_eq!(message.channel, Channel::Whisper);
    assert_eq!(message.speaker.as_deref(), Some("Calvix"));
    assert_eq!(message.body, "Vellum is really cool");
}

#[test]
fn the_two_bodies_forms_give_the_same_answer() {
    // MEASURED: 193 of 206 lines put `, ` between the preset and the quote,
    // and the rest go straight into it. Both are the same body.
    assert_eq!(
        state_after(&[PUKK]).messages.all()[0].body,
        "Bards have it easy in the arena."
    );
    assert_eq!(
        state_after(&[CALVIX]).messages.all()[0].body,
        "Vellum is really cool"
    );
}

#[test]
fn a_spell_emote_is_not_a_whisper() {
    // **THE FALSE POSITIVE THIS DESIGN AVOIDS**, and it is real wire:
    //
    //   <a exist="-10047921" noun="Wolfstarr">he</a> whispers a quiet invocation.
    //   <a exist="-11181000" noun="Naquen">Naquen</a> whispers arcane incantations, ...
    //
    // MEASURED: 523 lines pair a player link with `whispers` OUTSIDE any
    // preset, and they are spell-casting emotes. A text classifier matching
    // on the verb would report every cast in the room as a private message.
    //
    // The preset is what decides, so none of them are messages here.
    let state = state_after(&[
        r#"<a exist="-10047921" noun="Wolfstarr">he</a> whispers a quiet invocation."#,
        concat!(
            r#"<a exist="-11181000" noun="Naquen">Naquen</a> whispers arcane "#,
            r#"incantations, and the air about <a exist="-11181000" noun="Naquen">him</a> shimmers."#,
        ),
    ]);
    assert!(state.messages.is_empty(), "{:?}", state.messages.all());
}

#[test]
fn an_adverb_stays_in_the_verb() {
    // MEASURED: 14 distinct openers over 206 lines -- `says`, `asks`,
    // `exclaims`, and also `softly`, `squeakily`, `monotonously`, `darkly`.
    // The tail is adverbial and open, so the verb is text. The CHANNEL is
    // what is closed.
    let state = state_after(&[concat!(
        r#"<preset id='speech'><a exist="-1" noun="Pukk">Pukk</a> says softly</preset>, "#,
        r#""Quietly now.""#,
    )]);
    let message = &state.messages.all()[0];
    assert_eq!(message.verb, "says softly");
    assert_eq!(message.body, "Quietly now.");
}

#[test]
fn your_own_speech_has_no_speaker_link() {
    // MEASURED: 16 of the 206 carry no `exist` link, because the game writes
    // your own speech as `You say`. That is not a missing speaker -- it is
    // the game naming you the way it always does.
    let state = state_after(&[r#"<preset id='speech'>You say</preset>, "Hello.""#]);
    let message = &state.messages.all()[0];
    assert_eq!(message.speaker_id, None);
    assert_eq!(message.verb, "You say");
    assert_eq!(message.body, "Hello.");
}

#[test]
fn a_link_in_the_body_is_not_the_speaker() {
    // Real wire: `says, "Here are your winnings, <a exist=...>Ryeka</a>"`.
    // The speaker is inside the preset; a link after it names the LISTENER.
    // Taking the first link on the line would credit the wrong person.
    let state = state_after(&[concat!(
        r#"<preset id='speech'><a exist="-420891" noun="scripmaster">a scripmaster</a> "#,
        r#"says</preset>, "Here are your winnings, <a exist="-109" noun="Ryeka">Ryeka</a>.""#,
    )]);
    let message = &state.messages.all()[0];
    assert_eq!(message.speaker_id.as_deref(), Some("-420891"));
    assert!(message.body.contains("Ryeka"), "{}", message.body);

    // **The case above does not actually test the rule**, and a mutation
    // proved it: the speaker there is ALSO the first link on the line, so
    // taking the first link anywhere gives the same answer.
    //
    // This is the line that separates them -- the game names a bystander
    // before the speaker opens their mouth. Crediting the first link would
    // attribute the words to the wrong person, which is worse than losing
    // them.
    let state = state_after(&[concat!(
        r#"<a exist="-500" noun="Bystander">Bystander</a> nods as "#,
        r#"<preset id='speech'><a exist="-420891" noun="Pukk">Pukk</a> says</preset>, "Hello.""#,
    )]);
    let message = &state.messages.all()[0];
    assert_eq!(
        message.speaker_id.as_deref(),
        Some("-420891"),
        "the speaker is inside the preset, not first on the line"
    );
    assert_eq!(message.speaker.as_deref(), Some("Pukk"));
}

#[test]
fn an_unknown_preset_is_not_a_message() {
    // Not a catch-all for styled text. A preset the game adds later must not
    // arrive as speech nobody said.
    let state = state_after(&[r"<preset id='thought'>Something</preset> else"]);
    assert!(state.messages.is_empty());
}

#[test]
fn an_ordinary_line_is_not_a_message() {
    let state = state_after(&["You see a large rock."]);
    assert!(state.messages.is_empty());
}

mod history {
    use super::{CALVIX, Channel, PUKK, state_after};
    use cena_model::state::message::{MAX_MESSAGES, Message, Messages};

    #[test]
    fn messages_accumulate_in_order() {
        let state = state_after(&[PUKK, CALVIX]);
        let all = state.messages.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].speaker.as_deref(), Some("Pukk"), "oldest first");
        assert_eq!(all[1].speaker.as_deref(), Some("Calvix"));
    }

    #[test]
    fn the_last_on_a_channel_is_found() {
        let state = state_after(&[PUKK, CALVIX]);
        assert_eq!(
            state
                .messages
                .last_on(Channel::Speech)
                .and_then(|m| m.speaker.as_deref()),
            Some("Pukk")
        );
        assert_eq!(
            state
                .messages
                .last_on(Channel::Whisper)
                .and_then(|m| m.speaker.as_deref()),
            Some("Calvix")
        );
    }

    #[test]
    fn one_speakers_words_are_found_by_id() {
        let state = state_after(&[PUKK, CALVIX, PUKK]);
        assert_eq!(state.messages.from_speaker("-10007833").count(), 2);
        assert_eq!(state.messages.from_speaker("-99").count(), 0);
    }

    #[test]
    fn the_oldest_is_dropped_at_the_cap() {
        // Bounded for the reason `Chunk` is: a session in a busy town would
        // otherwise grow without limit.
        let mut held = Messages::default();
        for index in 0..MAX_MESSAGES + 5 {
            held.push(Message {
                channel: Channel::Speech,
                speaker_id: Some(index.to_string()),
                speaker: None,
                verb: "says".to_owned(),
                body: index.to_string(),
            });
        }
        assert_eq!(held.all().len(), MAX_MESSAGES);
        assert_eq!(held.all()[0].body, "5", "the first five are gone");
    }
}

#[test]
fn messages_survive_a_reconnect() {
    // **The contrast with the spell cooldowns, which are cleared.** A
    // cooldown is a claim about NOW and stops being true while we are away. A
    // message is a record that someone said something while we were
    // listening, and that stays true.
    //
    // Nothing re-sends them either -- the login burst carries no history --
    // so clearing would lose the only copy of something already observed.
    let mut state = state_after(&[PUKK]);
    assert!(!state.messages.is_empty(), "guard: heard first");
    state.invalidate_for_reconnect();
    assert_eq!(state.messages.all().len(), 1);
}
