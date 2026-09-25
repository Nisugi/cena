//! A bigshot profile in, a Hydra profile out (`plan/30` §4 and §5).
//!
//! The author has only bigshot profiles (Q8), so this is how a profile gets
//! in. It reads the policy keys, translates `wounded_eval` where it is one
//! of the known shapes, and **names what it drops**: every key it did not
//! carry, every guard it could not build, every `script` it turned into a
//! sequence to be written by hand. The notes go at the head of the written
//! file, where the person who opens it sees them.
//!
//! # Guards: translate, hold, never lose
//!
//! A step's guards go through bigshot's own reading of them (`plan/33`
//! §1), word by word, in `import/words.rs`, whose module docs table every
//! word and the branch it was read from.
//!
//! A word that cannot be translated **holds the step**: it is kept, with
//! the word named, and never runs. That covers the two words the model
//! cannot answer yet, the forms no single guard says, and the words that
//! never held anything back in bigshot (`check_state_condition` ends in
//! `else false`, `:4522`). A lost guard changes when a command fires, and
//! that is worse than a refusal. `censer` is no guard: it turns on the
//! profile's `censer_between_actions`.
//!
//! # `script <name>` becomes a sequence to be written
//!
//! bigshot's `script volley` ran a Lich script and waited. Hydra cannot read
//! Ruby, so the step becomes `volley` and an empty `[sequences] volley`
//! appears for the person to fill (`plan/30` §5 shows the author's, which
//! is two guards and seven commands). Until it is written the routine skips
//! it, and `;hunt check` says so.
//!
//! # What it reads, key by key
//!
//! The rows of `plan/30` §4's table. Every other key with a value that is
//! not blank or `false` is named as not imported, except bigshot's own
//! bookkeeping (`profile_current`, `save_profile_name`, `notes`).

use std::collections::{BTreeMap, BTreeSet};

use super::guard;
use super::profile::{Profile, Step, Target};
use super::yaml;
use crate::stance::Want;

/// bigshot's own bookkeeping, never policy.
const BOOKKEEPING: &[&str] = &["profile_current", "save_profile_name", "notes"];

/// The catch-all patterns bigshot profiles use for "any creature".
const ANY: &[&str] = &["(?:.+?)", "(?:.+)", "(?:.*)", ".+?", ".+", ".*"];

mod rest;
mod words;

use words::{Word, translate};

/// A profile brought in from bigshot, and what the bringing cost.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    /// The profile's name in Hydra.
    pub name: String,
    /// What was carried.
    pub profile: Profile,
    /// What was held, dropped, or left to be written by hand. Empty when
    /// nothing was.
    pub notes: Vec<String>,
}

impl Import {
    /// The file to write: the notes as a comment at the head, then the
    /// profile as TOML.
    ///
    /// # Errors
    ///
    /// The profile could not be written as TOML ([`Profile::to_toml`]).
    pub fn render(&self) -> Result<String, String> {
        let mut out = format!(
            "# Hydra hunt profile {:?}, imported from a bigshot profile.\n",
            self.name
        );
        if self.notes.is_empty() {
            out.push_str("# Everything in the bigshot profile was carried over.\n");
        } else {
            out.push_str("#\n# What the importer held, or could not carry:\n");
            for note in &self.notes {
                for line in note.lines() {
                    out.push_str("#   ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        out.push('\n');
        out.push_str(&self.profile.to_toml()?);
        Ok(out)
    }
}

/// Read a bigshot profile's YAML as the Hydra profile `name`.
///
/// # Errors
///
/// The text is not the flat YAML bigshot writes ([`yaml::read`]).
pub fn import(name: &str, yaml_text: &str) -> Result<Import, String> {
    let mut job = Job {
        source: Source::new(yaml::read(yaml_text)?),
        profile: Profile::default(),
        notes: Vec::new(),
    };
    job.rooms();
    job.stances();
    job.rest();
    job.lists();
    job.loot_flee_wander();
    job.targets();
    job.routines();
    for (key, value) in job.source.left() {
        job.notes
            .push(format!("not imported: {key} = {}", shorten(&value)));
    }
    Ok(Import {
        name: name.to_owned(),
        profile: job.profile,
        notes: job.notes,
    })
}

/// The bigshot keys, and which have been read.
struct Source {
    values: BTreeMap<String, String>,
    used: BTreeSet<String>,
}

impl Source {
    fn new(values: BTreeMap<String, String>) -> Self {
        Self {
            values,
            used: BTreeSet::new(),
        }
    }

    /// The key's text, empty when absent; the key is marked read.
    fn take(&mut self, key: &str) -> String {
        self.used.insert(key.to_owned());
        self.values.get(key).cloned().unwrap_or_default()
    }

    /// Every key not read whose value says something: not blank, not
    /// `false`, not bookkeeping.
    fn left(&self) -> Vec<(String, String)> {
        self.values
            .iter()
            .filter(|(key, value)| {
                !self.used.contains(*key)
                    && !BOOKKEEPING.contains(&key.as_str())
                    && !value.trim().is_empty()
                    && !value.trim().eq_ignore_ascii_case("false")
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

/// One import under way.
struct Job {
    source: Source,
    profile: Profile,
    notes: Vec<String>,
}

impl Job {
    fn take(&mut self, key: &str) -> String {
        self.source.take(key)
    }

    fn note(&mut self, text: String) {
        self.notes.push(text);
    }

    fn rooms(&mut self) {
        self.profile.rooms.hunting = self.room("hunting_room_id");
        self.profile.rooms.resting = self.room("resting_room_id");
        for entry in list(&self.take("hunting_boundaries")) {
            match number(&entry) {
                Some(id) => self.profile.rooms.boundaries.push(id),
                None => self.note(format!(
                    "hunting_boundaries: `{entry}` is not a room number; Hydra's rooms are the map's numbers"
                )),
            }
        }
    }

    /// A room key: the map's number, or a note. bigshot also takes `u<uid>`,
    /// which needs the map to resolve and is not resolved here.
    fn room(&mut self, key: &str) -> Option<u32> {
        let text = self.take(key);
        if text.trim().is_empty() {
            return None;
        }
        let id = number(&text);
        if id.is_none() {
            self.note(format!(
                "{key}: `{text}` is not a room number; Hydra's rooms are the map's numbers, and a `u` id needs the map to resolve"
            ));
        }
        id
    }

    fn stances(&mut self) {
        self.profile.stance.hunting = self.stance("hunting_stance");
        self.profile.stance.wander = self.stance("wander_stance");
        self.profile.stance.stand = self.stance("stand_stance");
    }

    /// A stance key, normalised as eohunter's `profile.rb` does: a stance
    /// by its first three letters or more, or a percent that is a multiple
    /// of ten. Anything else is noted and left unset.
    fn stance(&mut self, key: &str) -> Option<String> {
        let text = self.take(key);
        if text.trim().is_empty() {
            return None;
        }
        match Want::parse(&text) {
            Ok(Want::Named(stance)) => Some(stance.as_str().to_owned()),
            Ok(Want::Percent(percent)) => Some(percent.to_string()),
            Err(why) => {
                self.note(format!("{key}: {why}; left unset"));
                None
            }
        }
    }

    fn lists(&mut self) {
        self.profile.prepare = self.commands("hunting_prep_commands");
        self.profile.signs = list(&self.take("signs"));
        // bigshot's comments on its own settings: `flee_count`, "flee if
        // enemy count is >"; `invalid_targets`, "but don't count these";
        // `always_flee_from`, "and always flee from" (`bigshot.lic:3482-3485`).
        // So `invalid_targets` is a flee setting, not a list never attacked.
        self.profile.flee.uncounted = lowercased(list(&self.take("invalid_targets")));
        self.profile.flee.from = lowercased(list(&self.take("always_flee_from")));
        self.profile.flee.clouds = flag(&self.take("flee_clouds"));
        self.profile.flee.vines = flag(&self.take("flee_vines"));
        self.profile.flee.webs = flag(&self.take("flee_webs"));
        self.profile.flee.voids = flag(&self.take("flee_voids"));
        let message = self.take("flee_message");
        let message = message.trim();
        if message.chars().any(|c| ".*+?[](){}^$\\".contains(c)) {
            self.note(format!(
                "flee_message: `{message}` is a regex; Hydra matches plain phrases, so it was not carried"
            ));
        } else if !message.is_empty() {
            self.profile.flee.messages = message
                .split('|')
                .map(|m| m.trim().to_ascii_lowercase())
                .filter(|m| !m.is_empty())
                .collect();
        }
    }

    /// A command list (`split_xx`): repeats expanded, and `a and b`, which
    /// bigshot sent as a pair, sent one after the other.
    fn commands(&mut self, key: &str) -> Vec<String> {
        let mut out = Vec::new();
        for entry in expanded(&self.take(key)) {
            if entry.contains(" and ") {
                self.note(format!(
                    "{key}: `{entry}` is two commands joined with `and`; Hydra sends them one after the other"
                ));
                out.extend(entry.split(" and ").map(|s| s.trim().to_owned()));
            } else {
                out.push(entry);
            }
        }
        out
    }

    fn loot_flee_wander(&mut self) {
        let script = self.take("loot_script");
        self.profile.loot.script = (!script.trim().is_empty()).then(|| script.trim().to_owned());
        self.profile.loot.delay = flag(&self.take("delay_loot"));
        self.profile.loot.defensive = flag(&self.take("loot_stance"));
        self.profile.loot.box_in_hand = flag(&self.take("box_in_hand"));
        self.profile.flee.count = number(&self.take("flee_count"));
        self.profile.flee.lone_only = flag(&self.take("lone_targets_only"));
        self.profile.aim.ambush = lowercased(list(&self.take("ambush")));
        self.profile.aim.archery = lowercased(list(&self.take("archery_aim")));
        self.profile.boons.ignore = lowercased(list(&self.take("boons_ignore")));
        self.profile.boons.flee = lowercased(list(&self.take("boons_flee")));
        self.profile.wand.names = list(&self.take("wand"));
        let fresh = self.take("fresh_wand_container");
        self.profile.wand.fresh = (!fresh.trim().is_empty()).then(|| fresh.trim().to_owned());
        let dead = self.take("dead_wand_container");
        self.profile.wand.dead = (!dead.trim().is_empty()).then(|| dead.trim().to_owned());
        self.profile.wand.if_oom = flag(&self.take("wand_if_oom"));
        self.profile.priority = flag(&self.take("priority"));
        // bigshot turns autosneak on when it starts attacking and off when
        // it stops (`bigshot.lic:7295-7298`, `:7454-7457`): here, with the
        // commands sent on returning to hunt and on arriving to rest.
        if flag(&self.take("sneaky_sneaky")) {
            self.profile
                .prepare
                .push("movement autosneak on".to_owned());
            self.profile
                .rest
                .commands
                .insert(0, "movement autosneak off".to_owned());
        }
        self.profile.wander.ignore_disks = flag(&self.take("ignore_disks"));
        self.profile.react.bless = flag(&self.take("bless"));
        self.profile.react.deader = flag(&self.take("deader"));
        // Absent is bigshot's default, on.
        let pull = self.take("pull");
        if !pull.trim().is_empty() {
            self.profile.react.pull = flag(&pull);
        }
        // Absent is bigshot's default, on.
        let reaction = self.take("weapon_reaction");
        if !reaction.trim().is_empty() {
            self.profile.react.weapon_reaction = flag(&reaction);
        }
        let wait = self.take("wander_wait");
        if wait.trim().is_empty() {
            return;
        }
        match wait.trim().parse::<f64>() {
            Ok(seconds) if seconds >= 0.0 => self.profile.wander.wait = seconds,
            _ => self.note(format!("wander_wait: `{wait}` is not a number of seconds")),
        }
    }

    /// `targets`: `name(letter)` entries, a bare name meaning routine `a`,
    /// and a catch-all pattern meaning any creature.
    fn targets(&mut self) {
        for entry in list(&self.take("targets")) {
            let (name, routine) = match entry
                .strip_suffix(')')
                .and_then(|body| body.rsplit_once('('))
            {
                Some((name, letter))
                    if letter.len() == 1 && letter.chars().all(|c| c.is_ascii_alphabetic()) =>
                {
                    (
                        name.trim().to_ascii_lowercase(),
                        letter.to_ascii_lowercase(),
                    )
                }
                _ => (entry.to_ascii_lowercase(), "a".to_owned()),
            };
            let any = ANY.contains(&name.as_str());
            if !any && name.contains(['(', ')', '[', ']', '|', '\\', '^', '$', '*', '+', '?']) {
                self.note(format!(
                    "target {name:?}: bigshot read it as a pattern; Hydra matches it whole, as written"
                ));
            }
            self.profile.targets.push(Target {
                name: (!any).then_some(name),
                any,
                routine,
            });
        }
    }

    /// `hunting_commands` is routine `a`; `hunting_commands_b` to `_j` are
    /// `b` to `j`. Empty ones are not carried.
    fn routines(&mut self) {
        for letter in 'a'..='j' {
            let key = if letter == 'a' {
                "hunting_commands".to_owned()
            } else {
                format!("hunting_commands_{letter}")
            };
            let entries = expanded(&self.take(&key));
            if entries.is_empty() {
                continue;
            }
            let steps: Vec<Step> = entries.iter().map(|entry| self.step(entry)).collect();
            self.profile.routines.insert(letter.to_string(), steps);
        }
    }

    /// One routine entry: the verb and its guards, translated; held when
    /// any guard cannot be.
    fn step(&mut self, entry: &str) -> Step {
        if entry.contains(" and ") {
            return Step::held(
                entry,
                "two commands joined with `and` are not supported yet",
            );
        }
        let (verb, group) = split_bigshot(entry);
        let verb = match verb.strip_prefix("script ") {
            Some(name) => {
                let name = name.trim().to_owned();
                self.sequence_for(&name);
                name
            }
            None => verb.to_owned(),
        };
        let Some(group) = group else {
            return Step::parse(&verb).unwrap_or_else(|why| Step::held(entry, &why));
        };
        let tokens = match guard::tokens(group) {
            Ok(tokens) => tokens,
            Err(why) => return Step::held(entry, &why),
        };
        let mut guards = Vec::new();
        let mut held = Vec::new();
        for token in &tokens {
            match translate(&verb, token) {
                Ok(Word::Guard(guard)) => guards.push(guard),
                Ok(Word::Censer) => self.censer(),
                Err(why) => held.push(why),
            }
        }
        if !held.is_empty() {
            return Step::held(entry, &held.join("; "));
        }
        if guards.is_empty() {
            return Step::parse(&verb).unwrap_or_else(|why| Step::held(entry, &why));
        }
        let line = format!("{verb} ({})", guards.join(" "));
        Step::parse(&line).unwrap_or_else(|why| Step::held(entry, &why))
    }

    /// A routine carried `censer`: the censer is cast between every step
    /// (`plan/33` §2h), noted once.
    fn censer(&mut self) {
        if self.profile.censer_between_actions {
            return;
        }
        self.profile.censer_between_actions = true;
        self.note(
            "`censer` became censer_between_actions: Ethereal Censer is cast between routine steps \
             whenever it is off cooldown and affordable, as the word meant for the whole routine"
                .to_owned(),
        );
    }

    /// An empty sequence for `script <name>`, noted once.
    fn sequence_for(&mut self, name: &str) {
        if self.profile.sequences.contains_key(name) {
            return;
        }
        self.profile.sequences.insert(name.to_owned(), Vec::new());
        self.note(format!(
            "sequence `{name}` stands in for `script {name}`: bigshot ran a Lich script, which Hydra cannot read. \
             Write its steps under [sequences] {name}; until then the routine skips it"
        ));
    }
}

/// The verb and the guard group of a bigshot routine entry: bigshot's
/// `\((.*?)\)$`, the first `(` when the entry ends in `)`.
fn split_bigshot(entry: &str) -> (&str, Option<&str>) {
    let entry = entry.trim();
    match entry
        .strip_suffix(')')
        .and_then(|body| body.split_once('('))
    {
        Some((verb, group)) => (verb.trim(), Some(group.trim())),
        None => (entry, None),
    }
}

/// A term with its outer parentheses off, however many.
fn unparenthesised(term: &str) -> &str {
    let mut term = term.trim();
    while let Some(inner) = term.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
        term = inner.trim();
    }
    term
}

/// `Char.percent_health <= 60` is 60; `< 60` is 59.
fn health_at_most(term: &str) -> Option<u32> {
    let rest = ["Char.percent_health", "percenthealth", "checkhealth"]
        .iter()
        .find_map(|prefix| term.strip_prefix(prefix))?
        .trim();
    if let Some(n) = rest.strip_prefix("<=") {
        return n.trim().parse().ok();
    }
    let n: u32 = rest.strip_prefix('<')?.trim().parse().ok()?;
    Some(n.saturating_sub(1))
}

/// A whole number, or not.
fn number(text: &str) -> Option<u32> {
    text.trim().parse().ok()
}

/// bigshot's booleans: `true` and everything else.
fn flag(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("true")
}

/// A comma list, trimmed, blanks dropped.
fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn lowercased(items: Vec<String>) -> Vec<String> {
    items.into_iter().map(|s| s.to_ascii_lowercase()).collect()
}

/// bigshot's `split_xx` (eohunter `profile.rb`): a comma list, with `(xN)`
/// after an entry repeating it N times and `(xx)` five.
fn expanded(text: &str) -> Vec<String> {
    list(text)
        .iter()
        .flat_map(|entry| {
            let (entry, times) = repeated(entry);
            std::iter::repeat_n(entry, times)
        })
        .collect()
}

/// An entry and how many times it is meant: `fire(x3)` is `fire`, 3.
fn repeated(entry: &str) -> (String, usize) {
    let once = || (entry.to_owned(), 1);
    let Some(body) = entry.strip_suffix(')') else {
        return once();
    };
    let Some((verb, marker)) = body.rsplit_once('(') else {
        return once();
    };
    let times = match marker.strip_prefix(['x', 'X']) {
        Some("x" | "X") => 5,
        Some(digits) => match digits.parse::<usize>() {
            Ok(n) => n,
            Err(_) => return once(),
        },
        None => return once(),
    };
    (verb.trim().to_owned(), times)
}

/// The first line of a value, cut short.
fn shorten(value: &str) -> String {
    let line = value.lines().next().unwrap_or("");
    let cut: String = line.chars().take(60).collect();
    if cut.len() < line.len() || value.lines().count() > 1 {
        format!("{cut}...")
    } else {
        cut
    }
}
