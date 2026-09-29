//! Pure presentation of a session and the versioned frontend input vocabulary.
//!
//! Model values are projected explicitly; no model or protocol object is
//! serialized to a frontend. See `WIRE.md` for the version 1 contract.

mod hub;
mod input;
mod lines;
mod map_scene;
mod merge;
mod object_menu;
mod projection;
pub mod settings;
pub mod triggers;
mod view;
mod wire;

pub use hub::{Account, HubControl, HubRequest, Listing, Login, Password, RosterCard, Saved};
pub use input::{
    InputError, MAX_COMMAND_BYTES, MAX_REQUEST_ID_BYTES, validate_command, validate_line,
};
pub use lines::{MAX_LINE_BYTES, MAX_LINE_RUNS, painted, story_lines};
pub use map_scene::{EdgeKind, MapScene, MinimapView, SceneDoor, SceneEdge, SceneRoom};
pub use merge::{
    MATCH_WINDOW, MAX_MERGED_HISTORY, MERGED_STREAMS, MergedHistory, MergedLine, Merger,
};
pub use object_menu::{MenuEntry, MenuGroup, link_command, object_menu};
pub use projection::{room_description, room_player};
pub use view::{
    Closed, GroupView, HandView, HuntView, LifecycleView, MapLocationView, RoomItemView, RoomView,
    RoundtimeView, RunLink, SessionCard, SessionView, StoryLine, StyledRun, UnknownTagView,
    VitalView, VitalsView,
};
pub use wire::{ClientMessage, ReceiptStatus, ServerMessage, WIRE_VERSION};
