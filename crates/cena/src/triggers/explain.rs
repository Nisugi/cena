//! `;trigger test <line>`: what this character's triggers would do to a
//! line (`plan/45` §5c).
//!
//! It asks the session's own matcher, the one every line is answered with,
//! and runs the same `respond`, so what it says is what would happen:
//! `VellumFE`'s rule for its sorter preview, *"the transform is a pure
//! function, so the preview is always truthful."* The line is tested as
//! main-stream text, without the game's markup.

use cena_session::trigger::{Color, Matcher, Paint, Rule, Span};
use cena_session::{ChunkLine, Line};

/// What `matcher` makes of `words`, one sentence per line: each trigger that
/// matches and what it does, then what is shown and how it is painted.
pub(crate) fn explain(matcher: &Matcher, words: &str) -> Vec<String> {
    let line = Line::new("", ChunkLine::plain(words).runs);
    let text = line.text();
    let hits = matcher.hits("", &text);
    if hits.is_empty() {
        return vec!["Nothing matches: the line is shown as it came.".into()];
    }
    let mut out = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    for hit in &hits {
        if seen.contains(&hit.trigger) {
            continue;
        }
        seen.push(hit.trigger);
        let Some(trigger) = matcher.triggers().get(hit.trigger) else {
            continue;
        };
        let found = text.get(hit.span.clone()).unwrap_or_default();
        out.push(format!(
            "`{}` matches \"{found}\": {}.",
            trigger.name,
            does(&trigger.rule)
        ));
    }
    let shown = matcher.respond(&line);
    if shown.is_empty() {
        out.push("The line is not shown: it is squelched.".into());
    }
    for line in &shown {
        let text = line.text();
        let at = if line.stream.is_empty() {
            "the story".to_owned()
        } else {
            format!("stream `{}`", line.stream)
        };
        out.push(format!("Shown in {at}: {text}"));
        for paint in &line.paint {
            let painted = text.get(paint.span.clone()).unwrap_or_default();
            out.push(format!("  \"{painted}\": {}", looks(paint)));
        }
    }
    out
}

/// What a rule does, in words.
fn does(rule: &Rule) -> String {
    let mut parts = Vec::new();
    if rule.squelch {
        parts.push("squelch".to_owned());
    }
    if let Some(with) = &rule.substitute {
        parts.push(format!("substitute \"{with}\""));
    }
    if let Some(look) = &rule.look {
        let over = match look.span {
            Span::Match => "the match".to_owned(),
            Span::Line => "the line".to_owned(),
            Span::Group(group) => format!("group {group}"),
        };
        parts.push(format!(
            "look ({}) over {over}",
            colours(look.color, look.background, look.bold)
        ));
    }
    if let Some(to) = &rule.redirect {
        let stream = if to.stream.is_empty() {
            "the story"
        } else {
            &to.stream
        };
        let copy = if to.copy { ", as a copy" } else { "" };
        parts.push(format!("redirect to {stream}{copy}"));
    }
    parts.join(", ")
}

fn looks(paint: &Paint) -> String {
    colours(paint.color, paint.background, paint.bold)
}

fn colours(color: Option<Color>, background: Option<Color>, bold: bool) -> String {
    let mut parts = Vec::new();
    if let Some(color) = color {
        parts.push(format!("colour {color}"));
    }
    if let Some(background) = background {
        parts.push(format!("background {background}"));
    }
    if bold {
        parts.push("bold".to_owned());
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(file: &str) -> Option<Matcher> {
        let loaded = cena_behavior::triggers::read(file).ok()?;
        Matcher::new(loaded.triggers.for_character("Nisugi")).ok()
    }

    #[test]
    fn it_names_what_matches_what_it_does_and_how_the_line_is_shown() {
        let matcher = matcher(
            "[trigger.stunned]\ntext = 'stunned'\nlook = { color = '#ff4040', bold = true }\n\
             [trigger.shout]\ntext = 'You are'\nsubstitute = 'YOU ARE'\n",
        )
        .unwrap();
        assert_eq!(
            explain(&matcher, "You are stunned!"),
            [
                "`shout` matches \"You are\": substitute \"YOU ARE\".",
                "`stunned` matches \"stunned\": look (colour #ff4040, bold) over the match.",
                "Shown in the story: YOU ARE stunned!",
                "  \"stunned\": colour #ff4040, bold",
            ]
        );
    }

    #[test]
    fn a_squelch_and_a_redirect_say_where_the_line_went() {
        let matcher = matcher(
            "[trigger.hide]\ntext = 'gestures'\nsquelch = true\n\
             [trigger.move]\ntext = 'whispers'\nredirect = { stream = 'whispers' }\n",
        )
        .unwrap();
        let hidden = explain(&matcher, "Someone gestures.");
        assert_eq!(
            hidden.last().map(String::as_str),
            Some("The line is not shown: it is squelched.")
        );
        let moved = explain(&matcher, "Dicate whispers, \"hi\"");
        assert_eq!(
            moved.last().map(String::as_str),
            Some("Shown in stream `whispers`: Dicate whispers, \"hi\"")
        );
    }

    #[test]
    fn nothing_matching_says_so() {
        let matcher = matcher("[trigger.x]\ntext = 'zzz'\nsquelch = true\n").unwrap();
        assert_eq!(
            explain(&matcher, "You are stunned!"),
            ["Nothing matches: the line is shown as it came."]
        );
    }
}
