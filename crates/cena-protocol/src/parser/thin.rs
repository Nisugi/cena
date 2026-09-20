//! The thin tier: a typed frame for every wire tag M1 does not model.
//!
//! Split out of `dispatch.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap.
//!
//! `plan/12` section 7.1 scopes M1 to room, prompt and vitals; those have real
//! handlers in `dispatch.rs`. Everything else lands here, and the scope
//! decision is that it still **parses and renders**: a typed variant carrying
//! the wire's own attributes, never a `todo!()` and never a drop. Vellum
//! swallows roughly 50 of its known tags at `debug!` level
//! (`src/parser.rs:1044`); this module is the deliberate refusal to inherit
//! that.

use super::inner::inner_display_text;
use crate::frame::{ActiveEffect, Attrs, DialogWidgets, Frame};
use crate::tags;
use crate::text;

/// Shortest interval between mana/health pulses, in seconds.
///
/// The wire sends `<pulse mana="1"/>` bare -- VERIFIED as the only form in a
/// 272-file corpus sample, 1,419 of 1,419 occurrences -- so the interval is
/// implied rather than stated and a default is the only way to have one.
/// Vellum's (`src/parser/handlers.rs:761-765`); `0` would claim the pulse is
/// instantaneous.
const PULSE_MIN_SECS: u32 = 46;

/// Longest interval between mana/health pulses, in seconds. See
/// [`PULSE_MIN_SECS`].
const PULSE_MAX_SECS: u32 = 75;

/// The thin tier: a typed frame carrying the wire's own attributes.
///
/// Rule 2.2's second half lives here. A name in [`tags::is_known`] gets its
/// variant; a name that is not gets [`Frame::UnknownTag`] with the raw bytes,
/// so a protocol change announces itself instead of being swallowed.
pub(super) fn thin_frame(name: &str, tag: &str, dialog: Option<&str>) -> Frame {
    let attrs: Attrs = text::attributes(tag);
    let id = || text::attribute(tag, "id").unwrap_or_default();
    match name {
        "spell" => Frame::Spell {
            text: inner_display_text(tag),
        },
        "left" => Frame::LeftHand {
            item: inner_display_text(tag),
            link: text::link_from_tag(tag),
        },
        "right" => Frame::RightHand {
            item: inner_display_text(tag),
            link: text::link_from_tag(tag),
        },
        "roundTime" => Frame::RoundTime {
            value: text::attribute_u32(tag, "value").unwrap_or_default(),
        },
        "castTime" => Frame::CastTime {
            value: text::attribute_u32(tag, "value").unwrap_or_default(),
        },
        "timer" => Frame::AimTime {
            value: text::attribute_u32(tag, "value").unwrap_or_default(),
        },
        // KNOWN LIMIT, measured and deliberately not closed: a `<label>` with
        // a BODY (`<label id='x'>text</label>`) reports an empty `value` and
        // its text arrives as a separate `Text` frame.
        //
        // The body fallback below only fires for a body on the open tag, and
        // `<label>` is not in `is_paired`, so the paired form never reaches
        // here whole. Adding it there would close the gap -- and it is not
        // worth the line, because the form does not exist on this wire:
        //
        //   $ grep -h -o '<label'   <272 stratified files> | wc -l  -> 1486284
        //   $ grep -h -o '</label>' <272 stratified files> | wc -l  ->       0
        //
        // 1,486,284 opens, zero closes. Every one is self-closing with
        // `value=`. UPGRADE TRIGGER: a non-zero `</label>` count in the Tier 2
        // replay means adding `"label"` to `is_paired` in `parser.rs`.
        "label" => Frame::Label {
            id: id(),
            value: text::attribute(tag, "value").unwrap_or_else(|| inner_display_text(tag)),
            dialog: dialog.map(str::to_owned),
        },
        "crtrStatus" => Frame::CreatureStatus {
            id: text::attribute(tag, "exist").unwrap_or_default(),
            attrs,
        },
        "roommeta" => Frame::RoomMeta(room_meta(tag)),
        "closeDialog" | "closedialog" => Frame::CloseDialog { id: id() },
        "exposeDialog" | "exposeStream" | "exposeContainer" => Frame::Expose {
            kind: name.to_owned(),
            id: id(),
        },
        "deleteContainer" => Frame::DeleteContainer { id: id() },
        "app" => Frame::AppInfo {
            character: text::attribute(tag, "char").unwrap_or_default(),
            // `game` is the instance, and two characters of the same name in
            // Prime and Platinum are two different characters -- so a
            // multi-session client cannot identify a session without it.
            game: text::attribute(tag, "game").unwrap_or_default(),
            title: text::attribute(tag, "title").unwrap_or_default(),
        },
        "image" => Frame::InjuryImage {
            id: id(),
            name: text::attribute(tag, "name").unwrap_or_default(),
            dialog: dialog.map(str::to_owned),
        },
        // The `Icon` prefix is kept, deliberately, where Vellum strips it
        // (`src/parser/handlers.rs:427`). The wire id IS `IconSTUNNED` in all
        // 22,749 occurrences across a 272-file sample, and stripping is a
        // rename: it makes the frame disagree with the bytes for the benefit
        // of a presentation layer that does not exist yet. Rule 2.1 puts
        // parsing here and naming above; the layer that wants `STUNNED` can
        // strip five characters.
        //
        // `visible=` absent is treated as inactive, and that is safe rather
        // than merely convenient: the attribute is present in all 22,749 of
        // those occurrences, so "absent" is not a case the wire produces.
        "indicator" => Frame::StatusIndicator {
            id: id(),
            active: text::attribute(tag, "visible").as_deref() == Some("y"),
        },
        // `<objectives>` has NO arm here, deliberately. It is a paired tag,
        // so every form of it -- self-closing, empty, or carrying rows --
        // reaches `dispatch::objectives` and never this function. An arm
        // here was unreachable, and a mutation test proved it: hard-coding
        // its action to `FullRefresh` broke nothing, because nothing ran it.
        //
        // A stray `<objective>` outside any envelope IS reachable: typed as
        // itself rather than falling through to the placement-attrs bag.
        "objective" => stray_objective(tag),
        // A `<menu>` is assembled whole in `dispatch.rs`; this is the
        // stray-item path -- an `<mi>` outside any menu, which the wire is
        // not known to send. Typed as a one-item menu with no id rather than
        // dropped, so a wire change goes visible instead of silent.
        "menu" | "mi" => Frame::MenuResponse(crate::frame::Menu {
            id: id(),
            path: text::attribute(tag, "path"),
            categories: Vec::new(),
            items: vec![crate::frame::MenuItem {
                coord: text::attribute(tag, "coord"),
                noun: text::attribute(tag, "noun"),
                menu_cat: text::attribute(tag, "menu_cat"),
            }],
        }),
        "switchQuickBar" => Frame::QuickbarSwitch { id: id() },
        "openDialog" | "opendialog" => {
            let title = text::attribute(tag, "title");
            if text::attribute(tag, "resident").as_deref() == Some("true") {
                Frame::DialogPanelOpen {
                    id: id(),
                    title,
                    attrs,
                }
            } else {
                Frame::DialogOpen {
                    id: id(),
                    title,
                    attrs,
                }
            }
        }
        "cmdButton" | "closeButton" | "dropDownBox" | "editBox" | "upDownEditBox" | "skin"
        | "link" | "menuLink" | "sep" | "checkBox" | "radio" | "hScrollBar" | "vScrollBar" => {
            Frame::DialogWidgets(DialogWidgets {
                id: id(),
                dialog: dialog.map(str::to_owned),
                kind: name.to_owned(),
                widgets: vec![attrs],
            })
        }
        // Containers, inventory and the remaining singletons live in
        // `thin_frame_rest` so neither function exceeds the 100-line clippy
        // ceiling; the split is alphabetically arbitrary but the boundary is
        // stable, and both halves are one flat match.
        _ => thin_frame_rest(name, tag, attrs),
    }
}

/// The second half of [`thin_frame`]: containers, inventory, and the rest.
fn thin_frame_rest(name: &str, tag: &str, attrs: Attrs) -> Frame {
    let id = || text::attribute(tag, "id").unwrap_or_default();
    match name {
        "container" => Frame::Container {
            id: id(),
            title: text::attribute(tag, "title"),
            target: text::attribute(tag, "target"),
        },
        "clearContainer" => Frame::ClearContainer { id: id() },
        // The dictionary version the server just stated. Typed rather than
        // bagged because a client compares it against what it holds to know
        // whether it is current.
        "cmdtimestamp" => Frame::CmdTimestamp {
            version: text::attribute(tag, "data").unwrap_or_default(),
        },
        // The token is carried in `id=`, not in an attribute called `token`:
        // VERIFIED against `reference/VellumFE/src/parser/handlers.rs:910`,
        // `token: Self::extract_attribute(tag, "id")`. Reading `token` meant
        // the field was always empty, so no response could be matched to its
        // request. The tag is absent from a 272-file corpus sample, so this
        // was latent rather than live -- but an always-empty named field is a
        // claim the type makes and does not keep.
        // `inventoryManager` and `inventoryViewItem` are assembled rather
        // than thinned -- one in `dispatch.rs` (its body is `<i>` children on
        // one line), one in `view_item.rs` (its body spans lines). Neither
        // reaches this function.
        "launchURL" | "LaunchURL" => Frame::LaunchUrl {
            url: text::attribute(tag, "src")
                .or_else(|| text::attribute(tag, "url"))
                .unwrap_or_default(),
        },
        // `mana` is the attribute's VALUE, not its presence. `is_some()` made
        // the field carry no information at all: the wire always sends the
        // attribute, so every pulse read `true`. Measured over 272 corpus
        // files, the split is almost exactly even -- 710 `mana="1"` against
        // 709 `mana="0"` -- so half of all pulses were reported wrongly.
        // Vellum reads the value (`src/parser/handlers.rs:761`).
        //
        // The `min`/`max` defaults of 46/75 are load bearing and are Vellum's:
        // VERIFIED that every `<pulse>` in that sample is bare
        // (`<pulse mana="1"/>`), so the wire states the alternation and leaves
        // the interval implied.
        "pulse" => Frame::Pulse {
            mana: text::attribute(tag, "mana").as_deref() == Some("1"),
            min: text::attribute_u32(tag, "min").unwrap_or(PULSE_MIN_SECS),
            max: text::attribute_u32(tag, "max").unwrap_or(PULSE_MAX_SECS),
        },
        "worldEvent" => Frame::WorldEvent {
            realm: text::attribute(tag, "realm").unwrap_or_default(),
            expires_min: text::attribute_u32(tag, "expires"),
            text: inner_display_text(tag),
        },
        "PantheonStatus" => Frame::PantheonStatus {
            value: text::attribute_u32(tag, "value").unwrap_or_default(),
        },
        // `dynaStream` and `clearDynaStream` were here and are NOT window
        // declarations -- they feed and clear a streamBox control
        // (`Wrayth protocol.txt:169-170`). Handled in `dispatch.rs` beside
        // `<stream>`, which had the identical bug (review PR-4).
        "streamId" | "stream" => Frame::StreamWindow {
            id: id(),
            title: text::attribute(tag, "title"),
            subtitle: text::attribute(tag, "subtitle"),
            attrs,
        },
        // A known tag with no dedicated variant still becomes one: an effect
        // row when it looks like one, otherwise a window hint carrying its
        // attributes. Nothing is dropped.
        _ if tags::is_known(name) => known_fallback(name, tag, attrs),
        // Rule 2.2: unmodelled markup, carried whole, never dropped.
        _ => Frame::UnknownTag {
            name: name.to_owned(),
            raw: tag.to_owned(),
        },
    }
}

/// A known tag with no dedicated variant.
fn known_fallback(name: &str, tag: &str, attrs: Attrs) -> Frame {
    match name {
        "playerID" | "settings" | "settingsInfo" | "sentSettings" | "presets" | "palette"
        | "macros" | "endSetup" | "mode" | "FEVersion" | "LichWebUI" => Frame::WindowHints {
            id: name.to_owned(),
            attrs,
        },
        // `reward` was here and was destroying its own payload. The wire
        // sends `<reward type='fame' amount='20000'/>` -- self-closing, with
        // neither `id` nor `time` nor body text. Every field this arm reads
        // was therefore absent, and the result was
        // `ActiveEffect { category: "reward", id: "", text: "", time: None }`:
        // an empty husk, with `type` and `amount` dropped on the floor. That
        // is the drop-nothing rule (`plan/05` Rule 2.2) violated by a handler
        // that looked like it was handling something.
        //
        // VERIFIED against the corpus, 1,547-file stride-7 sample: 624
        // `<reward>`, **all 624** of the shape `<reward type='X' amount='X'/>`,
        // **0** carrying `id`, **0** carrying `time`. `type` is `fame` (312)
        // or `experience` (312).
        //
        // The fix is to delete it from this arm, not to grow the arm: falling
        // through to `WindowHints` below carries `attrs` whole, which keeps
        // `type` and `amount` and anything Simutronics adds later. Saga's
        // own parser reads a third form, `<reward type='custom' label='...'/>`
        // (`plan/15` §6.4); an `attrs` bag needs no change to carry it.
        "celebration" => Frame::ActiveEffect(ActiveEffect {
            category: name.to_owned(),
            id: text::attribute(tag, "id").unwrap_or_default(),
            text: inner_display_text(tag),
            time: text::attribute(tag, "time").and_then(|t| t.parse().ok()),
        }),
        _ => Frame::WindowHints {
            id: text::attribute(tag, "id").unwrap_or_else(|| name.to_owned()),
            attrs,
        },
    }
}

/// `<roommeta>`: eight environment codes, each `None` when unstated.
///
/// The codes are the game's own and are NOT decoded here; see
/// [`crate::frame::RoomMeta`] for why, and for the census that says all
/// eight arrive together.
fn room_meta(tag: &str) -> crate::frame::RoomMeta {
    let code = |name: &str| text::attribute(tag, name).and_then(|v| v.parse().ok());
    crate::frame::RoomMeta {
        weather: code("weather"),
        bonfire: code("bonfire"),
        inside: code("inside"),
        water: code("water"),
        sanctuary: code("sanctuary"),
        realm: code("realm"),
        climate: code("climate"),
        terrain: code("terrain"),
    }
}

/// An `<objective>` that arrived outside any `<objectives>` envelope.
///
/// Treated as a patch of one: the wire is not known to send this, and
/// assuming a refresh would let one stray row erase the list.
fn stray_objective(tag: &str) -> Frame {
    Frame::ObjectivesUpdate {
        action: crate::frame::ObjectivesAction::Patch,
        entries: vec![crate::parser::dispatch::objective(tag)],
    }
}
