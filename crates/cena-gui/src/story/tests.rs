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
        .map(|shown| match shown {
            Shown::Game(runs) => runs.iter().map(|run| run.text.as_str()).collect(),
            Shown::Typed(line) => format!("> {line}"),
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
    let Some(Shown::Game(thought)) = story.lines.get(1) else {
        panic!("the thought is a game line");
    };
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

#[test]
fn a_hole_is_marked_once_and_the_story_is_bounded() {
    let mut story = Story::default();
    story.typed("look");
    story.missed();
    story.missed();
    assert_eq!(texts(&story), ["> look", "(gap)"]);
    for n in 0..MAX_STORY {
        story.hear(&observed(0, said("", &format!("line {n}"))), None);
    }
    assert_eq!(story.lines.len(), MAX_STORY);
    assert_eq!(texts(&story)[0], "line 0");
}
