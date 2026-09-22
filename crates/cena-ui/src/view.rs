//! The small, explicit vocabulary the M4 frontend renders.

use serde::{Deserialize, Serialize};

/// Text and structural styling only. A preset is an opaque token, never CSS.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyledRun {
    pub text: String,
    pub bold: bool,
    pub monospace: bool,
    pub preset: Option<String>,
}

/// A complete display line. Truncation is visible rather than silent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryLine {
    pub stream: String,
    pub runs: Vec<StyledRun>,
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
    Styled { style: String },
    /// Goes to another window instead, which may itself be closed -- so this
    /// chains. UNVERIFIED against live traffic; see the model's module docs.
    Route { window: String },
}

/// Native lifecycle is supplied by the owner; projection never guesses it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LifecycleView {
    Connecting,
    Ready,
    Reconnecting {
        /// Retry number, unknown until native retry scheduling reports one.
        attempt: Option<u32>,
        /// Original scheduled backoff delay, not time remaining until retry.
        retry_delay_ms: Option<u64>,
        detail: Option<String>,
    },
    Closed {
        detail: Option<String>,
    },
}

/// Unknown, known empty, and holding are three different answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HandView {
    Unknown,
    Empty,
    Holding {
        id: Option<String>,
        noun: Option<String>,
        name: String,
    },
}

/// An interactable room entry; object identities stay verbatim strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomItemView {
    pub id: String,
    pub noun: String,
    pub text: String,
    pub status: Option<String>,
}

/// Nullable collections distinguish an unobserved feed from known emptiness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomView {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<Vec<StyledRun>>,
    pub exits: Option<Vec<String>>,
    pub creatures: Option<Vec<RoomItemView>>,
    pub objects: Option<Vec<RoomItemView>>,
    pub players: Option<Vec<RoomItemView>>,
}

/// Preserve the reported percentage and signed amounts independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VitalView {
    pub percent: u32,
    pub current: Option<i32>,
    pub max: Option<i32>,
}

/// A missing gauge is unknown, never zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VitalsView {
    pub health: Option<VitalView>,
    pub mana: Option<VitalView>,
    pub stamina: Option<VitalView>,
    pub spirit: Option<VitalView>,
}

/// Server epoch seconds and the remainder at the supplied observation time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundtimeView {
    pub ends_at: Option<u32>,
    pub remaining_seconds: Option<u32>,
}

/// Bounded diagnostic text, rendered literally rather than as HTML.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownTagView {
    pub name: String,
    pub raw: String,
    pub truncated: bool,
}

/// State needed by Story, room, hands, vitals, roundtime and connection status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionView {
    pub room: RoomView,
    pub left_hand: HandView,
    pub right_hand: HandView,
    pub vitals: VitalsView,
    pub roundtime: RoundtimeView,
    pub lifecycle: LifecycleView,
    pub prompt: Option<String>,
    pub unknown_tags: Vec<UnknownTagView>,
}
