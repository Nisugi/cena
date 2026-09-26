//! Quick hunting (`;hunt <name> quick`; bigshot's `;bigshot quick`,
//! `bigshot.lic:9859`): fight what is in this room until it is clear, then
//! stop.
//!
//! What quick changes, each where bigshot changes it:
//!
//! | | bigshot |
//! |---|---|
//! | the targets are `quickhunt_targets`, or every hostile creature, on the `quick` routine | `sort_npcs`, `:8652-8668` |
//! | the `quick` routine is `quick_commands`, else routine `a` | `find_routine`, `:7174-7180` |
//! | no resting, whatever the thresholds | `should_rest? && !$bigshot_quick`, `:7155`; fried and wounded, `:8775`, `:8886` |
//! | no fleeing | `:8552` |
//! | the room is the hunt's, whoever is here | `bigclaim?`, `:7095`; `no_players_hunt`, `:7217` |
//! | no wandering: the room clear, the hunt ends | `single_stop`, `:7402`; `bs_wander` skipped, `:7353` |

use super::engine::Hunt;
use super::profile::Target;

/// The routine quick targets run.
pub(super) const QUICK: &str = "quick";

impl Hunt {
    /// This hunt as a quick one: the room it is in, until it is clear.
    #[must_use]
    pub fn quick(mut self) -> Self {
        self.quick = true;
        let targets = std::mem::take(&mut self.profile.quick_targets);
        self.profile.targets = if targets.is_empty() {
            vec![Target {
                name: None,
                any: true,
                routine: QUICK.to_owned(),
            }]
        } else {
            targets
        };
        if !self.profile.routines.contains_key(QUICK) {
            let a = self.profile.routines.get("a").cloned().unwrap_or_default();
            self.profile.routines.insert(QUICK.to_owned(), a);
        }
        self
    }
}
