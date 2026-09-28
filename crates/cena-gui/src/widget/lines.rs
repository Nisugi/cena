//! The widgets of lines, the story and a stream, and how each draws them as
//! its own page says (the author, 2026-09-28, of the story and stream
//! windows' right-click: *"What would be in them?"*; `VellumFE`'s text
//! windows answer, `config/widgets.rs`): a time on each line, at its start
//! or its end, with seconds or not, on either clock (*"timestamp should also
//! offer the granularity, XX:XX, XX:XX:XX, XX:XX:XX AM/PM, 12/24 hour"*);
//! word wrap; and the story's prompts and what the player typed, each shown
//! or not.

use std::borrow::Cow;
use std::collections::VecDeque;

use cena_ui::StyledRun;
use egui::Id;
use serde::{Deserialize, Serialize};

use super::{Clicked, Seen};
use crate::story::{Hours, Shown, Stamp};
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
        if all.iter().any(|run| run.link.is_some()) {
            return text::linked(ui, job, &all).map(|acted| match acted {
                text::Acted::Clicked(link, at) => Clicked::Link(link, at),
                text::Acted::Quietly(line) => Clicked::Quietly(line),
            });
        }
        ui.label(job);
        None
    }

    /// A scrolled body of lines, newest at the bottom, where it stays unless
    /// the player scrolls back; sideways too when lines do not wrap.
    fn scrolled(self, ui: &mut egui::Ui, id: Id, add: impl FnOnce(&mut egui::Ui)) {
        let area = if self.wrap {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::both()
        };
        area.min_scrolled_height(0.0)
            .id_salt(id)
            .stick_to_bottom(true)
            .auto_shrink(false)
            .show(ui, |ui| {
                if !self.wrap {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                }
                add(ui);
            });
    }
}

/// The story, a stream's lines left out while a widget of it is `open`;
/// the link clicked in it, if one was.
pub(super) fn story(
    ui: &mut egui::Ui,
    lines: &VecDeque<(Stamp, Shown)>,
    open: &[String],
    (id, options): (Id, Lines),
) -> Option<Clicked> {
    let mut clicked = None;
    options.scrolled(ui, id.with("story"), |ui| {
        for (at, shown) in lines {
            match shown {
                Shown::Game(runs) => clicked = clicked.take().or(options.label(ui, *at, runs)),
                Shown::From(stream, runs) => {
                    if !open.contains(stream) {
                        clicked = clicked.take().or(options.label(ui, *at, runs));
                    }
                }
                Shown::Typed { prompt, line } => {
                    if options.echo {
                        ui.weak(options.stamped(*at, &format!("{prompt}{line}")));
                    }
                }
                Shown::Prompt(prompt) => {
                    if options.prompts {
                        ui.weak(options.stamped(*at, prompt));
                    }
                }
                Shown::Gap => {
                    ui.colored_label(WRONG, "Some lines were missed here.");
                }
            }
        }
    });
    // Its blank space is the floor: an object carried and let go there, on
    // no other object, is dropped (the author, 2026-09-28).
    clicked
        .or_else(|| crate::carry::target(ui, id.with("story"), "drop", None).map(Clicked::Quietly))
}

/// One of the game's streams; the link clicked in it, if one was.
pub(super) fn stream(
    ui: &mut egui::Ui,
    seen: &Seen<'_>,
    stream: &str,
    (id, options): (Id, Lines),
) -> Option<Clicked> {
    let mut clicked = None;
    options.scrolled(ui, id.with("stream"), |ui| {
        match seen.story.streams.get(stream) {
            Some(kept) => {
                for (at, runs) in &kept.lines {
                    clicked = clicked.take().or(options.label(ui, *at, runs));
                }
            }
            None => {
                ui.weak("Nothing yet.");
            }
        }
    });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::Story;
    use crate::widget::{Chosen, Widget};
    use cena_session::{Event, Frame, Generation, ObservedEvent, SessionId};
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
        story.hear(
            &heard(Event::Frame(Box::new(Frame::Prompt {
                time: "1000".to_owned(),
                text: ">".to_owned(),
            }))),
            None,
        );
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
}
