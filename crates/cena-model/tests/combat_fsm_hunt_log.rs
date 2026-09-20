//! `processor_inbound_spec.rb`, ported (3 of 3): the hunt-log defs audit of
//! 2026-09-07 -- one case per real-feed incident, five on raw chunk fixtures
//! in `tests/fixtures/combat_raw/`.
//!
//! See `combat_fsm_inbound.rs` for the harness contract.

#![allow(clippy::too_many_lines)]

mod fsm_harness;

use cena_model::state::combat::AttackEvent;
use cena_model::{GameState, OutcomeKind, ResolutionKind};
use fsm_harness::{bolded, dmg, flares, names, outs, parse, results, run, tid};

mod hunt_log_defs {
    use super::*;

    fn masto() -> String {
        bolded(123_956_079, "mastodon", "a heavily armored battle mastodon")
    }
    fn warg() -> String {
        bolded(123_985_834, "warg", "a niveous giant warg")
    }
    fn maiden() -> String {
        bolded(124_194_699, "shield-maiden", "a brawny gigas shield-maiden")
    }
    fn one(f: &cena_model::state::combat::ChunkFacts) -> &AttackEvent {
        assert_eq!(f.events.len(), 1, "one event, got {:?}", names(f));
        &f.events[0]
    }

    #[test]
    fn names_a_tangleweed_miss_as_a_tangleweed_attack_with_its_smr() {
        let f = parse(&[
            "<pushBold/>[SMR result: 71 (Open d100: -86, Bonus: 62)]<popBold/>",
            &format!(
                "The lashing emerald briar lashes out at {}, but is unable to grasp her.",
                maiden()
            ),
        ]);
        let ev = one(&f);
        assert_eq!(ev.name, "tangleweed");
        assert_eq!(tid(ev), Some(124_194_699));
        assert!(outs(ev).contains(&OutcomeKind::Miss));
        assert_eq!(results(ev), [Some(71)]);
    }

    #[test]
    fn records_a_warg_howl_as_an_inbound_fear_maneuver_with_its_ssr_and_our_save() {
        let f = parse(&[
            &format!(
                "{} sits back on its haunches and unleashes a long, high-pitched howl that sends a shiver of primal terror down your spine.",
                warg()
            ),
            "<pushBold/>[SSR result: 86 (Open d100: 18)]<popBold/>",
            &format!(
                "Fear still claws at your heart, but you stand fast against the {}'s unnerving howl!",
                bolded(123_985_834, "warg", "warg")
            ),
        ]);
        let ev = one(&f);
        assert_eq!((ev.name.as_str(), ev.inbound), ("howl", true));
        assert_eq!(ev.attacker.as_ref().and_then(|a| a.id), Some(123_985_834));
        assert_eq!(
            ev.resolutions.iter().map(|r| r.kind).collect::<Vec<_>>(),
            [ResolutionKind::Ssr]
        );
        assert!(outs(ev).contains(&OutcomeKind::Resisted));
    }

    #[test]
    fn records_a_mastodon_trumpet_as_an_inbound_fear_maneuver() {
        let f = parse(&[
            &format!(
                "{} raises its trunk and rears back onto its immense hind legs, blaring out a note of sheer fury!",
                masto()
            ),
            "<pushBold/>[SSR result: 79 (Open d100: 52)]<popBold/>",
            &format!(
                "You keep your wits amidst the {}'s angry trumpeting!",
                bolded(123_956_079, "mastodon", "mastodon")
            ),
        ]);
        let ev = one(&f);
        assert_eq!((ev.name.as_str(), ev.inbound), ("trumpet", true));
        assert!(outs(ev).contains(&OutcomeKind::Resisted));
    }

    #[test]
    fn parses_the_mastodon_tusk_attack_as_inbound_natural() {
        let f = parse(&[
            &format!("{} tries to spear you with its enormous tusks!", masto()),
            "  AS: +537 vs DS: +516 with AvD: +37 + d100 roll: +10 = +68",
            "   A clean miss.",
        ]);
        let ev = one(&f);
        assert_eq!((ev.name.as_str(), ev.inbound), ("natural", true));
        assert!(outs(ev).contains(&OutcomeKind::Miss));
    }

    #[test]
    fn names_the_attacker_from_the_creature_link_not_a_pronoun_link() {
        let zerk_his = bolded(123_957_785, "berserker", "his");
        let zerk = bolded(123_957_785, "berserker", "a tattooed gigas berserker");
        let f = parse(&[
            &format!(
                "Froth bubbling on {zerk_his} lips, {zerk} swings an immense fel-hafted handaxe at you in a murderous arc!"
            ),
            "You evade the attack by a hair!",
        ]);
        assert_eq!(
            one(&f).attacker.as_ref().map(|a| a.name.as_str()),
            Some("a tattooed gigas berserker")
        );
    }

    #[test]
    fn keeps_a_pre_emptive_warg_evade_on_the_warg_instead_of_a_targetless_unknown() {
        let f = parse(&[
            &format!(
                "With preternatural speed, {} bounds to safety as you move to attack {}, leaving you off-balance!",
                warg(),
                bolded(123_985_834, "warg", "it")
            ),
            "The arrow streaks off into the distance!",
        ]);
        let ev = one(&f);
        assert_eq!(tid(ev), Some(123_985_834));
        assert!(outs(ev).contains(&OutcomeKind::Evade));
        assert_eq!(ev.name, "unknown");
    }

    #[test]
    fn names_a_nocked_then_pre_empted_swing_as_the_fire_it_was_and_keeps_the_shroud() {
        let f = parse(&[
            "You nock a faewood arrow fletched with plain white feathers in your <a exist=\"129604585\" noun=\"bow\">glowbark long bow</a>.",
            &format!(
                "With preternatural speed, {} bounds to safety as you move to attack {}, leaving you off-balance!",
                warg(),
                bolded(123_985_834, "warg", "it")
            ),
            "A tenebrous shroud stitches itself into existence around you as you gracefully retreat into the shadows!",
            "The arrow streaks off into the distance!",
        ]);
        let ev = one(&f);
        assert_eq!(ev.name, "fire");
        assert_eq!(ev.weapon.as_deref(), Some("glowbark long bow"));
        assert_eq!(tid(ev), Some(123_985_834));
        assert_eq!(outs(ev).first(), Some(&OutcomeKind::Evade));
        assert_eq!(flares(ev), ["chameleon_shroud"]);
    }

    #[test]
    fn keeps_the_nocked_bow_when_a_creatures_inbound_swing_interleaves_before_the_fire() {
        let other = bolded(123_985_999, "warg", "A warg");
        let f = parse(&[
            "You nock a faewood arrow fletched with plain white feathers in your <a exist=\"129604585\" noun=\"bow\">glowbark long bow</a>.",
            &format!("{other} claws at you!"),
            "  AS: +176 vs DS: +76 with AvD: +20 + d100 roll: +64 = +184",
            "   ... and hits for 28 points of damage!",
            "   Smack to the eye bursts blood vessels.",
            &format!(
                "With preternatural speed, {} bounds to safety as you move to attack {}, leaving you off-balance!",
                warg(),
                bolded(123_985_834, "warg", "it")
            ),
            "The arrow streaks off into the distance!",
        ]);
        assert_eq!(f.events.len(), 2);
        let fire = f.events.iter().find(|e| !e.inbound).expect("fire");
        assert_eq!(
            (fire.name.as_str(), fire.weapon.as_deref(), tid(fire)),
            ("fire", Some("glowbark long bow"), Some(123_985_834))
        );
        // The interleaved swing is a fully resolved hit on us: it survives.
        let swing = f.events.iter().find(|e| e.inbound).expect("swing");
        assert_eq!(
            swing.attacker.as_ref().and_then(|a| a.id),
            Some(123_985_999)
        );
        assert_eq!(dmg(swing), [28]);
    }

    #[test]
    fn gives_the_swing_not_the_pinned_riders_one_hit_row_the_flare_that_follows() {
        let masto2 = bolded(130_490_001, "mastodon", "a heavily armored battle mastodon");
        let bers = bolded(130_490_002, "berserker", "a tattooed gigas berserker");
        let f = parse(&[
            &format!("You take aim and fire a faewood arrow at {masto2}!"),
            "  AS: +651 vs DS: +355 with AvD: +20 + d100 roll: +49 = +365",
            "   ... and hit for 70 points of damage!",
            "   Attack punctures the eye and connects with something really vital!",
            &format!("{bers} is pinned beneath {masto2} as it falls!"),
            "   ... 5 points of damage!",
            &format!(
                "   Blow leaves an imprint on {} chest!",
                bolded(130_490_002, "berserker", "the gigas berserker's")
            ),
            "A tenebrous shroud stitches itself into existence around you as you gracefully retreat into the shadows!",
        ]);
        let mut got: Vec<_> = f
            .events
            .iter()
            .map(|e| (e.name.as_str(), tid(e), dmg(e), flares(e)))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                (
                    "fire",
                    Some(130_490_001),
                    vec![70],
                    vec!["chameleon_shroud"]
                ),
                ("mount_collapse", Some(130_490_002), vec![5], vec![]),
            ]
        );
    }

    #[test]
    fn matches_the_live_trumpet_line_whose_pronouns_are_links_and_takes_the_fail_line_as_a_hit() {
        let its = bolded(123_956_079, "mastodon", "its");
        let f = parse(&[
            &format!(
                "{} raises {its} trunk and rears back onto {its} immense hind legs, blaring out a note of sheer fury!",
                masto()
            ),
            "<pushBold/>[SSR result: 174 (Open d100: 182)]<popBold/>",
            &format!(
                "The {}'s angry trumpeting startles you!",
                bolded(123_956_079, "mastodon", "mastodon")
            ),
            "Roundtime: 20 sec.",
        ]);
        let ev = one(&f);
        assert_eq!((ev.name.as_str(), ev.inbound), ("trumpet", true));
        assert!(outs(ev).contains(&OutcomeKind::Hit));
        assert_eq!(results(ev), [Some(174)]);
    }

    #[test]
    fn records_a_3p_feint_we_saw_through_as_an_inbound_feint_with_an_evade() {
        let f = parse(&[
            "<pushBold/>[SMR result: 20 (Open d100: 134, Penalty: 4)]<popBold/>",
            &format!(
                "{} feints high, but you aren't fooled for a second.",
                maiden()
            ),
        ]);
        let ev = one(&f);
        assert_eq!((ev.name.as_str(), ev.inbound), ("feint", true));
        assert!(outs(ev).contains(&OutcomeKind::Evade));
        assert_eq!(results(ev), [Some(20)]);
    }

    #[test]
    fn records_a_shield_push_and_its_whiff() {
        let her = bolded(124_194_699, "shield-maiden", "her");
        let f = parse(&[
            &format!(
                "{} raises {her} <a exist=\"124194700\" noun=\"targe\">golden targe</a> and attempts to push you away!",
                maiden()
            ),
            "<pushBold/>[SMR result: 17 (Open d100: 9, Penalty: 3)]<popBold/>",
            &format!(
                "{} completely misses you, stumbles, and flails around!",
                maiden()
            ),
        ]);
        let ev = one(&f);
        assert_eq!((ev.name.as_str(), ev.inbound), ("shield_push", true));
        assert!(outs(ev).contains(&OutcomeKind::Miss));
    }

    #[test]
    fn attributes_a_nearby_players_fiery_barbs_to_that_player_with_its_smr_and_damage() {
        let f = parse(&[
            &format!(
                "Fiery red barbs uncoil from the shadows near <a exist=\"-11152917\" noun=\"Burns\">Burns</a> and lash out at {}!",
                maiden()
            ),
            "<pushBold/>[SMR result: 104 (Open d100: 40, Bonus: 15)]<popBold/>",
            "   ... 15 points of damage!",
            "   Burst of flames to right arm toasts skin to elbows.",
        ]);
        let ev = one(&f);
        assert_eq!(ev.name, "fiery_barbs");
        assert!(ev.foreign_caster);
        assert_eq!(tid(ev), Some(124_194_699));
        assert_eq!(dmg(ev), [15]);
        assert_eq!(results(ev), [Some(104)]);
    }

    #[test]
    fn opens_a_barrier_block_with_no_attack_line_as_an_inbound_unknown() {
        let f = parse(&[&format!(
            "The thorny barrier surrounding you blocks the attack from the {}!",
            bolded(123_985_834, "warg", "giant warg")
        )]);
        let ev = one(&f);
        assert!(ev.inbound);
        assert_eq!(tid(ev), None);
        assert_eq!(ev.attacker.as_ref().and_then(|a| a.id), Some(123_985_834));
        assert_eq!(outs(ev), [OutcomeKind::Intercept]);
    }

    #[test]
    fn records_bleed_ticks_a_creatures_as_unowned_ours_as_inbound() {
        let theirs = parse(&[
            &format!(
                "Blood weeps from the {}'s open left arm wound.",
                bolded(123_956_079, "mastodon", "armored battle mastodon")
            ),
            "   ... 8 points of damage!",
        ]);
        let drips = parse(&[
            &format!(
                "The {}'s chest drips as {} continues to bleed.",
                bolded(123_985_834, "warg", "giant warg"),
                bolded(123_985_834, "warg", "it")
            ),
            "   ... 14 points of damage!",
        ]);
        let ours = parse(&[
            "Your right leg drips as you continue to bleed.",
            "   ... 3 points of damage!",
        ]);
        let t = one(&theirs);
        assert_eq!(
            (t.name.as_str(), tid(t), t.unowned, dmg(t)),
            ("bleed", Some(123_956_079), true, vec![8])
        );
        let d = one(&drips);
        assert_eq!(
            (d.name.as_str(), tid(d), d.unowned, dmg(d)),
            ("bleed", Some(123_985_834), true, vec![14])
        );
        let o = one(&ours);
        assert_eq!(
            (o.name.as_str(), o.inbound, dmg(o)),
            ("bleed", true, vec![3])
        );
    }

    #[test]
    fn names_a_missed_first_volley_arrow_volley_and_roots_the_round_on_it() {
        let f = parse(&fsm_harness::raw_fixture("volley_miss_first"));
        let mut n = names(&f);
        n.dedup();
        assert_eq!(n, ["volley"]);
        let miss = &f.events[0];
        assert_eq!(tid(miss), Some(126_382_122));
        assert_eq!(outs(miss), [OutcomeKind::Evade]);
        assert_eq!(results(miss), [Some(29)]);
        assert_eq!(miss.root, Some(0));
        assert!(f.events[1..].iter().all(|e| e.root == Some(0)));
        let sums: Vec<u32> = f
            .events
            .iter()
            .map(|e| e.hits.iter().map(|h| h.damage).sum())
            .collect();
        assert_eq!(sums, [0, 10, 35, 15, 10, 30]);
    }

    #[test]
    fn keeps_a_crit_rider_topple_whose_pronoun_is_a_link_on_the_swing() {
        let her = bolded(124_194_699, "shield-maiden", "her");
        let f = parse(&[
            &format!("You fire a faewood arrow at {}!", maiden()),
            "  AS: +646 vs DS: +414 with AvD: +38 + d100 roll: +42 = +312",
            "   ... and hit for 54 points of damage!",
            &format!(
                "   Deft slash to the {}'s left leg digs deep!",
                bolded(124_194_699, "shield-maiden", "gigas shield-maiden")
            ),
            "   Bone is chipped!",
            "<pushBold/>[SMR result: -84 (Open d100: 45, Penalty: 26)]<popBold/>",
            &format!(
                "Despite desperate windmilling to catch {her} balance, {} topples toward you!  You stumble into visibility as you try to dodge the falling shield-maiden.",
                maiden()
            ),
        ]);
        assert_eq!(names(&f), ["fire"]);
        assert_eq!(results(&f.events[0]), [Some(312), Some(-84)]);
    }

    #[test]
    fn parses_the_warg_jaw_hamstring_as_an_inbound_hamstring_with_its_miss() {
        let f = parse(&[
            &format!(
                "With a quick lunge, {} tries to hamstring you with {} jaws!",
                warg(),
                bolded(123_985_834, "warg", "its")
            ),
            "<pushBold/>[SMR result: 33 (Open d100: 37, Penalty: 29)]<popBold/>",
            &format!(
                "{}'s swing goes wide!",
                bolded(123_985_834, "warg", "A niveous giant warg")
            ),
        ]);
        let ev = one(&f);
        assert_eq!((ev.name.as_str(), ev.inbound), ("hamstring", true));
        assert!(outs(ev).contains(&OutcomeKind::Miss));
    }

    #[test]
    fn strips_a_possessive_baked_into_the_creature_link_and_squeezes_a_doubled_space() {
        let f = parse(&[&format!(
            "The thorny barrier surrounding you blocks the attack from the {}!",
            bolded(126_564_429, "skald", "gigas skald's")
        )]);
        assert_eq!(
            one(&f).attacker.as_ref().map(|a| a.name.as_str()),
            Some("gigas skald")
        );
        let f = parse(&[
            "The thorny barrier surrounding you blocks the attack from the <pushBold/><a exist=\"127089942\" noun=\" cannibal\">halfling  cannibal</a><popBold/>!",
        ]);
        assert_eq!(
            one(&f).attacker.as_ref().map(|a| a.name.as_str()),
            Some("halfling cannibal")
        );
    }

    #[test]
    fn holds_a_dispel_on_nock_pre_flare_across_the_chunk_boundary_so_the_next_shot_claims_it() {
        let mut state = GameState::default();
        let first = run(
            &mut state,
            &[
                "You nock a faewood <a exist=\"127300001\" noun=\"arrow\">arrow</a> fletched with plain white feathers in your <a exist=\"125479289\" noun=\"bow\">glowbark long bow</a>.",
                "You feel drained.",
                &format!(
                    " ** Your <a exist=\"125479289\" noun=\"bow\">glowbark long bow</a> glows brightly for a moment, consuming the magical energies around the {}! **",
                    bolded(123_956_079, "mastodon", "armored battle mastodon")
                ),
                " <pushBold/>[SMR result: 225 (Open d100: 40, Bonus: 110)]<popBold/>",
                "   ... 20 points of damage!",
                &format!(
                    "   The {}'s neck bones snap.",
                    bolded(123_956_079, "mastodon", "armored battle mastodon")
                ),
                "   Head looks precariously balanced now.",
            ],
        );
        assert!(first.events.is_empty());
        assert_eq!(state.combat().held_pre_flare_count(), 1);
        let f = run(
            &mut state,
            &[
                &format!("You fire a faewood arrow at {}!", masto()),
                "  AS: +644 vs DS: +301 with AvD: +20 + d100 roll: +90 = +453",
                "   ... and hit for 88 points of damage!",
                &format!(
                    "   Quick, powerful slash to the {}'s left knee!",
                    bolded(123_956_079, "mastodon", "armored battle mastodon")
                ),
            ],
        );
        assert_eq!(names(&f), ["fire"]);
        let dispel = f.events[0]
            .flares
            .iter()
            .find(|x| x.name == "dispel")
            .expect("dispel");
        assert!(dispel.pre);
        assert_eq!(
            dispel.hits.iter().map(|h| h.damage).collect::<Vec<_>>(),
            [20]
        );
        assert_eq!(
            dispel
                .resolutions
                .iter()
                .map(|r| r.result)
                .collect::<Vec<_>>(),
            [Some(225)]
        );
        assert_eq!(dmg(&f.events[0]), [88]);
        assert_eq!(state.combat().held_pre_flare_count(), 0);
    }

    #[test]
    fn claims_an_in_chunk_dispel_on_nock_pre_flare_for_the_shot_that_follows() {
        let f = parse(&fsm_harness::raw_fixture("dispel_on_nock"));
        assert_eq!(names(&f), ["fire"]);
        let fire = &f.events[0];
        assert_eq!(dmg(fire), [90]);
        let dispel = fire
            .flares
            .iter()
            .find(|x| x.name == "dispel")
            .expect("dispel");
        assert_eq!(
            dispel.hits.iter().map(|h| h.damage).collect::<Vec<_>>(),
            [15]
        );
        assert_eq!(
            dispel
                .resolutions
                .iter()
                .map(|r| r.result)
                .collect::<Vec<_>>(),
            [Some(175)]
        );
    }

    #[test]
    fn keeps_a_bloom_on_a_creature_forced_out_of_hiding_on_the_flare_not_a_phantom_fire() {
        let f = parse(&fsm_harness::raw_fixture("bloom_forced_out_of_hiding"));
        assert_eq!(names(&f), ["fire"]);
        let fire = &f.events[0];
        assert_eq!(dmg(fire), [138]);
        let blooms: Vec<&_> = fire
            .flares
            .iter()
            .filter(|x| x.name == "spectral_bloom")
            .collect();
        let sums: Vec<u32> = blooms
            .iter()
            .map(|b| b.hits.iter().map(|h| h.damage).sum())
            .collect();
        assert_eq!(sums, [15, 7, 25]);
        assert_eq!(
            blooms
                .last()
                .and_then(|b| b.target.as_ref())
                .map(|a| a.name.as_str()),
            Some("bloody halfling cannibal")
        );
    }
}
