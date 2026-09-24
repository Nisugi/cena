//! The small, explicit vocabulary the M4 frontend renders.

use serde::{Deserialize, Serialize};

/// Text and structural styling only. A preset is an opaque token, never CSS.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyledRun {
    /// Plain text; the browser inserts it as a text node, never as HTML.
    pub text: String,
    /// True when the run sat inside at least one `<pushBold>` level.
    pub bold: bool,
    /// True for `<output class="mono">` text; the browser adds `text-mono`.
    pub monospace: bool,
    /// The innermost `<preset id=>` or `<style id=>` token, if any. The browser
    /// maps known ids through its own allowlist to a CSS class and shows
    /// unmapped ones only as a tooltip.
    pub preset: Option<String>,
}

/// A complete display line. Truncation is visible rather than silent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryLine {
    /// The stream the text arrived on; the browser treats `""` and `main` as
    /// the Story itself and anything else as a separable window.
    pub stream: String,
    /// The line's styled fragments, in wire order.
    pub runs: Vec<StyledRun>,
    /// Set when an assembler bound cut this line's text, runs, stream name or
    /// a preset token; the browser appends a visible `[line truncated]` marker.
    pub truncated: bool,
    /// What this line's stream declared it does when its window is **closed**.
    ///
    /// # Why the declaration rather than a resolved destination
    ///
    /// The rule needs two things: what the stream declared (the model has it,
    /// from `<streamWindow ifClosed=>`) and which windows the viewer has open
    /// (only the viewer has it). The obvious design -- resolve it server-side
    /// and ship a destination -- does not fit: the hub encodes **one** message
    /// and broadcasts it to every viewer, so a per-viewer answer would mean
    /// re-encoding per client, and two viewers with different windows open
    /// would need different bytes.
    ///
    /// So the declaration travels and the viewer applies it. That still keeps
    /// one implementation of *reading* the wire's rule
    /// (`cena_model::state::stream_windows`), which is the part with the
    /// `Some("")`-versus-`None` trap in it.
    pub closed: Closed,
}

/// What a stream's text does when its window is closed: the wire's own rule,
/// read off `<streamWindow ifClosed= styleIfClosed=>`.
///
/// Mirrors `cena_model::state::stream_windows::Closed` as a wire DTO. The two
/// are deliberately separate types: this one is a versioned contract with a
/// JavaScript consumer, and the model's is free to change.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Closed {
    /// Falls through to the main story, unstyled. Also every ordinary
    /// main-window line, and any stream nobody declared.
    #[default]
    Main,
    /// **A duplicate: the server also sent this line to main.** `speech` is
    /// declared this way, which is why a viewer that renders every line shows
    /// everything the character says twice. Show it only in its own window.
    Drop,
    /// Falls through to main wearing this style: the inline-thoughts look.
    Styled {
        /// The `styleIfClosed=` token; the browser maps it through its preset
        /// allowlist, falling back to a generic styled-stream class.
        style: String,
    },
    /// Goes to another window instead, which may itself be closed -- so this
    /// chains. UNVERIFIED against live traffic; see the model's module docs.
    Route {
        /// The `ifClosed=` target window id.
        window: String,
    },
}

/// Native lifecycle is supplied by the owner; projection never guesses it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LifecycleView {
    /// Opening the transport, authenticating, or receiving the login burst.
    Connecting,
    /// Past the login burst: the native session considers state trustworthy.
    Ready,
    /// The transport is gone and a new connection is being opened.
    Reconnecting {
        /// Retry number, unknown until native retry scheduling reports one.
        attempt: Option<u32>,
        /// Original scheduled backoff delay, not time remaining until retry.
        retry_delay_ms: Option<u64>,
        /// Human-readable reason, appended after a dash in the status line.
        detail: Option<String>,
    },
    /// Terminal: the session itself has stopped and will not reconnect.
    Closed {
        /// Human-readable reason when the owner supplies one; the browser
        /// appends it after a dash in the status line.
        detail: Option<String>,
    },
}

/// Unknown, known empty, and holding are three different answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HandView {
    /// Not yet reported, or invalidated by a reconnect; shown as `Unknown`.
    Unknown,
    /// The game said this hand holds nothing.
    Empty,
    /// The game named an item in this hand.
    Holding {
        /// The item's `exist=` id, verbatim, when the wire gave one.
        id: Option<String>,
        /// The item's `noun=`, when the wire gave one.
        noun: Option<String>,
        /// The display text the browser shows for the hand.
        name: String,
    },
}

/// An interactable room entry; object identities stay verbatim strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomItemView {
    /// The link's `exist=` id, verbatim; may be negative (players, fixtures).
    pub id: String,
    /// The link's `noun=`: what a command targets the entry by.
    pub noun: String,
    /// The link's display text, which the browser lists.
    pub text: String,
    /// What a player is doing (`hiding`, `sitting`, ...); only ever set for
    /// players. The browser shows it in parentheses after `text`.
    pub status: Option<String>,
}

/// Nullable collections distinguish an unobserved feed from known emptiness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomView {
    /// The game's own room id from `<nav rm=>`; the browser shows it as `#id`.
    pub id: Option<String>,
    /// The room name from the room window's `subtitle=`, less its leading
    /// ` - `. `None` briefly on each arrival, since it follows `<nav>`.
    pub title: Option<String>,
    /// The room description as styled runs; `None` until one is observed.
    pub description: Option<Vec<StyledRun>>,
    /// Compass exits; `None` if unobserved, empty if the compass shows none.
    pub exits: Option<Vec<String>>,
    /// Creatures in the room; `None` until the `room objs` component arrives.
    pub creatures: Option<Vec<RoomItemView>>,
    /// Non-creature objects; `None` until the `room objs` component arrives.
    pub objects: Option<Vec<RoomItemView>>,
    /// Other players; `None` until the room's player list has been seen.
    pub players: Option<Vec<RoomItemView>>,
}

/// Preserve the reported percentage and signed amounts independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VitalView {
    /// The percentage the game reported, as sent.
    pub percent: u32,
    /// Current amount, which can be negative; `None` when only a percent came.
    pub current: Option<i32>,
    /// Maximum amount; `None` when only a percent came.
    pub max: Option<i32>,
}

/// A missing gauge is unknown, never zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VitalsView {
    /// Health gauge; `None` until the game reports it.
    pub health: Option<VitalView>,
    /// Mana gauge; `None` until the game reports it.
    pub mana: Option<VitalView>,
    /// Stamina gauge; `None` until the game reports it.
    pub stamina: Option<VitalView>,
    /// Spirit gauge; `None` until the game reports it.
    pub spirit: Option<VitalView>,
}

/// Server epoch seconds and the remainder at the supplied observation time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundtimeView {
    /// Server epoch second the roundtime ends; `None` if none was observed.
    pub ends_at: Option<u32>,
    /// Seconds left at projection time, saturating at `0`. `None` only when
    /// the server clock is unknown; the browser counts it down locally.
    pub remaining_seconds: Option<u32>,
}

/// Bounded diagnostic text, rendered literally rather than as HTML.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownTagView {
    /// The unrecognised tag's name, capped at 128 bytes.
    pub name: String,
    /// The tag's raw markup, capped at 1024 bytes.
    pub raw: String,
    /// Set when either `name` or `raw` was cut to its cap.
    pub truncated: bool,
}

/// State needed by Story, room, hands, vitals, roundtime and connection status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionView {
    /// The current room as far as it has been observed.
    pub room: RoomView,
    /// What the left hand holds, or that it is unknown.
    pub left_hand: HandView,
    /// What the right hand holds, or that it is unknown.
    pub right_hand: HandView,
    /// The four gauges, each independently unknown.
    pub vitals: VitalsView,
    /// Roundtime end and remainder.
    pub roundtime: RoundtimeView,
    /// Connection state, supplied by the owner rather than projected.
    pub lifecycle: LifecycleView,
    /// The last `<prompt>` text (e.g. `>`); the browser shows `>` when `None`.
    pub prompt: Option<String>,
    /// At most 32 unknown-tag diagnostics sampled from the model's ring.
    pub unknown_tags: Vec<UnknownTagView>,
}
