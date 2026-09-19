//! Tag dispatch: which frame each wire tag becomes.
//!
//! # The M1 scope boundary, made explicit
//!
//! `plan/12` §7.1 scopes M1 to **room, prompt and vitals**. Those get real
//! handlers here. Every other tag in [`crate::tags`] still produces a typed
//! frame -- it parses, it reaches the caller, it renders -- but its fields are
//! the wire's attributes rather than a modelled interpretation. That is the
//! scope decision, and it is why there is no `todo!()` and no `panic!()`
//! anywhere in this crate: both are denied by the workspace lints, and
//! `plan/06` §1.5 calls a never-panicking parser non-negotiable.
//!
//! # Two silent-drop paths from Vellum that are deliberately not ported
//!
//! 1. **The known-but-unhandled swallow.** Vellum logs at `debug!` and
//!    discards (`src/parser.rs:1044-1046`) -- about 50 of the 116 tags vanish
//!    with no user-visible trace. Defensible for a client that models the rest
//!    itself; wrong for Cena's thin-handler tier, where "renders as text" is
//!    the requirement. Here every known tag produces a frame.
//! 2. **The four-tag pre-drop.** `src/parser.rs:1030-1036` discards
//!    `<compDef `, `</compDef>`, `<streamWindow ` and `<skin ` *before* the
//!    unknown check, with no log at all -- not even `debug!`. Two of those four
//!    are in M1's room path. Not ported.
//!
//! # The THIRD drop path, which the port introduced itself, and is now closed
//!
//! The two above are Vellum's. This one was Cena's own, and it is the record
//! of the author's correction of 2026-09-18:
//!
//! > "vellum drops some things it doesn't want, just because we didn't know
//! > what to do with them at the time. So Cena shouldn't drop anything that
//! > comes in, it should all be processed and figured out how to handle."
//!
//! Three arms returned without emitting anything: `close_tag`'s
//! `_ if tags::is_known(name) => {}`, and `markup_tag`'s two `_ => {}` arms in
//! `markup.rs`. `<resource>` with no `picture=` was a fourth, by way of an
//! `if let` with no `else`. **Being in [`crate::tags`] was what silenced a
//! tag** -- exactly backwards, and the same dead-ratchet shape `tags.rs`
//! already records, where an explicit arm shields a name from the check meant
//! to catch it.
//!
//! MEASURED before the fix, by generating every table tag in open, close and
//! self-closing form and counting frames:
//!
//! ```text
//! $ cargo test -p cena-protocol --test every_tag_is_observable
//! 133 tag/form combinations produce NO frame
//! ```
//!
//! **123 close forms -- every tag in the table -- and 10 self-closing**
//! (`a b d i output popBold preset pushBold resource style`). The close forms
//! of `indicator`, `castTime`, `roommeta` and `nav` were among them.
//!
//! Note what this is NOT. Most of those 123 never arrive: a census over 60
//! corpus files finds only 22 element names that ever appear as a close tag at
//! all, and `</indicator>` is not one of them. The rule is about what arrives,
//! and a tag Simutronics starts sending tomorrow arrives without warning --
//! Rule 2.2's premise exactly. So the fix had to be cheap across all 123
//! rather than 123 modelled cases.
//!
//! It is closed by [`Frame::Structural`]: every arm that did nothing now
//! pushes one, carrying the raw bytes. `<a>`/`</a>` and `<d>`/`</d>` are the
//! bulk of them and their effect genuinely IS in `Text.link` -- but only if a
//! following text frame exists, and at end of line none does, so they emit
//! too.
//!
//! # The cost, measured rather than guessed
//!
//! Replaying 60 stratified corpus files through `Parser::push_bytes`:
//!
//! ```text
//! files=60 total_frames=5912327 structural=2098146 (35.5%)
//!    1508330  a          130761  pushBold      2827  openDialog
//!     212831  dialogData  73252  style         1019  resource
//!     130761  popBold     29408  d              526  stream
//! ```
//!
//! **Just over a third of all frames.** That is the honest price of the rule
//! and it is why [`Frame::is_structural`] ships in this crate rather than
//! being left to each consumer: a renderer, the replay differ and a behavior
//! all need the same one-line predicate, which is the rule of three
//! (`plan/05` §-1) met at introduction rather than anticipated.
//!
//! The risk is that `Structural` becomes a dumping ground the way Vellum's
//! `debug!` swallow did. Two things hold it: it carries `raw`, so nothing is
//! unrecoverable, and `tests/every_tag_is_observable.rs` counts what lands
//! here, so a tag *moving into* `Structural` shows up in a diff instead of
//! silently.

use super::Parser;
use super::inner::inner_text;
use super::markup::is_markup;
use super::thin::thin_frame;
use crate::frame::{Frame, ProgressBar};
use crate::numbers::{parse_amount, parse_duration_secs};
use crate::runs::Runs;
use crate::tags;
use crate::text;

impl Parser {
    /// Route one complete tag to its handler.
    ///
    /// `buffer` holds display text accumulated since the last frame; a handler
    /// that emits a frame flushes it first so ordering is preserved.
    pub(super) fn dispatch(&mut self, tag: &str, buffer: &mut String, frames: &mut Vec<Frame>) {
        let name = text::tag_name(tag);
        let closing = text::is_close_tag(tag);

        // Markup tags change style state and emit no frame of their own, but
        // they do split the text run, so the buffer flushes first.
        if is_markup(name) {
            self.flush(buffer, frames);
            self.markup_tag(tag, frames);
            return;
        }

        if closing {
            self.close_tag(name, buffer, frames);
            return;
        }

        match name {
            // --- prompt: also the resync barrier ---------------------------
            "prompt" => {
                self.flush(buffer, frames);
                self.prompt(tag, frames);
            }

            // --- room -----------------------------------------------------
            "nav" => {
                self.flush(buffer, frames);
                // **No `unwrap_or_default()`.** A bare `<nav/>` is a real
                // shape -- an arrival at a room with no UID -- and turning it
                // into `id: ""` hands a consumer an empty string it cannot
                // tell from a real UID (review MO-10).
                frames.push(Frame::RoomId {
                    id: text::attribute(tag, "rm"),
                });
            }
            "streamWindow" => {
                self.flush(buffer, frames);
                frames.push(Frame::StreamWindow {
                    id: text::attribute(tag, "id").unwrap_or_default(),
                    title: text::attribute(tag, "title"),
                    subtitle: text::attribute(tag, "subtitle"),
                    // The other 12 attribute names were dropped here. See the
                    // variant's docs for the census; `location` and `target`
                    // ride ~99% of these tags.
                    attrs: text::attributes(tag),
                });
            }
            // Same fact, two controls: a streamBox and a stream window.
            // `clearDynaStream` used to reach `thin.rs` and become a
            // `StreamWindow` -- a clear reported as a DECLARATION (PR-4).
            "clearStream" | "clearDynaStream" => self.clear_stream(tag, buffer, frames),
            "pushStream" => {
                self.flush(buffer, frames);
                let id = text::attribute(tag, "id").unwrap_or_default();
                self.open_stream(id, false, frames);
            }
            // `<stream id=X>...</stream>` is the PAIRED form of the same
            // redirect (wiki `:9`, `:50`: "Inline (paired) redirect"), so it
            // must establish the same routing context.
            //
            // It previously fell through to `thin.rs`'s
            // `"streamId" | "stream" | ...` arm and became a
            // `Frame::StreamWindow` -- a WINDOW DECLARATION -- while its body
            // went to `main` untagged. Measured on a named corpus file: 31
            // `<stream id="Spells">` rows, each a spell-list entry, delivered
            // as story prose. A behavior filtering on `stream == "Spells"`
            // saw none of them, and a consumer keeping a window registry was
            // told a window had just been declared.
            //
            // **`dynaStream` is the same shape one control over**: the wiki
            // (`reference/wiki_clean/Wrayth protocol.txt:169`) calls it a
            // "text-content feed for a streamBox control", so its paired form
            // establishes the same routing context. It had the same defect --
            // on the wiki's own example (`:304`) it declared a window nobody
            // sent and routed the body to `main` (PR-4).
            "stream" | "dynaStream" if !tag.trim_end().ends_with("/>") => {
                self.flush(buffer, frames);
                let id = text::attribute(tag, "id").unwrap_or_default();
                self.open_stream(id, true, frames);
            }
            // Self-closing: no body to route, and still not a window.
            // Structural, so nothing is dropped (Rule 2.2's floor) --
            // `thin.rs`'s fallthrough would make it a `WindowHints`, which is
            // the same wrong answer in a different shape.
            "dynaStream" => self.structural_only("dynaStream", tag, buffer, frames),
            "popStream" => {
                self.flush(buffer, frames);
                self.pop_stream(tag, frames);
            }
            "compass" => {
                self.flush(buffer, frames);
                frames.push(Frame::Compass {
                    directions: directions(tag),
                });
            }
            // These always close on the line they open; one that does not is
            // a MalformedTag and never reaches dispatch.
            "component" | "compDef" | "inv" => {
                self.flush(buffer, frames);
                self.inline_paired(tag, frames);
            }
            // `picture=` is what makes a `<resource>` a room picture. Without
            // it there is nothing to model -- but "nothing to model" is not
            // "nothing happened", and the `if let` alone emitted no frame at
            // all, which is the drop this task closes.
            "resource" => {
                self.flush(buffer, frames);
                match text::attribute_u32(tag, "picture") {
                    Some(id) => frames.push(Frame::RoomPicture { id }),
                    None => frames.push(Frame::structural(name, tag)),
                }
            }

            // --- vitals ---------------------------------------------------
            "progressBar" => {
                self.flush(buffer, frames);
                frames.push(progress_bar(tag, self.dialog.as_deref()));
            }
            "dialogData" => {
                self.flush(buffer, frames);
                let id = text::attribute(tag, "id").unwrap_or_default();
                // `clear='t'` on the OPEN tag is the wire's real "replace this
                // dialog's contents" signal, and it is paired, not
                // self-closing: measured over 40 corpus files, 26,745
                // `clear='t'` opens against **zero** self-closing ones. The
                // previous code read only `ends_with("/>")`, so the form that
                // actually occurs produced no frame at all -- a stale buff
                // list would never be cleared. Vellum reads it on the open tag
                // too (`src/parser/dialogs.rs:433-438`).
                //
                // A self-closing `dialogData` has no body to open either, so
                // both it and `clear='t'` mean the same thing: clear the
                // dialog. A paired one additionally becomes the enclosing
                // dialog for the `<progressBar>`s inside it.
                let clears =
                    text::attribute(tag, "clear").as_deref() == Some("t") || tag.ends_with("/>");
                if clears {
                    frames.push(Frame::ClearDialogData { id: id.clone() });
                }
                if !tag.ends_with("/>") {
                    // **A paired open emits, whether or not it clears.**
                    //
                    // Only the `clears` branch above pushed anything, so
                    // `<dialogData id='expr'>` with no `clear=` produced no
                    // frame at all: the dialog opened, its widgets followed,
                    // and nothing told a consumer which dialog they were in
                    // (review PR-3).
                    //
                    // `DialogOpen` is the existing frame for "a dialog is
                    // open" -- `openDialog` uses it -- so this is the same
                    // fact reaching the same place rather than new vocabulary.
                    frames.push(Frame::DialogOpen {
                        id: id.clone(),
                        title: None,
                        attrs: text::attributes(tag),
                    });
                    self.dialog = Some(id);
                }
            }
            "clearDialogData" => {
                self.flush(buffer, frames);
                frames.push(Frame::ClearDialogData {
                    id: text::attribute(tag, "id").unwrap_or_default(),
                });
            }

            // --- everything else: typed, thin, never dropped ---------------
            _ => {
                self.flush(buffer, frames);
                frames.push(thin_frame(name, tag, self.dialog.as_deref()));
            }
        }
    }

    /// `<popStream>`: close the stream it NAMES, not merely the innermost.
    ///
    /// Popping the innermost entry regardless made
    /// `<pushStream id='a'/><pushStream id='b'/><popStream id='a'/>` pop `b`
    /// while announcing that `a` was popped and resumed, so every following
    /// line was routed to the wrong window. Nesting is real -- 151 lines in a
    /// 272-file sample carry two pushes -- and since `pushStream` is not
    /// stack-disciplined (19,142 pushes to 14,714 pops), a mismatch is the
    /// expected case rather than the exotic one.
    fn pop_stream(&mut self, tag: &str, frames: &mut Vec<Frame>) {
        let id = text::attribute(tag, "id");
        match &id {
            // Remove the named stream wherever it sits. If it is not open at
            // all, touch nothing: popping an unrelated stream to honour a pop
            // that does not apply is the bug above.
            Some(named) => {
                if let Some(at) = self.streams.iter().rposition(|s| &s.id == named) {
                    self.streams.remove(at);
                }
            }
            // A bare `<popStream/>` means "the current one", which is 625 of
            // the pops in that sample.
            None => {
                self.streams.pop();
            }
        }
        frames.push(Frame::StreamPop { id });
        self.resume_enclosing(frames);
    }

    /// Open a redirect, recording which form opened it.
    ///
    /// Shared by `<pushStream>` and the paired `<stream id=>`, which establish
    /// the same routing context and differ only in how they close -- see
    /// [`OpenStream`](super::OpenStream).
    fn open_stream(&mut self, id: String, paired: bool, frames: &mut Vec<Frame>) {
        self.streams.push(super::OpenStream {
            id: id.clone(),
            paired,
        });
        frames.push(Frame::StreamPush { id });
    }

    /// Announce the stream that is current again after a close, if any.
    ///
    /// A scalar consumer tracks one current stream, so it needs telling that an
    /// enclosing redirect is back in force; nothing is emitted when the stack
    /// empties, because `""` is the main window and `StreamPop` already said so.
    fn resume_enclosing(&mut self, frames: &mut Vec<Frame>) {
        if let Some(resumed) = self.streams.last().map(|s| s.id.clone()) {
            frames.push(Frame::StreamResume { id: resumed });
        }
    }

    /// `<prompt>`: emit the frame, then resync.
    ///
    /// The resync is Vellum's owner decision of 2026-08-27
    /// (`src/parser/handlers.rs:290-314`) and is what makes the measured
    /// `pushStream`:`popStream` imbalance of 1.301 harmless. Prompts are
    /// trustworthy in both Lich and direct modes, so every prompt force-closes
    /// whatever was left open. This bounds state corruption to one round.
    fn prompt(&mut self, tag: &str, frames: &mut Vec<Frame>) {
        let time = text::attribute(tag, "time").unwrap_or_default();
        let inner = inner_text(tag);
        // **The forced pops come BEFORE the prompt.**
        //
        // A consumer that snapshots on `Prompt` -- which is the documented
        // round boundary, and what makes prompts "trustworthy in both modes"
        // above -- would otherwise snapshot with streams still open, and see
        // them close after the boundary they define. Vellum emits them first
        // (`src/parser/handlers.rs:306-314,328`). Found by review (PR-12).
        for open in std::mem::take(&mut self.streams).into_iter().rev() {
            frames.push(Frame::StreamPopForced { id: open.id });
        }
        frames.push(Frame::Prompt {
            time,
            // **STRIPPED, like every other text that reaches a terminal.**
            //
            // `emit.rs:48`, `emit.rs:135` and `inner.rs:63` all pair
            // `decode_entities` with `strip_control_chars`; this one did not,
            // and the prompt is printed straight to stderr by `run.rs`.
            //
            // The game socket is PLAIN TCP (`plan/10`), so the bytes are not
            // merely untrusted-because-remote, they are modifiable in flight.
            // `<prompt time="1">&#27;]52;c;...&#7;</prompt>` decodes to a real
            // ESC and BEL: an OSC-52 sequence writes the terminal's clipboard.
            // Entities are decoded HERE, so the control characters do not
            // exist on the wire for anything upstream to have caught.
            // Found by review (PR-9).
            text: text::strip_control_chars(&text::decode_entities(&inner)),
        });
        self.bold_depth = 0;
        self.presets.clear();
        self.links.clear();
        self.mono = false;
        self.dialog = None;
    }

    /// A `<component>`/`<compDef>`/`<inv>`, which always opens and closes on
    /// one line: 447,095 opens in a 272-file corpus sample, none spanning a
    /// line (see MULTI-LINE CAPTURES in `parser.rs`).
    fn inline_paired(&mut self, tag: &str, frames: &mut Vec<Frame>) {
        let id = text::attribute(tag, "id").unwrap_or_default();
        let mut unmodelled = Vec::new();
        let body = self.parse_runs_reporting(&inner_text(tag), &mut unmodelled);
        // `<inv>` is a container's contents, not a room component: same shape
        // on the wire, different frame, so the name is what decides.
        if text::tag_name(tag) == "inv" {
            frames.push(Frame::ContainerItem {
                container_id: id,
                content: body,
            });
        } else {
            frames.push(Frame::Component { id, body });
        }
        // **After the frame, in wire order.** A consumer sees the component it
        // can use, then whatever the body carried that this parser does not
        // model. Emitted here rather than inside `parse_runs_reporting`
        // because only the caller knows which frame the body belonged to, and
        // Rule 2.2's "survives to display" means *after* the thing it was
        // found in (review PR-1).
        for raw in unmodelled {
            let name = text::tag_name(&raw).to_owned();
            if tags::is_known(&name) {
                frames.push(Frame::structural(&name, &raw));
            } else {
                frames.push(Frame::UnknownTag { name, raw });
            }
        }
    }

    /// A closing tag that is not markup.
    ///
    /// **Every close emits.** The only branch here is *which* frame: a close
    /// whose name this parser does not know is [`Frame::UnknownTag`] and worth
    /// a look (Rule 2.2); a close whose name it does know is
    /// [`Frame::Structural`] and is not. Neither is silent -- being in
    /// [`crate::tags`] used to be what silenced a tag, and that is the third
    /// drop path recorded in this file's header.
    fn close_tag(&mut self, name: &str, buffer: &mut String, frames: &mut Vec<Frame>) {
        self.flush(buffer, frames);
        // `</dialogData>` is the one close with state to undo: it ends the
        // dialog that encloses the `<progressBar>`s inside it.
        if name == "dialogData" {
            self.dialog = None;
        }
        // `</stream>` ends the paired redirect its opener established, and it
        // emits `StreamPop` to say so.
        //
        // It used to remove the entry and emit `Frame::Structural`. That kept
        // the parser's own stack right while telling a frame-stream consumer
        // nothing -- and the frame stream is the only thing a router can see.
        // MEASURED on the author's 2025-04-18 capture: 31 self-contained
        // `<stream id="Spells">` rows, and a frame stream standing at depth 31
        // when the next prompt arrived. The prompt barrier could not paper over
        // it either, and that is the part worth remembering: `mem::take` had
        // nothing left to force-pop, because the closer had already removed the
        // entries. The barrier's guarantee held for the parser and was empty for
        // everyone downstream.
        //
        // It closes the innermost entry a PAIRED `<stream>` opened, so a stray
        // close cannot unroute an enclosing `<pushStream>`. That distinction is
        // what [`OpenStream`](super::OpenStream) records, and the measurement
        // there is why: the paired form is exactly balanced on real traffic
        // while `pushStream` is not, so a close with no paired entry open is not
        // this stream's close. Such a close falls through and emits
        // `Structural` -- "every close emits" is the rule above, and a close
        // that popped nothing is not a pop.
        if name == "stream"
            && let Some(at) = self.streams.iter().rposition(|s| s.paired)
        {
            let open = self.streams.remove(at);
            frames.push(Frame::StreamPop { id: Some(open.id) });
            self.resume_enclosing(frames);
            return;
        }
        if tags::is_known(name) {
            frames.push(Frame::structural(name, &format!("</{name}>")));
        } else {
            frames.push(Frame::UnknownTag {
                name: name.to_owned(),
                raw: format!("</{name}>"),
            });
        }
    }
}

/// The `<dir value=>` tokens inside a `<compass>`.
fn directions(tag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = tag;
    while let Some(at) = rest.find("<dir ") {
        rest = &rest[at..];
        let Some(end) = rest.find('>') else { break };
        if let Some(value) = text::attribute(&rest[..=end], "value") {
            out.push(value);
        }
        rest = &rest[end + 1..];
    }
    out
}

/// `<progressBar>` -- the M1 vitals handler.
///
/// `value=` is a **percentage**, not the current value; the numbers live in
/// `text=`. See [`crate::numbers`] for the two bugs this avoids.
fn progress_bar(tag: &str, dialog: Option<&str>) -> Frame {
    let text_attr = text::attribute(tag, "text").unwrap_or_default();
    Frame::ProgressBar(ProgressBar {
        id: text::attribute(tag, "id").unwrap_or_default(),
        dialog: dialog.map(str::to_owned),
        percent: text::attribute_u32(tag, "value").unwrap_or_default(),
        amount: parse_amount(&text_attr),
        time_remaining_secs: text::attribute(tag, "time")
            .as_deref()
            .and_then(parse_duration_secs),
        text: text_attr,
    })
}

/// Runs helper kept next to its only caller.
impl Runs {
    /// True when every run is whitespace.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.runs.iter().all(|r| r.text.trim().is_empty())
    }
}

impl Parser {
    /// Emit a tag as [`Frame::Structural`] and nothing else.
    ///
    /// For a tag that is real, carries no payload this parser models, and must
    /// not be guessed at -- a self-closing `<dynaStream/>` is the case that
    /// needed it. Split out to keep `dispatch` under clippy's 100-line limit,
    /// which the `dynaStream` arms took it past.
    /// Emit [`Frame::ClearStream`] for `clearStream` / `clearDynaStream`.
    ///
    /// Split out with `structural_only` for the same reason: `dispatch` is a
    /// match and clippy counts its lines, so a new arm pays for itself by
    /// moving a body down.
    fn clear_stream(&mut self, tag: &str, buffer: &mut String, frames: &mut Vec<Frame>) {
        self.flush(buffer, frames);
        frames.push(Frame::ClearStream {
            id: text::attribute(tag, "id").unwrap_or_default(),
        });
    }

    fn structural_only(
        &mut self,
        name: &str,
        tag: &str,
        buffer: &mut String,
        frames: &mut Vec<Frame>,
    ) {
        self.flush(buffer, frames);
        frames.push(Frame::structural(name, tag));
    }
}
