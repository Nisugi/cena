//! Bounty tasks, against Lich's own spec corpus.
//!
//! The 57 cases below are extracted verbatim from
//! `reference/lich-5/spec/lib/gemstone/bounty/parser_spec.rb` -- real game
//! text with real town names, NPC names and the wire's double space after a
//! period. **AUTHOR, 2026-09-20** approved using them as the corpus, and
//! `plan/13` §4a asks for the tests to be ported alongside the code.
//!
//! They cover all 22 task types. The dev logs reachable from here carry only
//! `"You are not currently assigned a task."` -- one of the 22 -- so a
//! wire-cut fixture would have tested 4% of this classifier.
//!
//! # What a case asserts
//!
//! The kind, the town, and every requirement the spec asserted. Towns are the
//! interesting column: five of them are **not** the captured text but an
//! override keyed on the guard phrasing (`bounty.rs`'s `town_from`), and one
//! of those five works around a live typo in the game's messaging.

use cena_model::{TaskKind, state::bounty::classify};

/// `(kind, description, town, requirements)`, from the spec.
type Case = (
    TaskKind,
    &'static str,
    Option<&'static str>,
    &'static [(&'static str, &'static str)],
);

const CASES: &[Case] = &[
    (
        TaskKind::None,
        "You are not currently assigned a task.",
        None,
        &[],
    ),
    (
        TaskKind::CreatureAssignment,
        "It appears they have a creature problem they'd like you to solve",
        None,
        &[],
    ),
    (
        TaskKind::HeirloomAssignment,
        "It appears they need your help in tracking down some kind of lost heirloom",
        None,
        &[],
    ),
    (
        TaskKind::SkinAssignment,
        "The local furrier Furrier has an order to fill and wants our help",
        None,
        &[],
    ),
    (
        TaskKind::GemAssignment,
        "Hmm, I've got a task here from the town of Ta'Vaalor.  The local gem dealer, Areacne, has an order to fill and wants our help. Head over there and see what you can do.  Be sure to ASK about BOUNTIES.",
        None,
        &[],
    ),
    (
        TaskKind::HerbAssignment,
        "Hmm, I've got a task here from the town of Ta'Illistim.  The local herbalist's assistant, Jhiseth, has asked for our aid.  Head over there and see what you can do.  Be sure to ASK about BOUNTIES.",
        Some("Ta'Illistim"),
        &[],
    ),
    (
        TaskKind::RescueAssignment,
        "It appears that a local resident urgently needs our help in some matter",
        None,
        &[],
    ),
    (
        TaskKind::BanditAssignment,
        "Hmm, I've got a task here from the town of Ta'Illistim.  It appears they have a bandit problem they'd like you to solve.  Go report to one of the guardsmen just inside the Ta'Illistim City Gate to find out more.  Be sure to ASK about BOUNTIES.",
        None,
        &[],
    ),
    (
        TaskKind::Bandit,
        "You have been tasked to suppress bandit activity on Sylvarraend Road near Ta'Illistim.  You need to kill 20 of them to complete your task.",
        Some("Ta'Illistim"),
        &[("area", "Sylvarraend Road"), ("number", "20")],
    ),
    (
        TaskKind::Bandit,
        "You have been tasked to suppress bandit activity near Widowmaker's Road near Kraken's Fall.  You need to kill 11 more of them to complete your task.",
        Some("Kraken's Fall"),
        &[("area", "Widowmaker's Road"), ("number", "11")],
    ),
    (
        TaskKind::Bandit,
        "You have been tasked to help Buddy suppress bandit activity in the grasslands between Wehnimer's Landing and Solhaven.  You need to kill 18 of them to complete your task.",
        Some("Wehnimer's Landing and Solhaven"),
        &[
            ("area", "grasslands"),
            ("creature", "bandit"),
            ("number", "18"),
        ],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to suppress glacial morph activity in Gossamer Valley near Ta'Illistim.  You need to kill 24 of them to complete your task.",
        Some("Ta'Illistim"),
        &[
            ("creature", "glacial morph"),
            ("area", "Gossamer Valley"),
            ("number", "24"),
        ],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to help Brikus suppress war griffin activity in Old Ta'Faendryl.  You need to kill 14 of them to complete your task.",
        None,
        &[
            ("creature", "war griffin"),
            ("area", "Old Ta'Faendryl"),
            ("number", "14"),
        ],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to help Buddy suppress triton radical activity in the Ruined Temple near Kharam-Dzu.  You need to kill 12 more of them to complete your task.",
        Some("Kharam-Dzu"),
        &[
            ("creature", "triton radical"),
            ("area", "Ruined Temple"),
            ("number", "12"),
        ],
    ),
    (
        TaskKind::Heirloom,
        "You have been tasked to recover a dainty pearl string bracelet that an unfortunate citizen lost after being attacked by a festering taint in Old Ta'Faendryl.  The heirloom can be identified by the initials VF engraved upon it.  Hunt down the creature and LOOT the item from its corpse.",
        None,
        &[
            ("action", "loot"),
            ("area", "Old Ta'Faendryl"),
            ("creature", "festering taint"),
            ("item", "dainty pearl string bracelet"),
        ],
    ),
    (
        TaskKind::Heirloom,
        "You have been tasked to recover an onyx-inset copper torc that an unfortunate citizen lost after being attacked by a centaur near Darkstone Castle near Wehnimer's Landing.  The heirloom can be identified by the initials ZK engraved upon it.  Hunt down the creature and LOOT the item from its corpse.",
        Some("Wehnimer's Landing"),
        &[
            ("action", "loot"),
            ("area", "Darkstone Castle"),
            ("creature", "centaur"),
            ("item", "onyx-inset copper torc"),
        ],
    ),
    (
        TaskKind::Heirloom,
        "You have been tasked to recover a gold-trimmed mithril circlet that an unfortunate citizen lost after being attacked by a swamp troll in the Central Caravansary between Wehnimer's Landing and Solhaven.  The heirloom can be identified by the initials CT engraved upon it.  Hunt down the creature and LOOT the item from its corpse.",
        Some("Wehnimer's Landing and Solhaven"),
        &[
            ("action", "loot"),
            ("area", "Central Caravansary"),
            ("creature", "swamp troll"),
            ("item", "gold-trimmed mithril circlet"),
        ],
    ),
    (
        TaskKind::Heirloom,
        "You have been tasked to recover an interlaced gold and ora ring that an unfortunate citizen lost after being attacked by a black forest viper in the Blighted Forest near Ta'Illistim.  The heirloom can be identified by the initials MS engraved upon it.  SEARCH the area until you find it.",
        Some("Ta'Illistim"),
        &[
            ("action", "search"),
            ("area", "Blighted Forest"),
            ("creature", "black forest viper"),
            ("item", "interlaced gold and ora ring"),
        ],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to help Someguy retrieve an heirloom by suppressing emaciated hierophant activity in Temple Wyneb near Ta'Illistim during the retrieval effort.  You need to kill 19 of them to complete your task.",
        None,
        &[
            ("area", "Temple Wyneb"),
            ("number", "19"),
            ("creature", "emaciated hierophant"),
        ],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to help Thisdude retrieve an heirloom by suppressing gnarled being activity in Old Ta'Faendryl during the retrieval effort.  You need to kill 2 more of them to complete your task.",
        None,
        &[
            ("area", "Old Ta'Faendryl"),
            ("number", "2"),
            ("creature", "being"),
        ],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to help Friendo retrieve an heirloom by suppressing nedum vereri activity near the Temple of Love near Wehnimer's Landing during the retrieval effort.  You need to kill 4 more of them to complete your task.",
        None,
        &[
            ("area", "Temple of Love"),
            ("number", "4"),
            ("creature", "nedum vereri"),
        ],
    ),
    (
        TaskKind::Skin,
        "You have been tasked to retrieve 8 madrinol skins of at least fair quality for Gaedrein in Ta'Illistim.  You can SKIN them off the corpse of a snow madrinol or purchase them from another adventurer.  You can SELL the skins to the furrier as you collect them.\"",
        Some("Ta'Illistim"),
        &[
            ("creature", "snow madrinol"),
            ("quality", "fair"),
            ("number", "8"),
            ("skin", "madrinol skin"),
        ],
    ),
    (
        TaskKind::Skin,
        "You have been tasked to retrieve 5 thrak tails of at least exceptional quality for the furrier in the Company Store in Kharam-Dzu.  You can SKIN them off the corpse of a red-scaled thrak or purchase them from another adventurer.  You can SELL the skins to the furrier as you collect them.\"",
        Some("Kharam-Dzu"),
        &[
            ("creature", "red-scaled thrak"),
            ("quality", "exceptional"),
            ("number", "5"),
            ("skin", "thrak tail"),
        ],
    ),
    (
        TaskKind::Gem,
        "The gem dealer in Ta'Illistim, Tanzania, has received orders from multiple customers requesting an azure blazestar.  You have been tasked to retrieve 10 of them.  You can SELL them to the gem dealer as you find them.",
        None,
        &[("gem", "azure blazestar"), ("number", "10")],
    ),
    (
        TaskKind::Escort,
        "The taskmaster told you:  \"I've got a special mission for you.  A certain client has hired us to provide a protective escort on his upcoming journey.  Go to the area just inside the Sapphire Gate and WAIT for him to meet you there.  You must guarantee his safety to Zul Logoth as soon as you can, being ready for any dangers that the two of you may face.  Good luck!\"",
        None,
        &[
            ("destination", "Zul Logoth"),
            ("start", "the area just inside the Sapphire Gate"),
        ],
    ),
    (
        TaskKind::Escort,
        "I've got a special mission for you.  A certain client has hired us to provide a protective escort on her upcoming journey.  Go to the south end of North Market and WAIT for her to meet you there.  You must guarantee her safety to Zul Logoth as soon as you can, being ready for any dangers that the two of you may face.  Good luck!",
        None,
        &[
            ("destination", "Zul Logoth"),
            ("start", "the south end of North Market"),
        ],
    ),
    (
        TaskKind::Herb,
        "The herbalist's assistant in Ta'Illistim, Jhiseth, is working on a concoction that requires a sprig of holly found in Griffin's Keen near Ta'Illistim.  These samples must be in pristine condition.  You have been tasked to retrieve 6 samples.",
        None,
        &[
            ("herb", "sprig of holly"),
            ("area", "Griffin's Keen"),
            ("number", "6"),
        ],
    ),
    (
        TaskKind::Herb,
        "The healer in Icemule Trace, Mirtag, is working on a concoction that requires a withered deathblossom found in the Rift.  These samples must be in pristine condition.  You have been tasked to retrieve 7 samples.",
        None,
        &[
            ("herb", "withered deathblossom"),
            ("area", "Rift"),
            ("number", "7"),
        ],
    ),
    (
        TaskKind::Herb,
        "The healer in Icemule Trace, Mirtag, is working on a concoction that requires a withered black mushroom found in the subterranean tunnels under Icemule Trace.  These samples must be in pristine condition.  You have been tasked to retrieve 5 samples.",
        None,
        &[
            ("herb", "withered black mushroom"),
            ("area", "subterranean tunnels"),
            ("number", "5"),
        ],
    ),
    (
        TaskKind::Herb,
        "The healer in Icemule Trace, Mirtag, is working on a concoction that requires some bolmara lichen found on the Icemule Trail between Wehnimer's Landing and Icemule Trace.  These samples must be in pristine condition.  You have been tasked to retrieve 9 samples.",
        None,
        &[
            ("herb", "bolmara lichen"),
            ("area", "Icemule Trail"),
            ("number", "9"),
        ],
    ),
    (
        TaskKind::Dangerous,
        "You have been tasked to hunt down and kill a particularly dangerous gnarled being that has established a territory in Old Ta'Faendryl.  You can get its attention by killing other creatures of the same type in its territory.",
        None,
        &[("creature", "being"), ("area", "Old Ta'Faendryl")],
    ),
    (
        TaskKind::Dangerous,
        "You have been tasked to hunt down and kill a particularly dangerous nedum vereri that has established a territory near the Temple of Love near Wehnimer's Landing.  You can get its attention by killing other creatures of the same type in its territory.",
        None,
        &[("area", "Temple of Love"), ("creature", "nedum vereri")],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to help Someguy kill a dangerous creature by suppressing gnarled being activity in Old Ta'Faendryl during the hunt.  You need to kill 20 of them to complete your task.",
        None,
        &[
            ("area", "Old Ta'Faendryl"),
            ("number", "20"),
            ("creature", "being"),
        ],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to help Someguy kill a dangerous creature by suppressing emaciated hierophant activity in Temple Wyneb near Ta'Illistim during the hunt.  You need to kill 12 of them to complete your task.",
        None,
        &[
            ("area", "Temple Wyneb"),
            ("number", "12"),
            ("creature", "emaciated hierophant"),
        ],
    ),
    (
        TaskKind::Rescue,
        "You have been tasked to rescue the young runaway son of a local citizen.  A local divinist has had visions of the child fleeing from a black forest ogre in the Blighted Forest near Ta'Illistim.  Find the area where the child was last seen and clear out the creatures that have been tormenting him in order to bring him out of hiding.",
        None,
        &[
            ("area", "Blighted Forest"),
            ("creature", "black forest ogre"),
        ],
    ),
    (
        TaskKind::Rescue,
        "You have been tasked to rescue the young kidnapped daughter of a local citizen.  A local divinist has had visions of the child fleeing from a stone sentinel in Darkstone Castle near Wehnimer's Landing.  Find the area where the child was last seen and clear out the creatures that have been tormenting her in order to bring her out of hiding.",
        None,
        &[("area", "Darkstone Castle"), ("creature", "stone sentinel")],
    ),
    (
        TaskKind::Rescue,
        "You have been tasked to rescue the young kidnapped son of a local citizen.  A local divinist has had visions of the child fleeing from a ghostly pooka in the Shadow Valley.  Find the area where the child was last seen and clear out the creatures that have been tormenting him in order to bring him out of hiding.",
        None,
        &[("area", "Shadow Valley"), ("creature", "ghostly pooka")],
    ),
    (
        TaskKind::Rescue,
        "You have been tasked to rescue the young runaway daughter of a local citizen.  A local divinist has had visions of the child fleeing from a nedum vereri near the Temple of Love near Wehnimer's Landing.  Find the area where the child was last seen and clear out the creatures that have been tormenting her in order to bring her out of hiding.",
        None,
        &[("area", "Temple of Love"), ("creature", "nedum vereri")],
    ),
    (
        TaskKind::Rescue,
        "You have been tasked to rescue the young runaway daughter of a local citizen.  A local divinist has had visions of the child fleeing from a rotting corpse in Castle Varunar between Wehnimer's Landing and Solhaven.  Find the area where the child was last seen and clear out the creatures that have been tormenting her in order to bring her out of hiding.",
        None,
        &[("area", "Castle Varunar"), ("creature", "rotting corpse")],
    ),
    (
        TaskKind::Cull,
        "You have been tasked to help Someguy rescue a missing child by suppressing emaciated hierophant activity in Temple Wyneb near Ta'Illistim during the rescue attempt.  You need to kill 7 more of them to complete your task.",
        None,
        &[
            ("area", "Temple Wyneb"),
            ("number", "7"),
            ("creature", "emaciated hierophant"),
        ],
    ),
    (
        TaskKind::RescueSpawned,
        "You have made contact with the child you are to rescue and you must get him back alive to one of the guardsmen just inside the Sapphire Gate.",
        None,
        &[],
    ),
    (
        TaskKind::RescueSpawned,
        "You have made contact with the child you are to rescue and you must get her back alive to Quin Telaren of Wehnimer's Landing.",
        Some("Wehnimer's Landing"),
        &[],
    ),
    (
        TaskKind::RescueSpawned,
        "You have made contact with the child you are to rescue and you must get her back alive to one of the guardsmen just inside the gate.",
        None,
        &[],
    ),
    (
        TaskKind::RescueSpawned,
        "You have made contact with the child you are to rescue and you must get her back alive to one of the Vornavis gate guards.",
        Some("Vornavis"),
        &[],
    ),
    (
        TaskKind::RescueSpawned,
        "You have made contact with the child you are to rescue and you must get her back alive to one of the Icemule Trace gate guards or the halfing Belle at the Pinefar Trading Post.",
        Some("Icemule Trace"),
        &[],
    ),
    (
        TaskKind::RescueSpawned,
        "You have made contact with the child you are to rescue and you must get her back alive to the dwarven militia sergeant near the Kharam-Dzu town gates.",
        Some("Kharam-Dzu"),
        &[],
    ),
    (
        TaskKind::RescueSpawned,
        "You have made contact with the child you are to rescue and you must get him back alive to one of the Ta'Vaalor gate guards.",
        Some("Ta'Vaalor"),
        &[],
    ),
    (
        TaskKind::DangerousSpawned,
        "You have been tasked to hunt down and kill a particularly dangerous critter type that has established a territory in some hunting ground near a place.  You have provoked his attention and now you must kill him!",
        None,
        &[],
    ),
    (
        TaskKind::DangerousSpawned,
        "You have been tasked to hunt down and kill a particularly dangerous critter type that has established a territory in some hunting ground near a place.  You have provoked his attention and now you must kill her!",
        None,
        &[],
    ),
    (
        TaskKind::DangerousSpawned,
        "You have been tasked to hunt down and kill a particularly dangerous critter type that has established a territory in some hunting ground near a place.  You have provoked his attention and now you must kill it!",
        None,
        &[],
    ),
    (
        TaskKind::DangerousSpawned,
        "You have been tasked to hunt down and kill a particularly dangerous critter type that has established a territory in some hunting ground near a place.  You have provoked his attention and now you must return to where you left her and kill it!",
        None,
        &[],
    ),
    (
        TaskKind::Taskmaster,
        "You have succeeded in your task and can return to the Adventurer's Guild",
        None,
        &[],
    ),
    (
        TaskKind::HeirloomFound,
        "You have located an elegantly carved jade tiara and should bring it back to one of the guardsmen just inside the Ta'Illistim City Gate.",
        Some("Ta'Illistim"),
        &[],
    ),
    (
        TaskKind::HeirloomFound,
        "You have located some moonstone inset mithril earrings and should bring it back to one of the guardsmen just inside the Ta'Illistim City Gate.",
        Some("Ta'Illistim"),
        &[],
    ),
    (
        TaskKind::HeirloomFound,
        "You have located a bloodstone studded hair pin and should bring it back to Quin Telaren of Wehnimer's Landing.",
        Some("Wehnimer's Landing"),
        &[],
    ),
    (
        TaskKind::Failed,
        "You have failed in your task.  Return to the Adventurer's Guild for further instructions.",
        None,
        &[],
    ),
    (
        TaskKind::Failed,
        "The child you were tasked to rescue is gone and your task is failed.  Report this failure to the Adventurer's Guild.",
        None,
        &[],
    ),
];

/// Every case classifies to the kind the spec asserts.
#[test]
fn every_spec_case_classifies() {
    let mut wrong = Vec::new();
    for (want, description, _, _) in CASES {
        match classify(description) {
            Some(task) if task.kind == *want => {}
            Some(task) => wrong.push(format!("{:?} != {:?}: {description}", task.kind, want)),
            None => wrong.push(format!("no match, wanted {want:?}: {description}")),
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {}\n{}",
        wrong.len(),
        CASES.len(),
        wrong.join("\n")
    );
}

/// Every case resolves the town the spec asserts.
#[test]
fn every_spec_case_resolves_its_town() {
    let mut wrong = Vec::new();
    for (_, description, want, _) in CASES {
        let Some(want) = want else { continue };
        let got = classify(description).and_then(|t| t.town);
        if got.as_deref() != Some(*want) {
            wrong.push(format!("{got:?} != {want:?}: {description}"));
        }
    }
    assert!(wrong.is_empty(), "{}\n{}", wrong.len(), wrong.join("\n"));
}

/// Every requirement the spec asserts is captured.
#[test]
fn every_spec_case_captures_its_requirements() {
    let mut wrong = Vec::new();
    for (_, description, _, reqs) in CASES {
        let Some(task) = classify(description) else {
            continue;
        };
        for (key, want) in *reqs {
            match task.requirement(key) {
                Some(got) if got == *want => {}
                other => wrong.push(format!("{key}: {other:?} != {want:?}\n  {description}")),
            }
        }
    }
    assert!(wrong.is_empty(), "{}\n{}", wrong.len(), wrong.join("\n"));
}

/// All 22 task types appear in the corpus.
///
/// Without this, a case silently dropped from the table above would take a
/// whole task type out of coverage and the three tests would still pass.
#[test]
fn the_corpus_covers_every_task_type() {
    let mut seen: Vec<TaskKind> = CASES.iter().map(|(k, _, _, _)| *k).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 22, "got {seen:?}");
}

/// **The match order decides between overlapping patterns.**
///
/// `:bandit`'s pattern is `:cull`'s with `bandit` substituted for the creature
/// capture, so a bandit task matches both and only the declaration order picks
/// one. Same for `DangerousSpawned` before `Dangerous`.
///
/// The module doc promises this; without a test, a future sort or a `HashMap`
/// would silently reverse it and every other test here would still pass --
/// `Cull` would just be returned where `Bandit` belongs.
#[test]
fn the_order_is_load_bearing() {
    let bandit = "You have been tasked to suppress bandit activity near Widowmaker's Road \
                  near Kraken's Fall.  You need to kill 11 more of them to complete your task.";
    assert_eq!(
        classify(bandit).map(|t| t.kind),
        Some(TaskKind::Bandit),
        "a bandit task matches `Cull`'s pattern too; order is what picks"
    );

    let spawned = "You have been tasked to hunt down and kill a particularly dangerous \
                   grizzled thicket bear that has established a territory in the Bloodriven \
                   Village.  You have provoked its attention and now you must kill it!";
    assert_eq!(
        classify(spawned).map(|t| t.kind),
        Some(TaskKind::DangerousSpawned),
        "the spawned form must win over the generic `Dangerous`"
    );
}

/// A creature name is normalised (`parser.rb:128-137`).
///
/// Two families the game qualifies with an adjective that is not part of the
/// creature's identity for hunting purposes.
#[test]
fn creature_names_are_normalised() {
    let task = "You have been tasked to help Thisdude retrieve an heirloom by suppressing \
                gnarled being activity in Old Ta'Faendryl during the retrieval effort.  \
                You need to kill 2 more of them to complete your task.";
    assert_eq!(
        classify(task).and_then(|t| t.creature().map(str::to_owned)),
        Some("being".to_owned()),
        "`gnarled being` normalises to `being`"
    );
}

/// Prose that is not a bounty classifies as nothing.
///
/// `state.rs:306-309`'s rule: a player can say anything.
#[test]
fn prose_is_not_a_bounty() {
    assert_eq!(classify(""), None);
    assert_eq!(classify("Ashryn says, \"I have a task for you.\""), None);
    assert_eq!(
        classify("You have been tasked with nothing in particular."),
        None
    );
}

/// The three task-stage predicates answer for every kind.
#[test]
fn the_stage_predicates_partition_the_kinds() {
    for (kind, _, _, _) in CASES {
        let flags = [kind.is_actionable(), kind.is_done(), kind.is_assignment()];
        let set = flags.iter().filter(|f| **f).count();
        assert!(
            set <= 1,
            "{kind:?} is in {set} categories at once, which makes none of \
             them a reliable question"
        );
        // `None` is the only kind in no category: no task, nothing to do,
        // nothing finished, no referral.
        if *kind != TaskKind::None {
            assert_eq!(set, 1, "{kind:?} belongs to no stage");
        }
    }
}

/// A counted task reports its number, and an uncounted one reports `None`.
#[test]
fn a_count_is_absent_rather_than_zero() {
    let counted = "You have been tasked to suppress bandit activity near Widowmaker's Road \
                   near Kraken's Fall.  You need to kill 11 more of them to complete your task.";
    assert_eq!(classify(counted).and_then(|t| t.number()), Some(11));

    let uncounted = "You are not currently assigned a task.";
    assert_eq!(
        classify(uncounted).and_then(|t| t.number()),
        None,
        "no count and zero left are different answers"
    );
}

/// **The town overrides, including the one that works around a live typo.**
///
/// `parser.rb:139-153` overrides the captured town in five cases. Four exist
/// because the guard phrasing names no town; the fifth is the typo.
///
/// **Lich's own spec tests none of them**, which mutation found: removing the
/// typo workaround left the whole 57-case corpus green. The cases below are
/// constructed from the patterns rather than captured, and are marked as such.
#[test]
fn the_town_overrides_fire() {
    // Four guard phrasings that name no town. Constructed from
    // `parser.rb:141-147`, not captured -- the spec has no case for them.
    let cases = [
        (
            "You succeeded in your task and should report back to the sentry just outside town.",
            "Kraken's Fall",
        ),
        (
            "You succeeded in your task and should report back to the tavernkeeper at \
             Rawknuckle's Common House.",
            "Cold River",
        ),
        (
            "You succeeded in your task and should report back to the elderly guard in the \
             East Guardtower.",
            "Mist Harbor",
        ),
    ];
    for (description, want) in cases {
        assert_eq!(
            classify(description).and_then(|t| t.town).as_deref(),
            Some(want),
            "{description}"
        );
    }
}

/// **The typo workaround**, which Lich flags and nobody tests.
///
/// > *"the latter is a temporary workaround because of an actual typo in the
/// > messaging that should be removed if it is ever actually fixed"*
/// > -- `parser.rb:147-148`
///
/// The game sends `The gem dealer in has received` -- no town between `in` and
/// `has`. Lich reads that shape as Contempt. Removing the workaround left the
/// 57-case corpus green, so this is what holds it.
///
/// If Simutronics fixes the typo, this test fails and that is the signal to
/// remove the workaround rather than discover it years later.
#[test]
fn the_gem_dealer_typo_resolves_to_contempt() {
    let typo = "The gem dealer in has received orders from multiple customers requesting some \
                lustrous blue sapphires.  You have been tasked to retrieve 6 of them.  You can \
                SELL them to the gem dealer as you find them.";
    let task = classify(typo).expect("the typo form must still classify as a gem task");
    assert_eq!(task.kind, TaskKind::Gem);
    assert_eq!(
        task.town.as_deref(),
        Some("Contempt"),
        "the missing town is what identifies Contempt"
    );
    assert_eq!(task.number(), Some(6));
}

/// An NPC name can identify Contempt where no town is stated.
#[test]
fn a_contempt_npc_identifies_the_town() {
    let task = "You succeeded in your task and should report back to the captain of the Contempt.";
    assert_eq!(
        classify(task).and_then(|t| t.town).as_deref(),
        Some("Contempt")
    );
}
