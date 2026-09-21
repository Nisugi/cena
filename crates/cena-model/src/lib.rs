//! cena-model: typed game state, events, and game data.
//!
//! `plan/12:77`. Two halves: [`state`] is the typed game state a session folds
//! frames into, and [`crit`] is the game-data half, ported from Lich per
//! `plan/13` §4a.
//!
//! > **`GameState` moved here from `cena-session` 2026-09-18 (author's call).**
//! > It had been built in the session, which left `cena-model` in the
//! > dependency chain `protocol -> model -> session` **carrying no traffic**:
//! > `cena-model` used nothing at all from `cena-protocol`, so the edge was
//! > declared and unused. That is why `cena-session` needing `cena-protocol`
//! > directly felt like a layer skip and was not one -- there was nothing in
//! > `cena-model` to skip.
//! >
//! > Typed game state built from frames is what `plan/12:77` says this crate
//! > is FOR, so the fix was to move the code down rather than to keep routing
//! > around a layer that did no work.

pub mod crit;
pub mod effects;
pub mod state;
pub mod status;

pub use effects::{Effect, Effects};
pub use state::armaments::{ArmamentKind, Armor, Coverage, Shield, Weapon};
pub use state::bounty::{Task, TaskKind};
pub use state::character::blocks::InfoReport;
pub use state::character::body::{Body, Track};
pub use state::character::enhancive::{Bonus, EnhanciveLine, EnhanciveTotals, Resource, Section};
pub use state::character::injured::{Able, Injuries};
pub use state::character::psm::{AscensionTable, PsmLine, PsmRanks, PsmSet};
pub use state::character::skills::{Skill, SkillKind, SkillLine, SkillSet};
pub use state::character::stats::{Identity, Stat, StatKind, StatLine, StatValue};
pub use state::character::vocabulary::{
    AccountType, Che, DeathsSting, PsmCategory, ResourceType, Society, Warcry,
};
pub use state::chunks::{Chunk, ChunkLine, MAX_CHUNK_LINES};
pub use state::claim::{Claim, Occupants};
pub use state::combat::Actor;
pub use state::combat::attack::{AmbushKind, AmbushPrefix, AttackLine, RedirectPrefix, TargetKind};
pub use state::combat::bracket::{AssaultName, BracketStart, SequenceName};
pub use state::combat::damage::DamageLine;
pub use state::combat::flare::FlareLine;
pub use state::combat::outcome::{Outcome, OutcomeKind};
pub use state::combat::resolution::{Resolution, ResolutionKind};
pub use state::combat::spell_loss::SpellLoss;
pub use state::combat::status::{StatusAction, StatusLine, StatusName};
pub use state::combat::ucs::{PositionTier, UcsAttack, UcsLine};
pub use state::containers::{ContainerEvent, Containers, ItemRef, ReadySlot, StoreMode, StowSlot};
pub use state::hands::Hand;
pub use state::resolve::{Held, Location, Match, Specificity, match_of};
// NOTE: `creature::Stat` and `creature::status::Classification` are NOT
// re-exported here: `character::stats::Stat` and `gameobj::Classification`
// already hold those names at the facade, and three types called `Stat` in
// one namespace would make every use site ambiguous to a reader rather than
// only to the compiler. They are reached by their module path.
pub use state::creature::status::{CreatureStatus, Status};
pub use state::creature::{Area, Attack, Creature, MessageKind, Treasure};
pub use state::creatures::{BodyPart, CreatureInstance, Creatures};
pub use state::gameobj::{Classification, ObjectTypes};
pub use state::menu::{LearnedCommands, MenuCommand, MenuCommands, ResolvedItem, category_path};
pub use state::societies::membership::MembershipLine;
pub use state::societies::{Ability, AbilityKind, AlternateCost, Cost, CostTiming, Target};
pub use state::streams::{LineTally, MAX_STREAM_LINES};
pub use state::vitals::{Vital, Vitals, VitalsExt};
pub use state::{
    Character, Container, DISK_NOUNS, Disk, Experience, Found, GameState, Injury, Inventory,
    MAX_UNKNOWN_TAGS, PlayerStatus, Room, RoomItem, UnknownTag, Where,
};
pub use status::StatusInfo;
