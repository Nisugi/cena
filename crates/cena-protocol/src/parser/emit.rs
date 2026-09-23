//! Emitting text: flushing the buffer into frames, and parsing a component
//! body into runs.
//!
//! Split out of `parser.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. It holds [`Parser::parse_runs`], which is the
//! Rule 2.1 fix: a component body reaches the layer above as structured text
//! and links, never as a markup string.
//!
//! This file was `capture.rs` and also held the multi-line capture path. That
//! path is gone -- see MULTI-LINE CAPTURES in `parser.rs` for the measurement
//! (0 occurrences in 1,230,355 wire lines, against a 4,297-line silent-loss
//! window) -- so the file is named for what remains.

use super::Parser;
use crate::frame::{Frame, Style, TextFrame};
use crate::runs::{Run, Runs};
use crate::text;

impl Parser {
    /// Mark the **last** text run of a finished line as ending it.
    ///
    /// Called once per `parse_line`, which is what makes the invariant a single
    /// line of code: a line's runs carry `false` until exactly one is promoted.
    /// A line whose frames are all non-text -- a bare `<prompt>`, an indicator --
    /// promotes nothing, which is correct: it contributed no display text, so
    /// there is no run for a printer to terminate.
    pub(super) fn mark_line_end(frames: &mut [Frame]) {
        if let Some(Frame::Text(text)) = frames
            .iter_mut()
            .rev()
            .find(|frame| matches!(frame, Frame::Text(_)))
        {
            text.ends_line = true;
        }
    }

    /// Emit the buffered text, if any, as a [`Frame::Text`].
    pub(super) fn flush(&mut self, buffer: &mut String, frames: &mut Vec<Frame>) {
        if buffer.is_empty() {
            return;
        }
        let content = std::mem::take(buffer);
        frames.push(self.text_frame(&content));
    }

    /// Wrap display text in the markup state currently open.
    pub(super) fn text_frame(&mut self, content: &str) -> Frame {
        let content = text::strip_control_chars(&text::decode_entities(content));
        // Text accumulates on EVERY open link, not only the outermost.
        //
        // Vellum accumulates onto one (`src/parser/text.rs:97-106`) because
        // only one surfaces there. Here the innermost `<a exist=>` surfaces
        // too, and giving it an empty `text` made it useless for exactly the
        // case it exists for: `ready list`'s item resolved to an id with no
        // name, so a caller could not tell two katars apart.
        //
        // The outermost still gets the whole run, so nothing that reads
        // `link` changes.
        for link in &mut self.links {
            link.text.push_str(&content);
        }
        Frame::Text(TextFrame {
            content,
            stream: self.current_stream(),
            style: self.style(),
            link: self.links.first().cloned(),
            inner_link: self.nested_object(),
            // Set by `mark_line_end` once the line is fully parsed: a run
            // cannot know here whether another follows it.
            ends_line: false,
        })
    }

    /// The markup open right now.
    fn style(&self) -> Style {
        Style {
            bold_depth: self.bold_depth,
            preset: self.presets.last().cloned(),
            mono: self.mono,
        }
    }

    /// The stream text currently belongs to; `""` is the main window.
    fn current_stream(&self) -> String {
        self.streams
            .last()
            .map(|s| s.id.clone())
            .unwrap_or_default()
    }

    /// Parse a component body into structured runs.
    ///
    /// This is the Rule 2.1 fix: the layer above receives text and links, not
    /// the markup string Vellum hands it (`src/parser.rs:803-832`).
    ///
    /// Markup state is saved and restored around the body, so an unbalanced
    /// `<pushBold>` inside a component -- which the corpus does contain -- does
    /// not leak bold onto the lines that follow it. Vellum earned roughly ten
    /// regression tests for exactly this class of leak
    /// (`src/parser/tests.rs:654,755,801,897`); recomputing from a saved
    /// snapshot removes the class rather than re-earning the tests.
    /// Parse a body into [`Runs`], surfacing tags it carried that this parser
    /// does not model.
    ///
    /// # Why the body's unknown tags were being dropped
    ///
    /// `markup_state`'s `_ => {}` arm swallows anything that is not one of the
    /// nine markup names, and inside a component body there is no frame vector
    /// for a `Structural` or an `UnknownTag` to go into. The comment here said
    /// that was acceptable because "the body's raw bytes are already
    /// recoverable from the `Component` frame that encloses it".
    ///
    /// **They are not.** `Component { id, body: Runs }` carries parsed runs and
    /// no raw bytes -- `frame.rs` says so directly ("`body` is parsed, not
    /// raw"). So `<component id='room objs'>... a <newThing id='1'/>rock.
    /// </component>` yielded runs and **no `UnknownTag` at all**: Rule 2.2
    /// violated on M1's own room path, which is where the wire puts creatures
    /// and loot (review PR-1).
    ///
    /// The names are collected here and emitted by the caller, after the frame
    /// the body belongs to -- so a consumer sees the component first and then
    /// what it could not understand, in wire order.
    pub(super) fn parse_runs_reporting(
        &mut self,
        body: &str,
        unmodelled: &mut Vec<String>,
    ) -> Runs {
        // **All four pieces of markup state, and all four the same way:
        // cleared on entry, restored on exit.**
        //
        // `mono` was not saved at all, so `<component id='x'><output
        // class="mono"/>a</component>after` left "after" -- and the rest of
        // the session -- in the mono font. And the three that were saved
        // disagreed on entry: presets and links started empty while bold was
        // inherited, so the same unbalanced `<pushBold/>` before a component
        // made its body bold and an unbalanced `<preset>` did not. A body is
        // its own context in both directions: nothing leaks out of it, and
        // nothing leaked from outside leaks in (review 2026-09-23).
        let saved_bold = std::mem::take(&mut self.bold_depth);
        let saved_mono = std::mem::take(&mut self.mono);
        let saved_presets = std::mem::take(&mut self.presets);
        let saved_links = std::mem::take(&mut self.links);

        let mut runs = Vec::new();
        let mut buffer = String::new();
        let mut rest = body;
        while !rest.is_empty() {
            let Some(start) = text::find_tag_start(rest) else {
                buffer.push_str(rest);
                break;
            };
            buffer.push_str(&rest[..start]);
            let tail = &rest[start..];
            let Some(close) = tail.find('>') else {
                buffer.push_str(tail);
                break;
            };
            let tag = &tail[..=close];
            rest = &tail[close + 1..];
            self.push_run(&mut buffer, &mut runs);
            // `markup_state`, not `markup_tag`: a component body becomes
            // `Runs`, where the markup's effect is already carried by each
            // run's own `style` and `link`.
            //
            // A tag that is NOT markup is collected rather than swallowed --
            // see this function's header for why the old comment's claim
            // (that the raw bytes were recoverable) was false.
            let name = super::text::tag_name(tag);
            if !super::markup::is_markup(name) && !name.is_empty() {
                // The WHOLE tag, not just the name: Rule 2.2 requires the raw
                // form to reach the user, and a name alone cannot be shown.
                unmodelled.push(tag.to_owned());
            }
            self.markup_state(tag);
        }
        self.push_run(&mut buffer, &mut runs);

        self.bold_depth = saved_bold;
        self.mono = saved_mono;
        self.presets = saved_presets;
        self.links = saved_links;
        Runs { runs }
    }

    /// Emit one run of a component body, if there is text pending.
    pub(super) fn push_run(&mut self, buffer: &mut String, runs: &mut Vec<Run>) {
        if buffer.is_empty() {
            return;
        }
        let content = text::strip_control_chars(&text::decode_entities(buffer));
        buffer.clear();
        for link in &mut self.links {
            link.text.push_str(&content);
        }
        runs.push(Run {
            text: content,
            style: self.style(),
            link: self.links.first().cloned(),
            inner_link: self.nested_object(),
        });
    }

    /// The innermost open `<a exist=>`, when the outermost link is not it.
    ///
    /// `<d cmd=...>a <a exist=...>katar</a></d>` -- the click is the `<d>`
    /// and the object is the `<a>`. Returning `None` when the outermost link
    /// IS the `exist` keeps the common case from carrying a duplicate.
    fn nested_object(&self) -> Option<crate::frame::Link> {
        if matches!(
            self.links.first().map(|l| &l.kind),
            Some(crate::frame::LinkKind::Exist { .. })
        ) {
            return None;
        }
        self.links
            .iter()
            .rev()
            .find(|l| matches!(l.kind, crate::frame::LinkKind::Exist { .. }))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A component body: text interleaved with markup and non-markup tags.
    ///
    /// **The shape the existing suite reached only twice, both unclosed.**
    /// MEASURED before writing this: `tests/parser_never_panics.rs` reaches
    /// `parse_runs_reporting` solely through two literal `<component>`
    /// fragments in `HOSTILE_FRAGMENTS`, neither of which closes. So the
    /// nesting, the bold-depth save/restore and the `unmodelled` collection
    /// were never exercised by a property at all.
    fn body() -> impl Strategy<Value = String> {
        let part = prop::sample::select(vec![
            "plain text",
            " ",
            "&amp;",
            "&gt;&lt;",
            "<pushBold/>",
            "<popBold/>",
            "<a exist=\"123\" noun=\"rock\">a rock</a>",
            "<d cmd=\"store weapon\">a <a exist=\"9\" noun=\"katar\">katar</a></d>",
            "<preset id=\"speech\">",
            "</preset>",
            "<output class=\"mono\"/>",
            "<output class=\"\"/>",
            // NOT markup -- these must be collected, never swallowed.
            "<newThing id=\"1\"/>",
            "<futureTag attr=\"v\">",
            "<unknownFutureTag/>",
        ]);
        prop::collection::vec(part, 0..12).prop_map(|parts| parts.concat())
    }

    /// The body's text with every tag removed, then decoded and stripped --
    /// the same two transforms `push_run` applies, in the same order.
    fn expected_text(body: &str) -> String {
        let mut out = String::new();
        let mut rest = body;
        while let Some(start) = text::find_tag_start(rest) {
            out.push_str(&rest[..start]);
            let tail = &rest[start..];
            let Some(close) = tail.find('>') else {
                // An unterminated tag becomes text, which is what the
                // function does with it too.
                out.push_str(tail);
                rest = "";
                break;
            };
            rest = &tail[close + 1..];
        }
        out.push_str(rest);
        text::strip_control_chars(&text::decode_entities(&out))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]

        /// **RULE 2.2a: the body's text is conserved exactly.**
        ///
        /// This is the property that earns the file. Concatenating every run's
        /// text must reproduce the body's non-markup text, decoded and
        /// control-stripped -- nothing dropped, duplicated or invented.
        ///
        /// It is a standing check on the invariant this crate exists to
        /// uphold, and it is the exact shape of this session's nested-link
        /// defect: `<d cmd=><a exist=>katar</a></d>` reached consumers with
        /// the inner link's `text` always empty, because `push_run` appended
        /// to `links.first_mut()` rather than to every open link. That was
        /// found by a `guard:` assertion in an unrelated ordering test, after
        /// four hand-written tests for the fix missed it. **A property does
        /// not depend on having thought of the case.**
        #[test]
        fn the_bodys_text_is_conserved(body in body()) {
            let mut parser = Parser::new();
            let mut unmodelled = Vec::new();
            let runs = parser.parse_runs_reporting(&body, &mut unmodelled);
            let joined: String = runs.runs.iter().map(|r| r.text.as_str()).collect();
            prop_assert_eq!(
                joined,
                expected_text(&body),
                "text was not conserved from {:?}",
                body
            );
        }

        /// **Every non-markup tag is reported, never swallowed.**
        ///
        /// The other half of Rule 2.2a, and the defect review PR-1 found:
        /// `Component { body: Runs }` carries no raw bytes, so a tag dropped
        /// here reaches nobody at all. The whole tag is kept rather than its
        /// name -- a name alone cannot be shown to a user.
        /// A first draft asserted only that REPORTED tags are well-formed,
        /// and a mutation that swallowed every unmodelled tag -- the PR-1
        /// defect itself -- passed it green. Over an empty vector the claim
        /// is vacuously true. **The property never required anything to be
        /// reported**, which is the failure mode `CLAUDE.md` names: the input
        /// reached the code, but the assertion faced the wrong way.
        ///
        /// So the count is pinned to the input: every non-markup tag in the
        /// body must appear, and only those.
        #[test]
        fn unmodelled_tags_are_reported_whole(body in body()) {
            let mut parser = Parser::new();
            let mut unmodelled = Vec::new();
            let _ = parser.parse_runs_reporting(&body, &mut unmodelled);

            // Counted from the input, independently of the parser.
            let expected = ["<newThing id=\"1\"/>", "<futureTag attr=\"v\">", "<unknownFutureTag/>"]
                .iter()
                .map(|t| body.matches(t).count())
                .sum::<usize>();
            prop_assert_eq!(
                unmodelled.len(),
                expected,
                "expected {} unmodelled tags from {:?}, got {:?}",
                expected,
                body,
                unmodelled
            );

            for tag in &unmodelled {
                prop_assert!(
                    body.contains(tag.as_str()),
                    "reported {tag:?}, which is not in {body:?}"
                );
                prop_assert!(
                    tag.starts_with('<') && tag.ends_with('>'),
                    "reported a fragment rather than a whole tag: {tag:?}"
                );
            }
        }

        /// A body **never leaks a whole tag into a run's text**.
        ///
        /// A first draft asserted no run text contains `<` at all, and
        /// proptest refuted it in four cases with `&gt;&lt;`, which decodes
        /// to `><`. **That `<` is text the game sent**, not markup: entities
        /// are decoded AFTER tags are removed, so a decoded `<` is a literal
        /// the user must see. Asserting otherwise would have demanded the
        /// parser corrupt legitimate output.
        ///
        /// The real claim is that no tag from the input survives whole into
        /// a run -- which is Rule 2.1 without forbidding a character the wire
        /// legitimately carries.
        #[test]
        fn no_run_carries_a_whole_tag(body in body()) {
            let mut parser = Parser::new();
            let mut unmodelled = Vec::new();
            let runs = parser.parse_runs_reporting(&body, &mut unmodelled);
            for run in &runs.runs {
                for tag in ["<pushBold/>", "<popBold/>", "<unknownFutureTag/>"] {
                    prop_assert!(
                        !run.text.contains(tag),
                        "the tag {tag} survived into run text: {:?}",
                        run.text
                    );
                }
            }
        }

        /// **Parser state is restored**, whatever the body did to it.
        ///
        /// The function saves and restores bold depth, presets and links
        /// because a component body is a nested context: an unbalanced
        /// `<pushBold/>` inside one must not bleed into the line after it.
        /// Without this, one malformed component emboldens the rest of the
        /// session.
        #[test]
        fn a_body_never_leaks_style_into_the_parser(body in body()) {
            let mut parser = Parser::new();
            let before = parser.bold_depth;
            let mut unmodelled = Vec::new();
            let _ = parser.parse_runs_reporting(&body, &mut unmodelled);
            prop_assert_eq!(parser.bold_depth, before, "bold depth leaked");
            prop_assert!(!parser.mono, "mono leaked out of {:?}", body);
        }

        /// Arbitrary text never panics and conserves just the same.
        ///
        /// `body()` emits plausible parts; this covers what it cannot express.
        #[test]
        fn arbitrary_bodies_are_conserved(body in ".*") {
            let mut parser = Parser::new();
            let mut unmodelled = Vec::new();
            let runs = parser.parse_runs_reporting(&body, &mut unmodelled);
            let joined: String = runs.runs.iter().map(|r| r.text.as_str()).collect();
            prop_assert_eq!(joined, expected_text(&body));
        }
    }

    #[test]
    fn a_body_neither_inherits_nor_leaks_markup_state() {
        // Review 2026-09-23. Mono leaked OUT (it was never saved), and bold
        // leaked IN while presets and links did not.
        let mut parser = Parser::new();
        let mut unmodelled = Vec::new();
        let _ = parser.parse_runs_reporting("<output class=\"mono\"/>a", &mut unmodelled);
        assert!(!parser.mono, "mono leaked out of the body");

        parser.bold_depth = 1;
        parser.mono = true;
        let runs = parser.parse_runs_reporting("plain", &mut unmodelled);
        let styles: Vec<_> = runs
            .runs
            .iter()
            .map(|r| (r.style.bold_depth, r.style.mono))
            .collect();
        assert_eq!(styles, [(0, false)], "outer markup leaked into the body");
        assert_eq!(
            (parser.bold_depth, parser.mono),
            (1, true),
            "and is restored after"
        );
    }

    #[test]
    fn the_nested_link_keeps_its_text() {
        // This session's own defect, pinned as a case rather than left to the
        // property alone: the property WOULD catch it, but a named test says
        // what was wrong. MEASURED at the time: 322 occurrences across 62 of
        // 208 live logs, and ZERO in the committed fixtures -- which is why
        // the golden corpus did not catch it.
        let mut parser = Parser::new();
        let mut unmodelled = Vec::new();
        let runs = parser.parse_runs_reporting(
            "<d cmd=\"store weapon\">a <a exist=\"9\" noun=\"katar\">katar</a></d>",
            &mut unmodelled,
        );
        let inner: Vec<_> = runs
            .runs
            .iter()
            .filter_map(|r| r.inner_link.as_ref())
            .collect();
        assert!(!inner.is_empty(), "the nested exist link reached no run");
        assert!(
            inner.iter().any(|l| l.text.contains("katar")),
            "the inner link's text was empty: {inner:#?}"
        );
    }
}
