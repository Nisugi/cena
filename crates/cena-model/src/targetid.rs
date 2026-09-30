//! Creature tags: `targetid.lic`'s idea, in Hydra (the author, 2026-09-30:
//! *"it adds a unique identifier to each creature and shows it after their
//! name. It's a way to distinguish them apart since you can't see their id
//! and their id is just a string of numbers"*).
//!
//! **A tag is handed out, not worked out from the id.** The script wrote an
//! id's low bits as its tag, and two creatures in a room could start alike.
//! The author: *"the first character being unique is more important, so you
//! don't have to base the tag off their id, just assign it to their id and
//! make sure it's unique with a unique starting character. creatures that
//! get cleaned up release their tag."* So each creature is given a [`Mark`]
//! when the registry first sees it: a character no other creature in the
//! room has, and random ones, kept in its instance and gone with it.
//!
//! **What a tag shows is the player's to choose** ([`Style`]): three slots,
//! each the unique character, a random one, a separator of the player's
//! (`-`, `/`), or nothing; and the creature's health as a percent, three
//! digits, before or after, or not at all. `A7-042`, `042/A`, `A7K`. The
//! letters come first, then the digits: *"we're hard pressed to have more
//! than 26 creatures in a room and if we do well there's another 10 numbers
//! we can lean on."*
//!
//! **A tag names its creature back** ([`resolve`]), by its characters, a
//! separator typed or not, the health never:
//!
//! - **The script's own commands**, as its players type them (*"I didn't
//!   realize the t commands were part of the script. That can be added
//!   back."*): `tkill A7`, or its shortest, `tk A`, its first character or
//!   more enough (`_build_targets`). A tag the room has not got says so, as
//!   the script says it; one two creatures start with sends nothing.
//! - **The game's own verbs**, a Hydra nicety: `kill A7K` sent as
//!   `kill #<its id>`, the whole tag only, and only after one of the
//!   script's verbs as the command's last word: `look at bag` is never
//!   taken for a tag.

use cena_protocol::frame::LinkKind;
use cena_protocol::runs::{Run, Runs};

use crate::line::Line;

/// The characters a tag is made of: the letters first, then the digits.
const ALPHABET: &[u8; 36] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// The characters handed to one creature: the one no other creature in its
/// room had when it was first seen, and a random one for each slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    /// Unique in the room it was first seen in.
    pub unique: char,
    /// Random, one for each of the three slots.
    pub random: [char; 3],
}

/// What one of a tag's three slots shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// The creature's unique character.
    Unique,
    /// A random character of its own.
    Random,
    /// This character, the same for every creature: `-`, `/`.
    Separator(char),
    /// Nothing.
    None,
}

impl Slot {
    /// As the settings write it: `unique`, `random`, `none`, or the
    /// separator itself.
    #[must_use]
    pub fn word(self) -> String {
        match self {
            Self::Unique => "unique".to_owned(),
            Self::Random => "random".to_owned(),
            Self::None => "none".to_owned(),
            Self::Separator(c) => c.to_string(),
        }
    }

    /// From the settings' `word`: a separator is one character that is not
    /// a letter or a digit, so it is never taken for part of a tag.
    #[must_use]
    pub fn of(word: &str) -> Option<Self> {
        let word = word.trim();
        match word.to_ascii_lowercase().as_str() {
            "unique" => Some(Self::Unique),
            "random" => Some(Self::Random),
            "none" | "" => Some(Self::None),
            _ => {
                let mut chars = word.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if !c.is_alphanumeric() && !c.is_whitespace() => {
                        Some(Self::Separator(c))
                    }
                    _ => None,
                }
            }
        }
    }
}

/// Where a tag shows the creature's health.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Health {
    /// Not at all.
    #[default]
    Off,
    /// Before its characters: `042/A`.
    Front,
    /// After them: `A7-042`.
    Back,
}

impl Health {
    /// As the settings write it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Front => "front",
            Self::Back => "back",
        }
    }

    /// From the settings' `word`.
    #[must_use]
    pub fn of(word: &str) -> Option<Self> {
        match word.trim().to_ascii_lowercase().as_str() {
            "off" | "none" => Some(Self::Off),
            "front" => Some(Self::Front),
            "back" => Some(Self::Back),
            _ => None,
        }
    }
}

/// How a tag looks: its three slots and its health.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    /// The three slots, in order.
    pub slots: [Slot; 3],
    /// Where the health goes.
    pub health: Health,
}

impl Default for Style {
    /// Three characters, the first unique: `A7K`.
    fn default() -> Self {
        Self {
            slots: [Slot::Unique, Slot::Random, Slot::Random],
            health: Health::Off,
        }
    }
}

/// A mark for a new creature, `id`, in a room whose other creatures'
/// unique characters are `taken`: the first letter free, a digit when the
/// letters are gone, and when all are (never yet), the first again. Its
/// random characters are drawn from the id, so the same creature seen
/// twice from nothing gets the same ones.
#[must_use]
pub fn assign(id: i64, taken: impl IntoIterator<Item = char>) -> Mark {
    let taken: Vec<char> = taken.into_iter().collect();
    let unique = ALPHABET
        .iter()
        .map(|b| char::from(*b))
        .find(|c| !taken.contains(c))
        .unwrap_or('A');
    let mut bits = id.unsigned_abs().wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut random = ['A'; 3];
    for slot in &mut random {
        *slot = char::from(ALPHABET[usize::try_from(bits % 36).unwrap_or(0)]);
        bits /= 36;
    }
    Mark { unique, random }
}

/// A creature's mark and its health, a whole percent 0 to 100, as a tag
/// wants them; `None` before the registry has handed it a mark.
#[must_use]
pub fn of(creature: &crate::state::creatures::CreatureInstance) -> Option<(Mark, u32)> {
    // Clamped first, so the cast neither wraps nor truncates past the range.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=100 before the cast"
    )]
    let health = creature.hp_percent().round().clamp(0.0, 100.0) as u32;
    Some((creature.mark()?, health))
}

/// The tag `style` makes of `mark`, with `health` a percent when it shows.
#[must_use]
pub fn tag(mark: Mark, style: Style, health: Option<u32>) -> String {
    let characters: String = style
        .slots
        .iter()
        .enumerate()
        .filter_map(|(at, slot)| match slot {
            Slot::Unique => Some(mark.unique),
            Slot::Random => Some(mark.random[at]),
            Slot::Separator(c) => Some(*c),
            Slot::None => None,
        })
        .collect();
    let health = health.map(|percent| format!("{:03}", percent.min(100)));
    match (style.health, health) {
        (Health::Front, Some(health)) => format!("{health}{characters}"),
        (Health::Back, Some(health)) => format!("{characters}{health}"),
        _ => characters,
    }
}

/// `tag`'s characters as a player types them: letters and digits, upper
/// case, the separators and the health left out.
fn typed(mark: Mark, style: Style) -> String {
    tag(
        mark,
        Style {
            health: Health::Off,
            ..style
        },
        None,
    )
    .chars()
    .filter(char::is_ascii_alphanumeric)
    .collect()
}

/// A typed tag as its characters: `a7-` as `A7`.
fn key(word: &str) -> String {
    word.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// `line` with each creature's tag after its name, as `style` makes it,
/// and a trigger's looks moved along with the words they were on; `None`
/// when it names none. `creature` answers an id with its mark and its
/// health, for a creature the registry has.
#[must_use]
pub fn tagged(
    line: &Line,
    style: Style,
    creature: impl Fn(i64) -> Option<(Mark, u32)>,
) -> Option<Line> {
    let runs = &line.runs.runs;
    let mut out = Vec::with_capacity(runs.len() + 1);
    let mut paint = line.paint.clone();
    let mut at = 0usize;
    let mut added = false;
    for (index, run) in runs.iter().enumerate() {
        at += run.text.len();
        out.push(run.clone());
        let Some(id) = named(run) else {
            continue;
        };
        // The last run of the link: a name split by a style is one name.
        if runs
            .get(index + 1)
            .is_some_and(|next| named(next) == Some(id))
        {
            continue;
        }
        let Some((mark, health)) = creature(id) else {
            continue;
        };
        let text = format!(" ({})", tag(mark, style, Some(health)));
        for span in &mut paint {
            if span.span.start >= at {
                span.span.start += text.len();
            }
            if span.span.end > at {
                span.span.end += text.len();
            }
        }
        at += text.len();
        out.push(Run {
            text,
            style: run.style.clone(),
            link: None,
            inner_link: None,
        });
        added = true;
    }
    added.then(|| Line {
        stream: line.stream.clone(),
        runs: Runs { runs: out },
        paint,
    })
}

/// The object a run names: a bold link, the creatures' mark.
fn named(run: &Run) -> Option<i64> {
    if run.style.bold_depth == 0 {
        return None;
    }
    let Some(LinkKind::Exist { id, .. }) = run.link.as_ref().map(|link| &link.kind) else {
        return None;
    };
    id.parse().ok()
}

/// The verbs a whole tag may follow: `targetid`'s own, less their `t`.
const VERBS: [&str; 11] = [
    "target", "attack", "ambush", "kill", "cast", "cman", "punch", "grapple", "kick", "jab",
    "mstrike",
];

/// The script's own commands: each, the fewest letters of it a player may
/// type, and the game's verb it sends (`targetid.lic`, `_build_targets`).
const T_VERBS: [(&str, usize, &str); 11] = [
    ("ttarget", 2, "target"),
    ("tattack", 3, "attack"),
    ("tambush", 3, "ambush"),
    ("tkill", 2, "kill"),
    ("tcast", 3, "cast"),
    ("tcman", 3, "cman"),
    ("tpunch", 2, "punch"),
    ("tgrapple", 2, "grapple"),
    ("tkick", 4, "kick"),
    ("tjab", 2, "jab"),
    ("tmstrike", 2, "mstrike"),
];

/// `line`, a command the player typed, with a creature's tag in it as the
/// game's own target, `#<id>`: one of the script's commands, `tk A7` or
/// `tk A`, sending `kill #123456`; or one of the game's verbs with the whole
/// tag, `kill A7K`. `creatures` are those in the room, each with its mark,
/// and `style` makes their tags. `None` when it has no tag to take;
/// `Some(Err)` saying why nothing is to be sent.
#[must_use]
pub fn resolve(
    line: &str,
    creatures: impl IntoIterator<Item = (i64, Mark)>,
    style: Style,
) -> Option<Result<String, String>> {
    let trimmed = line.trim();
    let verb = trimmed.split_whitespace().next()?;
    let creatures = creatures
        .into_iter()
        .map(|(id, mark)| (id, typed(mark, style)));
    if let Some(game) = t_verb(verb) {
        let rest = trimmed[verb.len()..].trim();
        return Some(t_command(game, rest, creatures));
    }
    if !VERBS.iter().any(|known| verb.eq_ignore_ascii_case(known)) {
        return None;
    }
    let (before, last) = trimmed.rsplit_once(char::is_whitespace)?;
    let wanted = key(last);
    if wanted.is_empty() {
        return None;
    }
    let mut matching = creatures.filter(|(_, typed)| *typed == wanted);
    let (id, _) = matching.next()?;
    if matching.next().is_some() {
        return Some(Err(format!(
            "The tag {wanted} is more than one creature's here; nothing was sent."
        )));
    }
    Some(Ok(format!("{} #{id}", before.trim_end())))
}

/// The game's verb one of the script's commands, `word`, sends: `word` is
/// the command, or as much of it as its fewest letters or more.
fn t_verb(word: &str) -> Option<&'static str> {
    let word = word.to_ascii_lowercase();
    T_VERBS
        .iter()
        .find(|(command, fewest, _)| word.len() >= *fewest && command.starts_with(&word))
        .map(|(_, _, game)| *game)
}

/// One of the script's commands, sending the game's `verb` at the creature
/// whose tag starts with the last word of `rest`, a maneuver's name before
/// it for `cman` and `mstrike`: `cman sweep #123456`.
fn t_command(
    verb: &str,
    rest: &str,
    creatures: impl Iterator<Item = (i64, String)>,
) -> Result<String, String> {
    let usage = || {
        let shown = verb.to_ascii_uppercase();
        if verb == "cman" {
            format!("Usage: T{shown} maneuver targetID")
        } else {
            format!("Usage: T{shown} targetID")
        }
    };
    let (before, last) = match rest.rsplit_once(char::is_whitespace) {
        Some((before, last)) if verb == "cman" || verb == "mstrike" => (before.trim(), last),
        Some(_) => return Err(usage()),
        None if verb == "cman" || rest.is_empty() => return Err(usage()),
        None => ("", rest),
    };
    let wanted = key(last);
    let not_found = || format!("Target with tag '{}' not found!", last.to_ascii_uppercase());
    if wanted.is_empty() {
        return Err(not_found());
    }
    let mut matching = creatures.filter(|(_, typed)| typed.starts_with(&wanted));
    let (id, _) = matching.next().ok_or_else(not_found)?;
    if matching.next().is_some() {
        return Err(format!(
            "More than one creature's tag here starts {wanted}; nothing was sent."
        ));
    }
    let verb = if before.is_empty() {
        verb.to_owned()
    } else {
        format!("{verb} {before}")
    };
    Ok(format!("{verb} #{id}"))
}

#[cfg(test)]
mod tests {
    use super::{Health, Mark, Slot, Style, assign, resolve, tag, tagged};
    use crate::line::Line;
    use cena_protocol::frame::{Link, LinkKind, Style as Markup};
    use cena_protocol::runs::{Run, Runs};

    const MARK: Mark = Mark {
        unique: 'A',
        random: ['K', '7', 'Q'],
    };

    /// The unique character is the first letter no other creature here
    /// has, then the digits; the random ones come from the id, the same
    /// each time.
    #[test]
    fn a_mark_is_handed_out_letters_first() {
        assert_eq!(assign(5, []).unique, 'A');
        assert_eq!(assign(5, ['A', 'B']).unique, 'C');
        let letters: Vec<char> = ('A'..='Z').collect();
        assert_eq!(assign(5, letters).unique, '0');
        assert_eq!(assign(99, []).random, assign(99, []).random);
    }

    /// The author's shapes: the slots in order, a separator as given,
    /// nothing for none, the health three digits before or after.
    #[test]
    fn a_tag_is_its_slots_and_its_health() {
        let style = |slots, health| Style { slots, health };
        assert_eq!(tag(MARK, Style::default(), Some(42)), "A7Q");
        assert_eq!(
            tag(
                MARK,
                style(
                    [Slot::Unique, Slot::Random, Slot::Separator('-')],
                    Health::Back
                ),
                Some(42)
            ),
            "A7-042"
        );
        assert_eq!(
            tag(
                MARK,
                style(
                    [Slot::Separator('/'), Slot::Unique, Slot::None],
                    Health::Front
                ),
                Some(100)
            ),
            "100/A"
        );
        assert_eq!(Slot::of("-"), Some(Slot::Separator('-')));
        assert_eq!(Slot::of("unique"), Some(Slot::Unique));
        assert_eq!(Slot::of("7"), None, "never a letter or digit");
        assert_eq!(Health::of("front"), Some(Health::Front));
    }

    fn run(text: &str, bold: bool, id: Option<&str>) -> Run {
        Run {
            text: text.to_owned(),
            style: Markup {
                bold_depth: u16::from(bold),
                ..Markup::default()
            },
            link: id.map(|id| Link {
                kind: LinkKind::Exist {
                    id: id.to_owned(),
                    noun: "troll".to_owned(),
                },
                text: text.to_owned(),
                coord: None,
            }),
            inner_link: None,
        }
    }

    /// A creature the registry has is followed by its tag; an object not in
    /// bold, a bold link the registry has not got, and plain words are not.
    #[test]
    fn a_creatures_name_is_tagged() {
        let line = Line::new(
            "",
            Runs {
                runs: vec![
                    run("A ", false, None),
                    run("hill troll", true, Some("33")),
                    run(" swings at ", false, None),
                    run("him", true, Some("34")),
                    run(" with a ", false, None),
                    run("club", false, Some("35")),
                    run("!", false, None),
                ],
            },
        );
        let style = Style {
            slots: [Slot::Unique, Slot::Random, Slot::Separator('-')],
            health: Health::Back,
        };
        let known = |id| (id == 33).then_some((MARK, 73));
        let tagged = tagged(&line, style, known).expect("a creature");
        assert_eq!(
            tagged.text(),
            "A hill troll (A7-073) swings at him with a club!"
        );
        let plain = Line::new(
            "",
            Runs {
                runs: vec![run("Hello.", false, None)],
            },
        );
        assert!(super::tagged(&plain, style, known).is_none());
    }

    /// The game's verbs take the whole tag, a separator typed or not; the
    /// script's commands its first character or more; a tag two creatures
    /// share, or one no creature here has, sends nothing and says why.
    #[test]
    fn a_tag_names_its_creature_back() {
        let style = Style {
            slots: [Slot::Unique, Slot::Random, Slot::Separator('-')],
            health: Health::Back,
        };
        let troll = (123_456, MARK);
        let other = (
            7,
            Mark {
                unique: 'B',
                random: ['7', '7', '7'],
            },
        );
        let sent = |line: &str| resolve(line, [troll, other], style);
        assert_eq!(sent("kill a7"), Some(Ok("kill #123456".to_owned())));
        assert_eq!(sent("kill A7-"), Some(Ok("kill #123456".to_owned())));
        assert_eq!(sent("kill a"), None, "the game's verbs take the whole tag");
        assert_eq!(sent("look a7"), None, "not a tag verb");
        assert_eq!(sent("tk a"), Some(Ok("kill #123456".to_owned())));
        assert_eq!(sent("tkill A7-"), Some(Ok("kill #123456".to_owned())));
        assert_eq!(sent("tcm sweep b"), Some(Ok("cman sweep #7".to_owned())));
        assert_eq!(sent("tm jab a"), Some(Ok("mstrike jab #123456".to_owned())));
        assert_eq!(sent("tkic a"), Some(Ok("kick #123456".to_owned())));
        assert!(matches!(sent("tk"), Some(Err(why)) if why.starts_with("Usage")));
        assert!(matches!(sent("tcm a"), Some(Err(why)) if why.starts_with("Usage")));
        assert_eq!(
            sent("tk zz"),
            Some(Err("Target with tag 'ZZ' not found!".to_owned()))
        );
        assert_eq!(sent("t a"), None, "not one of the script's");
        let twins = [(1, MARK), (2, MARK)];
        assert!(matches!(
            resolve("tk a", twins, style),
            Some(Err(why)) if why.starts_with("More than one")
        ));
    }
}
