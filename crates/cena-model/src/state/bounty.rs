//! Bounty tasks: what the Adventurer's Guild has asked for.
//!
//! Ports `lib/gemstone/bounty/parser.rb` (165 lines) and the parts of
//! `task.rb` that answer questions about a parsed task. A stateless classifier
//! over one command's output, which is the shape `plan/12` §3a asks for and
//! the shape `skills.rs` and `psm.rs` already take.
//!
//! # Two stages, and the wire distinguishes them
//!
//! A bounty has an **assignment** -- "go see the gem dealer" -- and then a
//! **task** with requirements once you ask about it. `TASK_MATCHERS` covers
//! both, which is why there is a `GemAssignment` beside a `Gem`, and why a
//! caller checking "do I have work to do" wants [`TaskKind::is_actionable`]
//! rather than merely "is it not `None`".
//!
//! # ORDER IS LOAD-BEARING
//!
//! `parser.rb:87` walks `TASK_MATCHERS` in declaration order and returns the
//! first match. That is not incidental: `:bandit` is declared before `:cull`
//! and its pattern is `:cull`'s with `bandit` substituted for the creature
//! capture, so a bandit task matches **both** and only the order decides. Same
//! for `:dangerous_spawned` before `:dangerous`.
//!
//! The matcher table preserves that order and `the_order_is_load_bearing` in the
//! tests asserts the two pairs specifically, so a future sort or a `HashMap`
//! cannot quietly reverse them.
//!
//! # The town is not always the captured town
//!
//! `determine_town` (`parser.rb:139-153`) overrides the capture in five cases,
//! four of them because the guard phrasing names no town at all:
//!
//! | Phrase | Town |
//! |---|---|
//! | `the sentry just outside town.` | Kraken's Fall |
//! | `the tavernkeeper at Rawknuckle's Common House.` | Cold River |
//! | `the elderly guard in the East Guardtower.` | Mist Harbor |
//! | mentions Captain, Reiya, Ataum or Galeb | Contempt |
//! | `gem dealer in has received` | Contempt |
//!
//! **The last is a workaround for a live typo**, and Lich says so: *"a
//! temporary workaround because of an actual typo in the messaging that should
//! be removed if it is ever actually fixed."* Ported with the comment, because
//! a reader who finds it and "fixes" it breaks Contempt gem bounties.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

/// What kind of bounty this is.
///
/// The `*Assignment` variants are the guild's referral; the others are the
/// task itself, with requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskKind {
    /// No task assigned.
    None,
    /// Go see someone about a bandit problem.
    BanditAssignment,
    /// Go see someone about a creature problem.
    CreatureAssignment,
    /// Go see the gem dealer.
    GemAssignment,
    /// Go track down a lost heirloom.
    HeirloomAssignment,
    /// Go see the herbalist.
    HerbAssignment,
    /// A local resident needs help.
    RescueAssignment,
    /// Go see the furrier.
    SkinAssignment,
    /// The task is done; report to the guild.
    Taskmaster,
    /// The heirloom is found; return it.
    HeirloomFound,
    /// The task is done; report to a specific guard.
    Guard,
    /// A dangerous creature has been provoked and must be killed.
    DangerousSpawned,
    /// The child has been found and must be escorted back.
    RescueSpawned,
    /// Kill N bandits.
    Bandit,
    /// Hunt a dangerous creature, which must first be provoked.
    Dangerous,
    /// Escort a client between two places.
    Escort,
    /// Collect N gems.
    Gem,
    /// Recover a lost heirloom from a creature's territory.
    Heirloom,
    /// Collect N herb samples.
    Herb,
    /// Clear creatures until the child appears.
    Rescue,
    /// Collect N skins of a given quality.
    Skin,
    /// Kill N of a creature type.
    Cull,
    /// The task was failed.
    Failed,
}

impl TaskKind {
    /// Is there work to do right now?
    ///
    /// **False for an assignment**, which is a referral rather than a task:
    /// the character must go and ASK about BOUNTIES before there is anything
    /// to hunt. Also false for the terminal states.
    #[must_use]
    pub const fn is_actionable(self) -> bool {
        matches!(
            self,
            Self::Bandit
                | Self::Cull
                | Self::Dangerous
                | Self::DangerousSpawned
                | Self::Escort
                | Self::Gem
                | Self::Heirloom
                | Self::HeirloomFound
                | Self::Herb
                | Self::Rescue
                | Self::RescueSpawned
                | Self::Skin
        )
    }

    /// Is the task finished, one way or the other?
    #[must_use]
    pub const fn is_done(self) -> bool {
        matches!(self, Self::Taskmaster | Self::Guard | Self::Failed)
    }

    /// Is this a referral rather than a task?
    #[must_use]
    pub const fn is_assignment(self) -> bool {
        matches!(
            self,
            Self::BanditAssignment
                | Self::CreatureAssignment
                | Self::GemAssignment
                | Self::HeirloomAssignment
                | Self::HerbAssignment
                | Self::RescueAssignment
                | Self::SkinAssignment
        )
    }
}

/// A parsed bounty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// Which kind.
    pub kind: TaskKind,
    /// The town the task belongs to, where one is known.
    pub town: Option<String>,
    /// Every named capture the pattern produced, by name.
    ///
    /// **A map rather than named fields**, and this is the one place in the
    /// character model where C21 argues the other way: the capture set differs
    /// per task kind (a skin task has `quality`, a gem task has `gem`, an
    /// escort has `start` and `destination`), so named fields would be twenty
    /// mostly-`None` options. `requirements` is Lich's own name for it.
    pub requirements: BTreeMap<String, String>,
}

impl Task {
    /// One requirement, if the pattern captured it.
    #[must_use]
    pub fn requirement(&self, name: &str) -> Option<&str> {
        self.requirements.get(name).map(String::as_str)
    }

    /// How many of the thing are needed, where the task counts.
    ///
    /// `parser.rb:113` converts `number` with `to_i`; this returns `None`
    /// rather than `0` for a task with no count, because "no count" and "zero
    /// left" are different answers.
    #[must_use]
    pub fn number(&self) -> Option<u32> {
        self.requirement("number")?.parse().ok()
    }

    /// The creature to kill, normalised.
    #[must_use]
    pub fn creature(&self) -> Option<&str> {
        self.requirement("creature")
    }

    /// Where the task takes place.
    #[must_use]
    pub fn area(&self) -> Option<&str> {
        self.requirement("area")
    }
}

/// The task patterns, **in match order**.
///
/// See the module docs: the order decides between `Bandit`/`Cull` and
/// `DangerousSpawned`/`Dangerous`, whose patterns overlap.
type Matcher = (TaskKind, &'static str);

/// Shared sub-patterns, expanded into the matchers below.
///
/// Named as Lich names them so the two files can be diffed.
mod frag {
    /// The guild's preamble, optional.
    pub const HMM: &str = r"(?:Hmm, I've got a task here from .*?(?P<town>[A-Z].*?)\..*?)?";
    /// Where a task takes place.
    pub const LOCATION: &str = r"(?:on|in|near) (?:the\s+)?(?P<area>[^.]+?)(?:\s+(?:near|between|under) (?P<town2>[^.]+))?";
    /// The many ways the game names a guard to report to.
    pub const GUARD: &str = r"(?:one of the guardsmen just inside the (?P<town3>Ta'Illistim) City Gate|one of the guardsmen just inside the Sapphire Gate|one of the guardsmen just inside the gate|one of the (?P<town4>.*) (?:gate|tunnel) guards|one of the (?P<town5>Icemule Trace) gate guards or the halfing Belle at the Pinefar Trading Post|Quin Telaren of (?P<town6>Wehnimer's Landing)|the dwarven militia sergeant near the (?P<town7>Kharam-Dzu) town gates|the sentry just outside town|the sentry just outside (?P<town8>Kraken's Fall)|the purser of (?P<town9>River's Rest)|the tavernkeeper at Rawknuckle's Common House|the captain of the (?P<town10>Contempt)|the elderly guard in the East Guardtower)";
    /// The herb task's body.
    pub const CONCOCTION: &str = r"is working on a concoction that requires (?:an?|some|several) (?P<herb>[^.]+?) found [oi]n (?:the\s+)?(?P<area>[^.]+?)(?:\s+(?:near|under|between) [^.]+)?\.  These samples must be in pristine condition\.  You have been tasked to retrieve (?P<number>\d+) (?:more\s+)?samples?\.";
    /// The taskmaster's quoted preamble.
    pub const MAYBE: &str = r"(?:The taskmaster told you:  \x22)";
}

/// Build the ordered matcher table.
///
/// Rust's `regex` crate has no alternation of named groups with the same name,
/// which Ruby allows -- so the `GUARD` union numbers its town captures
/// `town3`..`town10` and [`town_from`] reads whichever fired.
fn matchers() -> &'static Vec<(TaskKind, Regex)> {
    static MATCHERS: OnceLock<Vec<(TaskKind, Regex)>> = OnceLock::new();
    MATCHERS.get_or_init(|| {
        let specs: Vec<Matcher> = vec![
            (TaskKind::None, r"^You are not currently assigned a task"),
            (
                TaskKind::BanditAssignment,
                concat_frag(
                    frag::HMM,
                    r"It appears they have a bandit problem they'd like you to solve",
                ),
            ),
        ];
        // The table is built in two halves only because a `concat!` of
        // runtime strings needs a helper; the ORDER across both is the
        // declaration order of `parser.rb:25-79` and must not change.
        let mut out: Vec<(TaskKind, Regex)> = Vec::new();
        for (kind, pattern) in specs {
            if let Ok(re) = Regex::new(pattern) {
                out.push((kind, re));
            }
        }
        for (kind, pattern) in remaining_specs() {
            if let Ok(re) = Regex::new(&pattern) {
                out.push((kind, re));
            }
        }
        out
    })
}

/// Join a fragment to a literal.
fn concat_frag(prefix: &str, rest: &str) -> &'static str {
    // Leaked deliberately and once: `matchers()` is a `OnceLock`, so this runs
    // at most once per pattern for the life of the process, and the
    // alternative is threading owned `String`s through a `&'static` table for
    // no gain.
    Box::leak(format!("{prefix}{rest}").into_boxed_str())
}

/// The matchers that need runtime string assembly.
///
/// Declaration order continues from [`matchers`]; see the module docs on why
/// it matters.
fn remaining_specs() -> Vec<(TaskKind, String)> {
    // Split into three only to respect Rule 4.1's 100-line function cap; the
    // ORDER across all three is `parser.rb:25-79`'s declaration order and must
    // not change. See the module docs.
    let mut out = assignment_specs();
    out.extend(terminal_specs());
    out.extend(task_specs());
    out
}

/// The guild's referrals -- "go see the gem dealer".
fn assignment_specs() -> Vec<(TaskKind, String)> {
    let hmm = frag::HMM;
    let maybe = frag::MAYBE;
    vec![
        (
            TaskKind::CreatureAssignment,
            format!(
                r"{hmm}It appears they have a creature problem they'd like you to solve|{maybe}I've got an urgent mission for you\.  We have a creature problem we'd like you to solve\.  Go report to the (?P<town11>[A-Z].*?) to find out|{maybe}I've a favor to ask of you\.  We have a creature problem we'd like you to solve: you know, by killing\.  Go report to the (?P<town12>[A-Z].*?)"
            ),
        ),
        (
            TaskKind::GemAssignment,
            format!(
                r"{hmm}The local gem dealer, (?P<npc_name>[^,]+), has an order to fill and wants our help|All right\.  I've a mission for you\.  Our guest, the trader (?P<npc_name2>[^,]+)"
            ),
        ),
        (
            TaskKind::HeirloomAssignment,
            format!(
                r"{hmm}It appears they need your help in tracking down some kind of lost heirloom|{maybe}?It's time for you to earn your keep around here\.  I'd like you track down a lost heirloom\."
            ),
        ),
        (
            TaskKind::HerbAssignment,
            format!(
                r"{hmm}The local [^,]+?, (?P<npc_name>[^,]+), has asked for our aid\.  Head over there and see what you can do\.  Be sure to ASK about BOUNTIES\.|{maybe}I've got a mission for you\.  Our [^,]+?, (?P<npc_name2>[^,]+), has asked for our aid\.  Head over there and see what you can do\."
            ),
        ),
        (
            TaskKind::RescueAssignment,
            format!(
                r"{hmm}It appears that a local resident urgently needs our help in some matter"
            ),
        ),
        (
            TaskKind::SkinAssignment,
            format!(
                r"{hmm}The local furrier (?P<npc_name>.+) has an order to fill and wants our help|{maybe}?You look like you need work\.  The flesh merchant (?P<npc_name2>.+), down in the hold, has an order to fill and wants our help\."
            ),
        ),
    ]
}

/// Done, failed, or report-to-a-guard.
fn terminal_specs() -> Vec<(TaskKind, String)> {
    let guard = frag::GUARD;
    vec![
        (
            TaskKind::Taskmaster,
            r"^You have succeeded in your task and can return to the Adventurer's Guild".to_owned(),
        ),
        (
            TaskKind::HeirloomFound,
            format!(
                r"^You have located (?:an?|some) (?P<item>.+) and should bring (?:it back|your find) to {guard}\.$"
            ),
        ),
        (
            TaskKind::Guard,
            format!(r"^You succeeded in your task and should report back to {guard}\.$"),
        ),
    ]
}

/// The tasks themselves, with requirements.
fn task_specs() -> Vec<(TaskKind, String)> {
    let loc = frag::LOCATION;
    let guard = frag::GUARD;
    let maybe = frag::MAYBE;
    let conc = frag::CONCOCTION;
    vec![
        (
            TaskKind::DangerousSpawned,
            format!(
                r"^You have been tasked to hunt down and kill a particularly dangerous (?P<creature>[^.]+) that has established a territory {loc}\.  You have provoked (?:his|her|its) attention and now you must(?: return to where you left (?:him|her|it) and)? kill (?:him|her|it)!$"
            ),
        ),
        (
            TaskKind::RescueSpawned,
            format!(
                r"^You have made contact with the child you are to rescue and you must get (?:him|her) back alive to {guard}\.$"
            ),
        ),
        // **Before `Cull`**: a bandit task matches both patterns.
        (
            TaskKind::Bandit,
            format!(
                r"^You have been tasked to(?: help (?P<assist>\w+))? suppress (?P<creature>bandit) activity {loc}\.  You need to kill (?P<number>\d+) (?:more\s+)?of them to complete your task\.$"
            ),
        ),
        (
            TaskKind::Dangerous,
            format!(
                r"^You have been tasked to hunt down and kill a (?:particularly )?dangerous (?P<creature>[^.]+) that has established a territory {loc}\.  You can get its attention by killing other creatures of the same type in its territory\.$"
            ),
        ),
        (
            TaskKind::Escort,
            format!(
                r"{maybe}?I've got a special mission for you\.  A certain client has hired us to provide a protective escort on (?:his|her) upcoming journey\.  Go to (?P<start>[^.]+) and WAIT for (?:him|her) to meet you there\.  You must guarantee (?:his|her) safety to (?P<destination>[^.]+) as soon as you can, being ready for any dangers that the two of you may face\.  Good luck!\x22?$"
            ),
        ),
        (
            TaskKind::Gem,
            r"^The gem dealer in (?:(?P<town>[^,]+), (?P<npc_name>[^,]+), )?has received orders from multiple customers requesting (?:an?|some) (?P<gem>[^.]+)\.  You have been tasked to retrieve (?P<number>\d+) (?:more\s+)?of them\.  You can SELL them to the gem dealer as you find them\.$".to_owned(),
        ),
        (
            TaskKind::Heirloom,
            format!(
                r"^You have been tasked to recover (?:an?|some) (?P<item>[^.]+) that an unfortunate citizen lost after being attacked by an? (?P<creature>[^.]+?) {loc}\.  The heirloom can be identified by the initials \w+ engraved upon it\.  [^.]*?(?P<action>LOOT|SEARCH)[^.]+\.$"
            ),
        ),
        (
            TaskKind::Herb,
            format!(
                r"^The .+? in (?P<town>[^,]+?), (?P<npc_name>[^,]+), {conc}$|^The .+, (?P<npc_name2>[^,]+), aboard the (?P<town2b>\w+?) in .*? {conc2}$",
                conc2 = conc.replace("(?P<herb>", "(?P<herb2>").replace("(?P<area>", "(?P<area2>").replace("(?P<number>", "(?P<number2>")
            ),
        ),
        (
            TaskKind::Rescue,
            format!(
                r"^You have been tasked to rescue the young (?:runaway|kidnapped) (?:son|daughter) of a local citizen\.  A local divinist has had visions of the child fleeing from an? (?P<creature>[^.]+?) {loc}\.  Find the area where the child was last seen and clear out the creatures that have been tormenting (?:him|her) in order to bring (?:him|her) out of hiding\.$"
            ),
        ),
        (
            TaskKind::Skin,
            r"^You have been tasked to retrieve (?P<number>\d+) (?P<skin>[^.]+?)s? of at least (?P<quality>[^.]+) quality for (?P<npc_name>.+) in (?P<town>[^.]+?)\.  You can SKIN them off the corpse of an? (?P<creature>[^.]+) or purchase them from another adventurer\.  You can SELL the skins to the furrier as you collect them\.\x22?$".to_owned(),
        ),
        (
            TaskKind::Cull,
            // **The three "help X by suppressing" variants come FIRST.**
            // Ruby anchors each with its own trailing phrase (`during the
            // retrieval effort`), so its lazy `[^.]+?` stops in the right
            // place whatever order they are tried in. Rust's alternation
            // takes the leftmost branch that matches at all, so the generic
            // branch -- which has no trailing phrase -- would win and swallow
            // `during the retrieval effort` into `area`. Found by the spec
            // corpus: three cases had `area` = "Old Ta'Faendryl during the
            // retrieval effort".
            format!(
                r"^You have been tasked to help (?P<assist2>\w+) rescue a missing child by suppressing (?P<creature2>[^.]+) activity {loc2} during the rescue attempt\.  You need to kill (?P<number2>\d+) (?:more\s+)?of them to complete your task\.$|^You have been tasked to help (?P<assist3>\w+) retrieve an heirloom by suppressing (?P<creature3>[^.]+) activity {loc3} during the retrieval effort\.  You need to kill (?P<number3>\d+) (?:more\s+)?of them to complete your task\.$|^You have been tasked to help (?P<assist4>\w+) kill a dangerous creature by suppressing (?P<creature4>[^.]+) activity {loc4} during the hunt\.  You need to kill (?P<number4>\d+) (?:more\s+)?of them to complete your task\.$|^You have been tasked to(?: help (?P<assist>\w+))? suppress (?P<creature>[^.]+) activity {loc}\.  You need to kill (?P<number>\d+) (?:more\s+)?of them to complete your task\.$",
                loc2 = loc.replace("(?P<area>", "(?P<area2>").replace("(?P<town2>", "(?P<town2b>"),
                loc3 = loc.replace("(?P<area>", "(?P<area3>").replace("(?P<town2>", "(?P<town2c>"),
                loc4 = loc.replace("(?P<area>", "(?P<area4>").replace("(?P<town2>", "(?P<town2d>"),
            ),
        ),
        (
            TaskKind::Failed,
            r"^You have failed in your task|^The child you were tasked to rescue is gone and your task is failed\.  Report this failure to the Adventurer's Guild\.".to_owned(),
        ),
    ]
}

/// Normalise a creature name (`parser.rb:128-137`).
///
/// Two families the game qualifies with an adjective that is not part of the
/// creature's identity for hunting purposes.
fn normalize_creature(raw: &str) -> String {
    let words: Vec<&str> = raw.split_whitespace().collect();
    match words.as_slice() {
        [_, "being"] => "being".to_owned(),
        [_, "magna", "vereri"] => "magna vereri".to_owned(),
        _ => raw.to_owned(),
    }
}

/// Decide the town (`parser.rb:139-153`).
///
/// Four overrides exist because the guard phrasing names no town; the fifth is
/// a workaround for a typo in the game's own messaging.
fn town_from(description: &str, captured: Option<&str>) -> Option<String> {
    if description.ends_with("the sentry just outside town.") {
        return Some("Kraken's Fall".to_owned());
    }
    if description.ends_with("the tavernkeeper at Rawknuckle's Common House.") {
        return Some("Cold River".to_owned());
    }
    if description.ends_with("the elderly guard in the East Guardtower.") {
        return Some("Mist Harbor".to_owned());
    }
    // **The typo workaround.** Lich: "a temporary workaround because of an
    // actual typo in the messaging that should be removed if it is ever
    // actually fixed." Removing it breaks Contempt gem bounties.
    if description.contains("gem dealer in has received") {
        return Some("Contempt".to_owned());
    }
    for name in ["Captain", "Reiya", "Ataum", "Galeb"] {
        if description
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .any(|word| word == name)
        {
            return Some("Contempt".to_owned());
        }
    }
    captured.map(str::to_owned)
}

/// Classify one bounty description.
///
/// Returns `None` for text that is not a bounty at all -- `state.rs:306-309`'s
/// rule that a player can say anything.
#[must_use]
pub fn classify(description: &str) -> Option<Task> {
    let description = description.trim();
    if description.is_empty() {
        return None;
    }
    for (kind, regex) in matchers() {
        let Some(captures) = regex.captures(description) else {
            continue;
        };

        let mut requirements = BTreeMap::new();
        let mut captured_town = None;
        for name in regex.capture_names().flatten() {
            let Some(value) = captures.name(name) else {
                continue;
            };
            let value = value.as_str();
            // The duplicated-name suffixes exist only because Rust's `regex`
            // forbids two groups of one name in an alternation; strip them so
            // a caller sees Lich's vocabulary.
            let key = name.trim_end_matches(|c: char| c.is_ascii_digit());
            if key == "town" {
                captured_town = Some(value);
                continue;
            }
            let value = match key {
                "creature" => normalize_creature(value),
                // `parser.rb:116-117` downcases the action, so a caller
                // comparing against "loot" does not have to know the wire
                // shouts it.
                "action" => value.to_ascii_lowercase(),
                _ => value.to_owned(),
            };
            requirements.entry(key.to_owned()).or_insert(value);
        }

        let town = town_from(description, captured_town);
        if let Some(town) = town.clone() {
            requirements.insert("town".to_owned(), town);
        }
        return Some(Task {
            kind: *kind,
            town,
            requirements,
        });
    }
    None
}
