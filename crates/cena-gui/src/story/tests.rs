use super::*;
use cena_session::{Frame, SessionId};
use std::sync::Arc;

fn observed(generation: u32, event: Event) -> ObservedEvent {
    ObservedEvent {
        session: SessionId::FIRST,
        generation: Generation(generation),
        cursor: 1,
        event,
    }
}

fn said(stream: &str, text: &str) -> Event {
    Event::Line(Arc::new(cena_session::Line::new(
        stream,
        cena_session::ChunkLine::plain(text).runs,
    )))
}

/// A character who has seen the login burst's declarations: `speech` is a
/// copy of main (`ifClosed=''`), and `thoughts` falls to main in a style.
fn declared() -> GameState {
    let mut state = GameState::default();
    let window = |id: &str, attrs: &[(&str, &str)]| Frame::StreamWindow {
        id: id.into(),
        title: None,
        subtitle: None,
        attrs: attrs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    };
    state.apply(&window("speech", &[("id", "speech"), ("ifClosed", "")]));
    state.apply(&window(
        "thoughts",
        &[("id", "thoughts"), ("styleIfClosed", "thought")],
    ));
    state
}

fn texts(story: &Story) -> Vec<String> {
    story
        .lines
        .iter()
        .map(|(_, shown)| match shown {
            Shown::Game(runs) | Shown::From(_, runs) => {
                runs.iter().map(|run| run.text.as_str()).collect()
            }
            Shown::Typed { prompt, line } => format!("{prompt}{line}"),
            Shown::Prompt(prompt) => prompt.clone(),
            Shown::Gap => "(gap)".to_owned(),
        })
        .collect()
}

#[test]
fn a_stream_goes_where_the_game_declared() {
    let state = declared();
    let mut story = Story::default();
    for event in [
        said("", "You see a rock."),
        said("speech", "You say, \"hi\""),
        said("thoughts", "[General] hello"),
        said("familiar", "Your raven caws."),
    ] {
        story.hear(&observed(0, event), Some(&state));
    }
    assert_eq!(
        texts(&story),
        [
            "You see a rock.",
            "[General] hello",
            // Never declared: shown rather than lost on a guess.
            "Your raven caws.",
        ],
        "speech is a copy of main, so it is dropped"
    );
    let Some((_, Shown::From(stream, thought))) = story.lines.get(1) else {
        panic!("the thought is a line of its stream");
    };
    assert_eq!(stream, "thoughts");
    assert_eq!(
        thought[0].preset.as_deref(),
        Some("thought"),
        "in its style"
    );
}

#[test]
fn a_quiet_commands_report_stays_out_until_the_connection_changes() {
    let mut story = Story::default();
    story.hear(&observed(0, Event::Quiet(true)), None);
    story.hear(&observed(0, said("", "Name: Ashryn")), None);
    story.hear(
        &observed(0, said("thoughts", "[General] still heard")),
        None,
    );
    story.hear(&observed(1, said("", "after a reconnect")), None);
    assert_eq!(
        texts(&story),
        ["[General] still heard", "after a reconnect"]
    );
}

#[test]
fn hydra_speaks_in_its_own_pane_and_debug_is_not_shown() {
    let mut story = Story::default();
    story.hear(
        &observed(
            0,
            Event::Notice(Notice::line(NoticeKind::Info, "Hunt: resting.")),
        ),
        None,
    );
    story.hear(
        &observed(0, Event::Notice(Notice::line(NoticeKind::Debug, "tick"))),
        None,
    );
    assert!(story.lines.is_empty(), "not in the game's story");
    assert_eq!(story.said.len(), 1);
    assert_eq!(story.said[0].kind, NoticeKind::Info);
}

#[test]
fn banners_are_kept_a_while_and_only_the_newest_few() {
    let mut story = Story::default();
    let banner = |text: &str| {
        Event::Attention(Arc::new(cena_session::trigger::Attention {
            trigger: "t".to_owned(),
            sound: None,
            notify: None,
            alert: Some(text.to_owned()),
            cooldown: 0,
        }))
    };
    for n in 0..=MAX_ALERTS {
        story.hear(&observed(0, banner(&format!("banner {n}"))), None);
    }
    let now = Instant::now();
    let up: Vec<&str> = story.alerts_at(now).collect();
    assert_eq!(up.len(), MAX_ALERTS);
    assert_eq!(up[0], "banner 1", "the oldest went");
    assert_eq!(story.alerts_at(now + ALERT_FOR).count(), 0, "and all go");
}

fn prompt(text: &str) -> Event {
    Event::Frame(Box::new(Frame::Prompt {
        time: "1000".to_owned(),
        text: text.to_owned(),
    }))
}

/// A prompt follows what the game said, and shows again when it changes
/// (`R>` as roundtime starts), never twice in a row for nothing, nor empty,
/// nor after lines a quiet command kept out; what the player types is
/// echoed after the last prompt shown, as `VellumFE` echoes it.
#[test]
fn a_prompt_follows_what_the_game_said() {
    let mut story = Story::default();
    story.typed("look");
    for event in [
        said("", "[Town Square]"),
        prompt(">"),
        prompt(">"),
        prompt("R>"),
        said("", "You swing."),
        prompt("R>"),
        prompt(" "),
        Event::Quiet(true),
        said("", "Your inventory."),
        prompt("R>"),
        Event::Quiet(false),
    ] {
        story.hear(&observed(0, event), None);
    }
    story.typed("stance defensive");
    assert_eq!(
        texts(&story),
        [
            ">look",
            "[Town Square]",
            ">",
            "R>",
            "You swing.",
            "R>",
            "R>stance defensive"
        ]
    );
    assert_eq!(story.heard, 2, "a prompt is not a line heard");
}

/// The live prompt drops its `R` once roundtime has run out, the game
/// sending no prompt for that (`plan/15` §2a.1): not before, not with the
/// clock unknown, not in the scrollback. A bare `>` after it is not printed
/// again, and the echo follows the prompt as shown.
#[test]
fn the_live_prompt_drops_its_r_when_roundtime_ends() {
    let clock = |ends: u32| {
        let mut state = GameState::default();
        state.apply(&Frame::Prompt {
            time: "1000".to_owned(),
            text: ">".to_owned(),
        });
        state.roundtime_ends = Some(ends);
        state
    };
    let mut story = Story::default();
    for event in [
        said("", "You swing."),
        prompt("R>"),
        said("", "You swing again."),
        prompt("R>"),
    ] {
        story.hear(&observed(0, event), None);
    }
    story.settle(&GameState::default());
    assert_eq!(
        texts(&story).last().map(String::as_str),
        Some("R>"),
        "no clock"
    );
    story.settle(&clock(1030));
    assert_eq!(
        texts(&story).last().map(String::as_str),
        Some("R>"),
        "still in it"
    );
    story.settle(&clock(1000));
    story.hear(&observed(0, prompt(">")), None);
    story.typed("look");
    assert_eq!(
        texts(&story),
        ["You swing.", "R>", "You swing again.", ">", ">look"]
    );
    let mut hidden = Story::default();
    for event in [said("", "You hide."), prompt("HR>")] {
        hidden.hear(&observed(0, event), None);
    }
    hidden.settle(&clock(1000));
    assert_eq!(texts(&hidden), ["You hide.", "H>"], "only the R");
}

#[test]
fn a_hole_is_marked_once_and_the_story_is_bounded() {
    let mut story = Story::default();
    story.typed("look");
    story.missed();
    story.missed();
    assert_eq!(texts(&story), [">look", "(gap)"]);
    for n in 0..MAX_STORY {
        story.hear(&observed(0, said("", &format!("line {n}"))), None);
    }
    assert_eq!(story.lines.len(), MAX_STORY);
    assert_eq!(texts(&story)[0], "line 0");
}

/// The story counts the game's lines it heard and Hydra's messages it was
/// told, ever, for a tab not showing: not what the player typed, nor a
/// debug message, which it does not keep.
#[test]
fn it_counts_what_it_heard_and_was_told() {
    let mut story = Story::default();
    story.hear(&observed(0, said("", "You see a rock.")), None);
    story.typed("look");
    story.tell(Notice::line(NoticeKind::Info, "Hunt: resting."));
    story.tell(Notice::line(NoticeKind::Debug, "a detail"));
    assert_eq!((story.heard, story.told), (1, 1));
}

/// Every line of a stream is kept for a widget of that stream, even one
/// the game drops from the story (speech, main's copy); a line the game
/// sends to the story is marked with its stream, so the story can leave it
/// out while a widget of that stream is open.
#[test]
fn each_stream_is_kept_for_a_widget_of_it() {
    let state = declared();
    let mut story = Story::default();
    for (stream, text) in [
        ("", "You see a rock."),
        ("speech", "You say, \"hi\""),
        ("thoughts", "[General] hello"),
        ("thoughts", "[General] again"),
    ] {
        story.hear(&observed(0, said(stream, text)), Some(&state));
    }
    let kept = |id: &str| -> Vec<String> {
        story.streams.get(id).map_or_else(Vec::new, |kept| {
            kept.lines
                .iter()
                .map(|(_, runs)| runs.iter().map(|run| run.text.as_str()).collect())
                .collect()
        })
    };
    assert_eq!(
        kept("speech"),
        ["You say, \"hi\""],
        "kept, though dropped from the story"
    );
    assert_eq!(kept("thoughts"), ["[General] hello", "[General] again"]);
    assert_eq!(
        story.streams.get("thoughts").map(|kept| kept.heard),
        Some(2)
    );
    assert!(
        story.streams.get("").is_none(),
        "the story's own is the story"
    );
    let marked: Vec<Option<&str>> = story
        .lines
        .iter()
        .map(|(_, shown)| match shown {
            Shown::From(stream, _) => Some(stream.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(marked, [None, Some("thoughts"), Some("thoughts")]);
    let ids: Vec<&str> = story.streams.ids().collect();
    assert_eq!(ids, ["speech", "thoughts"]);
}

/// A stream keeps its newest lines, as many as `MAX_STREAM`, and counts
/// every one it heard.
#[test]
fn a_stream_keeps_its_newest_lines() {
    let mut story = Story::default();
    let over = super::streams::MAX_STREAM + 5;
    for n in 0..over {
        story.hear(
            &observed(0, said("thoughts", &format!("thought {n}"))),
            None,
        );
    }
    let kept = story.streams.get("thoughts").expect("kept");
    assert_eq!(kept.lines.len(), super::streams::MAX_STREAM);
    assert_eq!(kept.heard, u64::try_from(over).expect("small"));
    let first: String = kept.lines[0]
        .1
        .iter()
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(first, "thought 5");
}

/// The game's answer to a menu is its connection's: a new one forgets it.
#[test]
fn a_menus_answer_goes_with_its_connection() {
    let mut story = Story::default();
    let menu = cena_session::Menu {
        id: "1".to_owned(),
        ..cena_session::Menu::default()
    };
    story.hear(
        &observed(0, Event::Frame(Box::new(Frame::MenuResponse(menu)))),
        None,
    );
    assert!(story.menu.is_some());
    story.hear(&observed(1, said("", "You see a rock.")), None);
    assert!(story.menu.is_none());
}
