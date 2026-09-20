//! cena-protocol
//!
//! The wire layer. Bytes arrive; [`frame::Frame`]s leave. Rule 2.1
//! (`plan/05:270-274`) makes this the vocabulary boundary: nothing above this
//! crate ever sees a raw byte or an unparsed string, and the one deliberate
//! exception is [`frame::Frame::UnknownTag`], which Rule
//! 2.2 (`:276-283`) mandates.
//!
//! Read [`frame`] for what the wire can say, [`parser`] for how a line becomes
//! frames and why the parser is hand-rolled, and [`scrub`] for the redaction
//! the committed fixtures pass through.

pub mod frame;
pub mod numbers;
pub mod parser;
pub mod runs;
pub mod scrub;
pub mod tags;
pub mod text;

pub use frame::{Frame, Menu, MenuItem, Objective, ObjectivesAction, RoomMeta};
pub use parser::Parser;
