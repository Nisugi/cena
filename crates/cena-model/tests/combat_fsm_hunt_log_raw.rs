//! `processor_inbound_spec.rb`, ported (3 of 3, second half): the hunt-log
//! defs audit continued -- the raw chunk fixtures in
//! `tests/fixtures/combat_raw/`, and the gigas-village group.
//!
//! See `combat_fsm_inbound.rs` for the harness contract.

#![allow(clippy::too_many_lines)]

mod fsm_harness;

use cena_model::state::combat::AttackEvent;
use cena_model::state::combat::event::{Fact, Subject};
use cena_model::{GameState, OutcomeKind, ResolutionKind, StatusName};
use fsm_harness::{bolded, dmg, flares, names, outs, parse, results, run, tid};

mod hunt_log_defs {
    use super::*;

    fn one(f: &cena_model::state::combat::ChunkFacts) -> &AttackEvent {
        assert_eq!(f.events.len(), 1, "one event, got {:?}", names(f));
        &f.events[0]
    }

    #[test]
    fn resumes_our_shot_after_a_disciples_cloak_of_shadows_retaliation_so_our_flare_stays_ours() {
        let f = parse(&fsm_harness::raw_fixture("cloak_of_shadows_interrupt"));
        let got: Vec<_> = f
            .events
            .iter()
            .map(|e| (e.name.as_str(), e.inbound))
            .collect();
        assert_eq!(got, [("fire", false), ("cast", true)]);
        let (fire, cast) = (&f.events[0], &f.events[1]);
        assert_eq!(dmg(fire), [21]);
        let phos = fire
            .flares
            .iter()
            .find(|x| x.name == "phosphorescence")
            .expect("phos");
        assert_eq!(phos.hits.iter().map(|h| h.damage).collect::<Vec<_>>(), [25]);
        let mut fl = flares(fire);
        fl.sort_unstable();
        assert_eq!(fl, ["arcane_reflex", "natures_decay", "phosphorescence"]);
        assert!(cast.hits.is_empty() && cast.flares.is_empty());
        assert_eq!(outs(cast), [OutcomeKind::Warded]);
        assert_eq!(
            cast.resolutions.iter().map(|r| r.kind).collect::<Vec<_>>(),
            [ResolutionKind::CsTd]
        );
        let a = cast.attacker.as_ref().expect("attacker");
        assert_eq!(
            (a.id, a.name.as_str()),
            (Some(129_623_615), "flayed gigas disciple")
        );
        // the blind its flare inflicted names the resumed shot, though the cast emits last
        let blind = f.facts.iter().find_map(|x| match x {
            Fact::Status {
                status: StatusName::Blind,
                event,
                flare_seq,
                ..
            } => Some((*event, *flare_seq)),
            _ => None,
        });
        assert_eq!(blind, Some((Some(0), Some(3))));
    }

    #[test]
    fn leaves_an_ambient_recovery_line_off_the_current_attacks_event() {
        use cena_model::StatusAction::{Add, Remove};
        let orc = bolded(700_001, "orc", "a grizzled orc");
        let troll = bolded(700_002, "troll", "a hulking troll");
        let f = parse(&[
            &format!("You fire a faewood arrow at {orc}!"),
            "  AS: +651 vs DS: +355 with AvD: +20 + d100 roll: +49 = +365",
            "   ... and hit for 70 points of damage!",
            "   Strike pierces forearm!",
            &format!("   The {orc} is stunned!"),
            &format!("{troll} shakes off the stun!"),
        ]);
        let by_id = |id: i64| {
            f.facts.iter().find_map(|x| match x {
                Fact::Status {
                    subject: Subject::Creature(a),
                    status,
                    action,
                    event,
                    ..
                } if a.id == Some(id) => Some((*status, *action, *event)),
                _ => None,
            })
        };
        assert_eq!(by_id(700_001), Some((StatusName::Stunned, Add, Some(0))));
        assert_eq!(by_id(700_002), Some((StatusName::Stunned, Remove, None)));
    }

    #[test]
    fn keeps_an_ooze_splitting_on_the_hit_off_the_target_switcher() {
        let f = parse(&fsm_harness::raw_fixture("ooze_splatter_fire"));
        let got: Vec<_> = f.events.iter().map(|e| (e.name.as_str(), dmg(e))).collect();
        assert_eq!(got, [("fire", vec![51])]);
    }

    #[test]
    fn parses_the_sanguine_ooze_pseudopod_attacks_and_their_misses() {
        let ooze = bolded(129_583_295, "ooze", "a quivering sanguine ooze");
        let smash = parse(&[
            &format!("{ooze} manifests a thick pseudopod and brings it smashing down at you!"),
            "  AS: +556 vs DS: +585 with AvD: +38 + d100 roll: +39 = +48",
            "   A clean miss.",
        ]);
        let whip = parse(&[
            "<pushBold/>[SMR result: 47 (Open d100: 70, Penalty: 10)]<popBold/>",
            &format!(
                "{ooze} whips a thick pseudopod at you!  The goopy appendage flies wide before retracting back into the central mass of {}.",
                bolded(129_583_295, "ooze", "the ooze")
            ),
        ]);
        let s = one(&smash);
        assert_eq!(
            (s.name.as_str(), s.inbound, outs(s)),
            ("natural", true, vec![OutcomeKind::Miss])
        );
        let w = one(&whip);
        assert_eq!(
            (w.name.as_str(), w.inbound, outs(w), results(w)),
            ("natural", true, vec![OutcomeKind::Miss], vec![Some(47)])
        );
    }

    #[test]
    fn records_the_ooze_shrapnel_burst_and_the_disciple_rift_as_inbound_room_maneuvers() {
        let ooze = bolded(129_583_295, "ooze", "a quivering sanguine ooze");
        let shrap = parse(&[
            &format!(
                "Froth disturbs the surface of {ooze} as bubbling bulges form over {} surface, rapidly coagulating into red-black crystalline spikes.  With a convulsive shudder, the ooze flings them outward!",
                bolded(129_583_295, "ooze", "its")
            ),
            "<pushBold/>[SMR result: -32 (Open d100: -36, Penalty: 13)]<popBold/>",
            "Bobbing and weaving, you dodge the spray of shrapnel!",
        ]);
        let disc = bolded(129_629_246, "disciple", "a flayed gigas disciple");
        let her = bolded(129_629_246, "disciple", "her");
        let rift = parse(&[
            &format!(
                "Zeal twisting {her} features, {disc} raises a raw and fleshless hand overhead and draws it down, {her} shattered fingernails slicing open a tear in the fabric of the world."
            ),
            "Writhing, milky tentacles burst forth from the tortured spatial anomaly, grasping blindly through the areas they glisten with vile humors.",
            "<pushBold/>[SMR result: 61 (Open d100: 67, Penalty: 53)]<popBold/>",
        ]);
        let s = one(&shrap);
        assert_eq!(
            (s.name.as_str(), s.inbound, outs(s), results(s)),
            (
                "shrapnel_spray",
                true,
                vec![OutcomeKind::Evade],
                vec![Some(-32)]
            )
        );
        let r = one(&rift);
        assert_eq!(
            (
                r.name.as_str(),
                r.inbound,
                r.attacker.as_ref().and_then(|a| a.id),
                results(r)
            ),
            ("rift_tentacles", true, Some(129_629_246), vec![Some(61)])
        );
    }

    #[test]
    fn marks_a_spell_wearing_off_in_the_same_chunk_as_the_fatal_crit_as_a_death_drop() {
        use cena_model::state::combat::event::LossCause;
        let skald = bolded(160_902_365, "skald", "a grim gigas skald");
        let f = parse(&[
            &format!("You take aim and fire a faewood arrow at {skald}!"),
            "  AS: +632 vs DS: +500 with AvD: +32 + d100 roll: +70 = +234",
            "   ... and hit for 75 points of damage!",
            "   Incredible shot to the eye penetrates deep into skull!",
            &format!(
                "{skald} raises a hand as if to grasp for support as he collapses, life going out of his form."
            ),
            &format!("A white glow rushes away from {skald}."),
        ]);
        let loss = f.facts.iter().find_map(|x| match x {
            Fact::SpellLoss {
                subject,
                spell,
                spell_name,
                cause,
            } => Some((subject.id, *spell, spell_name.as_str(), *cause)),
            _ => None,
        });
        assert_eq!(
            loss,
            Some((
                Some(160_902_365),
                Some(303),
                "Prayer of Protection",
                Some(LossCause::Death)
            ))
        );
        assert!(!f.facts.iter().any(|x| matches!(
            x,
            Fact::Status {
                status: StatusName::Dispelled,
                ..
            }
        )));
    }

    #[test]
    fn keeps_nearby_players_gigas_village_spells_foreign() {
        let kal = r#"<a exist="-10930001" noun="Kalithra">Kalithra</a>"#;
        let skald = bolded(130_610_001, "skald", "a grim gigas skald");
        let song = parse(&[
            &format!(
                "{kal} skillfully weaves another verse into her harmony, directing the sound of her voice at {skald}."
            ),
            "  CS: +527 - TD: +469 + CvA: +19 + d100: +68 == +145",
            "  Warding failed!",
            &format!("{skald} reels under the force of the sonic vibrations!"),
            "   Sound waves disrupt for 63 damage!",
            "   ... 70 points of damage!",
            &format!(
                "   {} midsection swells painfully then bursts, sending the gigas skald everywhere.",
                bolded(130_610_001, "skald", "The gigas skald's")
            ),
        ]);
        let got: Vec<_> = song
            .events
            .iter()
            .map(|e| (e.name.as_str(), e.foreign_caster, outs(e), dmg(e)))
            .collect();
        assert_eq!(
            got,
            [
                ("spellsong", true, vec![OutcomeKind::WardFailed], vec![]),
                ("sonic_disruption", true, vec![], vec![63, 70]),
            ]
        );

        let rain = r#"<a exist="-10930002" noun="Raincail">Raincail</a>"#;
        let masto = bolded(130_610_002, "mastodon", "a heavily armored battle mastodon");
        let maiden = bolded(130_610_003, "shield-maiden", "a brawny gigas shield-maiden");
        let cry = parse(&[
            &format!("{rain} lets loose an eerie, modulating cry!"),
            "<pushBold/>[SSR result: 67 (Open d100: -62)]<popBold/>",
            &format!("{masto} is unaffected!"),
            "<pushBold/>[SSR result: 162 (Open d100: 45)]<popBold/>",
            &format!("{maiden} looks at {rain} in utter terror!"),
            &format!(
                "{} freezes in place, quivering with fright!",
                bolded(130_610_003, "shield-maiden", "The gigas shield-maiden")
            ),
        ]);
        let mut n = names(&cry);
        n.dedup();
        assert_eq!(n, ["fear_cry"]);
        assert!(cry.events.iter().all(|e| e.foreign_caster));
        assert_eq!(
            cry.events
                .iter()
                .map(|e| e.resolutions.len())
                .sum::<usize>(),
            2
        );

        let emyle = r#"<a exist="-10930003" noun="Emyle">Emyle</a>"#;
        let warg = bolded(130_610_004, "warg", "a niveous giant warg");
        let waves = parse(&[
            &format!("Golden brown waves billow outward from {emyle} to buffet {warg}!"),
            "<pushBold/>[SMR result: 118 (Open d100: 18, Bonus: 46)]<popBold/>",
            "   ... 5 points of damage!",
            "   Minor puncture to the back.",
        ]);
        let w = one(&waves);
        assert_eq!(
            (
                w.name.as_str(),
                w.foreign_caster,
                tid(w),
                dmg(w),
                results(w)
            ),
            (
                "golden_waves",
                true,
                Some(130_610_004),
                vec![5],
                vec![Some(118)]
            )
        );

        let gal = r#"<a exist="-10930004" noun="Galactic">Galactic</a>"#;
        let beam = parse(&[
            &format!(
                "{gal} draws down a shaft of dappled moonlight and bathes {masto} in its lambent glow."
            ),
            "<pushBold/>[SMR result: 243 (Open d100: 75, Bonus: 56)]<popBold/>",
            &format!("{masto} is caught fast, the light of Liabo arresting its movements."),
        ]);
        let b = one(&beam);
        assert_eq!(
            (b.name.as_str(), b.foreign_caster, tid(b), results(b)),
            ("moonbeam", true, Some(130_610_002), vec![Some(243)])
        );

        let own = parse(&[
            &format!("You gesture at {maiden}."),
            &format!(
                "Tapping the moons above, you draw down a shaft of swirling moonlight and bathe {maiden} in its muted glow."
            ),
            "<pushBold/>[SMR result: 322 (Open d100: 61, Bonus: 159)]<popBold/>",
            &format!("{maiden} is caught fast, the light of Lornon arresting her movements."),
        ]);
        let o = one(&own);
        assert_eq!(
            (o.name.as_str(), o.via_cast, tid(o), results(o)),
            ("moonbeam", true, Some(130_610_003), vec![Some(322)])
        );
    }

    #[test]
    fn attributes_a_nearby_players_flaming_aura_to_that_player_and_a_spiritual_malady_tick_once_unowned()
     {
        let oozeling = bolded(129_648_792, "oozeling", "a quivering sanguine oozeling");
        let aura = parse(&[
            &format!(
                "The flaming aura surrounding <a exist=\"-10174607\" noun=\"Meb\">Meb</a> lashes out at {oozeling}!"
            ),
            "<pushBold/>[SMR result: 153 (Open d100: 76, Bonus: 8)]<popBold/>",
            "   ... 25 points of damage!",
        ]);
        let a = one(&aura);
        assert_eq!(
            (
                a.name.as_str(),
                a.foreign_caster,
                tid(a),
                dmg(a),
                results(a)
            ),
            (
                "flaming_aura",
                true,
                Some(129_648_792),
                vec![25],
                vec![Some(153)]
            )
        );
        let mutant = bolded(129_575_116, "mutant", "a squamous reptilian mutant");
        let malady = parse(&[
            &format!("A spiritual malady wracks {mutant} causing 5 points of damage!"),
            "   ... 5 points of damage!",
            "   Unpleasant wound to right arm!",
        ]);
        let m = one(&malady);
        assert_eq!(
            (m.name.as_str(), m.unowned, tid(m), dmg(m)),
            ("spiritual_malady", true, Some(129_575_116), vec![5])
        );
    }

    #[test]
    fn parses_the_disciples_leech_fling_with_its_held_roll_and_same_line_dodge() {
        let disc = bolded(129_649_883, "disciple", "a flayed gigas disciple");
        let f = parse(&[
            "The layer of bark on you hardens and absorbs the attack!  The bark crackles, but maintains its form.",
            "<pushBold/>[SMR result: 0 (Open d100: 81, Penalty: 3)]<popBold/>",
            &format!(
                "{disc} reaches into a pouch at {} waist and draws back a hand covered in fat leeches, so deep a violet in hue as to be almost black.  With a fleshless sneer, she flings the parasites at you!  You duck to narrowly avoid the flying vermiforms!",
                bolded(129_649_883, "disciple", "her")
            ),
        ]);
        let mut got: Vec<_> = f
            .events
            .iter()
            .map(|e| (e.name.as_str(), outs(e)))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("natural", vec![OutcomeKind::Evade]),
                ("unknown", vec![OutcomeKind::Intercept])
            ]
        );
        let fling = f
            .events
            .iter()
            .find(|e| e.name == "natural")
            .expect("fling");
        assert_eq!(
            (
                fling.inbound,
                fling.attacker.as_ref().and_then(|a| a.id),
                results(fling)
            ),
            (true, Some(129_649_883), vec![Some(0)])
        );
    }

    #[test]
    fn gives_a_creatures_warding_spell_effect_line_the_caster_of_the_cast_it_follows() {
        let disc = bolded(129_649_881, "disciple", "a flayed gigas disciple");
        let f = parse(&[
            &format!(
                "A dark shadowy tendril rises up from {disc}, writhes its way up a <a exist=\"129604585\" noun=\"bow\">scorched glowbark long bow</a> towards you and lashes out malevolently..."
            ),
            &format!(
                "The force of {} power warps the air as it surges toward you!",
                bolded(129_649_881, "disciple", "a flayed gigas disciple's")
            ),
            "  CS: +497 - TD: +485 + CvA: +13 + d100: +92 - -5 == +122",
            "  Warding failed!",
            "The layer of bark on you hardens and absorbs the magical energy!  The bark crackles, but maintains its form.",
            "A nebulous haze shimmers into view around you, plunging inward to envelop your left eye!",
            "   ... 10 points of damage!",
            "   Left eyelid turns to dust, causing you to blink rapidly, or try to.",
            "Cloudy tendrils writhe throughout your form, ravaging you for 15 points of damage!",
        ]);
        let got: Vec<_> = f
            .events
            .iter()
            .map(|e| {
                (
                    e.name.as_str(),
                    e.inbound,
                    e.attacker.as_ref().and_then(|a| a.id),
                    dmg(e),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("cast", true, Some(129_649_881), vec![]),
                ("wither", true, Some(129_649_881), vec![10, 15])
            ]
        );
    }

    #[test]
    fn parses_the_ooze_vitality_drain_the_cannibal_ambush_swing_and_rot_and_neck_bleed_ticks() {
        let ooze = bolded(129_635_928, "ooze", "a quivering sanguine ooze");
        let drain = parse(&[
            &format!(
                "{ooze} whips a pseudopod toward you, brushing your exposed flesh.  The layer of bark on you hardens and absorbs the magical energy!  The bark crackles, but maintains its form."
            ),
            &format!(
                "  Dizziness rushes through you as {} appendage siphons away your vitality!",
                bolded(129_635_928, "ooze", "the ooze's")
            ),
            "   ... 20 points of damage!",
        ]);
        let d = one(&drain);
        assert_eq!(
            (
                d.name.as_str(),
                d.inbound,
                d.attacker.as_ref().and_then(|a| a.id),
                dmg(d)
            ),
            ("natural", true, Some(129_635_928), vec![20])
        );
        let cannibal = bolded(129_776_223, "cannibal", "a bloody halfling cannibal");
        let ambush = parse(&[
            &format!(
                "With an ululating shriek, {cannibal} leaps from the shadows and hammers blindly at you with grimy little fists!"
            ),
            "  AS: +476 vs DS: +585 with AvD: +25 + d100 roll: +48 = -36",
            "   A clean miss.",
        ]);
        let a = one(&ambush);
        assert_eq!(
            (a.name.as_str(), a.inbound, outs(a), a.resolutions.len()),
            ("natural", true, vec![OutcomeKind::Miss], 1)
        );
        let mutant = bolded(129_870_380, "mutant", "a squamous reptilian mutant");
        let ticks = parse(&[
            &format!("Skin peels off {mutant}'s body, exposing rotting flesh."),
            "   ... 2 points of damage!",
            "   Unpleasant wound to left arm!",
            &format!(
                "Trickles of blood course from {}'s neck.",
                bolded(129_870_380, "mutant", "the reptilian mutant")
            ),
            "   ... 4 points of damage!",
        ]);
        let got: Vec<_> = ticks
            .events
            .iter()
            .map(|e| (e.name.as_str(), e.unowned, dmg(e)))
            .collect();
        assert_eq!(got, [("rot", true, vec![2]), ("bleed", true, vec![4])]);
    }

    #[test]
    fn wraps_a_held_pre_flare_as_its_own_event_when_no_swing_follows_in_the_next_chunk() {
        let mut state = GameState::default();
        let first = run(
            &mut state,
            &[
                &format!(
                    " ** Your <a exist=\"125479289\" noun=\"bow\">glowbark long bow</a> glows brightly for a moment, consuming the magical energies around the {}! **",
                    bolded(123_956_079, "mastodon", "armored battle mastodon")
                ),
                "   ... 20 points of damage!",
            ],
        );
        assert!(first.events.is_empty());
        let f = run(&mut state, &["You are now in a defensive stance."]);
        let got: Vec<_> = f.events.iter().map(|e| (e.name.as_str(), tid(e))).collect();
        assert_eq!(got, [("dispel", Some(123_956_079))]);
    }
}
