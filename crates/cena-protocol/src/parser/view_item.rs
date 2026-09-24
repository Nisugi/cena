//! `<inventoryViewItem>`: one item's detail, captured across lines.
//!
//! **This parser's only multi-line capture**, and it is deliberate.
//!
//! MULTI-LINE CAPTURES in `parser.rs` records that an earlier, unbounded
//! capture path was measured and removed: 0 occurrences in 1,230,355 wire
//! lines, against a window in which 4,297 consecutive lines of game text
//! could be swallowed silently. That note leaves an explicit upgrade trigger
//! for the day the wire does send a tag spanning lines.
//!
//! This tag does. MEASURED over the author's September logs: 13 blocks, each
//! spanning 7 to 50 lines, with four `<result>` sections apiece. The note's
//! condition is honoured -- this is bounded by **lines**, not bytes, so the
//! worst case is a countable and small loss rather than an open-ended one.

use std::ops::ControlFlow;

use super::Parser;
use crate::frame::{Frame, ItemDetail, Link};
use crate::text;

/// How many lines one `<inventoryViewItem>` may span before the capture is
/// abandoned.
///
/// MEASURED maximum is 50. The bound is 200 -- four times observed, and small
/// enough that a torn block costs at most 200 lines of game text rather than
/// the 4,297 the old byte-bounded path could swallow.
pub(super) const MAX_VIEWITEM_LINES: u32 = 200;

/// The `<inventoryViewItem>` block currently being captured.
#[derive(Debug, Default)]
pub(super) struct ViewItem {
    /// `id=`: the request token.
    token: String,
    /// `exist=`: the item being described.
    exist: String,
    /// `state=`, when the server sent one.
    state: Option<String>,
    /// `closed` present on the envelope.
    closed: bool,
    /// Sections already closed.
    results: Vec<ItemDetail>,
    /// The open `<result>` section, if any.
    current: Option<ItemDetail>,
    /// The open `<a>`/`<d>` link, if any.
    open_link: Option<Link>,
    /// Lines consumed, against [`MAX_VIEWITEM_LINES`].
    lines: u32,
}

impl Parser {
    /// Account for a new physical line arriving inside an open capture.
    ///
    /// Only for a line that STARTS inside the capture: the line that opened
    /// it must not count against [`MAX_VIEWITEM_LINES`] and must not insert
    /// the newline that a real line boundary does. When the bound is passed,
    /// the capture is surfaced and closed, and the caller parses the line
    /// normally.
    pub(super) fn begin_captured_line(&mut self, frames: &mut Vec<Frame>) {
        let Some(view) = self.view_item.as_mut() else {
            return;
        };
        view.lines += 1;
        // A physical line boundary inside a section IS a newline in its
        // text: `analyze` and `inspect` arrive formatted with indented
        // tables and blank separators, and flattening them runs the
        // paragraphs together (`VellumFE/src/parser/handlers.rs:779-783`).
        if view.current.is_some() {
            view.append_text("\n");
        }
        // The loss bound. A block this long means the close was lost;
        // surfacing what we have beats consuming the stream forever.
        if view.lines > MAX_VIEWITEM_LINES {
            frames.push(self.finish_view_item(Some("truncated")));
        }
    }

    /// Walk `line` inside the capture, emitting only when the block ends.
    ///
    /// Returns what is left of the line when the capture ended part-way
    /// through it -- or was replaced by a second envelope -- so the caller
    /// carries on in whichever mode the parser is now in. Never recurses back
    /// into the line parser: see `parse_line_inner` for what that cost.
    pub(super) fn view_item_line<'a>(
        &mut self,
        line: &'a str,
        frames: &mut Vec<Frame>,
    ) -> Option<&'a str> {
        let mut rest = line;
        while !rest.is_empty() {
            let Some(start) = text::find_tag_start(rest) else {
                self.view_item_text(rest);
                break;
            };
            if start > 0 {
                let (prose, tail) = rest.split_at(start);
                self.view_item_text(prose);
                rest = tail;
            }
            let at_tag = rest;
            let Some(end) = rest.find('>') else {
                self.view_item_text(rest);
                break;
            };
            // A paired tag is taken WHOLE, as `parse_ordinary` takes it: its
            // body is the frame's content, and the non-styling arm below
            // hands it to `dispatch`, which expects the whole thing. A pair
            // whose close is not on this line is a `MalformedTag` there, so
            // it is one here too. `<prompt>` is exempt -- its arm ends the
            // capture and re-parses from `at_tag`, closed or not.
            let mut end = end + 1;
            let open = &rest[..end];
            let name = text::tag_name(open);
            if name != "prompt"
                && !open.ends_with("/>")
                && !text::is_close_tag(open)
                && super::is_paired(open)
            {
                let close = format!("</{name}>");
                let Some(at) = rest.find(&close) else {
                    frames.push(Frame::MalformedTag {
                        raw: rest.to_owned(),
                    });
                    break;
                };
                end = at + close.len();
            }
            let (tag, tail) = rest.split_at(end);
            rest = tail;
            if let ControlFlow::Break(after) = self.view_item_tag(tag, at_tag, rest, frames) {
                return after;
            }
        }
        None
    }

    /// Handle one tag inside the capture. `Break` means the block ended,
    /// carrying what of the line is left to parse; `at_tag` is the line from
    /// this tag onward, `rest` from just after it.
    fn view_item_tag<'a>(
        &mut self,
        tag: &str,
        at_tag: &'a str,
        rest: &'a str,
        frames: &mut Vec<Frame>,
    ) -> ControlFlow<Option<&'a str>> {
        let closing = text::is_close_tag(tag);
        match (text::tag_name(tag), closing) {
            // A second envelope while one is open. The close of the first
            // never arrived, so the response is torn: surface what was
            // captured and start the new one, rather than flattening the
            // second envelope into the first block's prose and losing it
            // entirely. `open_view_item`'s own guard cannot fire here,
            // because an open capture owns the line before dispatch sees it.
            //
            // The rest of the line goes on in whichever mode that leaves: the
            // new capture if it is open, ordinary feed if the envelope was
            // self-closing. It used to be dropped in the second case.
            ("inventoryViewItem", false) => {
                frames.push(self.finish_view_item(Some("malformed")));
                self.open_view_item(tag, frames);
                ControlFlow::Break((!rest.trim().is_empty()).then_some(rest))
            }
            ("inventoryViewItem", true) => {
                frames.push(self.finish_view_item(None));
                ControlFlow::Break((!rest.trim().is_empty()).then_some(rest))
            }
            // A prompt means the block was torn mid-send. Surface the partial
            // response rather than swallowing the rest of the session, and
            // let the prompt parse normally -- it is the resync barrier.
            ("prompt", _) => {
                frames.push(self.finish_view_item(Some("malformed")));
                ControlFlow::Break(Some(at_tag))
            }
            ("result", false) => {
                self.open_section(tag);
                ControlFlow::Continue(())
            }
            ("result", true) => {
                if let Some(view) = self.view_item.as_mut() {
                    view.close_section();
                }
                ControlFlow::Continue(())
            }
            ("br", _) => {
                self.view_item_text("\n");
                ControlFlow::Continue(())
            }
            // An item link inside the prose. VellumFE flattens these away
            // (`src/parser/handlers.rs:866`); this parser already types
            // links, so the noun in a description stays clickable.
            // `open_link`, not `link_from_tag`: a bare `<d>` has no `cmd=`,
            // so `link_from_tag` answers `None` and the link used to be lost
            // here while the same `<d>` anywhere else became `DirectText`.
            ("a" | "d", false) => {
                if let Some(view) = self.view_item.as_mut() {
                    view.open_link = Some(super::markup::open_link(tag));
                }
                ControlFlow::Continue(())
            }
            ("a" | "d", true) => {
                if let Some(view) = self.view_item.as_mut()
                    && let Some(link) = view.open_link.take()
                {
                    view.push_link(link);
                }
                ControlFlow::Continue(())
            }
            // Styling is flattened: a detail section is prose, and its text
            // is the capture's, not the parser's. So is a comment, which has
            // no name to dispatch on.
            (name, _) if super::markup::is_markup(name) || name.starts_with('!') => {
                ControlFlow::Continue(())
            }
            // **Everything else still happens.** This arm used to read
            // "every other inline tag is styling" and flatten it -- which
            // was true of `<b>` and false of `<pushStream>`, `<popStream>`,
            // `<progressBar>`, `<dialogData>`, `<nav>`, `<roundTime>` and
            // `<indicator>`, all of which vanished if they arrived while a
            // block was open. The stream stack never saw the push, so the
            // matching pop after the capture popped the WRONG stream (review
            // 2026-09-23).
            //
            // `dispatch` is the one answer for a tag, wherever it sits. The
            // capture keeps its own text semantics because the buffer it is
            // handed is empty: no prose reaches `dispatch`, only the tag.
            _ => {
                let mut no_text = String::new();
                self.dispatch(tag, &mut no_text, frames);
                ControlFlow::Continue(())
            }
        }
    }

    /// Begin a `<result command=>` section.
    fn open_section(&mut self, tag: &str) {
        let command = text::attribute(tag, "command").unwrap_or_default();
        let Some(view) = self.view_item.as_mut() else {
            return;
        };
        view.close_section();
        let section = ItemDetail {
            command,
            ..ItemDetail::default()
        };
        if tag.ends_with("/>") {
            // Self-closing: the section exists and is empty.
            view.results.push(section);
        } else {
            view.current = Some(section);
        }
    }

    /// Accumulate display text into the open section.
    fn view_item_text(&mut self, raw: &str) {
        let decoded = text::strip_control_chars(&text::decode_entities(raw));
        if let Some(view) = self.view_item.as_mut() {
            view.append_text(&decoded);
        }
    }

    /// Close the capture and produce its frame.
    fn finish_view_item(&mut self, force_state: Option<&str>) -> Frame {
        let mut view = self.view_item.take().unwrap_or_default();
        view.close_section();
        Frame::InventoryViewItem(crate::frame::ItemView {
            token: view.token,
            exist: view.exist,
            state: force_state.map(str::to_owned).or(view.state),
            closed: view.closed,
            results: view.results,
        })
    }

    /// Open a capture for an `<inventoryViewItem>` envelope.
    ///
    /// A self-closing envelope is a complete, empty response and emits at
    /// once.
    pub(super) fn open_view_item(&mut self, tag: &str, frames: &mut Vec<Frame>) {
        // A dangling capture means a previous block never closed. Surface it
        // rather than merging two responses into one.
        if self.view_item.is_some() {
            frames.push(self.finish_view_item(Some("malformed")));
        }
        self.view_item = Some(ViewItem {
            token: text::attribute(tag, "id").unwrap_or_default(),
            exist: text::attribute(tag, "exist").unwrap_or_default(),
            state: text::attribute(tag, "state"),
            // Presence is the signal; the value is irrelevant.
            closed: text::attribute(tag, "closed").is_some(),
            ..ViewItem::default()
        });
        if tag.ends_with("/>") {
            frames.push(self.finish_view_item(None));
        }
    }
}

impl ViewItem {
    /// Append to the open section, and to the open link if there is one.
    fn append_text(&mut self, text: &str) {
        if let Some(section) = self.current.as_mut() {
            section.text.push_str(text);
        }
        if let Some(link) = self.open_link.as_mut() {
            link.text.push_str(text);
        }
    }

    /// Move the open section into `results`, trimming its edge newlines.
    fn close_section(&mut self) {
        if let Some(mut section) = self.current.take() {
            // Trimmed in place: a detail section is up to 50 lines of
            // prose and reallocating it to drop edge newlines is waste.
            section
                .text
                .truncate(section.text.trim_end_matches('\n').len());
            let lead = section.text.len() - section.text.trim_start_matches('\n').len();
            section.text.drain(..lead);
            self.results.push(section);
        }
    }

    /// Record a finished link on the open section.
    fn push_link(&mut self, link: Link) {
        if let Some(section) = self.current.as_mut() {
            section.links.push(link);
        }
    }
}
