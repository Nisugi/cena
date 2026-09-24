//! Crossings no list of steps can describe (`plan/21` §4.6).
//!
//! Some areas rearrange themselves, and upstream's script for crossing one is
//! a *search*: explore, remember what led where, notice the area shifted,
//! start again. That is an algorithm, and it lives in Rust, in the Travel
//! behavior. The map carries only its **name and its arguments**.
//!
//! The set is open but small. A routine added by a later build is a name this
//! build cannot parse, so the exit loads as `Crossing::Unknown` -- impassable
//! -- and the walker routes around it (`crate::binary`, rule 1). This is the
//! one place a new upstream area can require a new Hydra, and the converter's
//! ratchet is what says so.
//!
//! This crate names them; it does not run them.

use serde::{Deserialize, Serialize};

use crate::room::RoomId;

/// A named search. In JSON: `{"name": "minotaur_maze", "rooms": [...]}`.
///
/// **The exit's own destination is always the goal** and is not repeated here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum Routine {
    /// The Elemental Confluence: rooms whose exits reshuffle. Explore by
    /// compass, learn what led where, relearn when a room's exits change.
    ///
    /// `leave` is how upstream's two goals are told apart. `false`: reach the
    /// exit's destination, a room of the plane. `true`: find the point of
    /// elemental tranquility and go through it, which lands outside.
    Confluence {
        /// `true` to leave the plane through the point of tranquility;
        /// `false` to reach the exit's destination on the plane.
        leave: bool,
    },
    /// The minotaur maze beneath the Landing: the same search over a fixed
    /// set of rooms. Leaving the set means the walker fell out and replans.
    MinotaurMaze {
        /// The maze's rooms; the walker being outside them means it fell out.
        rooms: Vec<RoomId>,
    },
    /// Follow signposts: each room on the way says which way to go from it,
    /// until the walker is at the exit's destination. The underwater route off
    /// River's Rest, where a current can carry the walker somewhere else on the
    /// route and the right direction depends on where it ends up.
    ///
    /// A room that is not in `dirs` means the walker is lost, and it replans;
    /// upstream swims in a random direction instead, which is not copied. The
    /// hands are emptied first when the walk starts in one of `hands_free_in`,
    /// and refilled at the end either way.
    Signposts {
        /// Put before each direction: `swim`.
        verb: String,
        /// `(room, direction)`, in upstream's order.
        dirs: Vec<(RoomId, String)>,
        /// Rooms where a walk starting there empties the hands first.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hands_free_in: Vec<RoomId>,
    },
    /// The Order of Voln's symbol of seeking: it offers a destination, a
    /// different one each time, and the walker asks again until the one offered
    /// is this exit's -- told by the room title the vision shows -- and then
    /// confirms. Upstream gives up when the offers come round to the first
    /// again, or after twenty.
    Seeking {
        /// Written down once the walker has arrived: which side of the Red
        /// Forest it sought its way into (`plan/21` §4.4).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remember: Option<(String, String)>,
    },
    /// The Mist Harbor trinket, named by the profile's `fwi_trinket`: get it
    /// out if it is not worn, turn it, put it back. From a town it remembers
    /// the room left as `fwi_return_room`; from Mist Harbor it returns there.
    /// **Arrival in Mist Harbor is a random room, or one a GM has set**, so the
    /// routine ends by finding out where it is and planning again.
    Trinket,
    /// A rogue guild's door: `lean door`, then each verb of the profile's
    /// `rogue_password` on the door, then `go door`. An exit crossed this way
    /// is priced impassable when the profile has no password, so the walker
    /// is never sent to a door it cannot open.
    GuildPassword,
    /// A round tower room with four flights of steps, listed in an order
    /// that changes: `look`, read which flight is on this `wall`
    /// (`northern`), and climb that one -- `climb steps`, `climb second
    /// steps`, and so on. Upstream gives up when no flight is on the wall.
    FlightOfSteps {
        /// The wall whose flight to climb, as the room describes it: `northern`.
        wall: String,
    },
    /// Go to each of `rooms` in turn, by the map, until one shows a thing
    /// whose name holds `sees`; then `enter` it. A portal or a doorframe
    /// that wanders. `by_uid`: the rooms are the game's numbers, not the
    /// map's. Where it leads is not promised, so it ends by planning again.
    SearchRooms {
        /// The rooms to visit, in order: map ids, or the game's numbers when
        /// `by_uid` is set.
        rooms: Vec<u32>,
        /// Whether `rooms` holds the game's room numbers rather than map ids.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        by_uid: bool,
        /// Text a shown thing's name must hold for the search to stop there.
        sees: String,
        /// The command sent to go through what was found.
        enter: String,
    },
    /// Room 16165: turn the mirror right to its stop, then tilt it until the
    /// light falls on the centre of the statue -- starting the turn again
    /// from the left if it flips over -- and `go shadow`.
    Mirror,
    /// Room 8373: look at each of the stone ring's four wedges and turn the
    /// ring until it points at its sigil, pushing each wedge as it is set.
    RingWedges,
    /// The Graveyard's bronze gate, 4140 and 4141: `go gate` until through.
    /// Between tries, cast the first known of Unlock, Consecrate, Bless Item
    /// and Force Projection at it, waiting for the mana and casting again
    /// when armour hinders; failing that a Warrior of 15 batters it, where
    /// `batter` allows; failing that, push it with empty hands.
    BronzeGate {
        /// Whether a Warrior of 15 may batter the gate when no spell serves.
        batter: bool,
    },
    /// Room 30850: the barrier's colour says which walk leads to its
    /// grotto. Walk it, `touch crystal`, walk back, and `go barrier`.
    ColourBarrier,
    /// A puzzle that belongs to one place and takes no arguments: the map
    /// names it, and the Travel behaviour knows how it goes. What each one
    /// does is written on [`Puzzle`].
    Puzzle {
        /// Which puzzle.
        puzzle: Puzzle,
    },
    /// A Chronomage day pass between two towns, `route` naming them as
    /// upstream's setting does -- `wl,imt`, from and then to. Take a pass
    /// that is valid for both towns out of the profile's `day_pass_sack`,
    /// dropping any that have expired; with none, and the profile saying to
    /// buy, walk to the clerk and ask twice, fetching silver from the bank
    /// once if short. Raise the pass, and put it back. The clerk's walk and
    /// what to ask for differ by town and are in the pinned scripts.
    ///
    /// **Which passes the walker holds is the planner's to find out before
    /// it prices anything** -- upstream does it inside the cost script, by
    /// opening the sack and reading each pass. Here that is the flag
    /// `day_pass:imt,wl`, the two towns in alphabetical order since one pass
    /// serves both ways (`cena_map::Cost` cannot send commands, and should
    /// not).
    DayPass {
        /// The two towns, from and then to, as upstream's setting spells
        /// them: `wl,imt`.
        route: String,
    },
    /// A crossing that stops part-way to **make another trip** and come
    /// back: to a bank, a shop, a ticket seller. See [`Errand`].
    Errand {
        /// Which errand.
        errand: Errand,
    },
    /// Walk a fixed circuit until something appears, then go through it. The
    /// Rift: its ways out -- a thread, a maw, a door, a mirror, a fissure --
    /// drift from room to room, so the walker goes round until it sees one.
    ///
    /// The walk starts at the walker's place on the circuit: its room's
    /// position in `starts` is the position in `dirs` to begin from, and
    /// `dirs` then repeats. The two lists are not the same length upstream
    /// and need not be. A walker whose room is not in `starts` is lost, and
    /// replans. **Where the way out lands is not known in advance**, so this
    /// routine always ends by finding out where it is and planning again.
    Patrol {
        /// The circuit's rooms, by position, for finding where to begin.
        /// `None` keeps a gap upstream left: positions matter.
        starts: Vec<Option<RoomId>>,
        /// The directions walked, repeated from the starting position.
        dirs: Vec<String>,
        /// What to look for among the room's objects. The first listed that is
        /// present wins.
        landmarks: Vec<Landmark>,
        /// Commands sent once through: `stand`, after climbing a thread.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<String>,
    },
}

/// The errands of [`Routine::Errand`], each pinned verbatim to upstream's
/// script as the puzzles are. They are what the Travel behaviour's stack of
/// trips is for (`plan/21` §4.7): every one may walk to a bank and a shop
/// and back before it crosses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Errand {
    /// Room 6274, the kobold village's way to River's Rest: the giant wants
    /// driftwood and a crystal amulet. Find them in the walker's
    /// containers, or buy what is missing -- bank, alchemist, bank -- then
    /// put both in the giant and go.
    GiantToRiversRest,
    /// Room 11032, the same giant the other way, where the driftwood comes
    /// from the general store.
    GiantFromRiversRest,
    /// Room 11753, Marshtown's pier: find a ticket that names the walker, in
    /// a hand or a container, or walk to the storefront and buy one; then
    /// wait for the smugglers' cutter and board.
    CutterFromMarshtown,
    /// Room 18677, the same cutter from under River's Rest's bridge, where
    /// the ticket seller is a trip away.
    CutterFromRiversRest,
    /// Room 6955, the gorge: the rim is reached through a pool, prying a
    /// gap with a sword bought for the purpose -- bank, shop, bank -- and
    /// recovered afterwards. The exit's cost asks for level 20.
    SwordInTheGorge,
}

/// The puzzles of [`Routine::Puzzle`]. Each is pinned to upstream's script,
/// verbatim, in the converter (`src/upstream_scripts/`), which is the
/// reference for what it must do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Puzzle {
    /// Rooms 3239 and 3264: open the gate; if it is locked, climb it (a good climber
    /// lightly loaded, with Sigil of Resolve if known), or Phase through
    /// it, or cast Unlock or Force Projection at it until it gives. Upstream
    /// gives up on a walker who can do none, **but only when the gate is
    /// locked**, which no plan can know. Lands elsewhere if it was neither.
    RolarenGate,
    /// Room 18178: Read the five runes and what each protrusion is set to, and
    /// push protrusions until the staircase forms.
    RuneStaircase,
    /// Room 10781: Look at each of three pillars and cast the sorcerer spell its
    /// symbol names at it. Sorcerers only, which the exit's cost says.
    ThreePillars,
    /// Room 14060: `open door`; if it will not, it wants a gem: put the one in
    /// the right hand into it. **Upstream pauses for the user to hold a
    /// cheap gem** when there is none.
    VaalornDoor,
    /// Room 6897: Touch the mural, match each verse it recites to its deity, and
    /// answer them in order.
    MuralOfDeities,
    /// Room 18893: Read the grid on the altar, then pull each coloured lever
    /// until it stands at the position the grid gives it.
    AltarLevers,
    /// Room 15571: The wizards' workshop: go to the side whose element the walker
    /// knows all three spells of, and cast each at its pillar. The exit is
    /// priced only for a walker who knows one full set.
    WorkshopPillars,
    /// Room 18748: Cast Eye Spy, send the eye along a fixed walk to read which
    /// rune glows on the basalt, and touch that rune.
    EyeSpyRunes,
    /// Room 2677: `go door`, or else push the tine until a stone is aligned,
    /// spend mana at the crown, touch it, and say the word the stone
    /// stands for.
    CrownDoor,
    /// Room 14726: `go bridge`; if it is pulled open, go under, climb the
    /// platform and turn the wheel, with strength spells if it will not
    /// budge. **Upstream pauses for help** when it still will not.
    BridgeWheel,
    /// Room 6486: The stone doors a familiar opens: send it to watch, read which
    /// ring it sees, pull that ring. The exit's cost asks for Call Familiar.
    FamiliarDoors,
    /// Room 9767: Touch the leaves in order, waiting while someone else is at
    /// them and starting again when they fade, then the outstretched hand.
    LabyrinthEntry,
}

/// One way out a [`Routine::Patrol`] looks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Landmark {
    /// The object's noun: `thread`.
    pub noun: String,
    /// The command that goes through it: `climb thread`.
    pub enter: String,
    /// Some must be worked open first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<Opening>,
}

/// Work a landmark open: send `command` until the game says `until`, at most
/// `tries` times, standing up again between tries if knocked down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opening {
    /// The command sent each try.
    pub command: String,
    /// Text in the game's reply that says the landmark is open.
    pub until: String,
    /// The most times `command` is sent.
    pub tries: u32,
}
