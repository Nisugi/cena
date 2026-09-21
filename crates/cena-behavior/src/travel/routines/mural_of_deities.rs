//! `Puzzle::MuralOfDeities`: the mural of room 6897.
//!
//! Upstream (`upstream_scripts/mural_of_deities.rb`): `touch mural`, and read
//! every line until "The face falls quiet and goes still.", matching each
//! verse the face recites to its deity -- 108 verses for 20 deities, copied
//! here whole and in upstream's order. Then `touch <deity>` for each, in the
//! order recited.
//!
//! The recital outlasts the prompt that answers `touch mural`, so what the
//! answer leaves unsaid is waited for: [`WAIT_MS`] at a time, [`MAX_WAITS`]
//! times. Upstream waits for ever; both numbers are this port's guess.
//!
//! Deviations: upstream sends the touches without waiting when the character
//! is one named player, which is not copied. A recital in which no verse was
//! known ends upstream having done nothing; here that is [`Next::Failed`],
//! which gives the exit up as the trip would have anyway.

use super::{Next, Seen, Solver};

/// The line that ends the recital.
const END: &str = "The face falls quiet and goes still.";
/// How long one wait for the recital's end lasts.
const WAIT_MS: u64 = 30_000;
/// How many such waits before the mural is given up.
const MAX_WAITS: u32 = 10;

/// Each deity and the verses that are theirs, as upstream lists them.
const VERSES: &[(&str, &[&str])] = &[
    (
        "Andelas",
        &[
            "A black cat crosses o'er a path of blood",
            "The crimson claw, the fang, the chase, the play",
            "The mice will play until the hunger bites",
            "There welcome lies, midst claws that spread with glee",
            "Yet one may smile, and be a predator",
        ],
    ),
    (
        "Charl",
        &[
            "No mercy lies in sea's tempestuous rage",
            "One ship is saved, one lost in tempest's din",
            "Still waters gather, 'ware the eye of storms",
            "The blood of tyrants feeds the hurricane",
            "The sailor lives to praise the storm-wild sea",
        ],
    ),
    (
        "Eonak",
        &[
            "Cold stone be given life by fiery forge",
            "Creation's soul, love's labor, earth and fire",
            "Let hammer strike the stone, the stone create",
            "The craftsman works, the iron becomes his hand",
            "The forger draws hard beauty from the ore",
        ],
    ),
    (
        "Fash'lo'nae",
        &[
            "Cold yellow eye draws magic from a rune",
            "Dark flame of knowledge burns the midnight lamp",
            "The face of wisdom in the mirror crack'd",
            "The old one, so 'tis said, is he who seeks",
            "Thou feed'st the hearth of knowledge with a fire",
        ],
    ),
    (
        "Gosaena",
        &[
            "In cold grey stone waits pale all-seeing eye",
            "No mortal knows the veil beyond the gate",
            "On silent wings, and peace beyond the dark",
            "Silence is silver, grey eternity",
            "The sickle falls and reaps the cycle's pause",
        ],
    ),
    (
        "Imaera",
        &[
            "Dance on the wild green, child of the wood",
            "In autumn's chill the harvest golden reap'd",
            "The healing herb, the oak, the barley grow",
            "The silent doe reclines in dappled glade",
            "To nature's sylvan bounty raise a toast",
        ],
    ),
    (
        "Ivas",
        &[
            "Beauty belies corruption to the core",
            "Behold temptation, bright deceiving lie",
            "Fine silk and velvet drape the dark decay",
            "The butterfly may twist into a wyrm",
            "Ye soft and bitter, sharp and demon-sweet",
        ],
    ),
    (
        "Jastev",
        &[
            "Carry the weight of ages before time",
            "Death will yet come, still beauty lies in store",
            "Preserve the beauty of the dark and light",
            "Preserve the past for those who come anon",
            "Seer of sad shadows, painter of Mystery",
            "Sing prophecy of poets in the night",
            "Speak into silence, silence answers back",
            "The artist weeps, old tears becoming paint",
        ],
    ),
    (
        "Jaston",
        &[
            "A snow-white feather dances on the air",
            "A sylvan zephyr rides the winds of light",
            "Above the green, white wings may soar afar",
            "Fly on the mistral wind, bright bird of song",
            "O love, bright spirit, rise on feathered wings",
        ],
    ),
    (
        "Koar",
        &[
            "A golden crown shines bright in moonlight grey",
            "All wisdom at thy side, rule o'er the moon",
            "Great mountain rises, crowned in sunlight gold",
            "Keep alert watch unquieted by sleep",
            "O King, keep watch from far beyond the ice",
            "The bringer of tense peace to light and dark",
            "The wisdom of the Law the balance keep",
            "Thy word is law, great majesty of old",
        ],
    ),
    (
        "Laethe",
        &[
            "A pang of sorrow calls a long-lost love",
            "Compassion doth within great sorrow lie",
            "Return, O love, and mend a longing heart",
            "The black rose grows among the purple graves",
            "Young love reflected in a pool of tears",
        ],
    ),
    (
        "Lorminstra",
        &[
            "Give all to life, that ye may death defy",
            "Hold fast the golden key that binds the gate",
            "In winter's cold doth life begin anew",
            "The key, the gate, to be or not to be",
            "The touch of death melts like the winter snow",
        ],
    ),
    (
        "Luukos",
        &[
            "Death's minions scream and wail in twisted dark",
            "Fear ye the serpent, mockery of death",
            "In promises of life twists falsehood black",
            "Serpent slithers among the fallen horde",
            "Unending torment feeds on souls of lies",
        ],
    ),
    (
        "Marlu",
        &[
            "A demon's wings beat time in shadow vile",
            "Ancient destroyer, lord of what was not",
            "Black shadow moves in timeless pit of ruin",
            "Guard ye the veil, lest demons darkness free",
            "Still-living ichor drips from blackened stone",
        ],
    ),
    (
        "Onar",
        &[
            "A dark blade's hiss disturbs the still night air",
            "All life has price, and death does not come cheap",
            "Cold death so swift, a whisper in the dark",
            "O sudden darkness, death a silent strike",
            "The white skull grins, the dark shape on the floor",
        ],
    ),
    (
        "Ronan",
        &[
            "I shroud my prayers in darkest cloak of night",
            "In dreams may darkness find a peaceful sleep",
            "O silver blade, flash brightly in the night",
            "Protector of the night, keep safe the dark",
            "Walk brave in endless shadow, guard the door",
            "Watch o'er the night, so we might keep the day",
        ],
    ),
    (
        "Sheru",
        &[
            "A nightmare drips black blood upon the stone",
            "Behold dark dreams of nightmare gallery",
            "False gold in dreams of blackness, crimson blood",
            "In moon-dark shadows howls the jackal wild",
            "Stark scream of terror pleads for break of day",
        ],
    ),
    (
        "Tonis",
        &[
            "A breath of life, a trinket to a thief",
            "Across the sky, a streak of flaming gold",
            "Be swift of foot and mind, and quickly fly",
            "On golden wings, a blur of wind and haste",
            "On wings begat of freedom prayers take flight",
            "The hand so quick, the surest eye is lost",
        ],
    ),
    (
        "Voaris",
        &[
            "A golden rose lies soft on sanguine field",
            "A king mayst not deny love pure and true",
            "Hold fast thy hands, and love shall find its way",
            "The love forbidden, fate shall not deny",
            "Young love is golden to the true of heart",
        ],
    ),
    (
        "Zelia",
        &[
            "A pixie's laugh, a dark demented soul",
            "A silver crescent flickers in thine eye",
            "Beyond the moons, mad laughter calls to thee",
            "The gift of madness touches whom it may",
            "Wild freedom lies beneath chaotic moon",
        ],
    ),
];

/// Whose verse this line holds. The first listed wins, as upstream's `elsif`.
fn deity_of(line: &str) -> Option<&'static str> {
    VERSES
        .iter()
        .find(|(_, verses)| verses.iter().any(|verse| line.contains(verse)))
        .map(|(deity, _)| *deity)
}

#[derive(Default)]
pub(super) struct MuralOfDeities {
    /// The deities, in the order recited.
    recited: Vec<&'static str>,
    waits: u32,
    at: At,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    Listening,
    /// How many deities have been touched.
    Touching(usize),
}

impl MuralOfDeities {
    /// Read what was heard. `true`: the face has gone still.
    fn listen(&mut self, seen: &Seen<'_>) -> bool {
        for line in seen.answer {
            let text = line.text();
            if let Some(deity) = deity_of(&text) {
                self.recited.push(deity);
            } else if text.contains(END) {
                return true;
            }
        }
        false
    }
}

impl Solver for MuralOfDeities {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => {
                self.at = At::Listening;
                Next::Put("touch mural".to_owned())
            }
            At::Listening => {
                if self.listen(seen) {
                    if self.recited.is_empty() {
                        return Next::Failed;
                    }
                    self.at = At::Touching(0);
                    return self.next(seen);
                }
                if self.waits >= MAX_WAITS {
                    return Next::Failed;
                }
                self.waits += 1;
                Next::Await(vec![END.to_owned()], WAIT_MS)
            }
            At::Touching(done) => match self.recited.get(done) {
                Some(deity) => {
                    self.at = At::Touching(done + 1);
                    Next::Put(format!("touch {deity}"))
                }
                None => Next::Done,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    fn wait() -> Next {
        Next::Await(vec![END.to_owned()], WAIT_MS)
    }

    fn put(command: &str) -> Next {
        Next::Put(command.to_owned())
    }

    #[test]
    fn verses_are_answered_in_the_order_recited_across_several_hearings() {
        let mut mural = MuralOfDeities::default();
        assert_eq!(Scene::at(1, 2).ask(&mut mural), put("touch mural"));
        let first = Scene::at(1, 2).answered(&[
            "You touch the mural.",
            "The face intones, \"The key, the gate, to be or not to be.\"",
        ]);
        assert_eq!(first.ask(&mut mural), wait());
        // What follows the face going still is not the recital's.
        let rest = Scene::at(1, 2).answered(&[
            "\"Silence is silver, grey eternity.\"",
            "\"A pixie's laugh, a dark demented soul.\"",
            END,
            "\"Yet one may smile, and be a predator.\"",
        ]);
        assert_eq!(rest.ask(&mut mural), put("touch Lorminstra"));
        assert_eq!(Scene::at(1, 2).ask(&mut mural), put("touch Gosaena"));
        assert_eq!(Scene::at(1, 2).ask(&mut mural), put("touch Zelia"));
        assert_eq!(Scene::at(1, 2).ask(&mut mural), Next::Done);
    }

    #[test]
    fn a_recital_that_fits_one_answer_needs_no_wait() {
        let mut mural = MuralOfDeities::default();
        Scene::at(1, 2).ask(&mut mural);
        let all = Scene::at(1, 2).answered(&["Thy word is law, great majesty of old", END]);
        assert_eq!(all.ask(&mut mural), put("touch Koar"));
    }

    #[test]
    fn a_face_that_never_goes_still_is_given_up() {
        let mut mural = MuralOfDeities::default();
        Scene::at(1, 2).ask(&mut mural);
        for _ in 0..MAX_WAITS {
            assert_eq!(Scene::at(1, 2).ask(&mut mural), wait());
        }
        assert_eq!(Scene::at(1, 2).ask(&mut mural), Next::Failed);
    }

    #[test]
    fn a_recital_with_no_verse_known_fails() {
        let mut mural = MuralOfDeities::default();
        Scene::at(1, 2).ask(&mut mural);
        let strange = Scene::at(1, 2).answered(&["A verse nobody wrote down.", END]);
        assert_eq!(strange.ask(&mut mural), Next::Failed);
    }

    /// The table against the script it was copied from: each deity's verses,
    /// joined as upstream's pattern joins them, sit just before its `push`.
    #[test]
    fn the_table_is_upstreams() {
        let script =
            include_str!("../../../../cena-mapdb-convert/src/upstream_scripts/mural_of_deities.rb");
        assert_eq!(script.matches("result.push").count(), VERSES.len());
        for (deity, verses) in VERSES {
            let pattern = format!("line =~ /{}/", verses.join("|"));
            let (_, after) = script.split_once(&pattern).expect("the verses, in order");
            let pushed = after.trim_start().trim_start_matches("result.push");
            let pushed = pushed.trim_start().trim_start_matches('(');
            assert!(pushed.starts_with(&format!("\"{deity}\"")), "{deity}");
        }
    }

    #[test]
    fn every_verse_is_its_own_deitys_and_the_table_is_whole() {
        assert_eq!(VERSES.len(), 20);
        let verses: usize = VERSES.iter().map(|(_, verses)| verses.len()).sum();
        assert_eq!(verses, 108);
        for (deity, verses) in VERSES {
            for verse in *verses {
                let mut mural = MuralOfDeities::default();
                Scene::at(1, 2).ask(&mut mural);
                let said = format!("The face says, \"{verse}.\"");
                assert_eq!(
                    Scene::at(1, 2).answered(&[&said, END]).ask(&mut mural),
                    put(&format!("touch {deity}"))
                );
            }
        }
    }
}
