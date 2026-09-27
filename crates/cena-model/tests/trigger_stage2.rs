//! `plan/45` Stage 2: what the model reads a line as (`event`), `only_if`,
//! the flag, and a condition's edges.
//!
//! Every wire line is one the classifier's own tests already read, so the
//! input reaches the code it claims to test: speech and whispers from
//! `message.rs`, the attacks from `combat_attack_lines.rs` and
//! `combat_fsm_inbound.rs`, the ghast from `vanished_unaccounted.rs`.

use cena_model::GameState;
use cena_model::guard::{Condition, Facts};
use cena_model::line::Line;
use cena_model::state::chunks::ChunkLine;
use cena_model::state::flags::{FlagChange, Until};
use cena_model::trigger::{Edges, Flag, LineEvent, Matcher, Pattern, Rule, Trigger};
use cena_protocol::Parser;

const PUKK: &str = concat!(
    r#"<preset id='speech'><a exist="-10007833" noun="Pukk">Pukk</a> says</preset>, "#,
    r#""Bards have it easy in the arena.""#,
);
const CALVIX: &str = concat!(
    r#"<preset id="whisper"><a exist="-10807620" noun="Calvix">Calvix</a> whispers,</preset> "#,
    r#""Vellum is really cool""#,
);
const MINE: &str =
    "You swing a sword at <pushBold/>a <a exist=\"1\" noun=\"kobold\">kobold</a><popBold/>!";
const AT_ME: &str = "<pushBold/><a exist=\"31038708\" noun=\"champion\">A muscular tattooed champion</a><popBold/> swings a dagger at you!";
const THEIRS: &str = "As Heavenscent attempts to strike with her star, a surge of power flows out of it, through Heavenscent, and leaps out at <pushBold/><a exist=\"555001\" noun=\"shield-maiden\">a brawny gigas shield-maiden</a><popBold/>!";
const FLEES: &str = concat!(
    "<pushBold/>A <a exist=\"8365650\" noun=\"ghast\">cadaverous tatterdemalion ghast</a>",
    "<popBold/> leans back on <pushBold/><a exist=\"8365650\" noun=\"ghast\">his</a>",
    "<popBold/> haunches and bounds <d>southwest</d>.",
);
const SILENCED: &str = "A pall of silence settles over you.";
const REACTION: &str = "You could use this opportunity to <d cmd='WEAPON TACKLE #77'>tackle</d>!";
const IDLE: &str = "YOU HAVE BEEN IDLE TOO LONG. PLEASE RESPOND.";

/// One wire line, through the parser, as the chunk holds it.
fn chunk_line(wire: &str) -> ChunkLine {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(format!("{wire}\n").as_bytes()) {
        state.apply(&frame);
    }
    state
        .open_chunk()
        .lines()
        .first()
        .cloned()
        .unwrap_or_default()
}

/// The same line as the session publishes it, on the main stream.
fn line(wire: &str) -> Line {
    Line::new("", chunk_line(wire).runs)
}

/// A state that has seen these wire lines, each ended by a prompt at the
/// game second given.
fn state_after(lines: &[(&str, u32)]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for (wire, second) in lines {
        let prompt = format!("{wire}\n<prompt time=\"{second}\">&gt;</prompt>\n");
        for frame in parser.push_bytes(prompt.as_bytes()) {
            state.apply(&frame);
        }
    }
    state
}

fn hidden(on: bool) -> String {
    let visible = if on { "y" } else { "n" };
    format!("<indicator id=\"IconHIDDEN\" visible=\"{visible}\"/>")
}

fn guards(written: &str) -> Vec<Condition> {
    Condition::parse_group(written).unwrap_or_default()
}

fn flag(name: &str) -> Flag {
    Flag {
        name: name.into(),
        seconds: None,
        clear: false,
    }
}

fn trigger(name: &str, rule: Rule) -> Trigger {
    Trigger {
        name: name.into(),
        rule,
    }
}

fn words(text: &str) -> Pattern {
    Pattern::Literal {
        text: text.into(),
        whole_word: true,
    }
}

#[test]
fn each_event_reads_the_lines_its_classifier_reads_and_no_others() {
    let cases: [(&str, &str, &str); 12] = [
        ("speech", PUKK, CALVIX),
        ("whisper", CALVIX, PUKK),
        ("my_attack", MINE, AT_ME),
        ("my_attack", MINE, THEIRS),
        ("attacked", AT_ME, MINE),
        ("their_attack", THEIRS, MINE),
        ("departure", FLEES, MINE),
        ("affliction", SILENCED, IDLE),
        ("affliction silenced", SILENCED, IDLE),
        ("incident", REACTION, SILENCED),
        ("incident weapon_reaction", REACTION, SILENCED),
        ("idle_warning", IDLE, SILENCED),
    ];
    for (word, is, is_not) in cases {
        let event = LineEvent::parse(word).unwrap();
        assert_eq!(event.to_string(), word, "written back as read");
        let yes = chunk_line(is);
        assert!(event.reads(&yes, &yes.text()), "{word}: {}", yes.text());
        let no = chunk_line(is_not);
        assert!(!event.reads(&no, &no.text()), "{word}: {}", no.text());
    }
    // A name narrows its word: the silence is not a binding, and the
    // weapon reaction is not a rooting.
    let silence = chunk_line(SILENCED);
    let bound = LineEvent::parse("affliction bound").unwrap();
    assert!(!bound.reads(&silence, &silence.text()));
    let reaction = chunk_line(REACTION);
    let rooted = LineEvent::parse("incident rooted").unwrap();
    assert!(!rooted.reads(&reaction, &reaction.text()));
}

#[test]
fn an_event_alone_hits_the_whole_line_and_beside_words_narrows_them() {
    let matcher = Matcher::new(vec![
        trigger(
            "any speech",
            Rule {
                event: LineEvent::parse("speech").ok(),
                squelch: true,
                ..Rule::default()
            },
        ),
        trigger(
            "easy in speech",
            Rule {
                pattern: Some(words("easy")),
                event: LineEvent::parse("speech").ok(),
                squelch: true,
                ..Rule::default()
            },
        ),
        trigger(
            "easy anywhere",
            Rule {
                pattern: Some(words("easy")),
                squelch: true,
                ..Rule::default()
            },
        ),
    ])
    .unwrap();
    let said = line(PUKK);
    let text = said.text();
    let hits = matcher.screened(&said, &text, None);
    let found: Vec<(&str, &str)> = hits
        .iter()
        .map(|hit| {
            (
                matcher.triggers()[hit.trigger].name.as_str(),
                &text[hit.span.clone()],
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            ("any speech", text.as_str()),
            ("easy in speech", "easy"),
            ("easy anywhere", "easy"),
        ]
    );
    // The same words, not spoken: only the trigger with no event.
    let plain = line("It is easy to say.");
    let plain_text = plain.text();
    let names: Vec<&str> = matcher
        .screened(&plain, &plain_text, None)
        .iter()
        .map(|hit| matcher.triggers()[hit.trigger].name.as_str())
        .collect();
    assert_eq!(names, ["easy anywhere"]);
}

#[test]
fn only_if_holds_a_trigger_back_until_its_words_hold() {
    let matcher = Matcher::new(vec![trigger(
        "sneaky",
        Rule {
            pattern: Some(words("Bards")),
            only_if: guards("hidden"),
            flag: Some(flag("heard")),
            ..Rule::default()
        },
    )])
    .unwrap();
    let said = line(PUKK);
    // The game has not said whether I am hidden: unknown does not hold.
    assert!(
        matcher
            .answer(&said, &GameState::default())
            .fired
            .is_empty()
    );
    let seen = state_after(&[(&hidden(false), 10)]);
    assert!(matcher.answer(&said, &seen).fired.is_empty());
    let hiding = state_after(&[(&hidden(true), 10)]);
    assert_eq!(matcher.answer(&said, &hiding).fired, [0]);
    // With no character to read, the preview takes `only_if` to hold.
    assert_eq!(matcher.respond(&said).len(), 1);
}

#[test]
fn a_squelched_line_still_fires_its_triggers() {
    let matcher = Matcher::new(vec![
        trigger(
            "hide",
            Rule {
                pattern: Some(words("Bards")),
                squelch: true,
                ..Rule::default()
            },
        ),
        trigger(
            "note",
            Rule {
                pattern: Some(words("arena")),
                flag: Some(flag("arena")),
                ..Rule::default()
            },
        ),
    ])
    .unwrap();
    let answer = matcher.answer(&line(PUKK), &GameState::default());
    assert!(answer.lines.is_empty(), "squelched");
    assert_eq!(answer.fired, [0, 1]);
}

#[test]
fn a_flag_is_set_for_a_time_or_until_cleared_and_its_guard_word_reads_it() {
    let timed = Flag {
        name: "Rift".into(),
        seconds: Some(30),
        clear: false,
    };
    assert_eq!(
        timed.change(Some(100)),
        FlagChange {
            name: "Rift".into(),
            until: Some(Until::Second(130)),
        }
    );
    assert_eq!(timed.change(None).until, Some(Until::Unknown));
    let mut state = state_after(&[("", 100)]);
    assert!(state.flags.apply(&timed.change(Some(100))));
    assert!(!state.flags.apply(&timed.change(Some(100))), "no change");
    assert_eq!(
        state.flags.holds("rift", Some(129)),
        Some(true),
        "ignoring case"
    );
    assert_eq!(state.flags.holds("rift", Some(130)), Some(false), "run out");
    assert_eq!(state.flags.holds("rift", None), None);
    assert_eq!(state.flags.holds("other", None), Some(false));
    let read = |state: &GameState| {
        guards("flag \"rift\"")
            .first()
            .and_then(|word| word.holds(&Facts::new(state, None)))
    };
    assert_eq!(read(&state), Some(true), "at 100, set until 130");
    let cleared = Flag {
        name: "rift".into(),
        seconds: None,
        clear: true,
    };
    assert!(state.flags.apply(&cleared.change(Some(101))));
    assert_eq!(read(&state), Some(false));
}

/// Each condition that fires as `hidden` reads `on` at each game second.
fn conditions_over(rearm: u32, readings: &[(Option<bool>, u32)]) -> Vec<bool> {
    let matcher = Matcher::new(vec![trigger(
        "hid",
        Rule {
            condition: guards("hidden"),
            rearm,
            flag: Some(flag("hid")),
            ..Rule::default()
        },
    )])
    .unwrap_or_default();
    let mut edges = Edges::default();
    let mut parser = Parser::new();
    let mut state = GameState::default();
    let mut fired = Vec::new();
    for &(on, second) in readings {
        let wire = on.map_or_else(String::new, hidden);
        let prompt = format!("{wire}\n<prompt time=\"{second}\">&gt;</prompt>\n");
        for frame in parser.push_bytes(prompt.as_bytes()) {
            state.apply(&frame);
        }
        fired.push(!edges.fire(&matcher, &state).is_empty());
    }
    fired
}

#[test]
fn a_condition_fires_on_the_change_to_true_and_then_waits_out_its_rearm() {
    let t = Some(true);
    let f = Some(false);
    assert_eq!(
        conditions_over(
            3,
            &[
                (f, 100), // first reading: taken silently
                (t, 101), // false to true: fires
                (t, 102), // still true: silent
                (f, 103),
                (t, 104), // one second false, under the rearm: silent
                (f, 105),
                (t, 109), // four seconds false: fires
            ]
        ),
        [false, true, false, false, false, false, true]
    );
}

#[test]
fn a_condition_already_true_when_first_read_does_not_fire() {
    let (t, f) = (Some(true), Some(false));
    assert_eq!(
        conditions_over(3, &[(t, 100), (t, 101), (f, 102), (t, 103)]),
        [false, false, false, true],
        "the first rise after load fires at once: no rearm before a first fire"
    );
}

#[test]
fn a_rearm_of_zero_fires_at_every_rise() {
    let (t, f) = (Some(true), Some(false));
    assert_eq!(
        conditions_over(0, &[(f, 100), (t, 100), (f, 100), (t, 100)]),
        [false, true, false, true]
    );
}

#[test]
fn a_condition_the_game_has_not_answered_is_no_reading() {
    // `hidden` is unknown until an indicator states it: nothing is taken,
    // so the first stated reading is the silent one.
    let t = Some(true);
    assert_eq!(
        conditions_over(3, &[(None, 100), (t, 101), (t, 102)]),
        [false, false, false]
    );
}
