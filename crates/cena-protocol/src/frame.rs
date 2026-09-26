//! The frame vocabulary: what the wire can say.
//!
//! Ported whole from `reference/VellumFE/src/parser.rs:37-405`
//! (`ParsedElement`), per `plan/13` §4a -- "port aggressively where knowledge
//! lives in the code" -- and CLAUDE.md, "do not reinvent the frame
//! vocabulary". Two years of protocol archaeology are encoded in this list and
//! none of it is rediscoverable cheaply.
//!
//! # Counts, measured rather than restated (plan/05 §-2)
//!
//! `ParsedElement` has **63** variants, not the 61 CLAUDE.md claims:
//!
//! ```text
//! $ sed -n '38,404p' src/parser.rs | grep -cE '^    [A-Z][A-Za-z0-9]*'
//! 63
//! ```
//!
//! run in `reference/VellumFE`, with the enum spanning `:37-405`. The names
//! deduplicate to 63, so no repeat inflates it. CLAUDE.md's "61 variants" and
//! its "~130 tags" are both wrong; see [`crate::tags`] for the second number.
//! Flagged for amendment under `plan/05` §10 rather than silently corrected.
//!
//! # What is deliberately NOT ported, and why
//!
//! **Four of the 63 are not wire vocabulary at all.** Porting them would teach
//! Cena a protocol the game does not speak -- the exact trap CLAUDE.md flags
//! for `vellumImg`.
//!
//! - `VellumImage`, `VellumCommand`, `VellumTimer`. VERIFIED absent from
//!   `KNOWN_WIRE_TAGS` (`grep -c '"vellumImg"' src/parser/text.rs` -> 0, same
//!   for the other two) and dispatched *above* the unknown-tag branch at
//!   `src/parser.rs:877-890`, so in Vellum they never reach it. They are
//!   injected by the client into its own log. Cena does not inject them, so
//!   they correctly fall to [`Frame::UnknownTag`] if one ever appears.
//! - `Event`. Not produced by any tag: it is produced by **user-configured
//!   regex detectors over text** (`use crate::config::EventAction` at
//!   `src/parser.rs:18`). A protocol parser whose output depends on a user's
//!   config file is not a protocol parser, and Rule 2.1 (`plan/05:270-274`)
//!   puts game meaning above this layer. It belongs in `cena-behavior`,
//!   consuming frames.
//!
//! `LichWebUI` is the same category one step removed: it is Lich's own
//! handshake, and since Cena *is* the Lich replacement it has no upstream Lich
//! to handshake with. Dropped too, and left to fall through to
//! [`Frame::UnknownTag`] if it ever arrives.
//!
//! # The arithmetic, stated so it can be checked
//!
//! This enum has **55** variants:
//!
//! ```text
//! $ awk '/^pub enum Frame \{/,/^\}/' src/frame/vocabulary.rs | grep -oE '^    [A-Z][A-Za-z0-9]*' | sort -u | wc -l
//! 55
//! ```
//!
//! (`sort -u` is load-bearing: `ActiveEffect` is both a variant name and the
//! struct it wraps, so without it the line matches twice and the count is 52.)
//!
//! This said **50**, beside the command that prints 51 -- a number restated
//! rather than re-run, refuted by the evidence quoted under it (review
//! PR-13). The missing one is `Structural`, a Cena addition the arithmetic
//! below never counted (`grep -c Structural` over Vellum's parser: 0).
//!
//! `63 - 5 - 5 - 7 + 9 = 55`, where:
//!
//! - **-5 not-wire variants**, each named above: `VellumImage`,
//!   `VellumCommand`, `VellumTimer`, `Event`, `LichWebUI`.
//! - **-5 by collapsing the widget family.** Vellum's `DialogButtons`,
//!   `DialogControls`, `DialogDropDowns`, `DialogFields`, `DialogLabelList`
//!   and `DialogProgressBars` are six variants of one shape -- "the game sent
//!   a widget tag with these attributes" -- so they become one
//!   [`payload::DialogWidgets`] keyed by the tag name. Rule of
//!   three (`plan/05` §-1). `DialogOpen` and `DialogPanelOpen` are unaffected;
//!   they carry a dialog, not a widget.
//! - **-7 declared but never constructed.** An earlier draft of this file
//!   counted `ClearActiveEffects`, `InjuryPopup`, `MindStateExp`,
//!   `QuickbarEntries`, `QuickbarOpen`, `SpellHand` and `TargetList` as
//!   ported, and the arithmetic read `63 - 5 - 5 + 4 = 57`. Nothing in this
//!   crate could produce any of them:
//!
//!   ```text
//!   $ grep -rn "Frame::SpellHand" crates/cena-protocol/ --include=*.rs | grep -v src/frame/vocabulary.rs
//!   (no output; same for the other six)
//!   ```
//!
//!   **A variant nothing can construct is not a port**, so they are gone and
//!   the count says 50. Five of the seven -- injuries, quickbar and the target
//!   dropdown -- are dialog modelling that `plan/12` §7.1 puts outside M1;
//!   their wire traffic is real (34,550 `id='injuries'` and 6,694
//!   `<switchQuickBar` in a 272-file sample) and they come back when that
//!   scope arrives, with handlers, in one commit. The other two were
//!   redundant rather than deferred: `SpellHand` is `Spell` again under
//!   another name for a different widget, which is render intent and Rule 2.1
//!   puts it above this crate; `ClearActiveEffects` is what
//!   [`Frame::ClearDialogData`] already emits now that `clear='t'` is read on
//!   the open tag.
//! - **+9 Cena adds** ([`Frame::EndSetup`], typed 2026-09-23 because readiness
//!   keys on it, is the eighth; [`Frame::PlayerId`], typed 2026-09-26 because
//!   the group's leader keys on it, the ninth): [`Frame::UnknownTag`] and
//!   [`Frame::MalformedTag`], both mandated by Rule 2.2; [`Frame::ClientCommand`] and
//!   [`Frame::ClientSettings`], which the gated corpus replay found in real
//!   traffic that Vellum's vocabulary does not name; and
//!   [`Frame::Structural`], which types the tags that carry no payload of
//!   their own so Rule 2.2's "nothing is silently dropped" holds for them too.
//!   This read `+4` and omitted `Structural`, which is where the 50 came
//!   from. It then read `+5`, before [`Frame::CmdListUpdate`] and
//!   [`Frame::CmdTimestamp`] were added: the extended feed pushes dictionary
//!   rows that Vellum never typed, because Vellum reads the `<menu>` and not
//!   the `<cmdlist>` that keeps its labels current.
//!
//! One variant is renamed rather than changed (`LaunchURL` ->
//! [`Frame::LaunchUrl`]), which nets to zero. Every dropped variant is
//! recorded above rather than quietly omitted.
//!
//! **Presentation is not ported either** (Rule 2.1: the `Frame` is the
//! vocabulary boundary; this crate carries wire shape, not render intent):
//!
//! - `SpanType` (`Normal | Link | Monsterbold | Spell | Speech`,
//!   `src/parser.rs:28-34`) is a render class. The preset id travels as data
//!   in [`Style::preset`] instead, so a preset the game invents tomorrow needs
//!   no change in this crate.
//! - `fg_color` / `bg_color` on `Text` are resolved against `self.presets`, a
//!   **user-configurable palette** (`src/parser/handlers.rs:112-124`). That
//!   makes parser output depend on user config -- the same bytes give
//!   different frames under a different theme, which is disqualifying for a
//!   replay test. Cena emits the markup structure and lets `cena-ui` resolve
//!   colour.
//! - The sentinels `DIRECT_LINK_SENTINEL` / `URL_LINK_SENTINEL`
//!   (`src/data/widget.rs:269-273`) stuff a kind discriminator into a
//!   `String`. [`LinkKind`] makes it a real enum, which removes both. The
//!   URL-scheme allowlist (`is_web_url`, `:278-280`) is click-safety policy
//!   and belongs with the layer that opens browsers.

mod methods;
mod payload;
mod vocabulary;

pub use payload::{
    ActiveEffect, Amount, Capacity, CmdListEntry, CmdListUpdate, Continuation, DialogWidgets,
    InventoryItem, InventoryResponse, ItemDetail, ItemView, Link, LinkKind, Menu, MenuItem,
    Objective, ObjectivesAction, ProgressBar, RoomMeta, Style, TextFrame,
};
pub use vocabulary::Frame;

/// Attribute bag: name/value pairs exactly as the wire spelled them.
pub type Attrs = Vec<(String, String)>;
