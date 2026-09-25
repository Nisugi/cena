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
//! §1), word by word:
//!
//! | bigshot | becomes | because |
//! |---|---|---|
//! | `hidden`, `!hidden` | the same | `bigshot.lic:4396` |
//! | `frozen` | `!immobilized` | `frozen` **skips** when the target is frozen (`:4418`), and Hydra's guards name when the step runs |
//! | `!frozen` | `immobilized` | " |
//! | `thpN`, `!thpN` | `thp N`, `!thp N` | `:4336` |
//! | `empoweredN` | `empowered_below N` | `:4319` skips when an Empowered of +N or more is up |
//! | `buffN` on a verb | `expiring "<its buff>" N` | `:4240`: skip while the verb's own buff has N s or less left; the buff comes from `:3228-3241` |
//!
//! A word bigshot accepts that Hydra has not built **holds the step**: it is
//! kept, with the word named, and never runs. A word bigshot does not accept
//! either holds the step too, and says so, because in bigshot it never held
//! anything back (`check_state_condition` ends in `else false`, `:4522`).
//! A lost guard changes when a command fires, and that is worse than a
//! refusal.
//!
//! `buffN` is carried as the author meant it, not as bigshot runs it:
//! `expiring` runs the step when the buff is down or about to lapse
//! (`guard.rs`'s module docs have the measurement).
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

/// Every guard word bigshot accepts (`bigshot.lic:3197`), bare: without
/// `!`, without the number an amount word takes, without the quoted text
/// an `E?"…"` word takes. MEASURED: 87 (`plan/33`).
const BIGSHOT_WORDS: &[&str] = &[
    "506",
    "ancient",
    "animate",
    "ascended",
    "ascension_boss",
    "barrage",
    "bearhug",
    "buff",
    "burst",
    "calm",
    "celerity",
    "censer",
    "challenging",
    "coupdegrace",
    "disease",
    "disengaged",
    "disoriented",
    "EB",
    "EC",
    "ED",
    "ES",
    "empowered",
    "e",
    "essence",
    "fatalcrit",
    "frozen",
    "flurry",
    "flying",
    "fury",
    "garrote",
    "h",
    "hidden",
    "holler",
    "hovering",
    "immobilized",
    "inferior",
    "justice",
    "k",
    "kneeling",
    "m",
    "mini_boss",
    "mob",
    "momentum",
    "mount",
    "noncorporeal",
    "once",
    "outside",
    "pcs",
    "poison",
    "prone",
    "pummel",
    "rapid",
    "rebuke",
    "reflex",
    "repeatdelay",
    "rider",
    "room",
    "rooted",
    "s",
    "scourge",
    "shout",
    "sitting",
    "sleeping",
    "smote",
    "splashy",
    "stunned",
    "surge",
    "sympathetic",
    "tailwind",
    "tier",
    "tier1",
    "tier2",
    "tier3",
    "thp",
    "thrash",
    "ucsdecent",
    "ucsexcellent",
    "ucsgood",
    "ucstierup",
    "undead",
    "v",
    "valid",
    "vigor",
    "voidweaver",
    "webbed",
    "wounded",
    "yowlp",
];

/// The buff each verb grants, for `buffN` (`bigshot.lic:3228-3241`).
/// `coupdegrace`'s is a pattern, `Empowered (+N)`, and is not here: a
/// pattern is not a name.
const BUFF_OF: &[(&str, &str)] = &[
    ("barrage", "Enh. Dexterity (+10)"),
    ("bearhug", "Enh. Strength (+10)"),
    ("flurry", "Slashing Strikes"),
    ("fury", "Enh. Constitution (+10)"),
    ("garrote", "Enh. Agility (+10)"),
    ("kweed", "Tangleweed Vigor"),
    ("pummel", "Concussive Blows"),
    ("shout", "Empowered (+20)"),
    ("thrash", "Forceful Blows"),
    ("weed", "Tangleweed Vigor"),
    ("yowlp", "Yertie's Yowlp"),
];

/// The catch-all patterns bigshot profiles use for "any creature".
const ANY: &[&str] = &["(?:.+?)", "(?:.+)", "(?:.*)", ".+?", ".+", ".*"];

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

    fn rest(&mut self) {
        self.profile.rest.fried = number(&self.take("fried"));
        self.profile.rest.overkill = number(&self.take("overkill")).unwrap_or(0);
        self.profile.rest.encumbered = number(&self.take("encumbered"));
        self.profile.rest.mana_below = number(&self.take("oom"));
        self.profile.rest.until.experience = number(&self.take("rest_till_exp"));
        self.profile.rest.until.mana = number(&self.take("rest_till_mana"));
        self.profile.rest.until.spirit = number(&self.take("rest_till_spirit"));
        self.profile.rest.until.stamina = number(&self.take("rest_till_percentstamina"));
        self.profile.rest.commands = self.commands("resting_commands");
        self.rest_when();
    }

    /// `wounded_eval`: a Ruby expression of terms joined by `||`. Each term
    /// that is one of the known shapes becomes a threshold; each that is
    /// not is named, and Hydra will not rest on it.
    fn rest_when(&mut self) {
        let text = self.take("wounded_eval");
        for term in text.split("||").map(str::trim).filter(|t| !t.is_empty()) {
            let bare = unparenthesised(term);
            if bare.contains("&&") {
                self.note(format!(
                    "rest.when: `{bare}` joins conditions with &&, which a profile cannot say; Hydra will not rest on it"
                ));
                continue;
            }
            match bare {
                "bleeding?" | "checkbleeding" => self.profile.rest.when.bleeding = true,
                "!Injured.able_to_use_ranged?" => self.profile.rest.when.cannot_use_ranged = true,
                "!Injured.able_to_cast?" => self.profile.rest.when.cannot_cast = true,
                _ => match health_at_most(bare) {
                    Some(percent) => self.profile.rest.when.health_at_most = Some(percent),
                    None => self.note(format!(
                        "rest.when: `{bare}` is not a shape Hydra reads; Hydra will not rest on it"
                    )),
                },
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
        self.profile.wander.ignore_disks = flag(&self.take("ignore_disks"));
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
                Ok(guard) => guards.push(guard),
                Err(why) => held.push(why),
            }
        }
        if !held.is_empty() {
            return Step::held(entry, &held.join("; "));
        }
        let line = format!("{verb} ({})", guards.join(" "));
        Step::parse(&line).unwrap_or_else(|why| Step::held(entry, &why))
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

/// One bigshot guard word as Hydra's, or why it cannot be.
fn translate(verb: &str, token: &str) -> Result<String, String> {
    let (bang, word) = token
        .strip_prefix('!')
        .map_or(("", token), |word| ("!", word));
    let flipped = if bang.is_empty() { "!" } else { "" };
    let lower = word.to_ascii_lowercase();
    if lower == "hidden" {
        return Ok(format!("{bang}hidden"));
    }
    if lower == "frozen" {
        return Ok(format!("{flipped}immobilized"));
    }
    if let Some(n) = numbered(&lower, "thp") {
        return Ok(format!("{bang}thp {n}"));
    }
    if let Some(n) = numbered(&lower, "empowered") {
        return Ok(format!("{bang}empowered_below {n}"));
    }
    if let Some(n) = numbered(&lower, "buff") {
        if !bang.is_empty() {
            return Err(format!("`{token}`: bigshot has no negated buff guard"));
        }
        let first = verb
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        return match BUFF_OF.iter().find(|(v, _)| *v == first) {
            Some((_, buff)) => Ok(format!("expiring \"{buff}\" {n}")),
            None if first == "coupdegrace" => Err(format!(
                "`{token}` on `{verb}`: its buff is a pattern, `Empowered (+N)`, and `expiring` takes a name"
            )),
            None => Err(format!(
                "`{token}` on `{verb}`: bigshot knows no buff for that verb, so the guard never held in bigshot either"
            )),
        };
    }
    if bigshot_knows(word) {
        return Err(format!("guard `{token}` is not built yet (`plan/33`)"));
    }
    Err(format!(
        "`{token}` is not a guard bigshot knows either; it never held this step back"
    ))
}

/// `thp20` with `thp` is 20.
fn numbered(lower: &str, word: &str) -> Option<u32> {
    lower.strip_prefix(word)?.parse().ok()
}

/// Whether bigshot's `:3197` alternation accepts the word: as written, or
/// with its number or quoted text removed. Case does not matter (`/i`).
fn bigshot_knows(word: &str) -> bool {
    let bare = word.split('"').next().unwrap_or(word);
    let known = |w: &str| BIGSHOT_WORDS.iter().any(|k| k.eq_ignore_ascii_case(w));
    if known(bare) {
        return true;
    }
    let stem = bare.trim_end_matches(|c: char| c.is_ascii_digit());
    !stem.is_empty() && known(stem)
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
