//! Ownership and lineage at the state-machine level: `holy_weapon_release_spec.rb`
//! (6 cases) and `processor_inbound_spec.rb`'s unowned-tick, foreign-latch,
//! multi-target and spawn-tree groups.

#![allow(clippy::too_many_lines)]

mod fsm_harness;

use cena_model::state::combat::event::Confidence;
use cena_model::{OutcomeKind, ResolutionKind};
use fsm_harness::{bolded, dmg, flares, names, parse, tid};

mod unowned_effect_tick {
    use super::*;

    fn skald() -> String {
        bolded(556_001, "skald", "a grim gigas skald")
    }

    #[test]
    fn flags_a_lone_pestilence_tick_as_unowned() {
        let f = parse(&[
            &format!(
                "Boils rupture all over {} causing 54 points of damage!",
                skald()
            ),
            "   ... 10 points of damage!",
        ]);
        let e = &f.events[0];
        assert_eq!(e.name, "pestilence");
        assert!(e.unowned && !e.is_ours());
        assert_eq!(tid(e), Some(556_001));
        assert!(dmg(e).contains(&54));
    }

    #[test]
    fn does_not_flag_a_pestilence_tick_that_follows_our_own_cast_in_blob() {
        let f = parse(&[
            &format!(
                "You exhale a virulent green mist toward {}, instantly infecting it!",
                skald()
            ),
            &format!(
                "Boils rupture all over {} causing 54 points of damage!",
                skald()
            ),
        ]);
        assert!(f.events.iter().all(|e| !e.unowned));
    }
}

mod foreign_aoe_latch {
    use super::*;

    fn warg() -> String {
        bolded(557_001, "warg", "a niveous giant warg")
    }
    fn maiden() -> String {
        bolded(557_002, "shield-maiden", "a brawny gigas shield-maiden")
    }

    #[test]
    fn attributes_an_anonymous_foreign_aoe_swing_chain_to_the_opener() {
        let f = parse(&[
            "Heavenscent wheels her star overhead before slamming it around in a wide arc to pulverize her foes!",
            "[SMR result: 281 (Open d100: 69, Bonus: 146)]",
            &format!(
                "As Heavenscent attempts to strike with her star, a surge of power flows out of it, through Heavenscent, and leaps out at {}!",
                maiden()
            ),
            &format!("Cloudy wisps swirl about {}.", maiden()),
            &format!(
                "A {} becomes ensnared in thick strands of webbing!",
                maiden()
            ),
            "  AS: +673 vs DS: +333 with AvD: +42 + d100 roll: +25 = +407",
            "   ... and hits for 159 points of damage!",
        ]);
        let damaging: Vec<&_> = f
            .events
            .iter()
            .filter(|e| e.hits.iter().any(|h| h.damage > 0))
            .collect();
        assert!(!damaging.is_empty());
        assert!(damaging.iter().all(|e| e.foreign_caster), "{:?}", names(&f));
    }

    #[test]
    fn clears_the_latch_when_we_act_keeping_our_own_attack_ours() {
        let f = parse(&[
            "Heavenscent wheels her star overhead before slamming it around in a wide arc to pulverize her foes!",
            &format!("You fire a firewheel arrow at {}!", warg()),
            "  AS: +500 vs DS: +200 with AvD: +30 + d100 roll: +40 = +370",
            "   ... and hits for 88 points of damage!",
        ]);
        let fire = f.events.iter().find(|e| e.name == "fire").expect("fire");
        assert!(!fire.foreign_caster);
        assert_eq!(dmg(fire), [88]);
    }

    #[test]
    fn does_not_let_our_dot_cast_on_one_creature_claim_a_foreign_tick_on_another() {
        let f = parse(&[
            &format!(
                "You exhale a virulent green mist toward {}, instantly infecting it!",
                warg()
            ),
            &format!(
                "Boils rupture all over {} causing 54 points of damage!",
                maiden()
            ),
        ]);
        let tick = f
            .events
            .iter()
            .find(|e| e.name == "pestilence" && dmg(e).contains(&54))
            .expect("tick");
        assert!(tick.unowned);
    }

    #[test]
    fn still_marks_our_own_dot_tick_on_the_creature_we_cast_on_as_ours() {
        let f = parse(&[
            &format!(
                "You exhale a virulent green mist toward {}, instantly infecting it!",
                warg()
            ),
            &format!(
                "Boils rupture all over {} causing 44 points of damage!",
                warg()
            ),
        ]);
        let tick = f
            .events
            .iter()
            .find(|e| e.name == "pestilence" && dmg(e).contains(&44))
            .expect("tick");
        assert!(!tick.unowned);
    }
}

#[test]
fn still_switches_targets_across_a_multi_target_aoe() {
    let f = parse(&[
        "You wheel your maul overhead before slamming it around in a wide arc to pulverize your foes!",
        "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
        "   ... and hits for 30 points of damage!",
        &format!("{} is struck!", bolded(500, "orc", "A greater orc")),
        "  AS: +400 vs DS: +220 with AvD: +30 + d100 roll: +60 = +270",
        "   ... and hits for 25 points of damage!",
        &format!("{} is struck!", bolded(501, "troll", "A cave troll")),
        "  AS: +400 vs DS: +210 with AvD: +30 + d100 roll: +40 = +260",
        "   ... and hits for 20 points of damage!",
    ]);
    let mut ids: Vec<i64> = f.events.iter().filter_map(tid).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids, [500, 501]);
}

#[test]
fn captures_the_crit_on_the_hit_with_its_stun() {
    let orc = bolded(7777, "orc", "a greater orc");
    let f = parse(&[
        &format!("You swing a slim short sword at {orc}!"),
        "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
        "   ... and hits for 30 points of damage!",
        "   Smack to the eye bursts blood vessels.",
    ]);
    let crit = f.events[0].hits[0].crit.as_ref().expect("crit");
    assert_eq!(crit.location.as_str(), "left_eye");
    assert_eq!(crit.stunned, 3);
    assert_eq!(crit.line, 3);
}

mod spawn_tree_lineage {
    use super::*;

    #[test]
    fn makes_a_lone_swing_its_own_root_with_no_parent() {
        let orc = bolded(4242, "orc", "a greater orc");
        let f = parse(&[
            &format!("You swing a slim short sword at {orc}!"),
            "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
            "   ... and hits for 30 points of damage!",
        ]);
        let e = &f.events[0];
        assert_eq!(
            (e.root, e.parent, e.parent_confidence),
            (Some(0), None, None)
        );
    }

    #[test]
    fn links_blinks_bracketed_spawned_cast_to_the_initiating_shot() {
        let orc = bolded(4242, "orc", "a greater orc");
        let f = parse(&[
            &format!("You swing a glowbark long bow at {orc}!"),
            "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
            "   ... and hits for 30 points of damage!",
            "Your glowbark long bow suddenly lights up with hundreds of tiny blue sparks!",
            "You close your eyes in a moment of intense concentration, channeling the pure natural power of your surroundings.",
            &format!("The surroundings advance upon {orc} with relentless fury!"),
            "  CS: +484 - TD: +293 + CvA: +25 + d100: +93 == +309",
            "  Warding failed!",
            &format!("{orc} is struck by a sharp piece of mist-covered debris!"),
            "   ... 56 points of damage!",
            "As swiftly as the chaos came to be, it recedes again into the surroundings.",
        ]);
        let child = f
            .events
            .iter()
            .find(|e| e.parent.is_some())
            .expect("a bracketed child");
        assert_eq!(
            (child.parent, child.root, child.parent_confidence),
            (Some(0), Some(0), Some(Confidence::Bracket))
        );
        assert_eq!(
            child.parent_flare.as_ref().map(|p| p.flare.as_str()),
            Some("blink")
        );
    }

    #[test]
    fn does_not_chain_an_unbracketed_follow_on_shot() {
        let orc = bolded(4242, "orc", "a greater orc");
        let f = parse(&[
            &format!("You fire a faewood arrow at {orc}!"),
            "   ... and hits for 20 points of damage!",
            &format!("You fire a faewood arrow at {orc}!"),
            "   ... and hits for 25 points of damage!",
        ]);
        let born: Vec<&_> = f.events.iter().filter(|e| e.is_attack_born()).collect();
        assert!(born.len() >= 2);
        assert!(born.iter().all(|e| e.parent.is_none()));
        for (i, e) in f.events.iter().enumerate() {
            assert_eq!(e.root, Some(i));
        }
    }

    #[test]
    fn shares_one_root_across_a_multi_target_aoe_hitting_different_creatures() {
        let orc = bolded(501, "orc", "a greater orc");
        let troll = bolded(502, "troll", "a cave troll");
        let f = parse(&[
            &format!("You fire a faewood arrow at {orc}!"),
            "   ... and hits for 20 points of damage!",
            // same attack, next creature: a target switch that is also an attack def
            &format!("You fire a faewood arrow at {troll}!"),
            "   ... and hits for 15 points of damage!",
        ]);
        let born: Vec<&_> = f.events.iter().filter(|e| e.is_attack_born()).collect();
        assert_eq!(born.len(), 2);
        assert_eq!(
            born.iter().map(|e| tid(e)).collect::<Vec<_>>(),
            [Some(501), Some(502)]
        );
        assert_eq!(
            born.iter().map(|e| e.root).collect::<Vec<_>>(),
            [Some(0), Some(0)]
        );
    }

    #[test]
    fn does_not_graft_a_foreign_attack_onto_our_spawn_tree() {
        let orc = bolded(501, "orc", "a greater orc");
        let troll = bolded(502, "troll", "a cave troll");
        let f = parse(&[
            &format!("You fire a faewood arrow at {orc}!"),
            "   ... and hits for 20 points of damage!",
            &format!("Heavenscent swings a warhammer at {troll}!"),
            "   ... and hits for 33 points of damage!",
        ]);
        let ours = f
            .events
            .iter()
            .position(|e| e.is_attack_born() && !e.foreign_caster)
            .expect("ours");
        let foreign = f
            .events
            .iter()
            .position(|e| e.is_attack_born() && e.foreign_caster)
            .expect("foreign");
        let fe = &f.events[foreign];
        assert_eq!(fe.root, Some(foreign));
        assert_ne!(fe.root, Some(ours));
        assert!(fe.parent.is_none());
    }
}

mod holy_weapon_release {
    use super::*;

    const TROLL: &str = r#"<pushBold/>a <a exist="212657781" noun="troll">bog troll</a><popBold/>"#;
    const MACE: &str = r#"<a exist="212333157" noun="mace">mithril mace</a>"#;

    /// `(name, roll kinds, damages, parent's name)`.
    type Row = (String, Vec<ResolutionKind>, Vec<u32>, Option<String>);

    fn summary(f: &cena_model::state::combat::ChunkFacts) -> Vec<Row> {
        f.events
            .iter()
            .map(|e| {
                (
                    e.name.clone(),
                    e.resolutions.iter().map(|r| r.kind).collect(),
                    dmg(e),
                    e.parent.map(|p| f.events[p].name.clone()),
                )
            })
            .collect()
    }

    fn pummel_with_verdict() -> Vec<String> {
        vec![
            format!(
                "You take a menacing step toward {TROLL}, sweeping your {MACE} out low to your side in your advance."
            ),
            "[SMR result: 165 (Open d100: 43, Bonus: 65)]".into(),
            format!("With deliberate brutality, you bring your {MACE} around to pummel {TROLL}!"),
            format!(
                "As you attempt to strike with your {MACE}, it sends a surge of power through you that quickly leaps out at {TROLL}!"
            ),
            format!("Violet flames erupt from beneath {TROLL}."),
            "  CS: +149 - TD: +120 + CvA: +17 + d100: +85 == +131".into(),
            "  Warding failed!".into(),
            format!("A column of seething violet flame envelops {TROLL} in its searing embrace!"),
            "   ... 20 points of damage!".into(),
            "  AS: +273 vs DS: +80 with AvD: +35 + d100 roll: +2 = +230".into(),
            "   ... and hit for 79 points of damage!".into(),
        ]
    }
    fn release_then_verdict() -> Vec<String> {
        vec![
            format!(
                "As you attempt to strike with your {MACE}, it sends a surge of power through you that quickly leaps out at {TROLL}!"
            ),
            format!("Violet flames erupt from beneath {TROLL}."),
            "  CS: +149 - TD: +120 + CvA: +17 + d100: +85 == +131".into(),
            "  Warding failed!".into(),
            format!("A column of seething violet flame envelops {TROLL} in its searing embrace!"),
            "   ... 20 points of damage!".into(),
        ]
    }
    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }
    fn s(name: &str, kinds: &[ResolutionKind], d: &[u32], parent: Option<&str>) -> Row {
        (
            name.into(),
            kinds.to_vec(),
            d.to_vec(),
            parent.map(str::to_owned),
        )
    }
    use ResolutionKind::{AsDs, CsTd, Smr};

    #[test]
    fn parents_the_released_spell_to_the_swing_and_returns_the_swing_its_roll() {
        let f = parse(&refs(&pummel_with_verdict()));
        assert_eq!(
            summary(&f),
            [
                s("pummel", &[Smr, AsDs], &[79], None),
                s("templars_verdict", &[CsTd], &[20], Some("pummel"))
            ]
        );
        assert_eq!(f.events[1].parent_confidence, Some(Confidence::Count));
        assert_eq!(
            f.events[1].parent_flare.as_ref().map(|p| p.flare.as_str()),
            Some("weapon_cast")
        );
    }

    #[test]
    fn does_not_read_a_later_swing_in_the_chunk_as_another_released_spell() {
        let mut lines = pummel_with_verdict();
        lines.push(format!("You swing a perfect {MACE} at {TROLL}!"));
        lines.push("  AS: +280 vs DS: +90 with AvD: +35 + d100 roll: +50 = +275".into());
        lines.push("   ... and hit for 40 points of damage!".into());
        let f = parse(&refs(&lines));
        assert_eq!(
            summary(&f),
            [
                s("pummel", &[Smr, AsDs], &[79], None),
                s("templars_verdict", &[CsTd], &[20], Some("pummel")),
                s("attack", &[AsDs], &[40], None),
            ]
        );
        assert!(!f.events[2].is_released());
    }

    #[test]
    fn lets_a_swing_with_a_linked_weapon_claim_the_pending_release_and_adopt_the_cast() {
        let mut lines = release_then_verdict();
        lines.push(format!("You swing a perfect {MACE} at {TROLL}!"));
        lines.push("  AS: +273 vs DS: +80 with AvD: +35 + d100 roll: +2 = +230".into());
        lines.push("   ... and hit for 79 points of damage!".into());
        let f = parse(&refs(&lines));
        assert_eq!(
            summary(&f),
            [
                s("attack", &[AsDs], &[79], None),
                s("templars_verdict", &[CsTd], &[20], Some("attack"))
            ]
        );
        assert_eq!(flares(&f.events[0]), ["weapon_cast"]);
        assert_eq!(f.events[0].weapon.as_deref(), Some("perfect mithril mace"));
    }

    #[test]
    fn consumes_the_release_on_the_release_before_swing_path_so_a_later_bolt_is_not_adopted() {
        let mut lines = release_then_verdict();
        lines.push(format!("You swing a perfect mithril mace at {TROLL}!"));
        lines.push("  AS: +273 vs DS: +80 with AvD: +35 + d100 roll: +2 = +230".into());
        lines.push("   ... and hit for 79 points of damage!".into());
        lines.push(format!("You hurl a fiery bolt at {TROLL}!"));
        lines.push("  AS: +120 vs DS: +80 with AvD: +25 + d100 roll: +60 = +125".into());
        lines.push("   ... and hit for 15 points of damage!".into());
        let f = parse(&refs(&lines));
        assert_eq!(
            summary(&f),
            [
                s("attack", &[AsDs], &[79], None),
                s("templars_verdict", &[CsTd], &[20], Some("attack")),
                s("bolt", &[AsDs], &[15], None),
            ]
        );
        assert!(!f.events[2].is_released());
    }

    #[test]
    fn holds_a_release_that_follows_a_settled_swing_for_the_next_swing() {
        let mut lines = vec![
            format!("You swing a perfect mithril mace at {TROLL}!"),
            "  AS: +260 vs DS: +80 with AvD: +35 + d100 roll: +10 = +225".into(),
            "   ... and hit for 10 points of damage!".into(),
        ];
        lines.extend(release_then_verdict());
        lines.push(format!("You swing a perfect mithril mace at {TROLL}!"));
        lines.push("  AS: +273 vs DS: +80 with AvD: +35 + d100 roll: +2 = +230".into());
        lines.push("   ... and hit for 79 points of damage!".into());
        let f = parse(&refs(&lines));
        assert_eq!(
            summary(&f),
            [
                s("attack", &[AsDs], &[10], None),
                s("attack", &[AsDs], &[79], None),
                s("templars_verdict", &[CsTd], &[20], Some("attack")),
            ]
        );
        assert!(f.events[0].flares.is_empty());
        assert_eq!(flares(&f.events[1]), ["weapon_cast"]);
        assert_eq!(f.events[2].parent, Some(1));
    }

    #[test]
    fn lets_a_released_bolt_keep_its_own_as_ds_and_damage() {
        let f = parse(&[
            &format!(
                "You take a menacing step toward {TROLL}, sweeping your {MACE} out low to your side in your advance."
            ),
            "[SMR result: 165 (Open d100: 43, Bonus: 65)]",
            &format!("With deliberate brutality, you bring your {MACE} around to pummel {TROLL}!"),
            &format!(
                "As you attempt to strike with your {MACE}, it sends a surge of power through you that quickly leaps out at {TROLL}!"
            ),
            &format!("You hurl a fiery bolt at {TROLL}!"),
            "  AS: +120 vs DS: +80 with AvD: +25 + d100 roll: +60 = +125",
            "   ... and hit for 15 points of damage!",
            "  AS: +273 vs DS: +80 with AvD: +35 + d100 roll: +2 = +230",
            "   ... and hit for 79 points of damage!",
        ]);
        assert_eq!(
            summary(&f),
            [
                s("pummel", &[Smr, AsDs], &[79], None),
                s("bolt", &[AsDs], &[15], Some("pummel"))
            ]
        );
        assert!(
            f.events[0].outcomes.is_empty() || f.events[0].outcomes.contains(&OutcomeKind::Hit)
        );
    }
}
