//! Pure presentation of a session and the versioned frontend input vocabulary.
//!
//! Model values are projected explicitly; no model or protocol object is
//! serialized to a frontend. See `WIRE.md` for the version 1 contract.

mod input;
mod lines;
mod merge;
mod projection;
mod sorter;
mod view;
mod wire;

pub use input::{InputError, MAX_COMMAND_BYTES, MAX_REQUEST_ID_BYTES, validate_command};
pub use lines::{LineAssembler, MAX_LINE_BYTES, MAX_LINE_RUNS, MAX_PENDING_STREAMS};
pub use merge::{MATCH_WINDOW, MERGED_STREAMS, MergedLine, Merger};
pub use view::{
    Closed, HandView, LifecycleView, MapLocationView, RoomItemView, RoomView, RoundtimeView,
    SessionCard, SessionView, StoryLine, StyledRun, UnknownTagView, VitalView, VitalsView,
};
pub use wire::{ClientMessage, ReceiptStatus, ServerMessage, WIRE_VERSION};
