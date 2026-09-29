//! The widgets of lines, the story and a stream, and how each draws them as
//! its own page says (the author, 2026-09-28, of the story and stream
//! windows' right-click: *"What would be in them?"*; `VellumFE`'s text
//! windows answer, `config/widgets.rs`): a time on each line, at its start
//! or its end, with seconds or not, on either clock (*"timestamp should also
//! offer the granularity, XX:XX, XX:XX:XX, XX:XX:XX AM/PM, 12/24 hour"*);
//! word wrap; and the story's prompts and what the player typed, each shown
//! or not.

use std::borrow::Cow;

use cena_ui::StyledRun;
use egui::Id;
use serde::{Deserialize, Serialize};

use super::find::Seek;
use super::{Clicked, Seen};
use crate::story::{Hours, Shown, Stamp, Story};
use crate::text::{self, WRONG};

/// Where a line's time goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stamps {
    /// No time.
    #[default]
    None,
    /// Before the line: `[7:08 PM] You swing ...`.
    Start,
    /// After it: `You swing ... [7:08 PM]`, `VellumFE`'s default.
    End,
}

/// How a widget of lines draws them: the story's, or a stream's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a page of switches: each on or off on its own, as the player picks"
)]
pub(crate) struct Lines {
    /// Where each line's time goes, if anywhere.
    pub(crate) stamps: Stamps,
    /// A time says its seconds.
    pub(crate) seconds: bool,
    /// The clock a time is said on.
    pub(crate) hours: Hours,
    /// A long line wraps; unwrapped, the widget scrolls sideways.
    pub(crate) wrap: bool,
    /// The story shows the game's prompts.
    pub(crate) prompts: bool,
    /// The story shows what the player typed.
    pub(crate) echo: bool,
}

impl Default for Lines {
    /// As the story has always drawn: no times, wrapped, prompts and what
    /// was typed shown.
    fn default() -> Self {
        Self {
            stamps: Stamps::None,
            seconds: false,
            hours: Hours::Twelve,
            wrap: true,
            prompts: true,
            echo: true,
        }
    }
}

impl Lines {
    /// `text` with the time `at` where this puts it.
    fn stamped(self, at: Stamp, text: &str) -> String {
        let said = at.said(self.seconds, self.hours);
        match self.stamps {
            Stamps::None => text.to_owned(),
            Stamps::Start => format!("[{said}] {text}"),
            Stamps::End => format!("{text} [{said}]"),
        }
    }

    /// A line of the game's as this draws it: its runs, with the time `at`
    /// where this puts it; the link clicked in it, if one was.
    fn label(self, ui: &mut egui::Ui, at: Stamp, runs: &[StyledRun]) -> Option<Clicked> {
        let run = |text: String| StyledRun {
            text,
            ..StyledRun::default()
        };
        let said = || at.said(self.seconds, self.hours);
        let all: Cow<'_, [StyledRun]> = match self.stamps {
            Stamps::None => Cow::Borrowed(runs),
            Stamps::Start => {
                let mut all = vec![run(format!("[{}] ", said()))];
                all.extend_from_slice(runs);
                Cow::Owned(all)
            }
            Stamps::End => {
                let mut all = runs.to_vec();
                all.push(run(format!(" [{}]", said())));
                Cow::Owned(all)
            }
        };
        let job = text::job(&all, ui.style());
        let drawn = ui.scope(|ui| {
            if all.iter().any(|run| run.link.is_some()) {
                return text::linked(ui, job, &all).map(|acted| match acted {
                    text::Acted::Clicked(link, at) => Clicked::Link(link, at),
                    text::Acted::Quietly(line) => Clicked::Quietly(line),
                });
            }
            ui.label(job);
            None
        });
        note_under(ui, drawn.response.rect, runs);
        drawn.inner
    }
}

/// Where the line under the pointer is noted, for the play window's
/// right-click: *Make a trigger from this line* (`plan/54` step 4).
const UNDER: &str = "story-line-under-pointer";

/// Note `runs`' words as the line under the pointer, if it is, drawn at
/// `rect`: with the frame it was seen in, so a line scrolled away is not
/// offered later.
fn note_under(ui: &egui::Ui, rect: egui::Rect, runs: &[StyledRun]) {
    let context = ui.ctx();
    if context
        .pointer_latest_pos()
        .is_some_and(|at| rect.contains(at) && ui.clip_rect().contains(at))
    {
        let frame = context.cumulative_frame_nr();
        let words = plain(runs).into_owned();
        context.data_mut(|data| data.insert_temp(egui::Id::new(UNDER), (frame, words)));
    }
}

/// The story line under the pointer this frame or the last, as its words.
pub(crate) fn line_under(context: &egui::Context) -> Option<String> {
    let (frame, words) =
        context.data(|data| data.get_temp::<(u64, String)>(egui::Id::new(UNDER)))?;
    (context.cumulative_frame_nr().saturating_sub(frame) <= 1 && !words.trim().is_empty())
        .then_some(words)
}

/// The story, a stream's lines left out while a widget of it is `open`,
/// split when the player scrolls back (`split.rs`); the link clicked in it,
/// if one was.
///
/// A prompt is drawn only when a line was drawn since the last one drawn, or
/// when it changed (`R>` to `>`): `VellumFE`'s rule (`core/messages/element.rs`,
/// the `Prompt` arm, and `popStream`'s "will skip next prompt"). It is kept
/// here, where what is left out is known, because a thought whose Thoughts
/// widget is open is not drawn, and the prompt after it was: a lone `>` in
/// the story for every line another widget showed.
pub(super) fn story(
    ui: &mut egui::Ui,
    story: &Story,
    open: &[String],
    (id, options): (Id, Lines),
) -> Option<Clicked> {
    let mut clicked = None;
    let first = (story.dropped, options.wrap);
    let scroll = super::split::asked(ui, id);
    let mut finder = Seek::of(
        ui,
        id,
        story
            .lines
            .iter()
            .filter_map(|(_, shown)| said(shown, open, options)),
    );
    super::split::scrolled(ui, id.with("story"), first, scroll, |ui, tops| {
        let mut prompts = Prompts::default();
        if let Some(finder) = &mut finder {
            finder.start();
        }
        for (at, shown) in &story.lines {
            tops.mark(ui);
            let text = finder.as_ref().and_then(|_| said(shown, open, options));
            let mut draw = |ui: &mut egui::Ui| match shown {
                Shown::Game(runs) => {
                    prompts.line(runs);
                    clicked = clicked.take().or(options.label(ui, *at, runs));
                }
                Shown::From(stream, runs) => {
                    if !open.contains(stream) {
                        prompts.line(runs);
                        clicked = clicked.take().or(options.label(ui, *at, runs));
                    }
                }
                Shown::Typed { prompt, line } => {
                    if options.echo {
                        ui.weak(options.stamped(*at, &format!("{prompt}{line}")));
                    }
                }
                Shown::Prompt(prompt) => {
                    if prompts.draws(prompt) && options.prompts {
                        ui.weak(options.stamped(*at, prompt));
                    }
                }
                Shown::Gap => {
                    ui.colored_label(WRONG, "Some lines were missed here.");
                }
            };
            match (&mut finder, text) {
                (Some(finder), Some(text)) => finder.line(ui, (&text, tops.player()), draw),
                _ => draw(ui),
            }
        }
    });
    // Its blank space is the floor: an object carried and let go there, on
    // no other object, is dropped (the author, 2026-09-28).
    clicked
        .or_else(|| crate::carry::target(ui, id.with("story"), "drop", None).map(Clicked::Quietly))
}

/// The text of a line of the story that Find looks through: one drawn,
/// the game's or an echo, never a prompt (`plan/52` step 6).
fn said<'a>(shown: &'a Shown, open: &[String], options: Lines) -> Option<Cow<'a, str>> {
    match shown {
        Shown::Game(runs) => Some(plain(runs)),
        Shown::From(stream, runs) if !open.contains(stream) => Some(plain(runs)),
        Shown::Typed { prompt, line } if options.echo => {
            Some(Cow::Owned(format!("{prompt}{line}")))
        }
        _ => None,
    }
}

/// A line's runs as the text they say.
fn plain(runs: &[StyledRun]) -> Cow<'_, str> {
    match runs {
        [one] => Cow::Borrowed(one.text.as_str()),
        _ => Cow::Owned(runs.iter().map(|run| run.text.as_str()).collect()),
    }
}

/// Which of the story's prompts are drawn, walking it in order.
#[derive(Default)]
struct Prompts<'a> {
    /// A line was drawn since the last prompt drawn.
    since: bool,
    /// The last prompt drawn; none yet.
    last: Option<&'a str>,
}

impl<'a> Prompts<'a> {
    /// A line was drawn: it earns the next prompt if it has something to
    /// read (`crate::story::visible`).
    fn line(&mut self, runs: &[StyledRun]) {
        self.since |= crate::story::visible(runs);
    }

    /// Whether `prompt`, next, is drawn: after a line drawn, or changed.
    fn draws(&mut self, prompt: &'a str) -> bool {
        let changed = self.last.is_none_or(|last| last.trim() != prompt.trim());
        let draws = self.since || changed;
        if draws {
            self.last = Some(prompt);
        }
        self.since = false;
        draws
    }
}

/// One of the game's streams, split when the player scrolls back; the link
/// clicked in it, if one was.
pub(super) fn stream(
    ui: &mut egui::Ui,
    seen: &Seen<'_>,
    stream: &str,
    (id, options): (Id, Lines),
) -> Option<Clicked> {
    let mut clicked = None;
    let kept = seen.story.streams.get(stream);
    // Its first kept line's number: what it heard, less what it keeps.
    let first = kept.map_or(0, |kept| {
        kept.heard
            .saturating_sub(u64::try_from(kept.lines.len()).unwrap_or(u64::MAX))
    });
    let scroll = super::split::asked(ui, id);
    let lines = kept.into_iter().flat_map(|kept| kept.lines.iter());
    let mut finder = Seek::of(ui, id, lines.map(|(_, runs)| plain(runs)));
    super::split::scrolled(
        ui,
        id.with("stream"),
        (first, options.wrap),
        scroll,
        |ui, tops| match kept {
            Some(kept) => {
                if let Some(finder) = &mut finder {
                    finder.start();
                }
                for (at, runs) in &kept.lines {
                    tops.mark(ui);
                    let mut draw = |ui: &mut egui::Ui| {
                        clicked = clicked.take().or(options.label(ui, *at, runs));
                    };
                    match &mut finder {
                        Some(finder) => finder.line(ui, (&plain(runs), tops.player()), draw),
                        None => draw(ui),
                    }
                }
            }
            None => {
                ui.weak("Nothing yet.");
            }
        },
    );
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::Story;
    use crate::widget::{Chosen, Widget};
    use cena_session::{Event, Generation, ObservedEvent, SessionId};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;
    use std::sync::Arc;

    fn heard(event: Event) -> ObservedEvent {
        ObservedEvent {
            session: SessionId::FIRST,
            generation: Generation::FIRST,
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

    /// A story that heard a swing, its prompt, and a thought, and the player
    /// typing `look`: every line at 7:08:05 in the evening.
    fn evening() -> Story {
        let mut story = Story::default();
        story.hear(&heard(said("", "You swing.")), None);
        story.hear(&heard(Event::Prompt(">".to_owned())), None);
        story.hear(&heard(said("thoughts", "[General] hello")), None);
        story.typed("look");
        for (at, _) in &mut story.lines {
            *at = Stamp::at(19, 8, 5);
        }
        story.streams.stamp_all("thoughts", Stamp::at(19, 8, 5));
        story
    }

    /// `widget` drawn with `lines`, over the evening's story.
    fn drawn(widget: Widget, lines: Lines) -> Harness<'static, ()> {
        let story = evening();
        let mut harness = Harness::builder()
            .with_size((420.0, 200.0))
            .build_ui(move |ui| {
                let seen = Seen {
                    snapshot: None,
                    story: &story,
                    hunt: None,
                    who: None,
                    open: &[],
                    minimap: None,
                };
                let chosen = Chosen {
                    lines: Some(lines),
                    ..Chosen::default()
                };
                let _ = widget.draw_with(ui, &seen, Id::new("lines"), &chosen);
            });
        harness.run();
        harness
    }

    /// A time goes where the page says, said as it says, on the game's
    /// lines, the prompt and what was typed alike; none, as always.
    #[test]
    fn each_line_says_its_time_as_the_page_says() {
        let with = |stamps, seconds, hours| Lines {
            stamps,
            seconds,
            hours,
            ..Lines::default()
        };
        for (lines, says) in [
            (Lines::default(), ["You swing.", ">", ">look"]),
            (
                with(Stamps::Start, false, Hours::Twelve),
                ["[7:08 PM] You swing.", "[7:08 PM] >", "[7:08 PM] >look"],
            ),
            (
                with(Stamps::End, true, Hours::TwentyFour),
                ["You swing. [19:08:05]", "> [19:08:05]", ">look [19:08:05]"],
            ),
        ] {
            let harness = drawn(Widget::Story, lines);
            for said in says {
                assert!(harness.query_by_label(said).is_some(), "{said}");
            }
        }
        let thoughts = drawn(
            Widget::Stream("thoughts".to_owned()),
            with(Stamps::Start, true, Hours::Twelve),
        );
        assert!(
            thoughts
                .query_by_label("[7:08:05 PM] [General] hello")
                .is_some(),
            "a stream's lines too"
        );
    }

    /// A long line wraps in a narrow widget, or with word wrap off stays one
    /// line, the widget scrolling sideways instead.
    #[test]
    fn a_long_line_wraps_unless_the_page_says_not() {
        let long = "You swing a steel broadsword at a kobold, which ducks, rolls, and comes up behind you snarling!";
        let height = |wrap: bool| {
            let mut story = Story::default();
            story.hear(&heard(said("", long)), None);
            let mut harness = Harness::builder()
                .with_size((160.0, 300.0))
                .build_ui(move |ui| {
                    let seen = Seen {
                        snapshot: None,
                        story: &story,
                        hunt: None,
                        who: None,
                        open: &[],
                        minimap: None,
                    };
                    let chosen = Chosen {
                        lines: Some(Lines {
                            wrap,
                            ..Lines::default()
                        }),
                        ..Chosen::default()
                    };
                    let _ = Widget::Story.draw_with(ui, &seen, Id::new("wrap"), &chosen);
                });
            harness.run();
            harness.get_by_label(long).rect().height()
        };
        let (wrapped, one) = (height(true), height(false));
        assert!(wrapped > one * 2.0, "{wrapped} against {one}");
    }

    /// The story leaves out its prompts, or what was typed, when the page
    /// says so, and nothing else.
    #[test]
    fn the_story_leaves_out_prompts_or_what_was_typed() {
        let quiet = drawn(
            Widget::Story,
            Lines {
                prompts: false,
                echo: false,
                ..Lines::default()
            },
        );
        assert!(quiet.query_by_label(">").is_none(), "no prompt");
        assert!(quiet.query_by_label(">look").is_none(), "no echo");
        assert!(quiet.query_by_label("You swing.").is_some());
        let prompts = drawn(
            Widget::Story,
            Lines {
                echo: false,
                ..Lines::default()
            },
        );
        assert!(prompts.query_by_label(">").is_some());
        assert!(prompts.query_by_label(">look").is_none());
    }

    /// A prompt after a line another widget shows is not drawn: with the
    /// Thoughts widget open, a thought leaves no lone `>` in the story; a
    /// prompt that changed still is, and with no Thoughts widget every
    /// prompt after a thought is (`VellumFE`'s rule, the author, 2026-09-28:
    /// *"vellum has some code to suppress prompts at times"*).
    #[test]
    fn a_prompt_after_a_line_shown_elsewhere_is_not_drawn() {
        let prompt = |text: &str| heard(Event::Prompt(text.to_owned()));
        let heard_all = move || {
            let mut story = Story::default();
            story.hear(&heard(said("", "You swing.")), None);
            story.hear(&prompt(">"), None);
            for _ in 0..2 {
                story.hear(&heard(said("thoughts", "[General] hello")), None);
                story.hear(&prompt(">"), None);
            }
            story.hear(&heard(said("thoughts", "[General] again")), None);
            story.hear(&prompt("R>"), None);
            story
        };
        let prompts = |open: Vec<String>| {
            let story = heard_all();
            let mut harness = Harness::builder()
                .with_size((420.0, 400.0))
                .build_ui(move |ui| {
                    let seen = Seen {
                        snapshot: None,
                        story: &story,
                        hunt: None,
                        who: None,
                        open: &open,
                        minimap: None,
                    };
                    let _ = Widget::Story.draw_with(ui, &seen, Id::new("p"), &Chosen::default());
                });
            harness.run();
            (
                harness.query_all_by_label(">").count(),
                harness.query_all_by_label("R>").count(),
            )
        };
        assert_eq!(prompts(vec!["thoughts".to_owned()]), (1, 1), "open");
        assert_eq!(prompts(Vec::new()), (3, 1), "no Thoughts widget");
    }

    /// A blank line drawn earns no prompt after it: nothing was said.
    #[test]
    fn a_prompt_after_a_blank_line_is_not_drawn() {
        let run = |text: &str| cena_session::ChunkLine::plain(text).runs;
        let mut story = Story::default();
        for shown in [
            Shown::Game(cena_ui::painted(&cena_session::Line::new(
                "",
                run("You swing."),
            ))),
            Shown::Prompt(">".to_owned()),
            Shown::Game(cena_ui::painted(&cena_session::Line::new("", run("  ")))),
            Shown::Prompt(">".to_owned()),
        ] {
            story.lines.push_back((Stamp::now(), shown));
        }
        let mut harness = Harness::builder()
            .with_size((420.0, 400.0))
            .build_ui(move |ui| {
                let seen = Seen {
                    snapshot: None,
                    story: &story,
                    hunt: None,
                    who: None,
                    open: &[],
                    minimap: None,
                };
                let _ = Widget::Story.draw_with(ui, &seen, Id::new("b"), &Chosen::default());
            });
        harness.run();
        assert_eq!(harness.query_all_by_label(">").count(), 1);
    }
}
