//! Hunt: the profile a hunt runs on, and how one gets in (`plan/30` §5 and
//! §7, M6a step 4).
//!
//! Six pieces. The first four are what a hunt reads, each pure and tested
//! without a game; the last two run it:
//!
//! | Piece | Module | What it is |
//! |---|---|---|
//! | the profile | [`profile`] | one TOML file: rooms, thresholds, stances, targets and their routines |
//! | the guards | [`guard`] | the closed vocabulary of preconditions a routine step may carry |
//! | the chain | [`chain`] | how a key resolves: character, then profile, then global, then the built-in default (`plan/12` §6a.2) |
//! | the importer | [`import`](mod@import) | a bigshot profile in, a Hydra profile out, with what it could not carry named |
//!
//! | the engine | [`engine`] | [`Hunt`], a pure state machine: the profile and the state in, one thing to do out |
//! | the driver | [`drive`] | the `async` layer that holds the authority, folds events, sends and walks |
//!
//! [`command`] is what a player types about any of it, and [`desk`] does it
//! for one session while it is being played.
//!
//! # Policy, not programs
//!
//! A profile says what to hunt, where, and when to stop; it does not say
//! how. There are no conditionals, loops or variables (`plan/12` §6a.1). A
//! **guard** is the one thing that looks like a condition and is not: a
//! named precondition on one step, from a vocabulary Hydra defines, each
//! reading one fact the model already holds and taking at most one number.
//! Guards on a step all have to hold, there is no *or*, and a word Hydra
//! does not know is refused when the profile loads. bigshot silently
//! ignores a misspelled guard (`plan/33` §1), which is how `(hiden)` runs a
//! step it was meant to hold back.
//!
//! # What is built, and what is held
//!
//! The vocabulary starts with the words Nisugi's profile uses (`plan/30`
//! §5): `hidden`, `thp N`, `empowered_below N`, `immobilized` and
//! `expiring "<name>" N`, each with its negation. The other words bigshot
//! accepts are evaluated in `plan/33`, which awaits the author; until a word
//! is built, a step carrying it imports **held**: kept in the profile with
//! the word named, and never run. A lost guard changes when a command
//! fires, and that is worse than a refusal.

mod aim;
mod boons;
mod censer;
pub mod chain;
pub mod command;
pub mod desk;
pub mod drive;
pub mod engine;
mod errands;
pub mod guard;
pub mod import;
pub mod profile;
mod react;
pub mod replies;
mod rest;
pub mod said;
mod wand;
mod wander;
mod wrack;
pub mod yaml;

pub use chain::{LoadError, Loaded, load};
pub use command::{Command, parse as parse_command};
pub use desk::Desk;
pub use drive::{HuntEnd, hunt};
pub use engine::{Ending, Here, Hunt, Said};
pub use guard::{Condition, Dialog, Fact, Facts, Guard, Measure, Used};
pub use import::{Import, import};
pub use profile::{Profile, Step};
