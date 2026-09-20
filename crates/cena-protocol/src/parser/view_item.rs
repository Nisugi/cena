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
    /// Feed one line to an open `<inventoryViewItem>` capture.
    ///
    /// Returns `None` when no capture is open, so the caller parses normally.
    pub(super) fn continue_view_item(&mut self, line: &str) -> Option<Vec<Frame>> {
        self.view_item.as_ref()?;
        if let Some(view) = self.view_item.as_mut() {
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
                let mut frames = vec![self.finish_view_item(Some("truncated"))];
                frames.extend(self.parse_line(line));
                return Some(frames);
            }
        }
        Some(self.view_item_line(line))
    }

    /// Continue the line that opened the capture, from just after the
    /// envelope tag.
    ///
    /// Separate from [`Parser::continue_view_item`] because this is not a new
    /// line: it must not count against [`MAX_VIEWITEM_LINES`] and must not
    /// insert the newline that a real line boundary does.
    pub(super) fn continue_line_in_capture(&mut self, rest: &str) -> Vec<Frame> {
        self.view_item_line(rest)
    }

    /// Walk one line inside the capture, emitting only when the block ends.
    fn view_item_line(&mut self, line: &str) -> Vec<Frame> {
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
            let Some(end) = rest.find('>') else {
                self.view_item_text(rest);
                break;
            };
            let (tag, tail) = rest.split_at(end + 1);
            rest = tail;
            if let Some(done) = self.view_item_tag(tag, rest) {
                return done;
            }
        }
        Vec::new()
    }

    /// Handle one tag inside the capture. `Some` means the block ended and
    /// the rest of the line is ordinary feed again.
    fn view_item_tag(&mut self, tag: &str, rest: &str) -> Option<Vec<Frame>> {
        let closing = text::is_close_tag(tag);
        match (text::tag_name(tag), closing) {
            // A second envelope while one is open. The close of the first
            // never arrived, so the response is torn: surface what was
            // captured and start the new one, rather than flattening the
            // second envelope into the first block's prose and losing it
            // entirely. `open_view_item`'s own guard cannot fire here,
            // because an open capture owns the line before dispatch sees it.
            ("inventoryViewItem", false) => {
                let mut frames = vec![self.finish_view_item(Some("malformed"))];
                self.open_view_item(tag, &mut frames);
                if !rest.trim().is_empty() && self.view_item.is_some() {
                    frames.extend(self.view_item_line(rest));
                }
                Some(frames)
            }
            ("inventoryViewItem", true) => {
                let mut frames = vec![self.finish_view_item(None)];
                if !rest.trim().is_empty() {
                    frames.extend(self.parse_line(rest));
                }
                Some(frames)
            }
            // A prompt means the block was torn mid-send. Surface the partial
            // response rather than swallowing the rest of the session, and
            // let the prompt parse normally -- it is the resync barrier.
            ("prompt", _) => {
                let mut frames = vec![self.finish_view_item(Some("malformed"))];
                frames.extend(self.parse_line(&format!("{tag}{rest}")));
                Some(frames)
            }
            ("result", false) => {
                self.open_section(tag);
                None
            }
            ("result", true) => {
                if let Some(view) = self.view_item.as_mut() {
                    view.close_section();
                }
                None
            }
            ("br", _) => {
                self.view_item_text("\n");
                None
            }
            // An item link inside the prose. VellumFE flattens these away
            // (`src/parser/handlers.rs:866`); this parser already types
            // links, so the noun in a description stays clickable.
            ("a" | "d", false) => {
                if let (Some(link), Some(view)) =
                    (text::link_from_tag(tag), self.view_item.as_mut())
                {
                    view.open_link = Some(link);
                }
                None
            }
            ("a" | "d", true) => {
                if let Some(view) = self.view_item.as_mut()
                    && let Some(link) = view.open_link.take()
                {
                    view.push_link(link);
                }
                None
            }
            // Every other inline tag is styling, and a detail section is
            // prose: flattened away.
            _ => None,
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
