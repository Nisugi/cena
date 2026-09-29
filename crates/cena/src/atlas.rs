//! The minimap's areas (`plan/53` §7): the map Hydra ships, each of its
//! areas laid out by hydra-mapper's engine and turned into the plain
//! [`cena_ui::MapScene`] the window draws ([`scene`]), every area laid out at
//! launch and kept ([`service`]), and each character followed across them
//! ([`follow`]).

mod follow;
mod scene;
mod service;

pub(crate) use follow::minimap;
pub(crate) use service::{Atlas, Waiting};
