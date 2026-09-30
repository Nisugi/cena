//! The trigger editor's live test (`plan/54` step 3): a line typed, run
//! through the real matcher, shown as it would be shown.
//!
//! The same `Matcher` every session answers its lines with, and its
//! `respond`, as `;trigger test` asks them: *"the transform is a pure
//! function, so the preview is always truthful"* (`VellumFE`'s rule, quoted
//! in `plan/45` §5c). The triggers are the file's, on and not refused, with
//! the form in place of its saved self, so a change is seen before it is
//! saved. As `;trigger test` says of itself: an `only if` is taken to hold,
//! a condition has no line to test, and a line typed has no markup for an
//! event that reads it (speech, whisper, a departure).

use cena_session::trigger::{Color, Matcher, Trigger};
use cena_session::{ChunkLine, Line};
use cena_ui::triggers::{Book, Form};

/// A line to test, and the stream it is on.
#[derive(Debug, Default)]
pub(super) struct Test {
    /// The line, as the game would send it.
    pub(super) line: String,
    /// Its stream; empty for the story.
    pub(super) stream: String,
}

/// What the matcher made of the line.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Outcome {
    /// The triggers that fired, by name.
    pub(super) fired: Vec<String>,
    /// The lines shown, each with its stream and paint.
    pub(super) shown: Vec<Line>,
    /// What a fired trigger would send, not sent by a test.
    pub(super) sends: Vec<String>,
    /// Why the form, as it stands, would be refused.
    pub(super) refused: Option<String>,
}

/// Run `test` through the file's triggers in `book`, with `form` (the
/// trigger `was`, or a new one) in place of its saved self.
pub(super) fn run(test: &Test, book: &Book, draft: Option<(Option<&str>, &Form)>) -> Outcome {
    let off: Vec<&str> = book
        .categories
        .iter()
        .filter(|(_, on)| !on)
        .map(|(name, _)| name.as_str())
        .collect();
    let mut triggers: Vec<Trigger> = book
        .triggers
        .iter()
        .filter(|entry| entry.enabled && entry.refused.is_none())
        .filter(|entry| !off.contains(&entry.category.as_str()))
        .filter(|entry| draft.is_none_or(|(was, _)| was != Some(entry.name.as_str())))
        .filter_map(|entry| {
            Some(Trigger {
                name: entry.name.clone(),
                rule: entry.form.rule().ok()?,
            })
        })
        .collect();
    let mut outcome = Outcome::default();
    if let Some((_, form)) = draft
        && form.enabled
    {
        match form.rule() {
            Ok(rule) => triggers.push(Trigger {
                name: form.name.clone(),
                rule,
            }),
            Err(why) => outcome.refused = Some(why),
        }
    }
    let matcher = match Matcher::new(triggers) {
        Ok(matcher) => matcher,
        Err(why) => {
            outcome.refused = Some(why);
            return outcome;
        }
    };
    let line = Line::new(test.stream.trim(), ChunkLine::plain(&test.line).runs);
    let words = line.text();
    for hit in matcher.screened(&line, &words, None) {
        let Some(trigger) = matcher.triggers().get(hit.trigger) else {
            continue;
        };
        if outcome.fired.contains(&trigger.name) {
            continue;
        }
        outcome.fired.push(trigger.name.clone());
        if let Some(send) = &trigger.rule.send {
            outcome.sends.push(format!("{}: {send}", trigger.name));
        }
    }
    outcome.shown = matcher.respond(&line);
    outcome
}

/// Draw the test: the line and its stream, and what the matcher made of it.
pub(super) fn show(
    ui: &mut egui::Ui,
    test: &mut Test,
    book: &Book,
    draft: Option<(Option<&str>, &Form)>,
) {
    ui.horizontal(|ui| {
        ui.strong("Test");
        ui.add(
            egui::TextEdit::singleline(&mut test.line)
                .hint_text("a line as the game would send it")
                .desired_width(ui.available_width() - 220.0),
        );
        ui.label("stream");
        ui.add(
            egui::TextEdit::singleline(&mut test.stream)
                .hint_text("the story")
                .desired_width(110.0),
        );
    });
    if test.line.is_empty() {
        ui.label(
            egui::RichText::new(
                "Type a line to see what fires, and how it is shown, before you save.",
            )
            .weak()
            .small(),
        );
        return;
    }
    let outcome = run(test, book, draft);
    if let Some(why) = &outcome.refused {
        ui.colored_label(
            ui.visuals().error_fg_color,
            format!("This trigger as it stands would be refused: {why}"),
        );
    }
    if outcome.shown.is_empty() {
        ui.label("→ not shown: it is squelched.");
    }
    for line in &outcome.shown {
        ui.horizontal_wrapped(|ui| {
            let at = if line.stream.is_empty() {
                "→".to_owned()
            } else {
                format!("→ [{}]", line.stream)
            };
            ui.label(at);
            ui.label(painted(ui, line));
        });
    }
    ui.label(if outcome.fired.is_empty() {
        "Nothing fires.".to_owned()
    } else {
        format!("Fires: {}", outcome.fired.join(", "))
    });
    for send in &outcome.sends {
        ui.label(egui::RichText::new(format!("Would send {send} (a test sends nothing)")).weak());
    }
}

/// A shown line as the story would paint it.
fn painted(ui: &egui::Ui, line: &Line) -> egui::text::LayoutJob {
    let text = line.text();
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let plain = ui.visuals().text_color();
    // Each byte's look, the paints laid on in order: a later one wins.
    let mut looks: Vec<(Option<Color>, Option<Color>, bool)> =
        vec![(None, None, false); text.len()];
    let len = looks.len();
    for paint in &line.paint {
        for at in paint.span.clone().filter(|at| *at < len) {
            let look = &mut looks[at];
            look.0 = paint.color.or(look.0);
            look.1 = paint.background.or(look.1);
            look.2 |= paint.bold;
        }
    }
    let mut job = egui::text::LayoutJob::default();
    let mut start = 0;
    while start < text.len() {
        let look = looks[start];
        let mut end = start + 1;
        while end < text.len() && (looks[end] == look || !text.is_char_boundary(end)) {
            end += 1;
        }
        let colour = |c: Color| crate::theme::rgb([c.red, c.green, c.blue]);
        job.append(
            &text[start..end],
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: look.0.map_or(
                    if look.2 {
                        ui.visuals().strong_text_color()
                    } else {
                        plain
                    },
                    colour,
                ),
                background: look.1.map_or(egui::Color32::TRANSPARENT, colour),
                ..egui::TextFormat::default()
            },
        );
        start = end;
    }
    job
}
