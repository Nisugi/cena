//! The catalog's kinds: what each is called, which of the Add-a-widget
//! list's groups it is in (`plan/49` §3, Saga's three and Hydra's own), and
//! the size it would like. Kept apart from the facade so a kind added in
//! Stage B is a variant and a line in each table here.

use std::borrow::Cow;

use egui::Vec2;
use serde::{Deserialize, Serialize};

use super::LINE;
use super::status::{Category, Indicator};

/// One kind of widget: what it shows.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Widget {
    /// The game's text: the main stream, and the streams whose window is
    /// closed, where the game declared they go.
    Story,
    /// Health, as a bar.
    Health,
    /// Mana, as a bar.
    Mana,
    /// Stamina, as a bar.
    Stamina,
    /// Spirit, as a bar.
    Spirit,
    /// What the right hand holds.
    RightHand,
    /// What the left hand holds.
    LeftHand,
    /// The roundtime left.
    Roundtime,
    /// The cast time left.
    CastTime,
    /// An aimed shot's aim left.
    Aim,
    /// The room as the game describes it, joined: its name and number, its
    /// description with what is here, who else is, and the ways out; its
    /// parts chosen on its own page (the author, 2026-09-28).
    Room,
    /// The room's name.
    RoomTitle,
    /// The room's description.
    RoomDescription,
    /// The creatures in the room, each with its status.
    Creatures,
    /// What else is in the room.
    Objects,
    /// The players in the room, painted by the character's triggers.
    Players,
    /// The ways out.
    Exits,
    /// Hydra's own messages, never in the game's text.
    Hydra,
    /// What the hunt is doing, and why it waits (`plan/47` step 8).
    Hunt,
    /// The stance, as a bar.
    Stance,
    /// Encumbrance, as a bar.
    Encumbrance,
    /// Encumbrance, in the game's sentence.
    EncumbranceDetail,
    /// The mind's fill of experience, as a bar.
    Mind,
    /// How near the next level, as a bar.
    NextLevel,
    /// The level.
    Level,
    /// Physical and mental training points.
    TrainingPoints,
    /// The experience numbers: total, field, ascension, long-term, deeds.
    ExperienceTotals,
    /// The spell prepared.
    Prepared,
    /// The society and rank.
    Society,
    /// The profession's resource, and its kin.
    Resources,
    /// Quests and bounties the game lists.
    Objectives,
    /// One status indicator: stunned, hidden, poisoned, ...
    Indicator(Indicator),
    /// One of the game's lists of effects.
    Effects(Category),
    /// The room's ways out, as a compass rose.
    Compass,
    /// The injury doll: each hurt part, wound over scar (`plan/55`).
    Injuries,
    /// Who is fighting in the room: friends and foes.
    Combat,
    /// One of the game's streams, by its id: thoughts, speech, logons, ...
    /// or any other the character has received (`plan/49` §3).
    Stream(String),
    /// The spells the game lists for the character.
    Spellbook,
    /// What the character keeps in reserve.
    Reserve,
    /// The containers the game has shown, and what each holds.
    Containers,
    /// When the next pulse comes.
    Pulse,
    /// The world events under way.
    WorldEvents,
    /// One of the game's own dialogs, by its id: the Betrayer panel, the
    /// combat panel, one an event adds; offered once the game has sent it.
    Dialog(String),
    /// The area the character is in, laid out, following it (`plan/53`).
    Minimap,
    /// Everything the model holds for the character, as it prints itself:
    /// for troubleshooting (the author, 2026-09-27).
    GameState,
}

/// A group of the Add-a-widget list, as `plan/49` §3 sorts Saga's panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Group {
    /// Text the game sends by stream.
    Streams,
    /// Lists of what the character has and is.
    Info,
    /// Bars, hands, clocks and pictures.
    Graphics,
    /// The status indicators, one a widget: Saga's Indicators graphic, in
    /// its nineteen parts.
    Indicators,
    /// What Hydra adds: its messages, the hunt, the room in its parts.
    Hydra,
}

impl Group {
    /// Every group, in the order the list shows them.
    pub(crate) const ALL: [Group; 5] = [
        Group::Streams,
        Group::Info,
        Group::Graphics,
        Group::Indicators,
        Group::Hydra,
    ];

    /// Its heading in the list.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Group::Streams => "Streams",
            Group::Info => "Info panels",
            Group::Graphics => "Graphics",
            Group::Indicators => "Indicators",
            Group::Hydra => "Hydra's own",
        }
    }
}

impl Widget {
    /// Every kind, in the order a list of them shows: the streams named
    /// here among them, not those a character may receive besides
    /// (`plan/49` §3), which the Add-a-widget list offers from the story.
    pub(crate) fn all() -> Vec<Widget> {
        Widget::PLAIN
            .into_iter()
            .chain(
                STREAMS
                    .iter()
                    .map(|(id, _)| Widget::Stream((*id).to_owned())),
            )
            .chain(Category::ALL.map(Widget::Effects))
            .chain(Indicator::ALL.map(Widget::Indicator))
            .collect()
    }

    /// The kinds that hold nothing but their kind.
    const PLAIN: [Widget; 41] = [
        Widget::Story,
        Widget::Health,
        Widget::Mana,
        Widget::Stamina,
        Widget::Spirit,
        Widget::RightHand,
        Widget::LeftHand,
        Widget::Roundtime,
        Widget::CastTime,
        Widget::Aim,
        Widget::Room,
        Widget::RoomTitle,
        Widget::RoomDescription,
        Widget::Creatures,
        Widget::Objects,
        Widget::Players,
        Widget::Exits,
        Widget::Hydra,
        Widget::Hunt,
        Widget::Minimap,
        Widget::GameState,
        Widget::Stance,
        Widget::Encumbrance,
        Widget::EncumbranceDetail,
        Widget::Mind,
        Widget::NextLevel,
        Widget::Level,
        Widget::TrainingPoints,
        Widget::ExperienceTotals,
        Widget::Prepared,
        Widget::Society,
        Widget::Resources,
        Widget::Objectives,
        Widget::Compass,
        Widget::Injuries,
        Widget::Combat,
        Widget::Spellbook,
        Widget::Reserve,
        Widget::Containers,
        Widget::Pulse,
        Widget::WorldEvents,
    ];

    /// What a player calls it: a standalone window's title, and its name in
    /// a list.
    pub(crate) fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            Widget::Story => "Story",
            Widget::Health => "Health",
            Widget::Mana => "Mana",
            Widget::Stamina => "Stamina",
            Widget::Spirit => "Spirit",
            Widget::RightHand => "Right hand",
            Widget::LeftHand => "Left hand",
            Widget::Roundtime => "Roundtime",
            Widget::CastTime => "Cast time",
            Widget::Aim => "Aim",
            Widget::Room => "Room",
            Widget::RoomTitle => "Room name",
            Widget::RoomDescription => "Room description",
            Widget::Creatures => "Creatures",
            Widget::Objects => "Objects",
            Widget::Players => "Players",
            Widget::Exits => "Exits",
            Widget::Hydra => "Hydra",
            Widget::Hunt => "Hunt",
            Widget::Minimap => "Minimap",
            Widget::GameState => "Game state",
            Widget::Stance => "Stance",
            Widget::Encumbrance => "Encumbrance",
            Widget::EncumbranceDetail => "Encumbrance, in words",
            Widget::Mind => "Mind",
            Widget::NextLevel => "Next level",
            Widget::Level => "Level",
            Widget::TrainingPoints => "Training points",
            Widget::ExperienceTotals => "Experience",
            Widget::Prepared => "Spell hand",
            Widget::Society => "Society",
            Widget::Resources => "Resources",
            Widget::Objectives => "Objectives",
            Widget::Compass => "Compass",
            Widget::Injuries => "Injuries",
            Widget::Combat => "Combat",
            Widget::Spellbook => "Spellbook",
            Widget::Reserve => "Reserve",
            Widget::Containers => "Containers",
            Widget::Pulse => "Pulse timer",
            Widget::WorldEvents => "World events",
            Widget::Indicator(indicator) => indicator.name(),
            Widget::Effects(category) => category.name(),
            Widget::Stream(id) => return stream_name(id),
            Widget::Dialog(id) => return dialog_name(id),
        })
    }

    /// Its group in the Add-a-widget list (`plan/49` §3): the creatures are
    /// Saga's Combat graphic; the room's other parts are Hydra's own.
    pub(crate) fn group(&self) -> Group {
        match self {
            Widget::Story | Widget::Stream(_) | Widget::Spellbook => Group::Streams,
            Widget::Health
            | Widget::Mana
            | Widget::Stamina
            | Widget::Spirit
            | Widget::RightHand
            | Widget::LeftHand
            | Widget::Roundtime
            | Widget::CastTime
            | Widget::Aim
            | Widget::Creatures
            | Widget::Mind
            | Widget::NextLevel
            | Widget::Level
            | Widget::TrainingPoints
            | Widget::ExperienceTotals
            | Widget::Prepared
            | Widget::Compass
            | Widget::Injuries
            | Widget::Combat
            | Widget::Reserve
            | Widget::Pulse => Group::Graphics,
            Widget::Stance
            | Widget::Encumbrance
            | Widget::EncumbranceDetail
            | Widget::Society
            | Widget::Resources
            | Widget::Objectives
            | Widget::Containers
            | Widget::WorldEvents
            | Widget::Dialog(_)
            | Widget::Effects(_) => Group::Info,
            Widget::Indicator(_) => Group::Indicators,
            Widget::Room
            | Widget::RoomTitle
            | Widget::RoomDescription
            | Widget::Objects
            | Widget::Players
            | Widget::Exits
            | Widget::Hydra
            | Widget::Hunt
            | Widget::Minimap
            | Widget::GameState => Group::Hydra,
        }
    }

    /// The group whose kinds may be a tab beside it: its own, but Hydra's
    /// messages and the hunt panel are text that scrolls as a stream's does,
    /// so they stack with the streams, and the streams with them (the
    /// author, 2026-09-30: *"I would like to be able to add hydra and hunt
    /// windows to a tabbed stream window"*).
    pub(crate) fn tab_group(&self) -> Group {
        match self {
            Widget::Hydra | Widget::Hunt => Group::Streams,
            _ => self.group(),
        }
    }

    /// How it draws its bar unless the player picks otherwise, when it is
    /// one bar: a vital says its label, numbers and percent; the pulse its
    /// words. `None` for a widget that is not a bar.
    pub(crate) fn bar_look(&self) -> Option<crate::bar::Look> {
        use crate::bar::{
            ENCUMBRANCE, Fills, HEALTH, LEVEL, Look, MANA, MIND, Place, SPIRIT, STAMINA, STANCE,
            Says,
        };
        let vital = Says {
            label: true,
            numbers: true,
            percent: true,
            words: false,
        };
        let worded = Says {
            label: true,
            numbers: false,
            percent: true,
            words: true,
        };
        let (says, color) = match self {
            Widget::Health => (vital, HEALTH),
            Widget::Mana => (vital, MANA),
            Widget::Stamina => (vital, STAMINA),
            Widget::Spirit => (vital, SPIRIT),
            // The game words these: the label, its word and the percent.
            Widget::Stance => (worded, STANCE),
            Widget::Encumbrance => (worded, ENCUMBRANCE),
            Widget::Mind => (worded, MIND),
            Widget::NextLevel => (worded, LEVEL),
            Widget::Pulse => (
                Says {
                    label: true,
                    numbers: false,
                    percent: false,
                    words: false,
                },
                MANA,
            ),
            _ => return None,
        };
        Some(Look {
            fills: Fills::Right,
            place: Place::Inside,
            says,
            color: [color.r(), color.g(), color.b()],
            overlay: None,
            background: None,
            fill_image: None,
            ring: Look::RING,
        })
    }

    /// Whether it holds the game's lines: the story or one of its streams,
    /// which the scrolling keys and Find work on. Hydra's own messages are
    /// not among them.
    pub(crate) fn has_lines(&self) -> bool {
        matches!(self, Self::Story | Self::Stream(_))
    }

    /// Whether it is one character's story, which never follows another
    /// character (`plan/49` §1 row 7): no window mixes two characters'
    /// story (`plan/29` §5a R2). Hydra's messages count as its story, and
    /// so does each of its streams.
    pub(crate) fn is_story(&self) -> bool {
        matches!(self, Widget::Story | Widget::Hydra | Widget::Stream(_))
    }

    /// The size it would like, when nothing else says: a bar or a hand is
    /// one line, a list a few, the story as much as it is given
    /// (`plan/28` §7e: geometry is the widget's to say).
    pub(crate) fn size(&self) -> Vec2 {
        let (width, height) = match self {
            Widget::Story => (480.0, 320.0),
            Widget::Hydra | Widget::Stream(_) => (320.0, 120.0),
            Widget::Hunt => (260.0, 90.0),
            Widget::GameState => (380.0, 420.0),
            Widget::Minimap => (260.0, 220.0),
            Widget::RoomDescription => (320.0, 80.0),
            Widget::Room => (320.0, 160.0),
            Widget::Creatures | Widget::Objects | Widget::Players => (260.0, 40.0),
            Widget::ExperienceTotals | Widget::Resources | Widget::Reserve => (260.0, 60.0),
            Widget::Spellbook | Widget::Containers => (280.0, 200.0),
            Widget::WorldEvents => (300.0, 80.0),
            Widget::Objectives | Widget::Effects(_) => (300.0, 100.0),
            Widget::Indicator(_) => (100.0, LINE),
            Widget::Compass => (160.0, 120.0),
            Widget::Injuries => (180.0, 240.0),
            Widget::Combat | Widget::Dialog(_) => (260.0, 140.0),
            Widget::Roundtime | Widget::CastTime | Widget::Aim => (110.0, LINE),
            Widget::Health
            | Widget::Mana
            | Widget::Stamina
            | Widget::Spirit
            | Widget::RightHand
            | Widget::LeftHand
            | Widget::RoomTitle
            | Widget::Exits
            | Widget::Stance
            | Widget::Encumbrance
            | Widget::EncumbranceDetail
            | Widget::Mind
            | Widget::NextLevel
            | Widget::Level
            | Widget::TrainingPoints
            | Widget::Prepared
            | Widget::Society
            | Widget::Pulse => (260.0, LINE),
        };
        Vec2::new(width, height)
    }
}

/// The streams the game sends that a player knows by name, by their ids,
/// as Saga lists them (`plan/49` §3): Voln is the Order's own thoughts
/// (`reference/lich-5/lib/common/markup.rb:213`). Mentor and Host come
/// only to those who hold the position, and are offered when received.
pub(crate) const STREAMS: [(&str, &str); 7] = [
    ("thoughts", "Thoughts"),
    ("speech", "Speech"),
    ("logons", "Arrivals"),
    ("death", "Deaths"),
    ("announcements", "Announcements"),
    ("familiar", "Familiar"),
    ("voln", "Voln"),
];

/// A stream's name: the one it is known by, or its id with a capital.
pub(crate) fn stream_name(id: &str) -> Cow<'static, str> {
    if let Some((_, name)) = STREAMS.iter().find(|(known, _)| *known == id) {
        return Cow::Borrowed(name);
    }
    let mut chars = id.chars();
    Cow::Owned(
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_default(),
    )
}

/// A dialog's name, from its id: `BetrayerPanel` is "Betrayer panel",
/// `combat` "Combat".
pub(crate) fn dialog_name(id: &str) -> Cow<'static, str> {
    let mut name = String::new();
    let mut last_lower = false;
    for c in id.chars() {
        if c.is_uppercase() && last_lower {
            name.push(' ');
            name.extend(c.to_lowercase());
        } else if name.is_empty() {
            name.extend(c.to_uppercase());
        } else {
            name.push(c);
        }
        last_lower = c.is_lowercase();
    }
    Cow::Owned(name)
}
