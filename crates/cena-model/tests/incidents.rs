//! The hunt's incidents (`state/incident.rs`), from lines shaped as the
//! patterns of Lich's `combat/defs/messages.rb` expect them.

use cena_model::state::chunks::ChunkLine;
use cena_model::state::incident::{Disarm, HiveTrap, Incident, read};
use cena_protocol::frame::{Link, LinkKind, Style};
use cena_protocol::runs::{Run, Runs};

/// A run of text, or an object `(id, noun)`, bolded or not, or a command.
enum Part<'a> {
    Text(&'a str),
    Item(&'a str, &'a str, &'a str),
    Creature(&'a str, &'a str, &'a str),
    Command(&'a str, &'a str),
}

fn line(parts: &[Part<'_>]) -> ChunkLine {
    let runs = parts
        .iter()
        .map(|part| {
            let (text, link, bold) = match *part {
                Part::Text(text) => (text, None, false),
                Part::Item(id, noun, text) | Part::Creature(id, noun, text) => (
                    text,
                    Some(Link {
                        kind: LinkKind::Exist {
                            id: id.to_owned(),
                            noun: noun.to_owned(),
                        },
                        text: text.to_owned(),
                        coord: None,
                    }),
                    matches!(part, Part::Creature(..)),
                ),
                Part::Command(cmd, text) => (
                    text,
                    Some(Link {
                        kind: LinkKind::Direct {
                            cmd: cmd.to_owned(),
                        },
                        text: text.to_owned(),
                        coord: None,
                    }),
                    false,
                ),
            };
            let style = Style {
                bold_depth: u16::from(bold),
                ..Style::default()
            };
            Run {
                text: text.to_owned(),
                style,
                link,
                inner_link: None,
            }
        })
        .collect();
    ChunkLine {
        runs: Runs { runs },
    }
}

#[test]
fn a_weapon_knocked_away_names_the_weapon() {
    let got = read(&line(&[
        Part::Text("Your "),
        Part::Item("123", "broadsword", "steel broadsword"),
        Part::Text(" is knocked from your grasp!"),
    ]));
    let Some(Incident::Disarmed { how, weapon }) = got else {
        panic!("a disarm: {got:?}");
    };
    assert_eq!(how, Disarm::Knocked);
    assert_eq!(weapon.map(|w| w.noun), Some("broadsword".to_owned()));
    assert!(matches!(
        read(&line(&[
            Part::Text("The webbing entangles your "),
            Part::Item("123", "broadsword", "steel broadsword"),
            Part::Text(", rendering it useless!"),
        ])),
        Some(Incident::Disarmed {
            how: Disarm::Webbed,
            ..
        })
    ));
}

/// Lich's weapon lines need the weapon's own link right after `Your`
/// (`combat/defs/messages.rb:94`, `:133`), and so do these (MO-B-4): words
/// with no weapon there disarm nothing, and the object elsewhere on the line
/// (a speaker, a creature's prize) is never taken for the weapon.
#[test]
fn a_weapon_line_needs_the_weapon_after_your() {
    for parts in [
        vec![Part::Text(
            "Your sword tears free from your hands and floats away.",
        )],
        vec![
            Part::Item("-5", "Bob", "Bob"),
            Part::Text(" mutters, Your sword tears free from your hands and floats away."),
        ],
        vec![
            Part::Text("The webbing entangles your sword, rendering it useless, says "),
            Part::Item("-5", "Bob", "Bob"),
        ],
        vec![
            Part::Item("-5", "Bob", "Bob"),
            Part::Text(": Your blade returns to normal."),
        ],
    ] {
        let got = read(&line(&parts));
        assert_eq!(got, None, "{parts:?}", parts = parts.len());
    }
    let got = read(&line(&[
        Part::Text("Your "),
        Part::Item("123", "sword", "vultite sword"),
        Part::Text(" tears free from your hands and floats away!"),
    ]));
    assert!(
        matches!(&got, Some(Incident::Disarmed { how: Disarm::Telekinetic, weapon: Some(w) }) if w.noun == "sword"),
        "{got:?}"
    );
}

#[test]
fn hazards_holds_and_charges() {
    for (text, want) in [
        (
            "You shiver slightly as an invisible rash covers your body.",
            Incident::ItchyCurse,
        ),
        (
            "The apparatus flickers with deadly radiance!",
            Incident::HiveTrap(HiveTrap::Apparatus),
        ),
        (
            "An unseen force entangles you, restricting your movement!",
            Incident::Entangled,
        ),
        (
            "A shadowy figure leaps from hiding to attack you!",
            Incident::Ambusher(None),
        ),
        (
            "You don't seem to be able to move to do that.",
            Incident::Rooted(None),
        ),
        (
            "You note some treasure of interest but are unable to pick any up.",
            Incident::ItemLimit,
        ),
        (
            "Your Swift Justice charges are increased to 4.",
            Incident::SwiftJustice(4),
        ),
        (
            "Your Swift Justice surges through you! Its charges are reduced to 2.",
            Incident::SwiftJustice(2),
        ),
        (
            "Nature's blessing of vitality departs as your arcane prowess returns to normal.",
            Incident::ArcaneReflex(false),
        ),
        (
            "You're now no longer aiming at anything in particular.",
            Incident::Aiming(None),
        ),
    ] {
        assert_eq!(read(&ChunkLine::plain(text)), Some(want), "{text}");
    }
    assert_eq!(read(&ChunkLine::plain("You swing a broadsword!")), None);
}

#[test]
fn an_arrow_stuck_names_the_creature_and_where() {
    let got = read(&line(&[
        Part::Text("The arrow sticks in a "),
        Part::Creature("77", "kobold", "kobold"),
        Part::Text("'s left arm!"),
    ]));
    let Some(Incident::ArrowStuck { creature, at }) = got else {
        panic!("an arrow: {got:?}");
    };
    assert_eq!(creature.map(|c| c.id), Some("77".to_owned()));
    assert_eq!(at, "arm");
}

#[test]
fn the_weapon_reaction_is_the_links_command() {
    assert_eq!(
        read(&line(&[
            Part::Text("You could use this opportunity to "),
            Part::Command("WEAPON TACKLE #77", "tackle"),
            Part::Text("!"),
        ])),
        Some(Incident::WeaponReaction("TACKLE #77".to_owned()))
    );
}
