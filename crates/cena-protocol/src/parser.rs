//! The line transducer: bytes in, [`Frame`]s out.
//!
//! # Why hand-rolled and not an XML library
//!
//! The wire has **no root element**. It is an unbounded sequence of fragments,
//! and several tags are mode switches with no close: a corpus sample of
//! 5,079,826 tags measured 23,328 `<style ...>` opens and **zero** `</style>`
//! closes, plus `<output class="mono"/>` / `<output class=""/>` as an
//! open/close pair that is self-closing at both ends. A tree-building parser
//! must either error or invent structure, and Rule 2.2 (`plan/05:276-283`)
//! forbids erroring on unknown input.
//!
//! The malformations that would justify a tolerant library are **absent**. In
//! that same sample: 0 tags split across a newline, 0 unquoted attribute
//! values, 0 tags opened without a `>` on the line. The wire is well-lexed
//! even though it is not well-formed. A substring scanner is sufficient, and
//! sufficient is the KISS answer (`plan/05` §-1). Vellum reached the same
//! conclusion: `quick-xml` is in its manifest but appears nowhere in
//! `src/parser*` -- only in config and static-data loaders.
//!
//! # The read boundary
//!
//! **A [`Parser`] consumes whole lines and never sees a TCP fragment.** The
//! socket reassembles with `read_until(b'\n')` before decoding, which is
//! Vellum's approach (`reference/VellumFE/src/network.rs:534`) and is correct:
//! the measurement above says the line is a safe frame boundary on this wire.
//! `read_until` on bytes rather than `read_line` on a `String` is also load
//! bearing -- the stream is CP1252, and `read_line` returns `InvalidData` on
//! any non-UTF-8 byte, which used to hard-disconnect Vellum.
//!
//! [`Parser::push_bytes`] implements that reassembly here, so the rule is
//! enforced by this crate rather than assumed of its caller, and so a test can
//! drive a split at every byte offset.
//!
//! **Where Vellum has a bug, this does not port it.** A tag with no `>` before
//! the line ends is appended to the text buffer with no log and no type
//! (`src/parser.rs:733-736`, "No closing >, treat rest as text"); parser state
//! then silently desyncs. Its own fixture contains this case
//! (`tests/fixtures/parser_edge_cases.xml:31`) and no test asserts anything
//! about it. Here it is [`Frame::MalformedTag`]: typed, logged, rendered.
//!
//! # MULTI-LINE CAPTURES: measured, and deliberately not built
//!
//! An earlier draft buffered a `<compDef>`/`<component>`/`<inv>` whose close
//! tag was on a later line, accumulating until the close arrived. That path is
//! **gone**, on evidence:
//!
//! ```text
//! $ grep -h -cE '<(compDef|component|inv)[^>]*>[^<]*$' <272 stratified files>
//! 0                       # opens with no close on the same line
//! $ grep -h -o '<compDef' <same files> | wc -l
//! 447095                  # opens in total
//! ```
//!
//! and, driving the parser itself over 60 of those files, **1,230,355 wire
//! lines with the capture state entered zero times.** Two years of traffic
//! does not contain the case.
//!
//! What it did contain was a black hole. The buffer was bounded by
//! `MAX_LINE_BYTES` of *accumulated body*, which is not a bound on frames
//! lost: at a realistic 60-character line, **4,297 consecutive lines of game
//! text were swallowed** before the cap fired -- 3,236 at 80 characters, 1,019
//! at 256. Only a `<prompt>` broke it sooner. So the machinery carried a
//! multi-thousand-line silent-loss risk for a case the wire has never
//! produced, which is precisely the abstraction `plan/05` §-1 says to not
//! build until the third occurrence.
//!
//! An unclosed paired tag is now [`Frame::MalformedTag`], same as any other
//! tag with no terminator: typed, surfaced, and bounded to its own line.
//! **Upgrade trigger:** if the Tier 2 replay ever reports a `MalformedTag`
//! whose `raw` begins `<compDef`, `<component` or `<inv`, the wire has started
//! sending this and the buffering path comes back -- bounded by *lines*, two
//! or three, not by bytes.
//!
//! # The prompt as a resync barrier
//!
//! Vellum's single best idea (`src/parser/handlers.rs:290-314`, owner decision
//! 2026-08-27): at every `<prompt>`, force-close open streams and drop
//! orphaned markup. It is what makes the push/pop imbalance harmless -- the
//! corpus has 19,142 `pushStream` against 14,714 `popStream`, because
//! `pushStream` is not a stack discipline. Prompts are frequent (207,626 in
//! the sample), so this **bounds all state corruption to one round**.
//!
//! # What this parser does NOT do
//!
//! `is_gsl_tag_line` is **not** ported. Vellum drops any line beginning `GS` +
//! a lowercase letter (`src/parser/text.rs:394`), which I confirmed by running
//! it also swallows `"GSrule violations are handled by the GameMasters."` and
//! `"GShaldi the merchant nods at you."` -- ordinary game prose, silently
//! discarded, contradicting the same file's stated policy. Cena talks to the
//! game directly as well as through Lich, so the latent case is live. GSL
//! framing, when needed, must key on the `\x1C` prefix that actually marks it.

use crate::frame::Frame;
use crate::text;

mod dispatch;
mod emit;
mod inner;
mod markup;
mod read;
mod thin;
mod wire;

use wire::{CLIENT_OPEN, COMMENT_CLOSE, SETTINGS_CLOSE, client_region};

/// Hard ceiling on a single accumulated line.
///
/// A close tag that never comes must not buffer forever; Vellum applies the
/// same guard to multi-line captures (`src/parser/handlers.rs:199-206`). At
/// the limit the buffer is flushed as text rather than grown, so a hostile or
/// broken peer costs bounded memory instead of the process.
const MAX_LINE_BYTES: usize = 256 * 1024;

/// Stateful, per-session line transducer.
///
/// One per session: it holds the stream stack and markup state for **that**
/// character's connection. Rule 5.2 (`plan/05:400-408`) -- no process globals
/// -- is why this is an owned value with no `static` behind it, and it is what
/// lets 3-25 sessions share a process.
#[derive(Debug, Default)]
pub struct Parser {
    /// Bytes received but not yet terminated by a newline.
    pending: Vec<u8>,
    /// Stream ids pushed and not yet popped; the last is current.
    streams: Vec<String>,
    /// Open `<pushBold>` scopes.
    bold_depth: u16,
    /// Innermost `<preset>` / `<style>` id, if any.
    presets: Vec<String>,
    /// Inside an `<output class="mono"/>` region.
    mono: bool,
    /// Open `<a>` / `<d>` links, outermost first.
    links: Vec<crate::frame::Link>,
    /// The `<dialogData id=>` currently open, if any.
    ///
    /// Vitals arrive as `<progressBar>` *inside* `<dialogData id='minivitals'>`,
    /// and the same bar id appears in other dialogs -- `health` in
    /// `minivitals` and `health2` in `injuries`. Without the enclosing id a
    /// consumer cannot tell which gauge it is looking at.
    dialog: Option<String>,
    /// Inside the login `<settings>` blob, whose close has not arrived.
    ///
    /// State rather than a within-line scan because the blob exceeds a line:
    /// 513,700 bytes in one measured case, which `MAX_LINE_BYTES` splits.
    in_settings: bool,
    /// Discarding the tail of a line that exceeded [`MAX_LINE_BYTES`].
    ///
    /// Set when the cap fires, cleared by the next newline. It is what lets
    /// the parser resume on a known boundary instead of mid-token: see
    /// [`Parser::push_bytes`] for the markup-into-prose bug that required it.
    dropping_oversized_line: bool,
}

impl Parser {
    /// A parser with no state. Cheap; one per session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Consume a line belonging to an open login `<settings>` blob.
    ///
    /// `Some(frames)` means the line was the blob's and `parse_line` is done
    /// with it; `None` means the blob is not open and the line is ordinary.
    ///
    /// Inside the blob the line is client configuration rather than game
    /// output, so it is consumed whole. A `<prompt>` breaks the region for the
    /// same reason it breaks a capture: a blob whose close never arrives must
    /// not swallow the rest of the session.
    fn continue_settings_blob(&mut self, line: &str) -> Option<Vec<Frame>> {
        if !self.in_settings {
            return None;
        }
        if let Some(at) = line.find(SETTINGS_CLOSE) {
            self.in_settings = false;
            let after = line[at + SETTINGS_CLOSE.len()..].to_owned();
            return Some(if after.is_empty() {
                Vec::new()
            } else {
                self.parse_line(&after)
            });
        }
        if !line.contains("<prompt") {
            return Some(Vec::new());
        }
        self.in_settings = false;
        None
    }

    /// Parse one complete line, newline already removed.
    ///
    /// Never panics on any input: that is `plan/06` §1.5's non-negotiable, and
    /// `tests/parser_never_panics.rs` drives it with arbitrary bytes and with
    /// real malformed corpus fragments.
    pub fn parse_line(&mut self, line: &str) -> Vec<Frame> {
        // **Wrapped so the line-end mark happens exactly once, on every path.**
        // The inner function has several early returns -- a settings blob, a
        // blank line, the oversized guard -- and marking at each is the version
        // that rots: a new early return would silently emit text runs that no
        // consumer could terminate, which is the bug this field exists to fix.
        let mut frames = self.parse_line_inner(line);
        Self::mark_line_end(&mut frames);
        frames
    }

    fn parse_line_inner(&mut self, line: &str) -> Vec<Frame> {
        let line = line.trim_end_matches(['\r', '\n']);
        let mut frames = Vec::new();

        if let Some(done) = self.continue_settings_blob(line) {
            return done;
        }

        // Preserve intentional blank lines: vertical spacing is content.
        if line.is_empty() {
            frames.push(self.text_frame(""));
            return frames;
        }

        let mut buffer = String::new();
        let mut rest = line;

        while !rest.is_empty() {
            let Some(start) = text::find_tag_start(rest) else {
                buffer.push_str(rest);
                break;
            };
            if start > 0 {
                buffer.push_str(&rest[..start]);
            }
            let tail = &rest[start..];

            // `<!-- CLIENT --> ... <!-- ENDCLIENT -->` is a client-echo
            // REGION, not a comment pair, and everything inside it is the
            // client's own traffic rather than server output. The Tier 2
            // replay found it by reporting `!--`, `stgupd`, `options`, `o`,
            // `w` and `<r` as unknown tag names -- all of them inside such a
            // region, none of them ever sent by the game.
            //
            // The player's typed command is the one part worth keeping:
            // `<!-- CLIENT --><c>;go2 3609<!-- ENDCLIENT -->` is how a replay
            // knows what the player did. So the region is consumed here and
            // its `<c>` body becomes a frame; the settings chatter around it
            // is discarded with the region rather than misread as protocol.
            if tail.starts_with(CLIENT_OPEN) {
                self.flush(&mut buffer, &mut frames);
                let (frame, after) = client_region(tail);
                if let Some(frame) = frame {
                    frames.push(frame);
                }
                rest = after;
                continue;
            }

            // `<settings> ... </settings>` is the client-configuration blob
            // the server sends once at login: window layout, palettes,
            // highlight strings, sound files, macros. It is a self-contained
            // document with its own vocabulary -- `h`, `dc`, `cmdline`,
            // `ignores`, `panels`, `toggles` and about twenty more, none of
            // them game protocol.
            //
            // VERIFIED in `GST-Nisugi/2024/11/xml/2024-11-07_20-24-19.xml`:
            // the open tag, the close tag and all 26 distinct inner names are
            // on line 6, one blob. The Tier 2 replay surfaced them a few at a
            // time as the sample widened, which is what made clear that the
            // right unit is the REGION, not a list of names to keep extending.
            //
            // Cena is its own client and keeps its own configuration, so the
            // blob is consumed whole and reported as one frame. Modelling its
            // contents would claim a protocol understanding this crate does
            // not have; letting its inner names reach `UnknownTag` would cry
            // wolf on every login.
            // The region is parser STATE, not a within-line scan, because the
            // blob is bigger than a line: VERIFIED at 513,700 bytes on one
            // line in `GST-Nisugi/2025/10/xml/2025-10-20_00-06-46.xml`, which
            // the MAX_LINE_BYTES guard necessarily splits. A scan that only
            // looked inside this line would lose the close and leak the blob's
            // 26 private element names into UnknownTag on every login.
            if tail.starts_with("<settings ") || tail.starts_with("<settings>") {
                self.flush(&mut buffer, &mut frames);
                frames.push(Frame::ClientSettings);
                let Some(at) = tail.find(SETTINGS_CLOSE) else {
                    self.in_settings = true;
                    break;
                };
                rest = &tail[at + SETTINGS_CLOSE.len()..];
                continue;
            }

            // Any other XML comment is not an element: it has no name, and `>`
            // may appear inside it, so it is consumed on its own terms.
            if let Some(body) = tail.strip_prefix("<!--") {
                let Some(at) = body.find(COMMENT_CLOSE) else {
                    self.flush(&mut buffer, &mut frames);
                    frames.push(Frame::MalformedTag {
                        raw: tail.to_owned(),
                    });
                    break;
                };
                rest = &body[at + COMMENT_CLOSE.len()..];
                continue;
            }

            let Some(close) = tail.find('>') else {
                // Rule 2.2, and Vellum's silent-desync bug fixed: a tag that
                // never closed is typed and logged, not smuggled into prose.
                self.flush(&mut buffer, &mut frames);
                frames.push(Frame::MalformedTag {
                    raw: tail.to_owned(),
                });
                break;
            };
            let open = &tail[..=close];
            rest = &tail[close + 1..];

            // A paired tag is handed to dispatch WHOLE -- `<prompt>x</prompt>`,
            // not `<prompt>` -- because its body is the frame's content. Cut
            // at the first `>` instead and every paired handler sees an empty
            // body, which is a failure three goldens caught. Vellum has the
            // same structure (`src/parser.rs:637-694`, its PAIRED_TAGS scan
            // runs before the single-tag scan).
            let mut tag = open;
            if !open.ends_with("/>") && !text::is_close_tag(open) && is_paired(open) {
                let close_tag = format!("</{}>", text::tag_name(open));
                // A paired tag whose close is not on this line is typed and
                // surfaced, never buffered: see MULTI-LINE CAPTURES in the
                // module header for the measurement that removed the
                // buffering path, and Rule 2.2 for why the answer is a frame
                // rather than a silent drop.
                let Some(at) = tail.find(&close_tag) else {
                    self.flush(&mut buffer, &mut frames);
                    frames.push(Frame::MalformedTag {
                        raw: tail.to_owned(),
                    });
                    break;
                };
                let end = at + close_tag.len();
                tag = &tail[..end];
                rest = &tail[end..];
            }

            self.dispatch(tag, &mut buffer, &mut frames);
        }

        self.flush(&mut buffer, &mut frames);
        frames
    }
}

/// Tags whose body is content and must be dispatched whole.
///
/// Ported from Vellum's `PAIRED_TAGS` (`src/parser.rs:637-651`), minus
/// `objectives`, whose body is a run of `<objective>` children rather than
/// text. `<a>` and `<d>` are absent deliberately: their bodies are display
/// text that belongs in the text stream, so they stay markup tags and their
/// close is what pops the link.
fn is_paired(tag: &str) -> bool {
    matches!(
        text::tag_name(tag),
        "prompt"
            | "compass"
            | "spell"
            | "left"
            | "right"
            | "component"
            | "compDef"
            | "inv"
            | "worldEvent"
    )
}
