//! `processor_inbound_spec.rb`, ported: attribution at the state-machine
//! level, through the real parser and `GameState`'s chunk boundary.
//!
//! Each case is one of Lich's, keeping its name and the real-feed incident
//! it pins. Cases that exercise `persist_event`, `process`'s emit order, or
//! the creature registry are not here -- they belong to the registry step.
//! Cases about ingestion provenance (`source`) have no counterpart.
//!
//! Every chunk is closed by a prompt through [`GameState::apply`], so this
//! also exercises the `close_chunk` wiring and the prompt-time stamp.

#![allow(clippy::too_many_lines)]

mod fsm_harness;

use cena_model::state::combat::event::{Fact, Subject};
use cena_model::{OutcomeKind, StatusName};
use fsm_harness::{bolded, dmg, flares, outs, parse, tid};

#[test]
fn does_not_apply_a_creatures_damage_to_itself_when_it_attacks_us() {
    let ogre = bolded(296_470_739, "ogre", "A mountain ogre");
    let f = parse(&[
        &format!("{ogre} swings a cudgel at you!"),
        "  AS: +176 vs DS: +76 with AvD: +20 + d100 roll: +64 = +184",
        "   ... and hits for 28 points of damage!",
        "   Smack to the eye bursts blood vessels.",
        // the emote that used to hand the ogre its own damage
        &format!("{ogre} throws her head back and laughs hysterically."),
    ]);
    assert_eq!(f.events.len(), 1);
    let e = &f.events[0];
    assert!(e.inbound);
    assert_eq!(tid(e), None);
    assert_eq!(e.attacker.as_ref().and_then(|a| a.id), Some(296_470_739));
    assert_eq!(dmg(e), [28]);
}

#[test]
fn keeps_an_inbound_event_from_adopting_a_bystander_creature() {
    let brawler = bolded(121_838_976, "brawler", "A triton brawler");
    let panther = bolded(999_111, "panther", "a bearded woodland panther");
    let f = parse(&[
        &format!("You notice {panther} nearby."),
        &format!("{brawler} swings a fist at you!"),
        "[SMR result: 139 (Open d100: 36, Bonus: 20)]",
        "   ... 4 points of damage!",
    ]);
    for e in &f.events {
        assert!(!matches!(tid(e), Some(121_838_976 | 999_111)));
    }
}

#[test]
fn still_applies_our_own_damage_to_the_creature_we_attacked() {
    let orc = bolded(4242, "orc", "a greater orc");
    let f = parse(&[
        &format!("You swing a slim short sword at {orc}!"),
        "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
        "   ... and hits for 30 points of damage!",
    ]);
    assert_eq!(f.events.len(), 1);
    assert_eq!(tid(&f.events[0]), Some(4242));
    assert_eq!(dmg(&f.events[0]), [30]);
    assert_eq!(
        f.events[0].at,
        Some(1),
        "stamped with the closing prompt's time"
    );
}

#[test]
fn does_not_apply_a_group_members_damage_to_the_casters_summoned_prop() {
    let sphere = bolded(527_976_197, "sphere", "ethereal sphere");
    let f = parse(&[
        &format!(
            "A blast of multihued plasma flares out from the center of the {sphere}, striking Sugiin!"
        ),
        "  CS: +443 - TD: +375 + CvA: +15 + d100: +43 - -5 == +131",
        "   Warding failed!",
        "   Sugiin is stricken for 29 points of damage!",
        "   ... 4 points of damage!",
        &format!("The {sphere} in a brawny gigas shield-maiden's hand vanishes from sight."),
    ]);
    assert!(f.events.iter().all(|e| tid(e) != Some(527_976_197)));
}

#[test]
fn never_adopts_the_attacker_as_its_own_victim_on_a_targetless_3p_attack() {
    let assassin = bolded(129_427_134, "assassin", "A wavering triton assassin");
    let f = parse(&[
        &format!("{assassin} leaps from hiding to attack!"),
        &format!(
            "A swirling burst of essence lashes out from {assassin}, consuming nearby magical energy!"
        ),
        "   ... 15 points of damage!",
        "   Plasma scorches a hole in your shield arm!",
        "   You are stunned for 1 round!",
    ]);
    assert!(f.events.iter().all(|e| tid(e) != Some(129_427_134)));
    // the 2p stun is OUR status, and a fact
    assert!(f.facts.iter().any(|x| matches!(
        x,
        Fact::Status {
            subject: Subject::Us,
            status: StatusName::Stunned,
            ..
        }
    )));
}

mod guardian_redirect_prefix {
    use super::*;

    fn maiden() -> String {
        bolded(1002, "shield-maiden", "a brawny gigas shield-maiden")
    }
    fn mastodon() -> String {
        bolded(1001, "mastodon", "a heavily armored battle mastodon")
    }
    fn redirect_line() -> String {
        format!(
            "Gritting her teeth with determination, {} raises her targe and throws herself between you and the mastodon to intercept your attack!",
            maiden()
        )
    }

    #[test]
    fn stamps_the_following_attack_and_opens_no_event_of_its_own() {
        let f = parse(&[
            &redirect_line(),
            &format!("You fire a faewood arrow at {}!", maiden()),
            "  AS: +652 vs DS: +394 with AvD: +38 + d100 roll: +32 = +328",
            "   ... and hit for 50 points of damage!",
        ]);
        assert_eq!(f.events.len(), 1);
        let fire = &f.events[0];
        assert_eq!(fire.name, "fire");
        assert_eq!(tid(fire), Some(1002));
        assert!(fire.outcomes.is_empty());
        let r = fire.redirect.as_ref().expect("redirect");
        assert_eq!(r.interceptor.id, Some(1002));
        assert_eq!(r.interceptor.name, "a brawny gigas shield-maiden");
        assert_eq!(r.intended, "mastodon");
        assert!(r.honored);
        assert_eq!(dmg(fire), [50]);
    }

    #[test]
    fn marks_an_announce_after_the_attack_line_as_unhonored() {
        let skald = bolded(1003, "skald", "a grim gigas skald");
        let f = parse(&[
            "You leap from hiding to strike!",
            &format!("You attempt to kick {skald}!"),
            &format!(
                "Gritting her teeth with determination, {} raises her targe and throws herself between you and the skald to intercept your attack!",
                maiden()
            ),
            &format!("You have good positioning against {skald}."),
            "  UAF: 746 vs UDF: 629 = 1.186 * MM: 92 + d100: 94 = 203",
            "  ... and hit for 71 points of damage!",
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "   ... and hit for 10 points of damage!",
        ]);
        assert_eq!(
            f.events.iter().map(tid).collect::<Vec<_>>(),
            [Some(1003), Some(1001)]
        );
        let kick = &f.events[0];
        assert_eq!(kick.name, "kick");
        assert_eq!(dmg(kick), [71]);
        let r = kick.redirect.as_ref().expect("redirect");
        assert_eq!(
            (r.intended.as_str(), r.honored, r.interceptor.id),
            ("skald", false, Some(1002))
        );
        assert!(f.events[1].redirect.is_none());
    }

    #[test]
    fn does_not_file_the_redirect_on_the_previous_attack_and_does_not_leak() {
        let f = parse(&[
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "  AS: +652 vs DS: +328 with AvD: +20 + d100 roll: +1 = +345",
            "   ... and hit for 63 points of damage!",
            " ** Fleeting and insubstantial, a whisper of shadow coalesces beside you, echoing your attack with one of its own! **",
            &redirect_line(),
            &format!("You fire a faewood arrow at {}!", maiden()),
            "   ... and hit for 50 points of damage!",
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "   ... and hit for 10 points of damage!",
        ]);
        assert_eq!(
            f.events.iter().map(tid).collect::<Vec<_>>(),
            [Some(1001), Some(1002), Some(1001)]
        );
        let intended: Vec<Option<&str>> = f
            .events
            .iter()
            .map(|e| e.redirect.as_ref().map(|r| r.intended.as_str()))
            .collect();
        assert_eq!(intended, [None, Some("mastodon"), None]);
        assert!(f.events[0].outcomes.is_empty());
        assert_eq!(flares(&f.events[0]), ["mirror_image"]);
        assert!(f.events[0].flares[0].outcomes.is_empty());
    }

    #[test]
    fn coexists_with_an_ambush_prefix() {
        let f = parse(&[
            "You leap from hiding to attack!",
            &redirect_line(),
            &format!(
                "You take aim and punch with a somnis katar at {}!",
                maiden()
            ),
            "  AS: +728 vs DS: +427 with AvD: +38 + d100 roll: +71 = +410",
            "   ... and hit for 90 points of damage!",
        ]);
        assert_eq!(f.events.len(), 1);
        assert!(f.events[0].ambush);
        assert!(f.events[0].aimed);
        assert_eq!(
            f.events[0].redirect.as_ref().map(|r| r.intended.as_str()),
            Some("mastodon")
        );
    }
}

mod hunters_afterimage_flare {
    use super::*;
    use cena_model::state::combat::event::Confidence;

    fn mastodon() -> String {
        bolded(1001, "mastodon", "a heavily armored battle mastodon")
    }

    #[test]
    fn attaches_to_the_shot_it_rode_and_leaves_the_echo_swing_its_own_damage() {
        let f = parse(&[
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "  AS: +652 vs DS: +375 with AvD: +20 + d100 roll: +20 = +317",
            "   ... and hit for 48 points of damage!",
            " ** A radiant afterimage of the arrow appears in your ready hand, coalescing to replace its predecessor! **",
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "  AS: +652 vs DS: +328 with AvD: +20 + d100 roll: +1 = +345",
            "   ... and hit for 63 points of damage!",
        ]);
        assert_eq!(f.events.len(), 2);
        assert_eq!(flares(&f.events[0]), ["hunters_afterimage"]);
        assert!(f.events[0].flares[0].hits.is_empty());
        assert_eq!(dmg(&f.events[0]), [48]);
        assert!(f.events[1].flares.is_empty());
        assert_eq!(dmg(&f.events[1]), [63]);
    }

    #[test]
    fn parents_the_echo_swing_to_the_shot_whose_afterimage_spawned_it() {
        let f = parse(&[
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "  AS: +652 vs DS: +375 with AvD: +20 + d100 roll: +20 = +317",
            "   ... and hit for 48 points of damage!",
            " ** A radiant afterimage of the arrow appears in your ready hand, coalescing to replace its predecessor! **",
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "  AS: +652 vs DS: +328 with AvD: +20 + d100 roll: +1 = +345",
            "   ... and hit for 63 points of damage!",
            " ** Fleeting and insubstantial, a mirror image of you shimmers into view at your side, echoing your attack with one of its own! **",
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "  AS: +652 vs DS: +300 with AvD: +20 + d100 roll: +50 = +422",
            "   ... and hit for 90 points of damage!",
        ]);
        let [shot, echo1, echo2] = f.events.as_slice() else {
            panic!("three events, got {}", f.events.len());
        };
        assert_eq!((shot.root, shot.parent), (Some(0), None));
        assert_eq!((echo1.root, echo1.parent), (Some(0), Some(0)));
        assert_eq!(
            echo1.parent_flare.as_ref().map(|p| p.flare.as_str()),
            Some("hunters_afterimage")
        );
        assert_eq!(echo1.parent_confidence, Some(Confidence::Count));
        // the mirror rode echo1, so echo2 is echo1's child, still rooted at the shot
        assert_eq!((echo2.root, echo2.parent), (Some(0), Some(1)));
        assert_eq!(
            echo2.parent_flare.as_ref().map(|p| p.flare.as_str()),
            Some("mirror_image")
        );
    }

    #[test]
    fn leaves_two_independent_shots_as_separate_roots() {
        let f = parse(&[
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "   ... and hit for 48 points of damage!",
            &format!("You fire a faewood arrow at {}!", mastodon()),
            "   ... and hit for 63 points of damage!",
        ]);
        assert_eq!((f.events[1].root, f.events[1].parent), (Some(1), None));
    }
}

mod ambush_prefix {
    use super::*;

    fn ghast() -> String {
        bolded(9001, "ghast", "a cadaverous tatterdemalion ghast")
    }
    const BUTCH: &str = r#"<a exist="-1000" noun="Butch">Butch</a>"#;

    #[test]
    fn flags_the_following_attack_instead_of_opening_its_own_event() {
        let f = parse(&[
            &format!("{BUTCH} leaps from hiding to strike!"),
            &format!("{BUTCH} attempts to punch {}!", ghast()),
            "  UAF: 681 vs UDF: 575 = 1.184 * MM: 103 + d100: 9 = 130",
            "   ... and hit for 33 points of damage!",
        ]);
        assert_eq!(f.events.len(), 1);
        assert_eq!(f.events[0].name, "uac");
        assert!(f.events[0].ambush);
        assert_eq!(dmg(&f.events[0]), [33]);
    }

    #[test]
    fn leaves_a_normal_attack_unflagged_and_does_not_leak() {
        let f = parse(&[
            &format!("{BUTCH} leaps from hiding to strike!"),
            &format!("{BUTCH} attempts to punch {}!", ghast()),
            "   ... and hit for 33 points of damage!",
            &format!("You swing a short sword at {}!", ghast()),
            "   ... and hits for 10 points of damage!",
        ]);
        assert_eq!(
            f.events.iter().map(|e| e.ambush).collect::<Vec<_>>(),
            [true, false]
        );
    }

    #[test]
    fn records_a_wholly_negated_ambush_via_its_intercept_outcome() {
        let executioner = bolded(9002, "executioner", "a triton executioner");
        let f = parse(&[
            &format!("{executioner} leaps from hiding to attack!"),
            &format!("The thorny barrier surrounding you blocks the attack from {executioner}!"),
        ]);
        let e = &f.events[0];
        assert_eq!(e.name, "ambush");
        assert!(e.ambush && e.inbound);
        assert_eq!(outs(e), [OutcomeKind::Intercept]);
        assert!(e.hits.is_empty());
    }
}

mod foreign_caster {
    use super::*;

    fn maiden() -> String {
        bolded(555_001, "shield-maiden", "a brawny gigas shield-maiden")
    }

    #[test]
    fn flags_a_paladin_weapon_infusion_proc_as_foreign_caster() {
        let f = parse(&[
            &format!(
                "As Heavenscent attempts to strike with her star, a surge of power flows out of it, through Heavenscent, and leaps out at {}!",
                maiden()
            ),
            "[SMR result: 271 (Open d100: 58, Bonus: 125)]",
            &format!(
                "The wisps solidify into thick strands of webbing that tighten about {}!",
                maiden()
            ),
            "   ... 20 points of damage!",
        ]);
        let e = &f.events[0];
        assert_eq!(e.name, "weapon_infusion");
        assert!(e.foreign_caster);
        assert!(!e.is_ours());
        assert_eq!(
            e.attacker.as_ref().map(|a| a.name.as_str()),
            Some("Heavenscent")
        );
        assert_eq!(tid(e), Some(555_001));
    }

    #[test]
    fn keeps_our_own_weapon_infusion_as_ours() {
        let f = parse(&[
            &format!(
                "As you attempt to strike with your star, it sends a surge of power through you that quickly leaps out at {}!",
                maiden()
            ),
            &format!("You swing a spiked star at {}!", maiden()),
            "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
            "   ... and hits for 30 points of damage!",
        ]);
        assert_eq!(f.events.len(), 1);
        let e = &f.events[0];
        assert!(!e.foreign_caster && e.is_ours());
        assert_eq!(tid(e), Some(555_001));
        assert_eq!(flares(e), ["weapon_cast"]);
        assert_eq!(e.hits.len(), 1);
    }
}

#[test]
fn coup_de_grace_records_the_kill_as_a_fatal_zero_damage_hit() {
    let zerk = bolded(121_654_846, "berserker", "a tattooed gigas berserker");
    let z = bolded(121_654_846, "berserker", "gigas berserker");
    let f = parse(&[
        &format!("You lunge towards the {z}, intending to finish her off!"),
        "<pushBold/>[SMR result: 276 (Open d100: 89, Bonus: 105)]<popBold/>",
        &format!(
            "You stiffen your fingers and drive them into the {z}'s neck, tearing out a handful of dripping trachea!  The gigas berserker gags just once."
        ),
        &format!("{zerk}'s fists tense with impotent rage as she surrenders to death."),
    ]);
    assert_eq!(f.events.len(), 1);
    let coup = &f.events[0];
    assert_eq!(coup.name, "coup_de_grace");
    assert_eq!(coup.hits.len(), 1);
    assert_eq!(coup.hits[0].damage, 0);
    let crit = coup.hits[0].crit.as_ref().expect("synthetic crit");
    assert!(crit.fatal && crit.is_coup_de_grace());
    assert_eq!(crit.location.as_str(), "neck");
    assert_eq!(coup.resolutions.len(), 1);
}
