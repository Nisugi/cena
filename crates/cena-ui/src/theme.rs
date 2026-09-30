//! The palette's tokens: every colour Hydra draws, named by what it means
//! (`plan/57` §3a), and a palette holding a colour for each.
//!
//! A token names a meaning, never a screen: *health*, not *the bar in the
//! vitals window*. A frontend asks the palette for a token's colour and
//! draws with it; a theme (`plan/57` step 2) fills the palette from its
//! recipe and pins. Until one does, [`Palette::bare`] holds the colours
//! Hydra drew before it had themes, each one as its literal was, over
//! egui's own dark mode: not a theme, and never shipped as one (the author,
//! 2026-09-30: *"There is no theme applied. I'm not sure all black should
//! be a theme"*).

/// A colour: red, green, blue, as a bar's look and the settings pages
/// already write one.
pub type Rgb = [u8; 3];

/// A colour Hydra draws, by its meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Token {
    // Accents and Hydra's own text.
    /// The accent: what is chosen, a guide while arranging, a find's hit.
    Accent,
    /// A link's colour in the story, where it is not bold.
    Link,
    /// Hydra's own words among the game's: its answers.
    Hydra,
    /// A warning: roundtime, a bar's notice, an alert's ground.
    Warning,
    /// Something lost or wrong: an error, a stun, lines missed.
    Wrong,
    // The game's text.
    /// A room's name, the `roomName` preset.
    RoomName,
    /// A creature's name: `monsterbold`, the room window's creatures.
    Creature,
    /// A player's name in the room window.
    Player,
    /// An object's name in the room window.
    Object,
    /// The `speech` preset.
    Speech,
    /// The `whisper` preset.
    Whisper,
    /// The `thought` preset.
    Thought,
    // Vitals: each bar's fill.
    /// Health.
    Health,
    /// Mana.
    Mana,
    /// Stamina.
    Stamina,
    /// Spirit.
    Spirit,
    /// The betrayer's Blood Points.
    Blood,
    /// Stance.
    Stance,
    /// Encumbrance.
    Encumbrance,
    /// The mind, and field experience.
    Mind,
    /// The next level.
    Level,
    // Injuries: the doll's dots and its body.
    /// A part unhurt.
    Unhurt,
    /// A rank 1 wound.
    Wound1,
    /// A rank 2 wound.
    Wound2,
    /// A rank 3 wound.
    Wound3,
    /// A rank 1 scar.
    Scar1,
    /// A rank 2 scar.
    Scar2,
    /// A rank 3 scar.
    Scar3,
    /// The body drawn under the dots when no picture is chosen.
    Body,
    // Status: an indicator lit, and each list of effects.
    /// Stunned, webbed, bound, calmed.
    Stunned,
    /// Bleeding, dead, throat cut.
    Bleeding,
    /// Poisoned, diseased, thorned.
    Poisoned,
    /// Hidden, invisible.
    Hidden,
    /// Silenced, asleep.
    Silenced,
    /// Standing, kneeling, sitting, prone, grouped.
    Posture,
    /// The Active Spells list's bars.
    ActiveSpells,
    /// The Buffs list's bars.
    Buffs,
    /// The Debuffs list's bars.
    Debuffs,
    /// The Cooldowns list's bars.
    Cooldowns,
    /// The pulse's seconds.
    Pulse,
    // The map.
    /// The minimap's ground.
    MapBackground,
    /// A room's fill.
    MapRoom,
    /// A room's edge.
    MapRoomEdge,
    /// A line with a direction.
    MapLine,
    /// A line without one, dashed.
    MapConnector,
    /// A way in, and its place's name.
    MapDoor,
    /// You.
    MapYou,
    /// Muted text on the map, for what is waiting.
    MapMuted,
    /// A route.
    MapRoute,
    /// A bank's mark.
    MarkBank,
    /// A furrier's mark.
    MarkFurrier,
    /// A gemshop's mark.
    MarkGemshop,
    /// A pawnshop's mark.
    MarkPawnshop,
    /// The Adventurer's Guild's mark.
    MarkGuild,
    /// A locksmith's mark.
    MarkLocksmith,
    /// A healer's mark.
    MarkHealer,
    /// An herbalist's mark.
    MarkHerbalist,
    /// An alchemist's mark.
    MarkAlchemist,
    // Chrome.
    /// A veil over what is dimmed while arranging, drawn part clear.
    Veil,
    /// The arranging grid's faint lines, drawn part clear.
    Grid,
    /// The calibrator's mark on the doll's picture.
    Mark,
}

impl Token {
    /// Every token, in the order they are listed.
    pub const ALL: [Token; 61] = [
        Token::Accent,
        Token::Link,
        Token::Hydra,
        Token::Warning,
        Token::Wrong,
        Token::RoomName,
        Token::Creature,
        Token::Player,
        Token::Object,
        Token::Speech,
        Token::Whisper,
        Token::Thought,
        Token::Health,
        Token::Mana,
        Token::Stamina,
        Token::Spirit,
        Token::Blood,
        Token::Stance,
        Token::Encumbrance,
        Token::Mind,
        Token::Level,
        Token::Unhurt,
        Token::Wound1,
        Token::Wound2,
        Token::Wound3,
        Token::Scar1,
        Token::Scar2,
        Token::Scar3,
        Token::Body,
        Token::Stunned,
        Token::Bleeding,
        Token::Poisoned,
        Token::Hidden,
        Token::Silenced,
        Token::Posture,
        Token::ActiveSpells,
        Token::Buffs,
        Token::Debuffs,
        Token::Cooldowns,
        Token::Pulse,
        Token::MapBackground,
        Token::MapRoom,
        Token::MapRoomEdge,
        Token::MapLine,
        Token::MapConnector,
        Token::MapDoor,
        Token::MapYou,
        Token::MapMuted,
        Token::MapRoute,
        Token::MarkBank,
        Token::MarkFurrier,
        Token::MarkGemshop,
        Token::MarkPawnshop,
        Token::MarkGuild,
        Token::MarkLocksmith,
        Token::MarkHealer,
        Token::MarkHerbalist,
        Token::MarkAlchemist,
        Token::Veil,
        Token::Grid,
        Token::Mark,
    ];

    /// Its name, as a theme file will write it: `health`, `map_room`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Token::Accent => "accent",
            Token::Link => "link",
            Token::Hydra => "hydra",
            Token::Warning => "warning",
            Token::Wrong => "wrong",
            Token::RoomName => "room_name",
            Token::Creature => "creature",
            Token::Player => "player",
            Token::Object => "object",
            Token::Speech => "speech",
            Token::Whisper => "whisper",
            Token::Thought => "thought",
            Token::Health => "health",
            Token::Mana => "mana",
            Token::Stamina => "stamina",
            Token::Spirit => "spirit",
            Token::Blood => "blood",
            Token::Stance => "stance",
            Token::Encumbrance => "encumbrance",
            Token::Mind => "mind",
            Token::Level => "level",
            Token::Unhurt => "unhurt",
            Token::Wound1 => "wound1",
            Token::Wound2 => "wound2",
            Token::Wound3 => "wound3",
            Token::Scar1 => "scar1",
            Token::Scar2 => "scar2",
            Token::Scar3 => "scar3",
            Token::Body => "body",
            Token::Stunned => "stunned",
            Token::Bleeding => "bleeding",
            Token::Poisoned => "poisoned",
            Token::Hidden => "hidden",
            Token::Silenced => "silenced",
            Token::Posture => "posture",
            Token::ActiveSpells => "active_spells",
            Token::Buffs => "buffs",
            Token::Debuffs => "debuffs",
            Token::Cooldowns => "cooldowns",
            Token::Pulse => "pulse",
            Token::MapBackground => "map_background",
            Token::MapRoom => "map_room",
            Token::MapRoomEdge => "map_room_edge",
            Token::MapLine => "map_line",
            Token::MapConnector => "map_connector",
            Token::MapDoor => "map_door",
            Token::MapYou => "map_you",
            Token::MapMuted => "map_muted",
            Token::MapRoute => "map_route",
            Token::MarkBank => "mark_bank",
            Token::MarkFurrier => "mark_furrier",
            Token::MarkGemshop => "mark_gemshop",
            Token::MarkPawnshop => "mark_pawnshop",
            Token::MarkGuild => "mark_guild",
            Token::MarkLocksmith => "mark_locksmith",
            Token::MarkHealer => "mark_healer",
            Token::MarkHerbalist => "mark_herbalist",
            Token::MarkAlchemist => "mark_alchemist",
            Token::Veil => "veil",
            Token::Grid => "grid",
            Token::Mark => "mark",
        }
    }

    /// The token named `name`, if any.
    #[must_use]
    pub fn named(name: &str) -> Option<Token> {
        Token::ALL.into_iter().find(|token| token.name() == name)
    }

    /// The colour Hydra drew for it before it had themes
    /// (`crates/cena-gui`'s literals as they were, most of them Despana's
    /// or `VellumFE`'s).
    #[must_use]
    pub const fn bare(self) -> Rgb {
        match self {
            Token::Accent | Token::Warning | Token::RoomName => [0xd7, 0xad, 0x63],
            Token::Link => [0x47, 0x7a, 0xb3],
            Token::Hydra => [0x8f, 0xc9, 0xa8],
            Token::Wrong => [0xf0, 0x96, 0x8c],
            Token::Creature => [0xbd, 0x8c, 0xff],
            Token::Player => [0x8c, 0xa8, 0xff],
            Token::Object => [0xb7, 0xbd, 0xc3],
            Token::Speech => [0xf0, 0xee, 0xe8],
            Token::Whisper | Token::Thought => [0xa9, 0xbb, 0xf5],
            Token::Health | Token::Bleeding | Token::Debuffs => [0xcd, 0x4d, 0x4d],
            Token::Mana | Token::ActiveSpells | Token::Pulse => [0x47, 0x84, 0xd9],
            Token::Stamina | Token::Buffs => [0x55, 0xb8, 0x6c],
            Token::Spirit => [0xcb, 0xa9, 0x42],
            Token::Blood => [0x8b, 0x1a, 0x1a],
            Token::Stance | Token::Posture => [0x4f, 0xa3, 0xa5],
            Token::Encumbrance => [0xb0, 0x7a, 0x3c],
            Token::Mind | Token::Silenced => [0x8e, 0x6b, 0xc9],
            Token::Level => [0xc9, 0xa2, 0x3c],
            Token::Unhurt => [0x33, 0x33, 0x33],
            Token::Wound1 => [0xaa, 0x55, 0x00],
            Token::Wound2 => [0xff, 0x88, 0x00],
            Token::Wound3 => [0xff, 0x00, 0x00],
            Token::Scar1 => [0x99, 0x99, 0x99],
            Token::Scar2 => [0x77, 0x77, 0x77],
            Token::Scar3 => [0x55, 0x55, 0x55],
            Token::Body => [0x4a, 0x4a, 0x50],
            Token::Stunned => [0xd8, 0xb4, 0x3a],
            Token::Poisoned => [0x6d, 0xa8, 0x3c],
            Token::Hidden | Token::Cooldowns => [0x7a, 0x86, 0xa8],
            Token::MapBackground => [0x11, 0x16, 0x1b],
            Token::MapRoom => [0x49, 0x7f, 0xa3],
            Token::MapRoomEdge => [0xa2, 0xc8, 0xdf],
            Token::MapLine => [0x6e, 0x99, 0xb5],
            Token::MapConnector => [0x38, 0x51, 0x64],
            Token::MapDoor => [0xff, 0xc7, 0x78],
            Token::MapYou => [0xdd, 0xdc, 0xd7],
            Token::MapMuted => [0x8e, 0x9f, 0xad],
            Token::MapRoute => [0x57, 0xf3, 0xcb],
            Token::MarkBank => [0xe5, 0xbe, 0x52],
            Token::MarkFurrier => [0xcf, 0xaa, 0x80],
            Token::MarkGemshop => [0x64, 0xda, 0xfa],
            Token::MarkPawnshop => [0xf3, 0xa2, 0x5c],
            Token::MarkGuild => [0x82, 0x9f, 0xff],
            Token::MarkLocksmith => [0xc4, 0xa0, 0xef],
            Token::MarkHealer => [0xf3, 0x8d, 0x99],
            Token::MarkHerbalist => [0x91, 0xd5, 0x78],
            Token::MarkAlchemist => [0x60, 0xd3, 0xbd],
            Token::Veil => [0x00, 0x00, 0x00],
            Token::Grid => [0xff, 0xff, 0xff],
            Token::Mark => [0xff, 0xff, 0x00],
        }
    }
}

/// A colour for every token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    colors: [Rgb; Token::ALL.len()],
}

impl Palette {
    /// The colours Hydra drew before it had themes: not a theme (see the
    /// module's doc).
    #[must_use]
    pub fn bare() -> Self {
        let mut colors = [[0; 3]; Token::ALL.len()];
        for (slot, token) in colors.iter_mut().zip(Token::ALL) {
            *slot = token.bare();
        }
        Self { colors }
    }

    /// `token`'s colour.
    #[must_use]
    pub fn get(&self, token: Token) -> Rgb {
        self.colors[Self::slot(token)]
    }

    /// Set `token`'s colour.
    pub fn set(&mut self, token: Token, color: Rgb) {
        self.colors[Self::slot(token)] = color;
    }

    /// `token`'s index: its place in [`Token::ALL`].
    fn slot(token: Token) -> usize {
        Token::ALL
            .iter()
            .position(|t| *t == token)
            .unwrap_or_default()
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::bare()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_token_is_listed_once_with_its_own_name() {
        let names: BTreeSet<&str> = Token::ALL.iter().map(|t| t.name()).collect();
        assert_eq!(names.len(), Token::ALL.len(), "two tokens share a name");
        for token in Token::ALL {
            assert_eq!(Token::named(token.name()), Some(token));
        }
        assert_eq!(Token::named("no such"), None);
    }

    #[test]
    fn the_bare_palette_answers_each_token_with_its_literal() {
        let palette = Palette::bare();
        for token in Token::ALL {
            assert_eq!(palette.get(token), token.bare(), "{}", token.name());
        }
        assert_eq!(palette.get(Token::Health), [0xcd, 0x4d, 0x4d]);
    }

    #[test]
    fn a_colour_set_is_the_one_read_back_and_no_other() {
        let mut palette = Palette::bare();
        palette.set(Token::Mana, [1, 2, 3]);
        assert_eq!(palette.get(Token::Mana), [1, 2, 3]);
        assert_eq!(palette.get(Token::Health), Token::Health.bare());
    }
}
