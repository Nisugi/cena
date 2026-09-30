//! Creature tags: `targetid.lic`'s idea, in Hydra (the author, 2026-09-30:
//! *"it adds a unique identifier to each creature and shows it after their
//! name. It's a way to distinguish them apart since you can't see their id
//! and their id is just a string of numbers"*).
//!
//! A tag is three characters of a creature's `exist` id written in
//! `targetid`'s alphabet, low bits first, so the same creature always has
//! the same tag, as the script's players know it (`lich_repo_mirror/lib/
//! targetid.lic`, `id_to_key`): `a hill troll (7QK)`. It is put after each
//! bold link to an object, the creatures' mark, as the script's pattern
//! puts it, and never after a pronoun.
//!
//! **A tag names its creature back.** The script gave the game a dozen new
//! verbs, `tkill 7QK`, because to Lich a tag is only text. Hydra knows each
//! tag's creature, so a player types the game's own verb, `kill 7QK`, and
//! Hydra sends `kill #<its id>` ([`resolve`]; the author: *"Targetting by
//! targetid is cool and ok"*). Only after one of the script's verbs, only as
//! the command's last word, only the whole three characters, and only a
//! creature in the room: `look at bag` is never taken for a tag.

use cena_protocol::frame::LinkKind;
use cena_protocol::runs::{Run, Runs};

use crate::line::Line;

/// `targetid`'s alphabet: digits and letters, no `0`, `1`, `I` or `O`.
const ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// How many characters a tag is unless the player chooses (`targetid`'s
/// own).
pub const DEFAULT_LENGTH: usize = 3;

/// The longest a tag may be: six characters are thirty bits, a whole
/// creature id, so a longer one says nothing more. The shortest is one; a
/// short tag is quicker to read and more often shared by two creatures in
/// a room (the author, 2026-09-30: *"3 can be default but a setting to
/// change it 1 - 4 probably works, maybe 1-6 just cause?"*).
pub const LONGEST: usize = 6;

/// The verbs a tag may follow: `targetid`'s own, less their `t`.
const VERBS: [&str; 11] = [
    "target", "attack", "ambush", "kill", "cast", "cman", "punch", "grapple", "kick", "jab",
    "mstrike",
];

/// What a bold link says that is no creature's name: `targetid`'s
/// `IGNORE_PATTERN`.
const PRONOUNS: [&str; 7] = ["he", "she", "his", "her", "him", "it", "its"];

/// The tag of the creature `id`, `length` characters long (1 to
/// [`LONGEST`]).
#[must_use]
pub fn tag(id: i64, length: usize) -> String {
    let mut bits = id;
    (0..length.clamp(1, LONGEST))
        .map(|_| {
            let char = ALPHABET[usize::try_from(bits & 31).unwrap_or(0)];
            bits >>= 5;
            char::from(char)
        })
        .collect()
}

/// `line` with each creature's tag, `length` long, after its name, and a
/// trigger's looks moved along with the words they were on; `None` when it
/// names none.
#[must_use]
pub fn tagged(line: &Line, length: usize) -> Option<Line> {
    let runs = &line.runs.runs;
    let mut out = Vec::with_capacity(runs.len() + 1);
    let mut paint = line.paint.clone();
    let mut at = 0usize;
    let mut added = false;
    for (index, run) in runs.iter().enumerate() {
        at += run.text.len();
        out.push(run.clone());
        let Some(id) = creature(run) else {
            continue;
        };
        // The last run of the link: a name split by a style is one name.
        if runs
            .get(index + 1)
            .is_some_and(|next| creature(next) == Some(id))
        {
            continue;
        }
        let text = format!(" ({})", tag(id, length));
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

/// The creature a run names: a bold link to an object, not a pronoun.
fn creature(run: &Run) -> Option<i64> {
    if run.style.bold_depth == 0 {
        return None;
    }
    let Some(LinkKind::Exist { id, .. }) = run.link.as_ref().map(|link| &link.kind) else {
        return None;
    };
    let word = run.text.trim();
    if PRONOUNS
        .iter()
        .any(|pronoun| word.eq_ignore_ascii_case(pronoun))
    {
        return None;
    }
    id.parse().ok()
}

/// `line`, a command the player typed, with a creature's tag, `length`
/// long, as its last word after one of the tag verbs, as the game's own
/// target, `#<id>`: `kill 7QK` as `kill #123456`. `creatures` are those in
/// the room. `None` when it has no tag to take; `Some(Err)` saying why
/// when the tag is more than one creature's, and nothing is to be sent: a
/// short tag can be.
#[must_use]
pub fn resolve(
    line: &str,
    creatures: impl IntoIterator<Item = i64>,
    length: usize,
) -> Option<Result<String, String>> {
    let trimmed = line.trim();
    let verb = trimmed.split_whitespace().next()?;
    if !VERBS.iter().any(|known| verb.eq_ignore_ascii_case(known)) {
        return None;
    }
    let (before, last) = trimmed.rsplit_once(char::is_whitespace)?;
    if last.len() != length.clamp(1, LONGEST) {
        return None;
    }
    let mut matching = creatures
        .into_iter()
        .filter(|id| tag(*id, length).eq_ignore_ascii_case(last));
    let id = matching.next()?;
    if matching.next().is_some() {
        return Some(Err(format!(
            "The tag {} is more than one creature's here; nothing was sent.",
            last.to_ascii_uppercase()
        )));
    }
    Some(Ok(format!("{} #{id}", before.trim_end())))
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_LENGTH, resolve, tag, tagged};
    use crate::line::Line;
    use cena_protocol::frame::{Link, LinkKind, Style};
    use cena_protocol::runs::{Run, Runs};

    /// `targetid.lic`'s `id_to_key`, worked by hand: 5 bits at a time, low
    /// first, each a letter of its alphabet.
    #[test]
    fn a_tag_is_the_id_in_targetids_alphabet() {
        // 0b00011_00010_00001: 1, 2, 3 -> '3', '4', '5'.
        assert_eq!(tag(0b00011_00010_00001, 3), "345");
        assert_eq!(tag(0, 3), "222");
        assert_eq!(tag(31, 3), "Z22");
    }

    fn run(text: &str, bold: bool, id: Option<&str>) -> Run {
        Run {
            text: text.to_owned(),
            style: Style {
                bold_depth: u16::from(bold),
                ..Style::default()
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

    /// A creature's name is followed by its tag; a pronoun, an object not in
    /// bold, and plain words are not.
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
        let tagged = tagged(&line, DEFAULT_LENGTH).expect("a creature");
        assert_eq!(
            tagged.text(),
            format!("A hill troll ({}) swings at him with a club!", tag(33, 3))
        );
        let plain = Line::new(
            "",
            Runs {
                runs: vec![run("Hello.", false, None)],
            },
        );
        assert!(super::tagged(&plain, DEFAULT_LENGTH).is_none());
    }

    /// A tag after a tag verb is the creature's id; any other word, verb or
    /// length is left alone; a tag two creatures share sends nothing.
    #[test]
    fn a_tag_names_its_creature_back() {
        let troll = 123_456;
        let key = tag(troll, 3).to_ascii_lowercase();
        assert_eq!(
            resolve(&format!("kill {key}"), [7, troll], 3),
            Some(Ok(format!("kill #{troll}")))
        );
        assert_eq!(
            resolve(&format!("cman sweep {key}"), [troll], 3),
            Some(Ok(format!("cman sweep #{troll}")))
        );
        assert_eq!(
            resolve(&format!("look {key}"), [troll], 3),
            None,
            "not a tag verb"
        );
        assert_eq!(resolve("kill troll", [troll], 3), None);
        assert_eq!(
            resolve(&format!("kill {key}"), [7], 3),
            None,
            "not in the room"
        );
        assert_eq!(resolve("kill", [troll], 3), None);
        // One character: 32 tags, and two creatures 32 apart share one.
        let one = tag(troll, 1);
        assert!(matches!(
            resolve(&format!("kill {one}"), [troll, troll + 32], 1),
            Some(Err(_))
        ));
        assert_eq!(
            resolve(&format!("kill {one}"), [troll], 1),
            Some(Ok(format!("kill #{troll}")))
        );
        assert_eq!(tag(troll, 6).len(), 6);
        assert_eq!(tag(troll, 9).len(), 6, "never longer than an id");
    }
}
