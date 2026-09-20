//! Combat facts applied to creatures: `status_order_spec.rb` (5), and the
//! `persist_event` / death-watch cases of `processor_inbound_spec.rb` that
//! needed the registry.

mod fsm_harness;

use cena_model::state::combat::event::Fact;
use cena_model::state::creature::status::Classification;
use cena_model::{BodyPart, GameState, StatusName};
use fsm_harness::{bolded, run};

fn lizard() -> String {
    bolded(777, "lizard", "a cave lizard")
}

/// A swing whose crit knocks the lizard down: *"Hit on the leg chars the
/// skin and eats into the underlying muscles."* is a fire crit with
/// `position: PRONE`.
fn knockdown() -> Vec<String> {
    vec![
        format!("You swing a broadsword at {}!", lizard()),
        "  AS: +300 vs DS: +100 with AvD: +30 + d100 roll: +50 = +280".into(),
        "   ... and hit for 40 points of damage!".into(),
        "   Hit on the leg chars the skin and eats into the underlying muscles.".into(),
    ]
}
fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}
fn prone(state: &GameState) -> bool {
    state
        .creatures()
        .get(777)
        .is_some_and(|c| c.has_status(StatusName::Prone, None))
}

mod status_order {
    use super::*;

    #[test]
    fn leaves_a_creature_that_stood_up_after_a_knockdown_crit_standing() {
        let mut state = GameState::default();
        let mut lines = knockdown();
        lines.push(format!("{} stands up.", lizard()));
        run(&mut state, &refs(&lines));
        assert!(
            !prone(&state),
            "the earlier knockdown was re-applied over the later stand-up"
        );
    }

    #[test]
    fn lets_a_knockdown_message_after_the_stand_up_win_being_later_still() {
        let mut state = GameState::default();
        let mut lines = knockdown();
        lines.push(format!("{} stands up.", lizard()));
        lines.push(format!("{} is knocked to the ground!", lizard()));
        run(&mut state, &refs(&lines));
        assert!(prone(&state));
    }

    #[test]
    fn does_not_carry_a_recovery_into_the_next_chunk() {
        let mut state = GameState::default();
        run(&mut state, &[&format!("{} stands up.", lizard())]);
        run(&mut state, &refs(&knockdown()));
        assert!(prone(&state));
    }

    #[test]
    fn lets_a_knockdown_crit_after_the_stand_up_win_being_later_still() {
        let mut state = GameState::default();
        let mut lines = knockdown();
        lines.push(format!("{} stands up.", lizard()));
        lines.extend(knockdown());
        run(&mut state, &refs(&lines));
        assert!(
            prone(&state),
            "the later knockdown crit was suppressed by the earlier stand-up"
        );
    }

    #[test]
    fn still_applies_a_knockdown_crit_when_nothing_later_reverses_it() {
        let mut state = GameState::default();
        run(&mut state, &refs(&knockdown()));
        assert!(prone(&state));
        let c = state.creatures().get(777).expect("registered on demand");
        assert_eq!(c.damage_taken(), 40);
        assert!(c.injury(BodyPart::LeftLeg) > 0 || c.injury(BodyPart::RightLeg) > 0);
    }
}

#[test]
fn a_foreign_casters_damage_is_never_applied_to_the_creature() {
    let maiden = bolded(555_001, "shield-maiden", "a brawny gigas shield-maiden");
    let mut state = GameState::default();
    run(
        &mut state,
        &[
            &format!(
                "As Heavenscent attempts to strike with her star, a surge of power flows out of it, through Heavenscent, and leaps out at {maiden}!"
            ),
            "[SMR result: 271 (Open d100: 58, Bonus: 125)]",
            &format!(
                "The wisps solidify into thick strands of webbing that tighten about {maiden}!"
            ),
            "   ... 20 points of damage!",
        ],
    );
    assert!(
        state
            .creatures()
            .get(555_001)
            .is_none_or(|c| c.damage_taken() == 0),
        "emitted for observers, never applied"
    );
}

#[test]
fn a_reactive_flare_on_an_inbound_attack_lands_on_the_attacker() {
    // an inbound swing with no creature target, whose spike flare strikes
    // the attacker: the flare's damage and stun must not be dropped
    let defender = bolded(808, "defender", "a triton defender");
    let mut state = GameState::default();
    run(
        &mut state,
        &[
            &format!("{defender} swings a cudgel at you!"),
            "  AS: +176 vs DS: +76 with AvD: +20 + d100 roll: +64 = +184",
            "   ... and hits for 10 points of damage!",
            // the real `shield_spike` announce (`combat_effects.tsv`, flare 109)
            &format!("A spike on your golden targe jabs into {defender}!"),
            "   ... 5 points of damage!",
            "   Smack to the eye bursts blood vessels.",
        ],
    );
    let c = state
        .creatures()
        .get(808)
        .expect("the attacker, via its flare");
    assert_eq!(
        c.damage_taken(),
        5,
        "only the spike's 5, never the 10 it dealt us"
    );
    assert!(c.has_status(StatusName::Stunned, None));
    assert!(
        c.stun_rounds(Some(1)).is_some(),
        "the crit's stun estimate, anchored to the prompt"
    );
}

#[test]
fn a_flare_crit_stuns_the_flares_own_creature() {
    let zerk = bolded(121_654_846, "berserker", "a tattooed gigas berserker");
    let masto = bolded(121_678_494, "mastodon", "a heavily armored battle mastodon");
    let mut state = GameState::default();
    run(
        &mut state,
        &[
            &format!("You fire a faewood arrow at {zerk}!"),
            "   ... and hit for 188 points of damage!",
            &format!(
                " ** A bloom of spectral light blossoms around {masto}, engulfing it in searing brilliance! **"
            ),
            "   ... 5 points of damage!",
            "   Smack to the eye bursts blood vessels.",
        ],
    );
    let reg = state.creatures();
    assert_eq!(
        reg.get(121_654_846)
            .map(cena_model::CreatureInstance::damage_taken),
        Some(188)
    );
    let m = reg.get(121_678_494).expect("the bloom's creature");
    assert_eq!(m.damage_taken(), 5);
    assert!(m.has_status(StatusName::Stunned, None));
    assert!(
        !reg.get(121_654_846)
            .is_some_and(|c| c.has_status(StatusName::Stunned, None))
    );
}

mod death_watch {
    use super::*;
    use cena_protocol::Parser;

    fn flag_dead(state: &mut GameState, id: i64) {
        let mut parser = Parser::new();
        let wire = format!("<crtrStatus exist=\"{id}\" hostile=\"1\" dead=\"1\"/>\n");
        for frame in parser.push_bytes(wire.as_bytes()) {
            state.apply(&frame);
        }
    }
    fn deaths(facts: &cena_model::state::combat::ChunkFacts) -> Vec<i64> {
        facts
            .facts
            .iter()
            .filter_map(|f| match f {
                Fact::Dead { creature } => creature.id,
                _ => None,
            })
            .collect()
    }
    fn kill_shot(state: &mut GameState) -> cena_model::state::combat::ChunkFacts {
        let masto = bolded(900, "mastodon", "a heavily armored battle mastodon");
        run(
            state,
            &[
                &format!("You fire a faewood arrow at {masto}!"),
                "   ... and hit for 120 points of damage!",
            ],
        )
    }

    #[test]
    fn a_touched_creature_flagged_dead_is_announced_once_on_a_later_quiet_chunk() {
        let mut state = GameState::default();
        let first = kill_shot(&mut state);
        assert!(deaths(&first).is_empty(), "alive at this point");
        assert!(state.creatures().is_watching(900));
        flag_dead(&mut state, 900);
        let quiet = run(&mut state, &["You feel more refreshed."]);
        assert_eq!(deaths(&quiet), [900]);
        assert!(
            state
                .creatures()
                .get(900)
                .is_some_and(|c| c.flag(Classification::Dead))
        );
        let again = run(&mut state, &["You feel more refreshed."]);
        assert!(deaths(&again).is_empty(), "announced once");
        assert!(!state.creatures().is_watching(900));
    }

    #[test]
    fn a_death_the_registry_already_shows_rides_the_chunk_that_caused_it() {
        let mut state = GameState::default();
        kill_shot(&mut state);
        flag_dead(&mut state, 900);
        let second = kill_shot(&mut state);
        assert_eq!(deaths(&second), [900]);
        assert_eq!(second.events.len(), 1, "the attack is in the same facts");
    }

    #[test]
    fn keeps_watching_a_survivor_until_it_dies() {
        let mut state = GameState::default();
        kill_shot(&mut state);
        for _ in 0..8 {
            assert!(deaths(&run(&mut state, &["You feel more refreshed."])).is_empty());
        }
        assert!(state.creatures().is_watching(900));
        flag_dead(&mut state, 900);
        assert_eq!(deaths(&run(&mut state, &["Roundtime: 1 sec."])), [900]);
    }

    /// The watch alone does not make it "once": shooting the corpse puts it
    /// back on the watch, still flagged dead. Found by a surviving mutant.
    #[test]
    fn a_corpse_touched_again_is_not_announced_a_second_time() {
        let mut state = GameState::default();
        kill_shot(&mut state);
        flag_dead(&mut state, 900);
        assert_eq!(deaths(&run(&mut state, &["Roundtime: 1 sec."])), [900]);
        let again = kill_shot(&mut state);
        assert!(
            deaths(&again).is_empty(),
            "announced once, however often touched"
        );
    }

    #[test]
    fn emits_nothing_for_a_creature_that_is_still_alive() {
        let mut state = GameState::default();
        kill_shot(&mut state);
        let quiet = run(&mut state, &["You feel more refreshed."]);
        assert!(deaths(&quiet).is_empty());
    }
}
