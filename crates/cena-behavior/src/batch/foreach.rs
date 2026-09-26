//! `;foreach`: what foreach.lic's command line means
//! (`reference/scripts/scripts/foreach.lic:1546-1770`).
//!
//! ```text
//! ;foreach [OPTIONS] [[ATTRIBUTE=]VALUE] in|on|under|behind <TARGETS>[; command; command...]
//! ```
//!
//! Parsed here, pure: the options, the filter, the targets and the commands,
//! with foreach.lic's rewrites of the commands already made (the verbs it
//! completes, the implicit `get item` before a first `sell` and the implicit
//! `return` after a lone `appraise`). Which items it comes to is
//! [`pick`](super::pick)'s, and one item's lines are [`build`](super::build)'s.
//!
//! **What is not built is refused by name**, never ignored: a `marked` that
//! was dropped would run the commands on every item, marked or not, which is
//! worse than being told. `plan/30` §7 (M6e) tables every feature of
//! foreach.lic, built or not, and why.

use regex::Regex;

/// What `;foreach` was asked to do.
#[derive(Clone, Debug)]
pub enum Command {
    /// Look, pick, and run the commands on each item (or list them).
    Run(Foreach),
    /// `;foreach stop`: stop the one running.
    Stop,
    /// Bare `;foreach`, or `;foreach help`: say how.
    Help,
}

/// One `;foreach`, parsed.
#[derive(Clone, Debug)]
pub struct Foreach {
    /// `unique`, `first`, `after`, `sorted`, `reversed`.
    pub options: Options,
    /// Which items count.
    pub filter: Filter,
    /// Where in each target to look.
    pub position: Position,
    /// Where to look, in order.
    pub targets: Vec<Target>,
    /// The commands, rewritten as foreach.lic rewrites them, before `item`
    /// and the rest are filled in. Empty lists the items instead
    /// (`:2132-2149`).
    pub commands: Vec<String>,
}

/// The options that come before the filter (`:1553-1630`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// `unique`: only the first item of each full name.
    pub unique: bool,
    /// `first N`, or a bare `N`: only the first N.
    pub first: Option<usize>,
    /// `after N` or `skip N`: skip the first N.
    pub after: Option<usize>,
    /// `sorted` or `nsorted`.
    pub sort: Sort,
    /// `reversed`: backwards, after any sort.
    pub reversed: bool,
}

/// How the items of each target are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// As the game listed them.
    #[default]
    Listed,
    /// `sorted`: by full name, leaving out `a`, `an`, `some`, `the`.
    Name,
    /// `nsorted`: by noun, then as `sorted`.
    Noun,
}

/// Which items count (`build_filter`, `:1436-1486`).
#[derive(Clone, Debug)]
pub enum Filter {
    /// No value, or `all`, `any`, `everything`.
    All,
    /// `type=none` or `sellable=none`: nothing in that table matches it.
    Unclassified(Attr),
    /// The attribute matches the pattern, in any case.
    Pattern {
        /// What is matched.
        attr: Attr,
        /// The value's `*` wildcards and `,` alternatives, or a `/pattern/`.
        pattern: Regex,
    },
}

/// What a filter reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attr {
    /// `type`, `t`: one of the item's `gameobj` types. The default.
    Type,
    /// `sellable`, `s`: one of the shops that buy it.
    Sellable,
    /// `name`, `m`: the link's text.
    Name,
    /// `fullname`, `f`, and `quick`, `q`: with the words either side.
    FullName,
    /// `noun`, `n`.
    Noun,
}

/// Where in a target to look: the word `look` is sent with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    /// `in`.
    In,
    /// `on`.
    On,
    /// `under`.
    Under,
    /// `behind`.
    Behind,
}

impl Position {
    /// The word, as `look` takes it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::On => "on",
            Self::Under => "under",
            Self::Behind => "behind",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        match word.to_ascii_lowercase().as_str() {
            "in" => Some(Self::In),
            "on" => Some(Self::On),
            "under" => Some(Self::Under),
            "behind" => Some(Self::Behind),
            _ => None,
        }
    }
}

/// One place to look.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A container by name or `#id`, looked in. `optional` is a trailing
    /// `?`: skipped, not stopped for, when closed or not there (`:1872`).
    Named {
        /// As typed, without the `?`.
        name: String,
        /// Whether it may be missing.
        optional: bool,
    },
    /// `floor`, `ground`, `room`: the items on the ground, not their
    /// contents.
    Ground,
    /// `loot`: what is inside each item on the ground.
    Loot,
}

/// How `;foreach` is used.
pub const USAGE: &[&str] = &[
    "foreach [options] [[attr=]value] in|on|under|behind <target>[,<target>...][; command; command...]",
    "  foreach gem in cloak; get item; appraise item; put item in container",
    "  foreach name=*quartz orb in backpack; get item; put item in sack",
    "  foreach gem in backpack; get item; ;sc 704 item; put item in container",
    "  foreach box in red sack?, cloak           (no commands: list what matches)",
    "attr: type (default), sellable, noun, name, fullname, quick (t s n m f q); * is a wildcard,",
    "      a,b matches either, /pattern/ is a regular expression, type=none is untyped",
    "options: unique, first N (or N), after N (or skip N), sorted, nsorted, reversed",
    "targets: a container, floor (ground, room), loot; a trailing ? skips it when missing",
    "in commands: item (#id), noun, name, container (#id); the separator may be ; / or |",
    "conveniences: move [to] <where>, return, waitrt, waitcastrt, sleep N, echo X,",
    "      waitfor X, waitre /X/, waitmana N (hp, spirit, stamina), unmark, ;<hydra command>",
    "foreach stop   stop it",
];

/// Parse a command line, without its symbol. `None` when the word is not
/// `foreach`; `Some(Err)` with the reason when it is and cannot be run.
///
/// `symbol` marks a command as a Hydra command; it is also a separator
/// when it is `;`, and foreach.lic's rule for that case is kept
/// (`:1673-1680`): `; ;sc 401 item` is the Hydra command `sc 401 item`.
#[must_use]
pub fn parse(line: &str, symbol: char) -> Option<Result<Command, String>> {
    let line = line.trim();
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    if !word.eq_ignore_ascii_case("foreach") {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() || rest.eq_ignore_ascii_case("help") {
        return Some(Ok(Command::Help));
    }
    if rest.eq_ignore_ascii_case("stop") {
        return Some(Ok(Command::Stop));
    }
    Some(foreach(rest, symbol).map(Command::Run))
}

/// The line's three parts (`:1546`), then each read.
fn foreach(rest: &str, symbol: char) -> Result<Foreach, String> {
    let shape =
        Regex::new(r"(?i)^\s*(.*?\b(?:in|on|under|behind)\s+.+?)\s*(?:([;/|])\s*(.*?)\s*)?$")
            .map_err(|e| e.to_string())?;
    let usage = || format!("Foreach: `{rest}` is not a foreach. Usage: {}", USAGE[0]);
    let parts = shape.captures(rest).ok_or_else(usage)?;
    let head = parts.get(1).map_or("", |m| m.as_str());
    let (options, head) = options(head)?;
    // `FILTER_PATTERN` (`:862`), read without regard to case, as the line
    // itself is (`:1546`).
    let filter_shape = Regex::new(
        r"(?i)^(?:(in|on|under|behind)|(?:(?:(.+)=)?(.+?)\s+(in|on|under|behind)))\s+(.+)$",
    )
    .map_err(|e| e.to_string())?;
    let found = filter_shape.captures(head).ok_or_else(usage)?;
    let position = found
        .get(1)
        .or_else(|| found.get(4))
        .and_then(|m| Position::parse(m.as_str()))
        .ok_or_else(usage)?;
    let targets_text = found.get(5).map_or("", |m| m.as_str());
    let filter = filter(
        found.get(2).map(|m| m.as_str()),
        found.get(3).map(|m| m.as_str()),
        targets_text,
    )?;
    let targets = targets(targets_text)?;
    let commands = match (parts.get(2), parts.get(3)) {
        (Some(separator), Some(text)) => {
            let separator = separator.as_str().chars().next().unwrap_or(';');
            commands(text.as_str(), separator, symbol)?
        }
        _ => Vec::new(),
    };
    Ok(Foreach {
        options,
        filter,
        position,
        targets,
        commands,
    })
}

/// The options before the filter, and what is left (`:1553-1630`). Each
/// may be given once.
fn options(head: &str) -> Result<(Options, &str), String> {
    let mut options = Options::default();
    let mut rest = head.trim_start();
    let twice = |word: &str| Err(format!("Foreach: '{word}' was given more than once."));
    while let Some((word, after)) = rest.split_once(char::is_whitespace) {
        let after = after.trim_start();
        if after.is_empty() {
            break;
        }
        let lower = word.to_ascii_lowercase();
        let number = |text: &str| text.parse::<usize>().ok();
        match lower.as_str() {
            "unique" => {
                if options.unique {
                    return twice(word);
                }
                options.unique = true;
            }
            "reverse" | "reversed" => {
                if options.reversed {
                    return twice(word);
                }
                options.reversed = true;
            }
            "sort" | "sorted" | "nsort" | "nsorted" | "nounsort" | "nounsorted" => {
                if options.sort != Sort::Listed {
                    return twice("sorted");
                }
                options.sort = if lower.starts_with('n') {
                    Sort::Noun
                } else {
                    Sort::Name
                };
            }
            "mark" | "marked" | "unmark" | "unmarked" | "register" | "registered"
            | "unregister" | "unregistered" => {
                return Err(format!(
                    "Foreach: '{word}' is not built: it reads the marked and registered notes \
                     in `inventory full`, which Hydra does not read yet."
                ));
            }
            "first" | "after" | "skip" => {
                let Some((count, beyond)) = after.split_once(char::is_whitespace) else {
                    break;
                };
                let Some(n) = number(count) else {
                    break;
                };
                let slot = if lower == "first" {
                    &mut options.first
                } else {
                    &mut options.after
                };
                if slot.is_some() {
                    return twice(if lower == "first" { "first" } else { "skip" });
                }
                *slot = Some(n);
                rest = beyond.trim_start();
                continue;
            }
            _ => match number(word) {
                // A bare number is `first` (`:1553`).
                Some(n) => {
                    if options.first.is_some() {
                        return twice("first");
                    }
                    options.first = Some(n);
                }
                None => break,
            },
        }
        rest = after;
    }
    Ok((options, rest))
}

/// The filter (`:1638-1656`, `build_filter` at `:1436-1486`).
fn filter(attr: Option<&str>, value: Option<&str>, targets: &str) -> Result<Filter, String> {
    let named = attr.map_or_else(|| "type".to_owned(), str::to_ascii_lowercase);
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(Filter::All);
    };
    // `:1645`. Lich's test is unanchored, so `small` and `wall` meant
    // everything too; here only the three words do.
    if named == "type"
        && ["all", "any", "everything"]
            .iter()
            .any(|w| value.eq_ignore_ascii_case(w))
    {
        return Ok(Filter::All);
    }
    let (attr, quick) = match named.as_str() {
        "type" | "t" => (Attr::Type, false),
        "sellable" | "s" => (Attr::Sellable, false),
        "name" | "m" => (Attr::Name, false),
        "fullname" | "f" => (Attr::FullName, false),
        "quick" | "q" => (Attr::FullName, true),
        "noun" | "n" => (Attr::Noun, false),
        _ => {
            return Err(format!(
                "Foreach: '{named}' is not an attribute: type, sellable, noun, name, fullname or quick."
            ));
        }
    };
    let untyped = matches!(attr, Attr::Type | Attr::Sellable)
        && (value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("unknown"));
    if untyped {
        return Ok(Filter::Unclassified(attr));
    }
    let pattern = pattern(value, quick)?;
    let kind = match attr {
        Attr::Type => Some(cena_session::gameobj::Classification::Type),
        Attr::Sellable => Some(cena_session::gameobj::Classification::Sellable),
        _ => None,
    };
    if let Some(kind) = kind
        && !cena_session::gameobj::categories(kind)
            .iter()
            .any(|category| pattern.is_match(category))
    {
        let instead = if value.contains(char::is_whitespace) {
            "name"
        } else {
            "noun"
        };
        return Err(format!(
            "Foreach: no item {} matches '{value}'. Did you mean `foreach {instead}={value} in {targets}`? \
             `foreach help` lists the rest.",
            if kind == cena_session::gameobj::Classification::Type {
                "type"
            } else {
                "sellable"
            }
        ));
    }
    Ok(Filter::Pattern { attr, pattern })
}

/// A value as a pattern: `/pattern/`, or `*` wildcards and `,` alternatives,
/// whole (`quick` matches anywhere). Always without regard to case.
fn pattern(value: &str, quick: bool) -> Result<Regex, String> {
    if let Some(inner) = value.strip_prefix('/')
        && let Some((body, flags)) = inner.rsplit_once('/')
        && flags.chars().all(|c| c.is_ascii_alphabetic())
    {
        return Regex::new(&format!("(?i){body}"))
            .map_err(|e| format!("Foreach: /{body}/ is not a pattern: {e}"));
    }
    let alternatives: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .map(|piece| format!("(?:{})", regex::escape(piece).replace(r"\*", ".*")))
        .collect();
    let joined = alternatives.join("|");
    let source = if quick {
        format!("(?i)(?:{joined})")
    } else {
        format!("(?i)^(?:{joined})$")
    };
    Regex::new(&source).map_err(|e| format!("Foreach: '{value}' is not a pattern: {e}"))
}

/// The targets, each once (`:1771-1886`).
fn targets(text: &str) -> Result<Vec<Target>, String> {
    let mut out: Vec<Target> = Vec::new();
    for piece in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (name, optional) = match piece.strip_suffix('?') {
            Some(name) => (name.trim(), true),
            None => (piece, false),
        };
        let lower = name.to_ascii_lowercase();
        if let Some(why) = not_built_target(&lower) {
            return Err(format!("Foreach: `{name}` is not built: {why}"));
        }
        let target = match lower.as_str() {
            "floor" | "ground" | "room" => Target::Ground,
            "loot" => Target::Loot,
            _ => Target::Named {
                name: name.to_owned(),
                optional,
            },
        };
        if !out.contains(&target) {
            out.push(target);
        }
    }
    if out.is_empty() {
        return Err("Foreach: in what? Name a container.".to_owned());
    }
    Ok(out)
}

/// Why a target foreach.lic knows is not built, if it is one.
fn not_built_target(lower: &str) -> Option<&'static str> {
    match lower {
        "inv" | "inventory" => Some(
            "it reads `inventory full`, which Hydra does not read yet. Name the containers \
             instead: `in backpack, cloak`.",
        ),
        "fastinv" | "fastinventory" | "qinv" | "qinventory" | "worn" => Some(
            "it needs the list of what is worn, which Hydra does not keep yet. Name the \
             containers instead: `in backpack, cloak`.",
        ),
        "locker" => Some(
            "the locker (its manifest, and opening and closing it) is not built. Name the \
             containers in it instead.",
        ),
        "desc" => Some("the items in the room's description are not kept apart yet."),
        "prev" | "previous" | "last" => Some("the last run's items are not kept."),
        _ => None,
    }
}

/// The verbs foreach.lic completes from their first letters (`:1693`), as
/// `typed|rest`: `sel` is `sell`, `l` is `look`.
const COMPLETED: &[&str] = &[
    "dr|op",
    "plac|e",
    "sel|l",
    "ap|praise",
    "reg|ister",
    "ge|t",
    "tak|e",
    "r|ead",
    "l|ook",
    "ana|lyze",
    "ins|pect",
    "loc|ker",
];

/// Commands that are a verb alone and mean it on the item (`:1725`).
const ON_THE_ITEM: &[&str] = &[
    "drop", "place", "sell", "appraise", "stash", "register", "mark", "unmark", "get", "take",
    "read", "look", "analyze", "inspect", "trash",
];

/// A first command on the item that needs it in hand first (`:1742`).
const FETCHED: &[&str] = &[
    "place", "sell", "trash", "appraise", "register", "mark", "unmark",
];

/// Of those, the ones that leave it in hand, so a lone one puts it back.
const RETURNED: &[&str] = &["appraise", "register", "mark", "unmark"];

/// The commands, split and rewritten (`:1659-1768`).
fn commands(text: &str, separator: char, symbol: char) -> Result<Vec<String>, String> {
    let mut pieces: Vec<&str> = text.split(separator).map(str::trim).collect();
    // Ruby's `split` drops trailing empties, and only those.
    while pieces.last().is_some_and(|piece| piece.is_empty()) {
        pieces.pop();
    }
    let count = pieces.iter().filter(|piece| !piece.is_empty()).count();
    let mut out = Vec::new();
    let mut next_is_hydra = false;
    for (index, piece) in pieces.iter().enumerate() {
        // `!` asked Lich not to wait (`:1669`). Every line here waits for its
        // prompt, so it is accepted and has no effect.
        let command = piece.strip_prefix('!').unwrap_or(piece).trim();
        if command.is_empty() {
            // Split on the symbol, a Hydra command leaves an empty piece
            // before it (`:1673-1680`).
            next_is_hydra = separator == symbol;
            continue;
        }
        if std::mem::take(&mut next_is_hydra) || command.starts_with(symbol) {
            let bare = command.trim_start_matches(symbol).trim();
            if bare
                .split_whitespace()
                .next()
                .is_some_and(|w| w.eq_ignore_ascii_case("foreach"))
            {
                return Err(format!(
                    "Foreach: `{symbol}{bare}` would run a foreach inside this one, and one runs at a time."
                ));
            }
            out.push(format!("{symbol}{bare}"));
            continue;
        }
        let command = rewritten(command)?;
        let lower = command.to_ascii_lowercase();
        if index == 0 {
            if lower == "drop item" {
                out.push("_drag item drop".to_owned());
                continue;
            }
            if let Some(verb) = FETCHED.iter().find(|verb| lower == format!("{verb} item")) {
                out.push("get item".to_owned());
                out.push(command);
                if count == 1 && RETURNED.contains(verb) {
                    out.push("return".to_owned());
                }
                continue;
            }
        }
        out.push(command);
    }
    Ok(out)
}

/// One command with its verb completed and its implicit `item` added, or why
/// it is not built.
fn rewritten(command: &str) -> Result<String, String> {
    // `/^(\w+)(.*)$/`: the first word, as word characters.
    let split = command
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(command.len());
    let (typed, rest) = command.split_at(split);
    let typed = typed.to_ascii_lowercase();
    let not_built = |why: &str| Err(format!("Foreach: `{command}` is not built: {why}"));
    if typed == "giveitem" {
        return not_built("send `give item to <name>`, then `waitfor <name> has accepted`.");
    }
    let mut command = command.to_owned();
    if !typed.is_empty()
        && let Some(verb) = COMPLETED.iter().find(|verb| {
            let (least, more) = verb.split_once('|').unwrap_or((verb, ""));
            typed.len() >= least.len() && format!("{least}{more}").starts_with(&typed)
        })
    {
        command = format!("{}{rest}", verb.replace('|', ""));
    }
    let lower = command.to_ascii_lowercase();
    if lower == "locker" {
        return not_built("the locker is not built.");
    }
    if lower == "pause" {
        return not_built(
            "Hydra has no pause. `foreach stop`, then run it again with `after <n>`.",
        );
    }
    if ON_THE_ITEM.contains(&lower.as_str()) {
        command.push_str(" item");
    }
    if command.to_ascii_lowercase().starts_with("stash ") {
        return not_built("it puts into Lich's lootsack settings, which Hydra does not have.");
    }
    Ok(command)
}
