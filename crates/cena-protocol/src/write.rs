//! Markup written back: tags as the game would send them, from what was read
//! of them.
//!
//! The one parser reads the game's markup into frames; this writes the few
//! tags a reader of the game's stream needs to be told again. Its reader is
//! the player's own Lich started mid-session, which Hydra tells what it
//! missed with a login built from its model, as of the moment it attaches
//! (`plan/51` §7, step 4; the author, 2026-09-28: *"We know what the login
//! blob consists of, so we can just build it in the moment and send
//! accurate info"*).
//!
//! Here, with the parser, because the markup is this crate's: nothing above
//! it spells a tag. What is written is read back by the parser to what it
//! was written from (the tests below, and the model's own).

use crate::frame::{Link, LinkKind};
use crate::runs::{Run, Runs};

/// `text` with the XML entities escaped: all five, since Lich's attribute
/// reader decodes all five (`reference/lich-5/lib/common/xmlparser.rb:366`).
#[must_use]
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            c => escaped.push(c),
        }
    }
    escaped
}

/// `<name a="v" .../>`.
#[must_use]
pub fn empty(name: &str, attrs: &[(&str, &str)]) -> String {
    format!("<{name}{}/>", attributes(attrs))
}

/// `<name a="v" ...>body</name>`, `body` being markup already.
#[must_use]
pub fn element(name: &str, attrs: &[(&str, &str)], body: &str) -> String {
    format!("<{name}{}>{body}</{name}>", attributes(attrs))
}

fn attributes(attrs: &[(&str, &str)]) -> String {
    let mut written = String::new();
    for (name, value) in attrs {
        written.push(' ');
        written.push_str(name);
        written.push_str("=\"");
        written.push_str(&escape(value));
        written.push('"');
    }
    written
}

/// `runs` as markup the parser reads back to them: their bold, preset and
/// mono regions, and their links, an object's inside a command's where the
/// wire nests them.
#[must_use]
pub fn runs(runs: &Runs) -> String {
    let mut writing = Writing::default();
    for run in &runs.runs {
        writing.run(run);
    }
    writing.finish()
}

/// What is open while runs are written.
#[derive(Default)]
struct Writing {
    written: String,
    bold: u16,
    mono: bool,
    preset: Option<String>,
    link: Option<Link>,
    inner: Option<Link>,
}

impl Writing {
    fn run(&mut self, run: &Run) {
        if self.preset != run.style.preset {
            self.close_links();
            if self.preset.take().is_some() {
                self.written.push_str("</preset>");
            }
            if let Some(preset) = &run.style.preset {
                self.written.push_str(&opening("preset", &[("id", preset)]));
                self.preset = Some(preset.clone());
            }
        }
        if !same(self.link.as_ref(), run.link.as_ref()) {
            self.close_links();
            if let Some(link) = &run.link {
                self.written.push_str(&open(link));
                self.link = Some(link.clone());
            }
        }
        if !same(self.inner.as_ref(), run.inner_link.as_ref()) {
            if self.inner.take().is_some() {
                self.written.push_str("</a>");
            }
            if let Some(inner) = &run.inner_link {
                self.written.push_str(&open(inner));
                self.inner = Some(inner.clone());
            }
        }
        while self.bold < run.style.bold_depth {
            self.written.push_str("<pushBold/>");
            self.bold += 1;
        }
        while self.bold > run.style.bold_depth {
            self.written.push_str("<popBold/>");
            self.bold -= 1;
        }
        if self.mono != run.style.mono {
            self.written.push_str(if run.style.mono {
                "<output class=\"mono\"/>"
            } else {
                "<output class=\"\"/>"
            });
            self.mono = run.style.mono;
        }
        self.written.push_str(&escape(&run.text));
    }

    fn close_links(&mut self) {
        if self.inner.take().is_some() {
            self.written.push_str("</a>");
        }
        if let Some(link) = self.link.take() {
            self.written.push_str(close(&link));
        }
    }

    fn finish(mut self) -> String {
        self.close_links();
        while self.bold > 0 {
            self.written.push_str("<popBold/>");
            self.bold -= 1;
        }
        if self.mono {
            self.written.push_str("<output class=\"\"/>");
        }
        if self.preset.is_some() {
            self.written.push_str("</preset>");
        }
        self.written
    }
}

/// Whether two runs' links are one element: the same kind and coordinate.
/// Not their text, which the parser gives each run as the link's text so
/// far.
fn same(one: Option<&Link>, other: Option<&Link>) -> bool {
    match (one, other) {
        (Some(one), Some(other)) => one.kind == other.kind && one.coord == other.coord,
        (None, None) => true,
        _ => false,
    }
}

/// The opening tag of `link`, as the parser reads it back
/// (`text::link_from_tag`, `parser/markup.rs`'s `open_link`).
fn open(link: &Link) -> String {
    let (name, mut attrs): (&str, Vec<(&str, &str)>) = match &link.kind {
        LinkKind::Exist { id, noun } => ("a", vec![("exist", id), ("noun", noun)]),
        LinkKind::Direct { cmd } => ("d", vec![("cmd", cmd)]),
        LinkKind::DirectText => ("d", Vec::new()),
        LinkKind::Url { href } => ("a", vec![("href", href)]),
        LinkKind::NotActionable => ("a", Vec::new()),
    };
    if let Some(coord) = &link.coord {
        attrs.push(("coord", coord));
    }
    opening(name, &attrs)
}

fn opening(name: &str, attrs: &[(&str, &str)]) -> String {
    format!("<{name}{}>", attributes(attrs))
}

fn close(link: &Link) -> &'static str {
    match link.kind {
        LinkKind::Direct { .. } | LinkKind::DirectText => "</d>",
        LinkKind::Exist { .. } | LinkKind::Url { .. } | LinkKind::NotActionable => "</a>",
    }
}

#[cfg(test)]
mod tests {
    use super::{empty, escape, runs};
    use crate::runs::{Run, Runs};
    use crate::{Frame, Parser};

    /// Every component body in `markup`, parsed.
    fn bodies(markup: &str) -> Vec<(String, Runs)> {
        Parser::new()
            .push_bytes(markup.as_bytes())
            .into_iter()
            .filter_map(|frame| match frame {
                Frame::Component { id, body } => Some((id, joined(body))),
                _ => None,
            })
            .collect()
    }

    /// `body` with neighbouring runs of one style and link joined: a tag
    /// that changed nothing splits a run on the wire, and means nothing to
    /// a reader.
    fn joined(body: Runs) -> Runs {
        let mut runs: Vec<Run> = Vec::new();
        for run in body.runs {
            match runs.last_mut() {
                Some(last)
                    if last.style == run.style
                        && last.link == run.link
                        && last.inner_link == run.inner_link =>
                {
                    last.text.push_str(&run.text);
                }
                _ => runs.push(run),
            }
        }
        Runs { runs }
    }

    /// What the parser read of a room -- objects, bold creatures, exits,
    /// a command link with an object inside it -- is written back to markup
    /// it reads to the same runs.
    #[test]
    fn a_room_is_read_back_as_it_was_read() {
        let room = concat!(
            include_str!("../tests/fixtures/room.xml"),
            include_str!("../tests/fixtures/room_populated.xml"),
            include_str!("../tests/fixtures/login_setup.xml"),
            "<component id='room objs'>You also see <pushBold/>a <a exist=\"1\" noun=\"kobold\">kobold</a><popBold/>, ",
            "<d cmd=\"store WEAPON clear\">a <a exist=\"2\" noun=\"katar\">bent katar</a></d> &amp; <preset id=\"speech\">a &lt;sign&gt;</preset>.</component>\n",
        );
        let read = bodies(room);
        assert!(read.len() > 8, "the fixtures' components: {}", read.len());
        for (id, body) in read {
            let written = format!("<component id='{id}'>{}</component>\n", runs(&body));
            let again = bodies(&written);
            assert_eq!(again, [(id, body)], "{written}");
        }
    }

    #[test]
    fn attributes_and_text_are_escaped() {
        assert_eq!(escape(r#"a<b>&"c'"#), "a&lt;b&gt;&amp;&quot;c&apos;");
        assert_eq!(
            empty(
                "streamWindow",
                &[("id", "main"), ("subtitle", " - [Bob's]")]
            ),
            "<streamWindow id=\"main\" subtitle=\" - [Bob&apos;s]\"/>"
        );
    }
}
