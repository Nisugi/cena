//! The minimap's areas (`plan/53` §7): the map Hydra ships, each of its
//! areas laid out by hydra-mapper's engine and turned into the plain
//! [`cena_ui::MapScene`] the window draws ([`scene`]), every area laid out at
//! launch and kept ([`service`]), each character followed across them
//! ([`follow`]), and a room of the map said in the story ([`room`]).

mod follow;
mod room;
mod scene;
mod service;

pub(crate) use follow::minimap;
pub(crate) use room::command as room_command;
pub(crate) use service::{Atlas, Waiting};
