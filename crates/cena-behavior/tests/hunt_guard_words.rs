//! The guard words `plan/33` kept, renamed and merged, beyond Nisugi's six
//! (`hunt_guard.rs`): what each reads, and that unknown skips.
//!
//! Every test puts the fact the word reads into the state and takes it away
//! again, so a word wired to the wrong fact, or to none, turns it red.

use cena_behavior::hunt::{Condition, Facts, Here, Hunt, Profile, Said, Used};
use cena_map::RoomId;
use cena_session::{
    Amount, Effect, Frame, GameState, Link, LinkKind, ProgressBar, PsmCategory, PsmLine, PsmRanks,
    Run, Runs, TextFrame,
};

/// One guard, parsed. `None` is a broken fixture, which every test unwraps
/// into a failure.
fn one(text: &str) -> Option<Condition> {
    let mut group = Condition::parse_group(text).ok()?;
    (group.len() == 1).then(|| group.remove(0))
}

/// `text` against `state`, aimed at `target`, with nothing else known.
fn reads(text: &str, state: &GameState, target: Option<i64>) -> Option<bool> {
    one(text)?.holds(&Facts::new(state, target))
}

/// A state whose server clock reads `second`.
fn at(second: u32) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state
}

/// A run of plain text, or a link to `id` named `name` with this `noun`.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn run(text: &str, link: Option<(i64, &str)>, bold: bool) -> Run {
    let mut run = Run {
        text: text.to_owned(),
        style: Default::default(),
        link: link.map(|(id, noun)| Link {
            kind: LinkKind::Exist {
                id: id.to_string(),
                noun: noun.to_owned(),
            },
            text: text.to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    if bold {
        run.style.bold_depth = 1;
    }
    run
}

/// A creature seen in the room, named `name`, and no `<crtrStatus>` yet.
fn seen(mut state: GameState, id: i64, noun: &str, name: &str) -> GameState {
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![run(name, Some((id, noun)), true)],
        },
    });
    state
}

/// A creature in the room with these `<crtrStatus>` attributes.
fn tagged(state: GameState, id: i64, noun: &str, name: &str, attrs: &[(&str, &str)]) -> GameState {
    let mut state = seen(state, id, noun, name);
    let mut attrs: Vec<(String, String)> = attrs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    attrs.insert(0, ("exist".to_owned(), id.to_string()));
    state.apply(&Frame::CreatureStatus {
        id: id.to_string(),
        attrs,
    });
    state
}

/// Mastodons with these ids and `<crtrStatus>` attributes, all in the room
/// at once: one `room objs` names them all, as the game re-states the list.
fn herd(creatures: &[(i64, &[(&str, &str)])]) -> GameState {
    let mut state = at(1_000);
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: creatures
                .iter()
                .map(|(id, _)| run("mastodon", Some((*id, "mastodon")), true))
                .collect(),
        },
    });
    for (id, attrs) in creatures {
        let mut attrs: Vec<(String, String)> = attrs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        attrs.insert(0, ("exist".to_owned(), id.to_string()));
        state.apply(&Frame::CreatureStatus {
            id: id.to_string(),
            attrs,
        });
    }
    state
}

/// A hostile mastodon, id 42, with these attributes as well.
fn mastodon(attrs: &[(&str, &str)]) -> GameState {
    let mut all = vec![("hostile", "1")];
    all.extend_from_slice(attrs);
    tagged(at(1_000), 42, "mastodon", "mastodon", &all)
}

/// A swing that hits creature `id`, a cave lizard named in bold, and the
/// prompt that closes it: the fight registers a creature on its own, with
/// no `<crtrStatus>` (`cena-model` `tests/combat_registry.rs`).
#[expect(
    clippy::default_trait_access,
    reason = "the text's style type is not re-exported for behaviors"
)]
fn fought(mut state: GameState, id: i64) -> GameState {
    let lines: [&[(&str, bool)]; 3] = [
        &[
            ("You swing a broadsword at ", false),
            ("a cave lizard", true),
            ("!", false),
        ],
        &[(
            "  AS: +300 vs DS: +100 with AvD: +30 + d100 roll: +50 = +280",
            false,
        )],
        &[("   ... and hit for 40 points of damage!", false)],
    ];
    for line in lines {
        for (at, (text, bold)) in line.iter().enumerate() {
            let mut frame = TextFrame {
                content: (*text).to_owned(),
                stream: String::new(),
                style: Default::default(),
                link: bold.then(|| Link {
                    kind: LinkKind::Exist {
                        id: id.to_string(),
                        noun: "lizard".to_owned(),
                    },
                    text: (*text).to_owned(),
                    coord: None,
                }),
                inner_link: None,
                ends_line: at + 1 == line.len(),
            };
            if *bold {
                frame.style.bold_depth = 1;
            }
            state.apply(&Frame::Text(frame));
        }
    }
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state
}

/// A vitals bar: `percent`, and the amount when `Some`.
fn bar(state: &mut GameState, id: &str, percent: u32, amount: Option<(i32, i32)>) {
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: id.to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent,
        text: amount.map_or_else(|| id.to_owned(), |(c, m)| format!("{id} {c}/{m}")),
        amount: amount.map(|(current, max)| Amount { current, max }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

/// An effect listed in `category` with this end time; the first in its
/// category states the dialog, as the game's `clear='t'` does.
fn effect(state: &mut GameState, id: &str, category: &str, text: &str, ends_at: u32) {
    if !state.effects.saw_category(category) {
        state.effects.clear_category(category);
    }
    state.effects.insert(
        id.to_owned(),
        Effect {
            category: category.to_owned(),
            text: text.to_owned(),
            ends_at: Some(ends_at),
            percent: 50,
        },
    );
}

#[test]
fn every_word_reads_back_as_written() {
    for text in [
        "self_kneeling",
        "!disease",
        "poison",
        "outside",
        "splashy",
        "!nomagic",
        "alone",
        "health_at_least 50",
        "mana_at_least 30",
        "stamina_at_least 20",
        "spirit_at_least 5",
        "encumbrance_at_least 20",
        "buff \"Rapid Fire\"",
        "!spell \"Celerity\"",
        "cooldown \"Burst of Swiftness\"",
        "debuff \"Confused\"",
        "stunned",
        "webbed",
        "sleeping",
        "calm",
        "disoriented",
        "kneeling",
        "sitting",
        "flying",
        "hovering",
        "!rooted",
        "!down",
        "ascended",
        "ascension_boss",
        "challenging",
        "disengaged",
        "inferior",
        "mini_boss",
        "mount",
        "rider",
        "sympathetic",
        "undead",
        "noncorporeal",
        "ancient",
        "fatalcrit",
        "smote",
        "position 2",
        "position_at_least 1",
        "ucstierup",
        "injured \"leftLeg\" 2",
        "stunned_for 3",
        "helpless",
        "coup_ready",
        "targets_at_least 2",
        "targets_at_most 1",
        "once",
        "once_here",
        "every 10",
    ] {
        let parsed = one(text).unwrap_or_else(|| panic!("`{text}` does not parse"));
        assert_eq!(parsed.to_string(), text, "`{text}` reads back as written");
    }
    // A body part in words reads, and is written back in Lich's spelling.
    assert_eq!(
        one("injured \"left leg\" 2").map(|c| c.to_string()),
        Some("injured \"leftLeg\" 2".to_owned())
    );
    let tail = Condition::parse_group("injured \"tail\" 2").unwrap_err();
    assert!(tail.contains("leftLeg"), "the parts are listed: {tail}");
    let bare = Condition::parse_group("buff Rapid").unwrap_err();
    assert!(bare.contains("double quotes"), "{bare}");
    let old = Condition::parse_group("encumbrance_below 20").unwrap_err();
    assert!(old.contains("encumbrance_at_least N"), "{old}");
}

#[test]
fn my_conditions_are_unknown_until_the_game_says() {
    let mut state = GameState::default();
    for (word, id) in [
        ("disease", "diseased"),
        ("poison", "poisoned"),
        ("self_kneeling", "kneeling"),
    ] {
        assert_eq!(reads(word, &state, None), None, "{word}: never reported");
        state.status.set(id, true);
        assert_eq!(reads(word, &state, None), Some(true), "{word}");
        assert_eq!(
            reads(&format!("!{word}"), &state, None),
            Some(false),
            "!{word}"
        );
        state.status.set(id, false);
        assert_eq!(reads(word, &state, None), Some(false), "{word}");
    }
}

#[test]
fn vitals_read_points_except_health_which_is_a_percent() {
    let mut state = GameState::default();
    assert_eq!(reads("mana_at_least 30", &state, None), None, "no bar yet");
    // Mana is points: 25 of 100 is under 30 however full the bar is drawn.
    bar(&mut state, "mana", 90, Some((25, 100)));
    assert_eq!(reads("mana_at_least 30", &state, None), Some(false));
    assert_eq!(reads("mana_at_least 25", &state, None), Some(true));
    bar(&mut state, "stamina", 10, Some((40, 400)));
    assert_eq!(reads("stamina_at_least 40", &state, None), Some(true));
    assert_eq!(reads("stamina_at_least 41", &state, None), Some(false));
    bar(&mut state, "spirit", 100, Some((7, 10)));
    assert_eq!(reads("spirit_at_least 8", &state, None), Some(false));
    // A bar that states no amount cannot answer a count of points.
    bar(&mut state, "spirit", 70, None);
    assert_eq!(reads("spirit_at_least 1", &state, None), None);
    // Health is a percent: 70% of 300 is 210, and 210 is not the answer.
    bar(&mut state, "health", 70, Some((210, 300)));
    assert_eq!(reads("health_at_least 70", &state, None), Some(true));
    assert_eq!(reads("health_at_least 71", &state, None), Some(false));
}

#[test]
fn encumbrance_at_least_runs_at_or_above_the_percent() {
    let mut state = GameState::default();
    assert_eq!(reads("encumbrance_at_least 20", &state, None), None);
    state.character.encumbrance_percent = Some(20);
    assert_eq!(reads("encumbrance_at_least 20", &state, None), Some(true));
    assert_eq!(reads("encumbrance_at_least 21", &state, None), Some(false));
    assert_eq!(reads("!encumbrance_at_least 21", &state, None), Some(true));
}

#[test]
fn outside_is_an_exits_line_that_reads_paths() {
    let mut state = GameState::default();
    assert_eq!(reads("outside", &state, None), None, "no exits line yet");
    for (line, outdoors) in [
        ("Obvious paths: north, east", true),
        ("Obvious exits: north, east", false),
    ] {
        state.apply(&Frame::Component {
            id: "room exits".into(),
            body: Runs {
                runs: vec![run(line, None, false)],
            },
        });
        assert_eq!(reads("outside", &state, None), Some(outdoors), "{line}");
    }
}

#[test]
fn map_tags_are_unknown_until_the_map_places_the_room() {
    let state = GameState::default();
    let tags = ["splashy".to_owned()];
    let unplaced = Facts::new(&state, None);
    let placed = Facts {
        tags: Some(&tags),
        ..unplaced
    };
    let bare = Facts {
        tags: Some(&[]),
        ..unplaced
    };
    let splashy = one("splashy").unwrap();
    let nomagic = one("nomagic").unwrap();
    assert_eq!(splashy.holds(&unplaced), None);
    assert_eq!(splashy.holds(&placed), Some(true));
    assert_eq!(nomagic.holds(&placed), Some(false));
    assert_eq!(nomagic.holds(&bare), Some(false));
}

#[test]
fn alone_is_the_rooms_claim() {
    let mut state = GameState::default();
    assert_eq!(
        reads("alone", &state, None),
        None,
        "who is here was never said"
    );
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    assert_eq!(reads("alone", &state, None), Some(true));
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs {
            runs: vec![
                run("Also here: ", None, false),
                run("Qizhmur", Some((-9, "Qizhmur")), false),
            ],
        },
    });
    assert_eq!(reads("alone", &state, None), Some(false));
    assert_eq!(reads("!alone", &state, None), Some(true));
}

#[test]
fn each_target_status_is_its_own_tag() {
    for (word, attr) in [
        ("stunned", "stunned"),
        ("webbed", "webbed"),
        ("sleeping", "sleeping"),
        ("calm", "calmed"),
        ("disoriented", "disoriented"),
        ("kneeling", "kneeling"),
        ("sitting", "sitting"),
        ("flying", "flying"),
        ("hovering", "hovering"),
        ("rooted", "rooted"),
    ] {
        let with = mastodon(&[(attr, "1")]);
        let without = mastodon(&[]);
        assert_eq!(
            reads(word, &with, Some(42)),
            Some(true),
            "{word} with {attr}"
        );
        assert_eq!(
            reads(word, &without, Some(42)),
            Some(false),
            "{word} without"
        );
        assert_eq!(reads(word, &with, None), None, "{word}: no target");
        // A status the word does not name leaves it false.
        let other = if attr == "flying" {
            "hovering"
        } else {
            "flying"
        };
        let unrelated = mastodon(&[(other, "1")]);
        assert_eq!(
            reads(word, &unrelated, Some(42)),
            Some(false),
            "{word} with {other}"
        );
    }
}

#[test]
fn down_is_bigshots_prone_set() {
    for attr in [
        "sleeping", "webbed", "stunned", "kneeling", "sitting", "prone", "immobile",
    ] {
        assert_eq!(
            reads("down", &mastodon(&[(attr, "1")]), Some(42)),
            Some(true),
            "{attr}"
        );
    }
    for attr in ["flying", "hovering", "rooted", "calmed", "disoriented"] {
        assert_eq!(
            reads("down", &mastodon(&[(attr, "1")]), Some(42)),
            Some(false),
            "{attr}"
        );
    }
}

#[test]
fn flags_are_unknown_until_a_tag_arrives() {
    // A creature the fight registered, which no `<crtrStatus>` has described.
    let untagged = fought(at(1_000), 42);
    let creature = untagged.creatures().get(42);
    assert!(
        creature.is_some_and(|c| !c.flags_known()),
        "the swing registers it, and nothing has flagged it"
    );
    assert_eq!(reads("ascended", &untagged, Some(42)), None);
    assert_eq!(
        reads("fatalcrit", &untagged, Some(42)),
        Some(false),
        "it is a creature"
    );
    for (word, attr) in [
        ("ascended", "ascended"),
        ("ascension_boss", "AscensionBoss"),
        ("challenging", "challenging"),
        ("disengaged", "disengaged"),
        ("inferior", "inferior"),
        ("mini_boss", "MiniBoss"),
        ("mount", "mount"),
        ("rider", "rider"),
        ("sympathetic", "sympathetic"),
    ] {
        assert_eq!(
            reads(word, &mastodon(&[(attr, "1")]), Some(42)),
            Some(true),
            "{word}"
        );
        assert_eq!(reads(word, &mastodon(&[]), Some(42)), Some(false), "{word}");
    }
}

#[test]
fn undead_and_noncorporeal_are_the_creatures_types() {
    let zombie = tagged(at(1_000), 7, "zombie", "zombie", &[("hostile", "1")]);
    let ghost = tagged(at(1_000), 8, "ghost", "ghost", &[("hostile", "1")]);
    assert_eq!(reads("undead", &zombie, Some(7)), Some(true));
    assert_eq!(reads("noncorporeal", &ghost, Some(8)), Some(true));
    assert_eq!(reads("undead", &mastodon(&[]), Some(42)), Some(false));
    assert_eq!(reads("noncorporeal", &mastodon(&[]), Some(42)), Some(false));
}

#[test]
fn ancient_is_the_name_except_the_ghoul_master() {
    for (name, ancient) in [
        ("grizzled troll", true),
        ("ancient wyrm", true),
        ("ancient ghoul master", false),
        ("troll", false),
    ] {
        let noun = name.rsplit(' ').next().unwrap_or(name);
        let state = tagged(at(1_000), 5, noun, name, &[("hostile", "1")]);
        assert_eq!(reads("ancient", &state, Some(5)), Some(ancient), "{name}");
    }
}

#[test]
fn stunned_is_the_tag_and_stunned_for_is_the_time_left() {
    // The tag says stunned and nothing has said for how long: no time to count on.
    let stunned = mastodon(&[("stunned", "1")]);
    assert_eq!(reads("stunned", &stunned, Some(42)), Some(true));
    assert_eq!(reads("stunned_for 1", &stunned, Some(42)), Some(false));
    assert_eq!(reads("stunned_for 0", &stunned, Some(42)), Some(true));
}

#[test]
fn helpless_is_a_creature_that_cannot_act() {
    assert_eq!(
        reads("helpless", &mastodon(&[("webbed", "1")]), Some(42)),
        Some(true)
    );
    assert_eq!(
        reads("helpless", &mastodon(&[("sleeping", "1")]), Some(42)),
        Some(true)
    );
    assert_eq!(
        reads("helpless", &mastodon(&[("flying", "1")]), Some(42)),
        Some(false)
    );
    assert_eq!(reads("helpless", &mastodon(&[]), None), None);
}

#[test]
fn the_fights_record_answers_for_a_creature_and_not_without_one() {
    let fresh = mastodon(&[]);
    for (text, fresh_answer) in [
        ("fatalcrit", false),
        ("smote", false),
        ("ucstierup", false),
        ("position 1", false),
        ("position 0", true),
        ("position_at_least 1", false),
        ("!position_at_least 1", true),
        ("injured \"leftLeg\" 1", false),
    ] {
        assert_eq!(reads(text, &fresh, Some(42)), Some(fresh_answer), "{text}");
        assert_eq!(reads(text, &fresh, None), None, "{text}: no target");
        assert_eq!(
            reads(text, &fresh, Some(99)),
            None,
            "{text}: not a creature here"
        );
    }
}

#[test]
fn coup_ready_needs_the_ranks_and_the_health() {
    let with_coup = |mut state: GameState, ranks: u16| {
        state.character.psms.replace_category(
            PsmCategory::CombatManeuver,
            &[PsmLine {
                mnemonic: "coupdegrace".to_owned(),
                display_name: "Coup de Grace".to_owned(),
                ranks: PsmRanks {
                    ranks,
                    max: 5,
                    known_by_bold: false,
                },
                kind: "Attack".to_owned(),
                category: String::new(),
            }],
        );
        state
    };
    let low = mastodon(&[("health", "5"), ("maxhealth", "100")]);
    assert_eq!(
        reads("coup_ready", &low, Some(42)),
        None,
        "no maneuver table yet"
    );
    // Rank 2 on a creature that can act: 2 x 5% of 100 is 10 hit points.
    let trained = with_coup(low, 2);
    assert_eq!(reads("coup_ready", &trained, Some(42)), Some(true));
    let high = with_coup(mastodon(&[("health", "50"), ("maxhealth", "100")]), 2);
    assert_eq!(reads("coup_ready", &high, Some(42)), Some(false));
    // Health the game never stated is a guess, and a guess does not coup.
    let unstated = with_coup(
        tagged(at(1_000), 6, "thing", "strange thing", &[("hostile", "1")]),
        5,
    );
    assert_eq!(reads("coup_ready", &unstated, Some(6)), None);
}

#[test]
fn an_effect_is_read_in_its_own_dialog_by_its_name() {
    let mut state = at(1_000);
    assert_eq!(
        reads("buff \"Rapid Fire\"", &state, None),
        None,
        "no Buffs seen"
    );
    effect(&mut state, "1", "Buffs", "Rapid Fire", 1_060);
    assert_eq!(reads("buff \"Rapid Fire\"", &state, None), Some(true));
    assert_eq!(
        reads("buff \"rapid\"", &state, None),
        Some(true),
        "a prefix, any case"
    );
    assert_eq!(
        reads("buff \"Fire\"", &state, None),
        Some(false),
        "not the middle"
    );
    assert_eq!(
        reads("spell \"Rapid Fire\"", &state, None),
        None,
        "no Active Spells seen"
    );
    effect(&mut state, "2", "Active Spells", "Celerity", 900);
    assert_eq!(
        reads("spell \"Celerity\"", &state, None),
        Some(false),
        "expired"
    );
    effect(&mut state, "3", "Cooldowns", "Burst of Swiftness", 1_030);
    assert_eq!(
        reads("!cooldown \"Burst of Swiftness\"", &state, None),
        Some(false)
    );
    effect(&mut state, "4", "Debuffs", "Confused", 1_010);
    assert_eq!(reads("debuff \"Confused\"", &state, None), Some(true));
    assert_eq!(
        reads("buff \"Confused\"", &state, None),
        Some(false),
        "wrong dialog"
    );
}

#[test]
fn targets_are_counted_as_valid_ones() {
    let hostile: &[(&str, &str)] = &[("hostile", "1")];
    let two = herd(&[(42, hostile), (43, hostile)]);
    assert_eq!(reads("targets_at_least 2", &two, None), Some(true));
    assert_eq!(reads("targets_at_least 3", &two, None), Some(false));
    assert_eq!(reads("targets_at_most 1", &two, None), Some(false));
    let with_dead = herd(&[(42, hostile), (43, hostile), (44, &[("dead", "1")])]);
    assert_eq!(
        reads("targets_at_most 2", &with_dead, None),
        Some(true),
        "a dead one is not a target"
    );
    assert_eq!(reads("targets_at_least 3", &with_dead, None), Some(false));
}

#[test]
fn once_and_every_read_what_was_sent_in_this_room() {
    let state = at(100);
    let mut used = Used::new();
    let facts = |used: &Used, target, second| -> Vec<Option<bool>> {
        let state = at(second);
        let facts = Facts {
            state: &state,
            target: Some(target),
            tags: None,
            used: Some(used),
            step: "fire (once)",
        };
        ["once", "once_here", "every 10"]
            .iter()
            .map(|word| one(word).and_then(|c| c.holds(&facts)))
            .collect()
    };
    assert_eq!(facts(&used, 1, 100), [Some(true); 3], "nothing sent yet");
    used.record("fire (once)", Some(1), state.game_time_now());
    assert_eq!(
        facts(&used, 1, 105),
        [Some(false), Some(false), Some(false)]
    );
    assert_eq!(
        facts(&used, 2, 110),
        [Some(true), Some(false), Some(true)],
        "another target, ten seconds on"
    );
    used.record("hide", Some(1), Some(110));
    assert_eq!(
        facts(&used, 2, 110)[0],
        Some(true),
        "another step's record is its own"
    );
    used.clear();
    assert_eq!(facts(&used, 1, 111), [Some(true); 3], "a new room");
}

/// The engine sends a `once` step at a creature one time, and again in a
/// new room.
#[test]
fn the_engine_keeps_the_record_and_forgets_it_on_moving() {
    let profile = Profile::parse(
        r#"
targets = [{ any = true, routine = "a" }]
[rooms]
hunting = 10
[routines]
a = ["fire (once)", "hide"]
"#,
    )
    .unwrap();
    let mut hunt = Hunt::new(profile, 1);
    let mut state = mastodon(&[]);
    state.room.id = Some("10".to_owned());
    state.status.set("standing", true);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state.targeting.read("#42", None);
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    let mut sent = Vec::new();
    for _ in 0..4 {
        if let Said::Send { line, .. } = hunt.tick(&state, here, state.game_time_now()) {
            sent.push(line);
        }
    }
    assert_eq!(
        sent,
        ["fire #42", "hide", "hide", "hide"],
        "fire once at #42"
    );
    // The same creature, one room on: the room's record is gone.
    state.room.id = Some("11".to_owned());
    state.targeting.read("#42", None);
    let mut moved = Vec::new();
    for _ in 0..2 {
        if let Said::Send { line, .. } = hunt.tick(&state, here, state.game_time_now()) {
            moved.push(line);
        }
    }
    assert!(
        moved.contains(&"fire #42".to_owned()),
        "fired again after moving: {moved:?}"
    );
}
