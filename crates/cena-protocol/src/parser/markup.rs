//! Markup state: the tags that style text instead of describing events.
//!
//! Split out of `dispatch.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap.
//!
//! These tags carry **no modelled frame**. They open and close bold, preset
//! and mono regions and push and pop links, and the effect shows up as the
//! [`crate::frame::Style`] and [`crate::frame::Link`] carried by the text
//! frames around them. That is why they live apart from the dispatcher: the
//! question "which frame is this tag" has no modelled answer for them.
//!
//! # They still emit, and this is the drop-nothing fix
//!
//! They used to emit nothing at all, via two `_ => {}` arms. That is fine
//! exactly while a neighbouring text frame follows -- and it is a silent drop
//! the moment one does not. `<preset id='speech'>said</preset>` at end of line
//! produced one `Text` showing the preset opening and **nothing** showing it
//! close; a bare `<popBold/>` produced no frames whatsoever. Every markup tag
//! now also emits [`crate::frame::Frame::Structural`], so the tag is
//! recoverable from `raw` whether or not a neighbour arrives.
//!
//! `<b>` is the interesting case. It does **not** feed `bold_depth`, and must
//! not: measured over 60 stratified corpus files, 234,531 `<b>` blocks and
//! 234,531 of them wrapping a `<pushBold/>...<popBold/>` pair, with **zero**
//! containing no `pushBold`. Counting both would report depth 2 on every
//! monsterbold creature in the game. `Structural` is how it becomes visible
//! without being double-counted.
//!
//! This paragraph used to say "`<b>` and `<i>`", and that `<i>` "does not
//! occur on this wire at all (0 in those 60 files)". The zero was real and
//! the conclusion was wrong: those files held no `<inventoryManager>`
//! response, which is the only place `<i>` appears. See [`is_markup`].

use super::Parser;
use crate::frame::Frame;
use crate::text;

impl Parser {
    /// Update markup state from a style/link tag, and record that it arrived.
    ///
    /// The state change is what a following [`Frame::Text`] reports; the
    /// [`Frame::Structural`] is what makes the tag visible when no following
    /// text frame exists. See the module header.
    pub(super) fn markup_tag(&mut self, tag: &str, frames: &mut Vec<Frame>) {
        self.markup_state(tag);
        frames.push(Frame::structural(text::tag_name(tag), tag));
    }

    /// The state half of [`Parser::markup_tag`], with no frame emitted.
    ///
    /// Separate because `parse_runs` replays markup *inside* a component body
    /// to build [`crate::runs::Runs`], where the effect is already carried by
    /// each run's own `style` and `link` -- a `Structural` frame there would
    /// have nowhere to go, since `parse_runs` returns runs and not frames.
    pub(super) fn markup_state(&mut self, tag: &str) {
        let name = text::tag_name(tag);
        if text::is_close_tag(tag) {
            match name {
                "a" | "d" => {
                    self.links.pop();
                }
                "preset" | "style" => {
                    self.presets.pop();
                }
                // `</b>` and `</i>` carry no state: bold depth is owned by
                // `<pushBold>`/`<popBold>`, which are separate tags. See the
                // module header for the 234,531/234,531 measurement that says
                // giving them one would double-count monsterbold.
                _ => {}
            }
            return;
        }
        match name {
            "pushBold" => self.bold_depth = self.bold_depth.saturating_add(1),
            "popBold" => self.bold_depth = self.bold_depth.saturating_sub(1),
            // A FONT INSTRUCTION, not a block boundary:
            //
            // > **AUTHOR, 2026-09-20:** *"the mono marker is for the frontend
            // > more so they know to swap between normal font and mono font
            // > for the output. then swap back at the output \"\"."*
            //
            // Which is why it is carried as a flag on the runs rather than
            // used to frame anything. `<output class="mono"/>` opens both a
            // `feat list` table AND the usage text a bare `feat` prints, so a
            // consumer that treated it as "a table starts here" would parse
            // `USAGE: FEAT {feat}` as rank rows. `cena-model`'s
            // `a_bare_command_opens_no_table` holds that line.
            "output" => {
                self.mono = text::attribute(tag, "class").as_deref() == Some("mono");
            }
            "preset" | "style" => {
                // `<style id=""/>` closes the region rather than opening one.
                match text::attribute(tag, "id") {
                    Some(id) if !id.is_empty() => self.presets.push(id),
                    _ => {
                        self.presets.pop();
                    }
                }
            }
            "a" | "d" => self.links.push(open_link(tag)),
            // `<b>` and `<i>` reach here and change nothing, deliberately.
            _ => {}
        }
    }
}

/// The [`Link`](crate::frame::Link) an opening `<a>` or `<d>` starts.
///
/// Shared with the `<inventoryViewItem>` capture, which used to call
/// [`text::link_from_tag`] alone and so dropped every bare `<d>` -- the
/// fallback below lived only here, and a `<d>` in an item description lost
/// its link while the same `<d>` anywhere else kept one (review 2026-09-23).
pub(super) fn open_link(tag: &str) -> crate::frame::Link {
    if let Some(link) = text::link_from_tag(tag) {
        return link;
    }
    let kind = if text::tag_name(tag) == "d" {
        // A bare `<d>` has no cmd: the link TEXT is the command, and
        // `LinkKind::DirectText` says so in the type. It was
        // `Direct { cmd: String::new() }` here, which shipped an empty
        // command to the consumer -- on 85% of direct links, every room exit
        // among them. See `LinkKind::DirectText`.
        crate::frame::LinkKind::DirectText
    } else {
        // An `<a>` carrying NONE of href/exist/cmd is not a command link, and
        // must not be turned into one.
        //
        // `DirectText` means "send the link text as a command", so
        // `<a char='Someone' game='GSIV'>Someone</a>` (wiki `:317-318`)
        // became a link that would send the player's NAME to the game. That
        // is invention, not omission -- the failure mode the drop-nothing
        // rule is least able to tolerate, because a consumer cannot tell a
        // fabricated command from a real one.
        //
        // No link is the honest answer: the text still reaches the consumer,
        // and the attributes still ride the surrounding `Frame::Structural`'s
        // raw bytes. The same applies to any `<a>` attribute Simutronics adds
        // later -- it arrives as plain text rather than as a command nobody
        // authored.
        crate::frame::LinkKind::NotActionable
    };
    crate::frame::Link {
        kind,
        text: String::new(),
        coord: None,
    }
}

/// Tags that only change style state.
///
/// **`i` was here and does not belong.** It was written in this crate's first
/// commit (`2aaba9c`) as `"b" | "i"`, which is HTML instinct: those two travel
/// together in HTML, so both were typed. `GemStone` has no italic tag.
///
/// Neither reference supports it. `VellumFE` lists `i` only in its known-tags
/// table (`src/parser/text.rs:182`) and never styles on it; Lich's
/// `common/xmlparser.rb` has no `i` handling at all.
///
/// MEASURED over the author's September logs -- **5,364 `<i>` tags, and all
/// 5,364 are `<inventoryManager>` item rows. Zero italics:**
///
/// ```sh
/// grep -oh '<i[ >]' *.xml | wc -l                                  # 5364
/// grep -ohE '<inventoryManager [^>]*>.*' *.xml | grep -oh '<i[ >]' | wc -l
/// #                                                                  5364
/// ```
///
/// The cost was not cosmetic. Claiming the tag here meant every inventory
/// item degraded to `Frame::Structural { name: "i", raw: "<i id=... />" }`
/// with its attributes trapped in an unparsed string -- the same shape the
/// golden corpus caught in `crtrStatus`. A styling arm was eating a payload.
///
/// This is Rule 2.2a in its purest form: not a partially consumed payload,
/// but one consumed by the **wrong handler entirely**, on an assumption that
/// was never checked against a source.
///
/// `b` is real and stays: 91,254 in the same logs, `</b>` balanced exactly,
/// wrapping bold creature names.
pub(super) fn is_markup(name: &str) -> bool {
    matches!(
        name,
        "a" | "d" | "b" | "preset" | "style" | "pushBold" | "popBold" | "output"
    )
}
