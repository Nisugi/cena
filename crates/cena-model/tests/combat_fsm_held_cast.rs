//! `processor_inbound_spec.rb`, ported (2 of 3): the bare-cast hold across a
//! chunk boundary, and the glowbark chain (flare-attributed statuses).
//!
//! See `combat_fsm_inbound.rs` for the harness contract.

#![allow(clippy::too_many_lines)]

mod fsm_harness;

use cena_model::state::combat::event::{Fact, Subject};
use cena_model::{GameState, StatusName};
use fsm_harness::{bolded, dmg, flares, names, parse, run, tid};

mod bare_cast_held_across_a_chunk_boundary {
    use super::*;

    fn zerk() -> String {
        bolded(121_654_846, "berserker", "a tattooed gigas berserker")
    }
    fn gesture_chunk() -> Vec<String> {
        vec![
            format!("You gesture at {}.", zerk()),
            "Cast Roundtime 1 Second.".into(),
        ]
    }
    fn lash_chunk() -> Vec<String> {
        vec![
            "A violently lashing emerald briar bestrewn with unnaturally sharp spikes suddenly sprouts from the ground and begins to thrash about violently!".into(),
            "<pushBold/>[SMR result: 149 (Open d100: 57)]<popBold/>".into(),
            format!("The lashing emerald briar lashes out violently at {}, dragging her to the ground!", zerk()),
            "   ... 10 points of damage!".into(),
            "   Blow to the diaphragm.".into(),
        ]
    }
    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }

    #[test]
    fn holds_the_gesture_and_lets_the_next_chunks_spell_result_supersede_it() {
        let mut state = GameState::default();
        let first = run(&mut state, &refs(&gesture_chunk()));
        assert!(first.events.is_empty());
        assert!(state.combat().holds_a_cast());
        let f = run(&mut state, &refs(&lash_chunk()));
        assert_eq!(names(&f), ["tangleweed"]);
        let tw = &f.events[0];
        assert!(tw.via_cast);
        assert_eq!(tid(tw), Some(121_654_846));
        assert_eq!(dmg(tw), [10]);
        assert_eq!(tw.resolutions.len(), 1, "the SMR that followed the gesture");
        assert!(!state.combat().holds_a_cast());
    }

    #[test]
    fn emits_the_held_cast_as_itself_when_the_next_chunk_starts_a_new_2p_attack() {
        let mut state = GameState::default();
        run(&mut state, &refs(&gesture_chunk()));
        let f = run(
            &mut state,
            &[
                &format!("You fire a faewood arrow at {}!", zerk()),
                "  AS: +663 vs DS: +271 with AvD: +27 + d100 roll: +80 = +499",
                "   ... and hit for 188 points of damage!",
            ],
        );
        assert_eq!(names(&f), ["cast", "fire"]);
        assert!(!f.events[1].via_cast);
    }

    #[test]
    fn emits_the_held_cast_at_the_end_of_a_quiet_chunk_rather_than_holding_it_forever() {
        let mut state = GameState::default();
        run(&mut state, &refs(&gesture_chunk()));
        let f = run(&mut state, &["You feel more refreshed."]);
        assert_eq!(names(&f), ["cast"]);
        assert!(!state.combat().holds_a_cast());
    }

    #[test]
    fn still_supersedes_when_a_mirror_image_echoed_the_gesture() {
        let mut lines = vec![
            format!("You gesture at {}.", zerk()),
            " ** Fleeting and insubstantial, a mirror image of you shimmers into view at your side, echoing your attack with one of its own! **".into(),
            "Nothing happens.".into(),
            "Cast Roundtime 1 Second.".into(),
        ];
        lines.extend(lash_chunk());
        let f = parse(&refs(&lines));
        assert_eq!(names(&f), ["tangleweed"]);
        assert!(f.events[0].via_cast);
        assert!(flares(&f.events[0]).contains(&"mirror_image"));
    }

    #[test]
    fn does_not_split_the_lash_when_the_rider_dismounts_mid_line() {
        let masto = bolded(123_259_310, "mastodon", "a heavily armored battle mastodon");
        let maiden = bolded(123_241_966, "shield-maiden", "a brawny gigas shield-maiden");
        let f = parse(&[
            "<pushBold/>[SMR result: 173 (Open d100: 68, Bonus: 4)]<popBold/>",
            &format!(
                "The lashing emerald briar lashes out violently at {masto}, dragging it to the ground!"
            ),
            &format!(
                "{maiden} leaps from the back of {masto} as it topples, narrowly avoiding being pinned beneath its mount!"
            ),
            "   ... 5 points of damage!",
            "   Attempt to snare hips shaken loose.",
        ]);
        let got: Vec<_> = f
            .events
            .iter()
            .map(|e| (e.name.as_str(), tid(e), dmg(e), e.resolutions.len()))
            .collect();
        assert_eq!(got, [("tangleweed", Some(123_259_310), vec![5], 1)]);
    }

    #[test]
    fn hands_the_line_back_to_the_lash_after_the_pinned_riders_single_hit() {
        let masto = bolded(130_483_104, "mastodon", "a heavily armored battle mastodon");
        let maiden = bolded(130_483_100, "shield-maiden", "a brawny gigas shield-maiden");
        let f = parse(&[
            "<pushBold/>[SMR result: 125 (Open d100: 12, Bonus: 9)]<popBold/>",
            &format!(
                "The lashing emerald briar lashes out violently at {masto}, dragging it to the ground!"
            ),
            &format!("{maiden} is pinned beneath {masto} as it falls!"),
            "   ... 5 points of damage!",
            &format!(
                "   Blow raises a welt on {} left arm.",
                bolded(130_483_100, "shield-maiden", "the gigas shield-maiden's")
            ),
            "   ... 5 points of damage!",
            "   Attempt to grab from behind shrugged off.",
            &format!(
                "You notice a number of the briar's nettles scrape into {} skin.  It suddenly looks very weak!",
                bolded(
                    130_483_104,
                    "mastodon",
                    "a heavily armored battle mastodon's"
                )
            ),
        ]);
        let mut got: Vec<_> = f
            .events
            .iter()
            .map(|e| (e.name.as_str(), tid(e), dmg(e), e.resolutions.len()))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("mount_collapse", Some(130_483_100), vec![5], 0),
                ("tangleweed", Some(130_483_104), vec![5], 1)
            ]
        );
    }

    #[test]
    fn still_supersedes_in_blob_when_gesture_and_lash_share_a_chunk() {
        let mut lines = gesture_chunk();
        lines.extend(lash_chunk());
        let f = parse(&refs(&lines));
        assert_eq!(names(&f), ["tangleweed"]);
        assert!(f.events[0].via_cast);
    }
}

mod glowbark_chain {
    use super::*;

    fn zerk() -> String {
        bolded(121_654_846, "berserker", "a tattooed gigas berserker")
    }
    fn masto() -> String {
        bolded(121_678_494, "mastodon", "a heavily armored battle mastodon")
    }
    fn chunk() -> Vec<String> {
        vec![
            format!("You fire a faewood arrow at {}!", zerk()),
            "  AS: +663 vs DS: +271 with AvD: +27 + d100 roll: +80 = +499".into(),
            "   ... and hit for 188 points of damage!".into(),
            format!("   Crossing slash to chest catches the {}'s attention!", bolded(121_654_846, "berserker", "gigas berserker")),
            format!(" ** Countless points of pale phosphorescence awaken across your glowbark long bow, rapidly brightening before bursting into brilliant light around {}! **", zerk()),
            "   ... 20 points of damage!".into(),
            format!("   Wreath of energy burns away the {}'s hair and leaves skin blackened!", bolded(121_654_846, "berserker", "gigas berserker")),
            format!("You blinded {}!", zerk()),
            " ** Phosphorescent light races through the glowbark as its entire surface blossoms with dazzling radiance, flooding the surroundings in ghostly light! **".into(),
            format!(" ** A bloom of spectral light blossoms around {}, engulfing it in searing brilliance! **", masto()),
            "   ... 5 points of damage!".into(),
            format!("   Plasma scalds the {}'s stomach leaving painful red streaks.", bolded(121_678_494, "mastodon", "armored battle mastodon")),
            format!("You blinded {}!", masto()),
            format!("The arrow sticks in {}'s chest!", zerk()),
            "Roundtime: 3 sec.".into(),
        ]
    }
    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }

    #[test]
    fn keeps_the_bloom_damage_on_the_spectral_bloom_flare_as_one_event() {
        let f = parse(&refs(&chunk()));
        assert_eq!(f.events.len(), 1);
        let ev = &f.events[0];
        assert_eq!(ev.name, "fire");
        assert_eq!(tid(ev), Some(121_654_846));
        assert_eq!(dmg(ev), [188]);
        assert_eq!(
            flares(ev),
            ["phosphorescence", "glowbright", "spectral_bloom"]
        );
        let bloom = ev.flares.last().expect("bloom");
        assert_eq!(bloom.target.as_ref().and_then(|a| a.id), Some(121_678_494));
        assert_eq!(bloom.hits.iter().map(|h| h.damage).collect::<Vec<_>>(), [5]);
        assert_eq!(
            ev.flares[0]
                .hits
                .iter()
                .map(|h| h.damage)
                .collect::<Vec<_>>(),
            [20]
        );
        // the phosphorescence crit is on the FLARE's hit, not the swing's
        assert_eq!(
            ev.hits[0].crit.as_ref().map(|c| c.location.as_str()),
            Some("chest")
        );
        assert!(ev.flares[0].hits[0].crit.is_some());
    }

    #[test]
    fn keeps_a_crit_rider_smr_on_the_bloom_instead_of_orphaning_it() {
        let mut lines = chunk();
        let i = lines
            .iter()
            .position(|l| l.contains("Plasma scalds"))
            .expect("line");
        lines[i] = format!(
            "   Fiery blast of plasma blows the {}'s leg into a bloody spray!",
            bolded(121_678_494, "mastodon", "armored battle mastodon")
        );
        lines.insert(
            i + 1,
            "<pushBold/>[SMR result: 35 (Open d100: 55, Penalty: 23)]<popBold/>".into(),
        );
        lines.insert(i + 2, format!("Despite desperate windmilling to catch its balance, {} topples toward you!  You stumble into visibility as you try to dodge.", masto()));
        lines.insert(
            i + 3,
            format!(
                "   The {} is stunned!",
                bolded(121_678_494, "mastodon", "armored battle mastodon")
            ),
        );
        let f = parse(&refs(&lines));
        assert_eq!(names(&f), ["fire"]);
        let bloom = f.events[0].flares.last().expect("bloom");
        assert_eq!(bloom.name, "spectral_bloom");
        assert_eq!(
            bloom
                .resolutions
                .iter()
                .map(|r| r.result)
                .collect::<Vec<_>>(),
            [Some(35)]
        );
    }

    #[test]
    fn emits_each_blind_with_the_flare_seq_of_the_flare_that_caused_it() {
        let f = parse(&refs(&chunk()));
        let blinds: Vec<(Option<i64>, Option<usize>, Option<usize>)> = f
            .facts
            .iter()
            .filter_map(|x| match x {
                Fact::Status {
                    subject: Subject::Creature(a),
                    status: StatusName::Blind,
                    event,
                    flare_seq,
                    ..
                } => Some((a.id, *flare_seq, *event)),
                _ => None,
            })
            .collect();
        assert_eq!(
            blinds,
            [
                (Some(121_654_846), Some(1), Some(0)),
                (Some(121_678_494), Some(3), Some(0))
            ]
        );
    }

    #[test]
    fn files_a_flare_and_status_line_on_its_own_flare_not_the_one_before_it() {
        let disc = bolded(129_649_881, "disciple", "a flayed gigas disciple");
        let f = parse(&[
            &format!(
                "** Your <a exist=\"129604585\" noun=\"bow\">glowbark long bow</a> glows brightly for a moment, consuming the magical energies around {}! **",
                bolded(129_649_881, "disciple", "the gigas disciple")
            ),
            &format!("You fire a faewood arrow at {disc}!"),
            "  AS: +652 vs DS: +602 with AvD: +32 + d100 roll: +95 = +177",
            "   ... and hit for 30 points of damage!",
            &format!(
                "   Strike pierces {} forearm!",
                bolded(129_649_881, "disciple", "the gigas disciple's")
            ),
            &format!(
                "   {} is stunned!",
                bolded(129_649_881, "disciple", "The gigas disciple")
            ),
            &format!("{disc} is buffeted by a burst of wind and pushed back!"),
            &format!("The earthy, sweet aroma clinging to {disc} grows more pervasive."),
            "Vital energy infuses you, hastening your arcane reflexes!",
        ]);
        assert_eq!(
            flares(&f.events[0]),
            ["dispel", "breeze", "natures_decay", "arcane_reflex"]
        );
        let seq_of = |wanted: StatusName| -> Vec<(Option<i64>, Option<usize>)> {
            f.facts
                .iter()
                .filter_map(|x| match x {
                    Fact::Status {
                        subject: Subject::Creature(a),
                        status,
                        flare_seq,
                        ..
                    } if *status == wanted => Some((a.id, *flare_seq)),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(
            seq_of(StatusName::NaturesDecay),
            [(Some(129_649_881), Some(3))]
        );
        // the swing's own crit stun is not the dispel pre-flare's doing
        assert_eq!(seq_of(StatusName::Stunned), [(Some(129_649_881), None)]);
    }
}
