//! A login built from what this state knows, as of now: what a reader of the
//! game's stream who starts late is told (`plan/51` §7, step 4).
//!
//! That reader is the player's own Lich, switched on mid-session. It knows
//! only what it reads, and it missed the login. The author, 2026-09-28:
//! *"We know what the login blob consists of, so we can just build it in the
//! moment and send accurate info."* So it is told the character as this state
//! has it now, in the tags a login would have told it with, and in the order
//! Lich needs them: the game's code before the character's name (Lich keys
//! the character's folder on both, `reference/lich-5/lib/common/xmlparser.rb:918-938`),
//! the room's number before its parts, the prompt last.
//!
//! What it tells is what Lich keeps and a script reads. Lich pushes the same
//! list to a frontend attached late, from its own model
//! (`reference/lich-5/lib/gemstone/detachable_client_init.rb`: vitals, spell,
//! hands, indicators, stance, mind, encumbrance, injuries, compass); this adds
//! who and where the character is, the room's parts, the effects, the
//! timers, and what it wears.
//!
//! **What it cannot tell**, because this state does not keep it: the room's
//! styled name (Lich reads it from `style id='roomName'`, and the title says
//! the same), a creature's status flags (`crtrStatus`), the nervous system's
//! rank, and the target list, which the game sends again with every round.
//!
//! Each tag is written by `cena_protocol::write`, so nothing here spells
//! markup but tag and attribute names; and what it writes, this model reads
//! back to what it was written from (the tests).

use cena_protocol::write::{element, empty, escape, runs};

use super::GameState;
use super::hands::Hand;

/// The indicators Lich keeps, as it spells them, told whether lit or not:
/// its own list for a frontend attached late
/// (`reference/lich-5/lib/gemstone/detachable_client_init.rb:17-18`).
const INDICATORS: [&str; 13] = [
    "IconBLEEDING",
    "IconPOISONED",
    "IconDISEASED",
    "IconSTANDING",
    "IconKNEELING",
    "IconSITTING",
    "IconPRONE",
    "IconSTUNNED",
    "IconHIDDEN",
    "IconINVISIBLE",
    "IconDEAD",
    "IconWEBBED",
    "IconJOINED",
];

impl GameState {
    /// The login this state would be told by, as of now: the tags a reader
    /// of the game's stream, starting now, needs to know what this state
    /// knows. Empty until the game has named the character, since a login
    /// that says nothing of who is no login.
    #[must_use]
    pub fn login(&self) -> String {
        let Some(name) = &self.character.name else {
            return String::new();
        };
        let now = self.game_time_now();
        let mut login = String::new();
        if let Some(id) = &self.character.player_id {
            login.push_str(&empty("playerID", &[("id", id)]));
        }
        if let Some(code) = &self.character.game_code {
            login.push_str(&empty("settingsInfo", &[("instance", code)]));
        }
        let game = self.character.instance.as_deref().unwrap_or_default();
        login.push_str(&empty("app", &[("char", name), ("game", game)]));
        login.push('\n');
        self.tell_gauges(&mut login);
        self.tell_self(&mut login);
        self.tell_room(&mut login);
        self.tell_effects(&mut login, now);
        self.tell_worn(&mut login);
        if let Some(now) = now {
            for (tag, ends) in [
                ("roundTime", self.roundtime_ends),
                ("castTime", self.cast_time_ends),
            ] {
                if let Some(ends) = ends.filter(|ends| *ends > now) {
                    login.push_str(&empty(tag, &[("value", &ends.to_string())]));
                }
            }
            let prompt = self.prompt.as_deref().unwrap_or(">");
            login.push_str(&element(
                "prompt",
                &[("time", &now.to_string())],
                &escape(prompt),
            ));
            login.push('\n');
        }
        login
    }

    /// The vitals, stance, mind, experience and encumbrance, each in the
    /// dialog it comes in.
    fn tell_gauges(&self, login: &mut String) {
        let vitals: String = self
            .vitals
            .iter()
            .map(|(id, vital)| {
                let value = vital.percent.to_string();
                match vital.amount() {
                    Some((current, max)) => {
                        let text = format!("{id} {current}/{max}");
                        empty(
                            "progressBar",
                            &[("id", id), ("value", &value), ("text", &text)],
                        )
                    }
                    None => empty("progressBar", &[("id", id), ("value", &value)]),
                }
            })
            .collect();
        dialog(login, "minivitals", &vitals);
        let character = &self.character;
        let bar = |id: &str, percent: Option<u32>, text: Option<&String>| {
            percent.map_or_else(String::new, |percent| {
                let value = percent.to_string();
                let text = text.map_or("", String::as_str);
                empty(
                    "progressBar",
                    &[("id", id), ("value", &value), ("text", text)],
                )
            })
        };
        let label = |id: &str, value: Option<&String>| {
            value.map_or_else(String::new, |value| {
                empty("label", &[("id", id), ("value", value)])
            })
        };
        dialog(
            login,
            "stance",
            &bar(
                "pbarStance",
                character.stance_percent,
                character.stance.as_ref(),
            ),
        );
        let experience = &character.experience;
        dialog(
            login,
            "expr",
            &[
                label("yourLvl", experience.level.as_ref()),
                bar(
                    "mindState",
                    experience.mind_percent,
                    experience.mind_state.as_ref(),
                ),
                bar(
                    "nextLvlPB",
                    experience.next_level_percent,
                    experience.next_level.as_ref(),
                ),
            ]
            .concat(),
        );
        dialog(
            login,
            "encum",
            &[
                bar(
                    "encumlevel",
                    character.encumbrance_percent,
                    character.encumbrance.as_ref(),
                ),
                label("encumblurb", character.encumbrance_detail.as_ref()),
            ]
            .concat(),
        );
    }

    /// The indicators, the prepared spell, the hands, the injuries.
    fn tell_self(&self, login: &mut String) {
        for id in INDICATORS {
            let visible = if self.status.get(id) { "y" } else { "n" };
            login.push_str(&empty("indicator", &[("id", id), ("visible", visible)]));
        }
        if let Some(spell) = &self.prepared {
            let attrs: Vec<(&str, &str)> = self
                .prepared_id
                .as_deref()
                .map(|id| ("exist", id))
                .into_iter()
                .collect();
            login.push_str(&element("spell", &attrs, &escape(spell)));
        }
        for (tag, hand) in [("left", &self.left_hand), ("right", &self.right_hand)] {
            match hand {
                Hand::Unknown => {}
                Hand::Empty => login.push_str(&element(tag, &[], "Empty")),
                Hand::Holding { id, noun, name } => {
                    let mut attrs = Vec::new();
                    if let Some(id) = id {
                        attrs.push(("exist", id.as_str()));
                    }
                    if let Some(noun) = noun {
                        attrs.push(("noun", noun.as_str()));
                    }
                    login.push_str(&element(tag, &attrs, &escape(name)));
                }
            }
        }
        login.push('\n');
        // Each part the model knows of, as the injury window shows it: whole,
        // or its scar and then its wound, since a wound image leaves the scar
        // under it known (`character/body.rs`). The nervous system as the
        // window gives it, a rank without its kind (`character/nerves.rs`).
        let mut images = String::new();
        for part in super::character::body::ALL_PARTS {
            let injury = self
                .character
                .injuries
                .get(part)
                .copied()
                .unwrap_or_default();
            if !injury.is_hurt() {
                images.push_str(&empty("image", &[("id", part), ("name", part)]));
            } else if part == "nsys" {
                let rank = match self.character.nerves.rank {
                    0 => injury.wound.max(injury.scar),
                    rank => rank,
                };
                let name = format!("Nsys{rank}");
                images.push_str(&empty("image", &[("id", part), ("name", &name)]));
            } else {
                if injury.scar > 0 {
                    let name = format!("Scar{}", injury.scar);
                    images.push_str(&empty("image", &[("id", part), ("name", &name)]));
                }
                if injury.wound > 0 {
                    let name = format!("Injury{}", injury.wound);
                    images.push_str(&empty("image", &[("id", part), ("name", &name)]));
                }
            }
        }
        dialog(login, "injuries", &images);
    }

    /// The room: its number, its name, its parts, its exits, its codes.
    fn tell_room(&self, login: &mut String) {
        let room = &self.room;
        let Some(id) = &room.id else {
            return;
        };
        login.push_str(&empty("nav", &[("rm", id)]));
        if let Some(title) = &room.title {
            let subtitle = format!(" - {title}");
            for window in ["main", "room"] {
                login.push_str(&empty(
                    "streamWindow",
                    &[("id", window), ("subtitle", &subtitle)],
                ));
            }
        }
        login.push_str(&empty("clearStream", &[("id", "room")]));
        login.push_str(&empty("pushStream", &[("id", "room")]));
        for (id, body) in room.components() {
            login.push_str(&element("component", &[("id", id)], &runs(body)));
            login.push('\n');
        }
        login.push_str(&empty("popStream", &[("id", "room")]));
        if let Some(exits) = &room.exits {
            let dirs: String = exits
                .iter()
                .map(|dir| empty("dir", &[("value", dir)]))
                .collect();
            login.push_str(&element("compass", &[], &dirs));
        }
        if let Some(meta) = &room.meta {
            let codes = [
                ("weather", meta.weather),
                ("bonfire", meta.bonfire),
                ("inside", meta.inside),
                ("water", meta.water),
                ("sanctuary", meta.sanctuary),
                ("realm", meta.realm),
                ("climate", meta.climate),
                ("terrain", meta.terrain),
            ];
            let values: Vec<(&str, String)> = codes
                .iter()
                .filter_map(|(name, code)| code.map(|code| (*name, code.to_string())))
                .collect();
            let attrs: Vec<(&str, &str)> = values
                .iter()
                .map(|(name, code)| (*name, code.as_str()))
                .collect();
            login.push_str(&empty("roommeta", &attrs));
        }
        login.push('\n');
    }

    /// Each effects dialog the game has sent, emptied and told again: each
    /// effect with the time it has left, or `Indefinite`.
    fn tell_effects(&self, login: &mut String, now: Option<u32>) {
        for category in crate::effects::EFFECT_DIALOGS {
            if !self.effects.saw_category(category) {
                continue;
            }
            let bars: String = self
                .effects
                .in_category(category)
                .map(|(id, effect)| {
                    let left = now
                        .and_then(|now| self.effects.remaining(id, now))
                        .map_or_else(|| "Indefinite".to_owned(), clock);
                    empty(
                        "progressBar",
                        &[
                            ("id", id),
                            ("value", &effect.percent.to_string()),
                            ("text", &effect.text),
                            ("time", &left),
                        ],
                    )
                })
                .collect();
            login.push_str(&element(
                "dialogData",
                &[("id", category), ("clear", "t")],
                &bars,
            ));
            login.push('\n');
        }
    }

    /// What the character wears, as the `inv` stream lists it.
    fn tell_worn(&self, login: &mut String) {
        let Some(items) = self.worn.items() else {
            return;
        };
        login.push_str(&empty("clearStream", &[("id", "inv")]));
        login.push_str(&empty("pushStream", &[("id", "inv")]));
        login.push_str("Your worn items are:\n");
        for item in items {
            login.push_str("  ");
            login.push_str(&element(
                "a",
                &[("exist", &item.id), ("noun", &item.noun)],
                &escape(&item.text),
            ));
            login.push('\n');
        }
        login.push_str(&empty("popStream", &[]));
        login.push('\n');
    }
}

/// `<dialogData id=>` around `body`, when there is any.
fn dialog(login: &mut String, id: &str, body: &str) {
    if !body.is_empty() {
        login.push_str(&element("dialogData", &[("id", id)], body));
        login.push('\n');
    }
}

/// Seconds as the effects dialogs give a time left: `HH:MM:SS`.
fn clock(seconds: u32) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}
