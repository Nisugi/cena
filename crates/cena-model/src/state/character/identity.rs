//! Who the character is, from the login burst: its name and instance
//! (`<app>`), its number (`<playerID>`), and its game's code
//! (`<settingsInfo>`).
//!
//! Moved down out of `character.rs` when the game's code, kept for a Lich
//! started late (`plan/51` §7, step 4), took that file past its cap
//! (`plan/05` Rule 4.1: move code down, do not raise the cap).

use super::Character;

impl Character {
    /// Record who this character is, from the login burst.
    ///
    /// Empty strings are refused rather than stored: they would name a file
    /// after nobody, and `character_store::store_path` would reject them
    /// anyway -- better to hold `None` and say "not yet known".
    pub(crate) fn identify(&mut self, frame: &cena_protocol::Frame) {
        match frame {
            cena_protocol::Frame::AppInfo {
                character, game, ..
            } => {
                if !character.is_empty() {
                    self.name = Some(character.clone());
                }
                if !game.is_empty() {
                    self.instance = Some(game.clone());
                }
            }
            cena_protocol::Frame::PlayerId { id } if !id.is_empty() => {
                self.player_id = Some(id.clone());
            }
            cena_protocol::Frame::SettingsInfo {
                instance: Some(code),
                ..
            } if !code.is_empty() => {
                self.game_code = Some(code.clone());
            }
            _ => {}
        }
    }

    /// The character's own `exist` id, as the game's links name it: what
    /// tells a consumer "that link is me". `None` until `<playerID>` arrives.
    ///
    /// `-(10,000,000 + playerID)`. VERIFIED for one character, in two
    /// fixtures cut from one log (`cena-protocol/tests/FIXTURES.md`):
    /// `<playerID id='966483'/>` (`login_burst.xml:3`), and the same session's
    /// `info` naming the character `<a exist="-10966483">`
    /// (`character_info.xml:3`) -- the id Lich's own example gives him too
    /// (`gemstone/group.rb:482`). INFERRED for everyone else: every player
    /// link in Lich's examples is `-10` and six digits (`group.rb:417-509`).
    /// Lich stores the number (`common/xmlparser.rb:910-911`) and never
    /// compares it with a link, so no source states the rule.
    #[must_use]
    pub fn exist_id(&self) -> Option<String> {
        let number: u64 = self.player_id.as_deref()?.parse().ok()?;
        Some(format!("-{}", number.checked_add(10_000_000)?))
    }
}
