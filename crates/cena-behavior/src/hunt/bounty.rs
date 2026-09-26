//! Bounty mode (`;hunt <name> bounty`; bigshot's `;bigshot bounty`,
//! `bigshot.lic:3345-3350`): hunt until the bounty is done or a new one can
//! be taken, rest, and end there, leaving the bounty's errand to whoever
//! runs it next (`should_hunt?`, `:8980-8986`).
//!
//! When it is done is bigshot's `bounty_eval` (`set_bounty_eval`,
//! `:3813-3856`, and `bounty_check?`, `:8929-8943`), read off the task the
//! model keeps (`state.bounty`):
//!
//! | The task | Done when | bigshot |
//! |---|---|---|
//! | succeeded, located, failed | the mind is not saturated, or the task is a bandit one | `:3823-3824` |
//! | none assigned | no `Next Bounty` cooldown runs | `:3825` |
//! | the child found | a child is in the room | `:3827-3828` |
//! | gems | as many of the gem as asked for are in the worn containers | `:3847-3852` |
//! | skins | as many loose skins as asked for are in the worn containers, or as many in the matching bundles, each bundle measured once | `:3831-3845` |
//! | anything, while bandits are here on a bandit bounty | never | `:8931` |
//!
//! The containers are the model's (`state.inventory`, Lich's
//! `GameObj.containers`): what the game has shown of each worn container.
//! bigshot measures every bundle at every check; Hydra's hunt never adds to a
//! bundle, so each is measured once, and not while hidden, as bigshot waits.

use std::collections::BTreeMap;

use cena_session::{GameState, RoomItem, Task, TaskKind, gameobj};

use super::engine::Hunt;

/// Bounty mode, and whether it began on a bandit bounty.
#[derive(Debug, Default)]
pub(super) struct BountyMode {
    /// `;hunt <name> bounty`.
    pub(super) on: bool,
    /// bigshot's `@BANDIT_HUNTING`: the bounty named bandits when first read.
    bandits: Option<bool>,
    /// Each skin bundle measured, by id: the skins it holds.
    bundles: BTreeMap<String, u32>,
    /// The bundle whose `measure` is out.
    measuring: Option<String>,
}

impl Hunt {
    /// This hunt in bounty mode: it ends at the rest once the bounty is done
    /// or a new one can be taken.
    #[must_use]
    pub fn bounty(mut self) -> Self {
        self.bounty_mode.on = true;
        self
    }

    /// Whether the bounty is done or a new one is ready: bigshot's
    /// `bounty_check?`. Never outside bounty mode, nor before the bounty has
    /// been read.
    pub(super) fn bounty_done(&mut self, state: &GameState) -> bool {
        if !self.bounty_mode.on {
            return false;
        }
        let Some(kind) = state.bounty.kind() else {
            return false;
        };
        let bandits = *self.bounty_mode.bandits.get_or_insert(matches!(
            kind,
            TaskKind::Bandit | TaskKind::BanditAssignment
        ));
        // bigshot's `target.type =~ /bandit/`: the object table's type.
        let bandit_here = || {
            self.could_fight(state).any(|creature| {
                creature
                    .noun
                    .as_deref()
                    .is_some_and(|noun| gameobj::classify(noun, &creature.name).is("bandit"))
            })
        };
        if bandits && bandit_here() {
            return false;
        }
        match kind {
            TaskKind::Taskmaster | TaskKind::HeirloomFound | TaskKind::Guard | TaskKind::Failed => {
                bandits
                    || state
                        .character
                        .experience
                        .mind_percent
                        .is_none_or(|mind| mind < 100)
            }
            TaskKind::None => !next_bounty_cooling(state),
            TaskKind::Gem => state
                .bounty
                .task()
                .is_some_and(|task| gems_in_hand(task, state)),
            TaskKind::Skin => state
                .bounty
                .task()
                .is_some_and(|task| self.skins_in_hand(task, state)),
            TaskKind::RescueSpawned => state.creatures().in_room().any(|creature| {
                creature
                    .name
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .any(|word| word.eq_ignore_ascii_case("child"))
            }),
            _ => false,
        }
    }
}

impl Hunt {
    /// bigshot's two skin counts, each on its own: the loose skins, and
    /// what the matching bundles hold, as far as they have been measured.
    fn skins_in_hand(&self, task: &Task, state: &GameState) -> bool {
        let (Some(wanted), Some(needed)) = (task.requirement("skin"), task.number()) else {
            return false;
        };
        let loose = skin_name(wanted);
        let items: Vec<&RoomItem> = worn_contents(state).collect();
        let lying = items
            .iter()
            .filter(|item| !loose.is_empty() && contains_ignoring_case(&item.text, &loose))
            .count();
        let bundled: u32 = items
            .iter()
            .filter(|item| {
                bundle_of(wanted).is_some_and(|b| contains_ignoring_case(&item.text, &b))
            })
            .filter_map(|item| self.bounty_mode.bundles.get(&item.id))
            .sum();
        u32::try_from(lying).unwrap_or(u32::MAX) >= needed || bundled >= needed
    }

    /// `measure #id` for a skin bundle not yet measured, in bounty mode on a
    /// skin bounty and not hidden.
    pub(super) fn bounty_measure(&mut self, state: &GameState) -> Option<super::said::Said> {
        if !self.bounty_mode.on || state.status.known().hidden() == Some(true) {
            return None;
        }
        let task = state
            .bounty
            .task()
            .filter(|task| task.kind == TaskKind::Skin)?;
        let bundle = bundle_of(task.requirement("skin")?)?;
        let id = worn_contents(state)
            .find(|item| {
                contains_ignoring_case(&item.text, &bundle)
                    && !self.bounty_mode.bundles.contains_key(&item.id)
            })?
            .id
            .clone();
        self.bounty_mode.measuring = Some(id.clone());
        Some(super::said::Said::Send {
            line: format!("measure #{id}"),
            target: None,
        })
    }

    /// The answer to a bundle's `measure`: `count a total of N`. A bundle
    /// the answer does not count holds none, so it is not asked again.
    pub(super) fn bounty_replied(&mut self, lines: &[&str]) {
        let Some(id) = self.bounty_mode.measuring.take() else {
            return;
        };
        let counted = lines.iter().find_map(|line| {
            let rest = &line[line.find("count a total of ")? + "count a total of ".len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        });
        self.bounty_mode.bundles.insert(id, counted.unwrap_or(0));
    }
}

/// bigshot's gem count: the worn containers hold as many as the gem dealer
/// asked for.
fn gems_in_hand(task: &Task, state: &GameState) -> bool {
    let (Some(gem), Some(needed)) = (task.requirement("gem"), task.number()) else {
        return false;
    };
    let gem = gem.trim();
    let held = worn_contents(state)
        .filter(|item| !gem.is_empty() && contains_ignoring_case(&item.text, gem))
        .count();
    u32::try_from(held).unwrap_or(u32::MAX) >= needed
}

/// What the worn containers hold, as the game has shown them: bigshot's
/// `@CONTAINERS`, the worn items that are containers. Every container the
/// session has seen while the worn list is unread.
fn worn_contents(state: &GameState) -> impl Iterator<Item = &RoomItem> {
    let worn = state.worn.items();
    state
        .inventory
        .containers()
        .filter(move |(window, container)| {
            let id = container.target.as_deref().unwrap_or(window);
            worn.is_none_or(|items| items.iter().any(|item| item.id == id))
        })
        .flat_map(|(_, container)| container.items.iter())
}

/// bigshot's skin name, as it matches a loose skin: lowercase, no final
/// `s`, and `tooth`, `hoof`, `ruff` (`:3836-3840`).
fn skin_name(wanted: &str) -> String {
    let mut name = wanted.trim().to_ascii_lowercase();
    if name.ends_with('s') {
        name.pop();
    }
    name.replace("teeth", "tooth")
        .replace("hooves", "hoof")
        .replace("hoove", "hoof")
        .replace("ruffs", "ruff")
}

/// `bundle of <the skin's last two words>` (`@BUNDLE_SKIN`, `:3834`); none
/// for a one-word skin, as bigshot's match finds none.
fn bundle_of(wanted: &str) -> Option<String> {
    let words: Vec<&str> = wanted.split_whitespace().collect();
    let [.., first, last] = words.as_slice() else {
        return None;
    };
    Some(format!("bundle of {first} {last}"))
}

fn contains_ignoring_case(text: &str, part: &str) -> bool {
    text.to_ascii_lowercase()
        .contains(&part.to_ascii_lowercase())
}

/// The guild's `Next Bounty` cooldown is running.
fn next_bounty_cooling(state: &GameState) -> bool {
    let Some(now) = state.game_time_now() else {
        return true;
    };
    state.effects.in_category("Cooldowns").any(|(id, effect)| {
        effect.text.starts_with("Next Bounty") && state.effects.active(id, now) == Some(true)
    })
}
