//! Three creature facts a hunt needs (`inventory/12` §2): a hazard that is
//! not a target, someone's familiar, companion or summon, and an ending with
//! no corpse, with a boss's phase beside it.
//!
//! Every wire line here is synthetic, built from the scripts' own patterns
//! (`state/hazard.rs`, `state/creatures/ally.rs`, `state/creatures/prose.rs`
//! cite them). No committed fixture carries one of these lines.

use cena_model::state::combat::event::Fact;
use cena_model::state::creatures::ally::classify;
use cena_model::state::hazard;
use cena_model::{Ally, BossPhase, CreatureInstance, Ending, GameState};
use cena_protocol::Parser;

fn feed(state: &mut GameState, wire: &str) {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
}

/// A `room objs` listing one bolded creature with its status tag.
fn listed(id: i64, noun: &str, name: &str, status: &str) -> String {
    format!(
        "<component id='room objs'>  You also see<crtrStatus exist=\"{id}\" {status}/><b> <pushBold/>a <a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/></b>.</component>\n"
    )
}

/// A creature's link as a line carries it: bolded.
fn link(id: i64, noun: &str, name: &str) -> String {
    format!("<pushBold/><a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/>")
}

const PROMPT: &str = "<prompt time=\"100\">&gt;</prompt>\n";

/// Every `Fact::Dead` published since the last call, by id.
fn deaths(state: &mut GameState) -> Vec<Option<i64>> {
    state
        .combat_mut()
        .take_facts()
        .iter()
        .flat_map(|chunk| chunk.facts.iter())
        .filter_map(|fact| match fact {
            Fact::Dead { creature } => Some(creature.id),
            _ => None,
        })
        .collect()
}

// --- 1. hazards ------------------------------------------------------------

#[test]
fn a_wasp_nest_and_a_shimmering_fungus_are_hazards_not_targets() {
    let mut state = GameState::default();
    feed(
        &mut state,
        &listed(9100, "nest", "wasp nest", "hostile=\"1\""),
    );
    let nest = state.creatures().get(9100).expect("the nest");
    assert!(nest.hazard());
    // Still hostile and still a valid target by Lich's rule: the hazard is
    // a fact beside those, and skipping it is the hunt's call.
    assert_eq!(nest.hostile(), Some(true));
    assert!(nest.valid_target());

    assert!(hazard::is_creature("Shimmering Fungus"));
    assert!(!hazard::is_creature("cinder wasp"));
    assert!(!hazard::is_creature("wasp"));
    // A boon adjective in front, stripped as the bestiary retries.
    feed(
        &mut state,
        &listed(9101, "nest", "glowing wasp nest", "hostile=\"1\""),
    );
    assert!(
        state
            .creatures()
            .get(9101)
            .is_some_and(CreatureInstance::hazard)
    );
    feed(
        &mut state,
        &listed(9102, "wasp", "cinder wasp", "hostile=\"1\""),
    );
    assert!(state.creatures().get(9102).is_some_and(|c| !c.hazard()));
}

#[test]
fn the_rooms_hazard_objects_are_whole_words_and_never_a_disk() {
    let mut state = GameState::default();
    feed(
        &mut state,
        "<component id='room objs'>  You also see a <a exist=\"9001\" noun=\"cloud\">gas cloud</a>, a <a exist=\"9002\" noun=\"web\">sticky web</a>, a <a exist=\"9003\" noun=\"cobweb\">dusty cobweb</a>, a <a exist=\"9004\" noun=\"disk\">web-woven disk</a> and a <a exist=\"9005\" noun=\"vortex\">windy vortex</a>.</component>\n",
    );
    let ids: Vec<&str> = state.room.hazards().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["9001", "9002", "9005"]);
    assert!(hazard::is_object("Black Void"));
    assert!(!hazard::is_object("vineyard map"));
}

// --- 2. familiars, companions, summons -------------------------------------

#[test]
fn a_familiar_a_companion_and_a_summon_are_read_off_the_name_and_noun() {
    assert_eq!(
        classify("fluffy black cat", Some("cat")),
        Some(Ally::Familiar)
    );
    assert_eq!(
        classify("dusty-eyed earth wyrdling", Some("wyrdling")),
        Some(Ally::Familiar)
    );
    assert_eq!(
        classify("peak snowcat", Some("snowcat")),
        Some(Ally::Companion)
    );
    assert_eq!(classify("short red grik", Some("grik")), Some(Ally::Demon));
    assert_eq!(classify("caped figure", Some("figure")), Some(Ally::Demon));
    assert_eq!(
        classify("lazy wild brown dog", Some("dog")),
        Some(Ally::Demon)
    );
    // The Igaesha deviation: a colour of more than one word.
    assert_eq!(
        classify("cloud of pale grey fog", Some("fog")),
        Some(Ally::Demon)
    );
    assert_eq!(
        classify("fluctuant spring spirit", Some("spirit")),
        Some(Ally::Passive)
    );
    assert_eq!(classify("kobold", Some("kobold")), None);
}

#[test]
fn the_bestiary_and_recolors_exceptions_outrank_the_tables() {
    // In xmlpatch's own exclusions, and a template.
    assert_eq!(classify("mountain snowcat", Some("snowcat")), None);
    // Templates the frozen exclusions missed: a companion noun, a familiar
    // spelling.
    assert_eq!(classify("wharf rat", Some("rat")), None);
    assert_eq!(classify("water hound", Some("hound")), None);
    assert_eq!(classify("large black cat", Some("cat")), None);
    // A template behind a boon adjective.
    assert_eq!(classify("glowing wharf rat", Some("rat")), None);
    // recolor's named hostile: the demon's `'ra` form.
    assert_eq!(
        classify("lean-bodied deep indigo abyran", Some("abyran")),
        Some(Ally::Demon)
    );
    assert_eq!(
        classify("lean-bodied deep indigo abyran'ra", Some("abyran")),
        None
    );
    assert_eq!(classify("tree spirit", Some("spirit")), None);
}

#[test]
fn the_servers_hostile_flag_outranks_the_name() {
    let mut state = GameState::default();
    feed(
        &mut state,
        &listed(
            601,
            "snowcat",
            "peak snowcat",
            "health=\"100\" maxhealth=\"100\"",
        ),
    );
    let pet = state.creatures().get(601).expect("the companion");
    assert_eq!(pet.ally(), Some(Ally::Companion));
    assert_eq!(pet.hostile(), Some(false));
    feed(
        &mut state,
        &listed(602, "snowcat", "peak snowcat", "hostile=\"1\""),
    );
    let foe = state.creatures().get(602).expect("the same name, hostile");
    assert_eq!(foe.ally(), None);
}

/// **The case the hunt is exposed to**: registered off a combat line, so no
/// `<crtrStatus>` has said anything and `hostile()` is `None`.
#[test]
fn a_companion_no_status_has_described_is_still_an_ally() {
    let mut state = GameState::default();
    feed(
        &mut state,
        &format!(
            "You blinded a {}!\n{PROMPT}",
            link(555, "snowcat", "peak snowcat")
        ),
    );
    let pet = state.creatures().get(555).expect("registered by the line");
    assert_eq!(pet.hostile(), None);
    assert_eq!(pet.ally(), Some(Ally::Companion));
}

// --- 3. endings and phases --------------------------------------------------

#[test]
fn a_vaporized_creature_is_dead_leaves_no_corpse_and_is_counted_once() {
    let mut state = GameState::default();
    feed(
        &mut state,
        &listed(2418, "servant", "horned kobold servant", "hostile=\"1\""),
    );
    assert!(
        state
            .creatures()
            .get(2418)
            .is_some_and(CreatureInstance::valid_target)
    );
    feed(
        &mut state,
        &format!(
            "Rather abrupt decompression causes a {} to explode!\n{PROMPT}",
            link(2418, "servant", "horned kobold servant")
        ),
    );
    let gone = state.creatures().get(2418).expect("still known");
    assert_eq!(gone.ending(), Some(Ending::Vaporized));
    assert!(!gone.valid_target());
    assert!(!gone.corpse(), "nothing to loot");
    assert_eq!(state.creatures().targets().count(), 0);
    assert_eq!(deaths(&mut state), [Some(2418)]);
    feed(&mut state, PROMPT);
    assert!(deaths(&mut state).is_empty(), "a kill is counted once");

    // The next listing leaves it out: it is accounted for, not hiding.
    feed(
        &mut state,
        &listed(2419, "kobold", "big ugly kobold", "hostile=\"1\""),
    );
    assert_eq!(state.creatures().departed().collect::<Vec<_>>(), [2418]);
    assert_eq!(state.creatures().vanished_unaccounted().count(), 0);
    // The control: one that vanishes with no line is reported.
    feed(&mut state, &listed(7, "rat", "giant rat", "hostile=\"1\""));
    let unaccounted: Vec<i64> = state.creatures().vanished_unaccounted().collect();
    assert_eq!(unaccounted, [2419]);
}

/// The blob's own order (`creature_message.rs` quotes the author): the
/// `dead="1"` tag first, the prose after. The flag alone would call it a
/// corpse; the line says there is nothing to loot.
#[test]
fn a_vaporized_creature_the_wire_flagged_dead_first_is_still_no_corpse() {
    let mut state = GameState::default();
    feed(
        &mut state,
        &listed(2418, "servant", "horned kobold servant", "hostile=\"1\""),
    );
    feed(
        &mut state,
        "<crtrStatus exist=\"2418\" hostile=\"1\" dead=\"1\"/>\n",
    );
    assert!(
        state
            .creatures()
            .get(2418)
            .is_some_and(CreatureInstance::corpse)
    );
    feed(
        &mut state,
        &format!(
            "Rather abrupt decompression causes a {} to explode!\n{PROMPT}",
            link(2418, "servant", "horned kobold servant")
        ),
    );
    let gone = state.creatures().get(2418).expect("still known");
    assert!(!gone.corpse(), "the flag said dead, the line said no body");
    assert_eq!(deaths(&mut state), [Some(2418)]);
}

#[test]
fn every_vaporizing_line_is_read_and_a_listing_that_names_it_again_outranks_it() {
    for line in [
        "Blast disperses the {} into a fine mist.",
        "The body of a {} inverts as the intense vacuum rips it to shreds!",
    ] {
        let mut state = GameState::default();
        feed(
            &mut state,
            &listed(2418, "servant", "horned kobold servant", "hostile=\"1\""),
        );
        let text = line.replace("{}", &link(2418, "servant", "horned kobold servant"));
        feed(&mut state, &format!("{text}\n{PROMPT}"));
        let gone = state.creatures().get(2418).expect("known");
        assert_eq!(gone.ending(), Some(Ending::Vaporized), "{line}");
        // The game lists it again, dead: there is a body after all.
        feed(
            &mut state,
            &listed(2418, "servant", "horned kobold servant", "dead=\"1\""),
        );
        let back = state.creatures().get(2418).expect("known");
        assert_eq!(back.ending(), None);
        assert!(back.corpse());
    }
}

#[test]
fn a_line_about_an_unknown_creature_changes_nothing() {
    let mut state = GameState::default();
    feed(
        &mut state,
        &format!(
            "Rather abrupt decompression causes a {} to explode!\n",
            link(31, "servant", "horned kobold servant")
        ),
    );
    assert!(state.creatures().get(31).is_none());
}

#[test]
fn the_captain_leaves_by_a_portal_alive_and_unlooted() {
    let mut state = GameState::default();
    feed(
        &mut state,
        &listed(
            7001,
            "captain",
            "battle-worn Empyrean captain",
            "hostile=\"1\"",
        ),
    );
    // No link: the one captain in the room.
    feed(
        &mut state,
        "Staggering, the battle-worn Empyrean captain rips open a rainbowed portal and dives into it!\n",
    );
    let captain = state.creatures().get(7001).expect("known");
    assert_eq!(captain.ending(), Some(Ending::Portal));
    assert!(!captain.valid_target() && !captain.corpse());
    assert_eq!(state.creatures().fled(7001), Some(None));
    feed(&mut state, PROMPT);
    assert!(deaths(&mut state).is_empty(), "a retreat is not a kill");
}

#[test]
fn the_captain_collapses_then_fades_leaving_nothing() {
    let mut state = GameState::default();
    let captain = link(7001, "captain", "battle-worn Empyrean captain");
    feed(
        &mut state,
        &listed(
            7001,
            "captain",
            "battle-worn Empyrean captain",
            "hostile=\"1\"",
        ),
    );
    feed(
        &mut state,
        &format!("A look of surprise crosses a {captain}'s face before she collapses.\n{PROMPT}"),
    );
    let fallen = state.creatures().get(7001).expect("known");
    assert_eq!(fallen.ending(), Some(Ending::Collapsed));
    assert!(fallen.corpse() && !fallen.valid_target());
    assert_eq!(deaths(&mut state), [Some(7001)]);
    feed(
        &mut state,
        &format!("The form of a {captain} stretches, fading away with a final shimmer.\n{PROMPT}"),
    );
    let faded = state.creatures().get(7001).expect("known");
    assert_eq!(faded.ending(), Some(Ending::Faded));
    assert!(!faded.corpse());
    assert!(deaths(&mut state).is_empty(), "the same kill, once");
}

#[test]
fn two_captains_and_no_link_is_not_guessed() {
    let mut state = GameState::default();
    feed(
        &mut state,
        "<component id='room objs'>  You also see<crtrStatus exist=\"7001\" hostile=\"1\"/><b> <pushBold/>a <a exist=\"7001\" noun=\"captain\">battle-worn Empyrean captain</a><popBold/></b> and<crtrStatus exist=\"7002\" hostile=\"1\"/><b> <pushBold/>a <a exist=\"7002\" noun=\"captain\">battle-worn Empyrean captain</a><popBold/></b>.</component>\n",
    );
    feed(
        &mut state,
        "The battle-worn Empyrean captain tears open a portal and steps into it, vanishing.\n",
    );
    assert!(state.creatures().in_room().all(|c| c.ending().is_none()));
}

#[test]
fn the_cold_wyrm_is_in_the_phase_the_last_line_stated() {
    let mut state = GameState::default();
    let wyrm = || link(8001, "wyrm", "silver-scaled cold wyrm");
    feed(
        &mut state,
        &listed(8001, "wyrm", "silver-scaled cold wyrm", "hostile=\"1\""),
    );
    let phase = |state: &GameState| {
        state
            .creatures()
            .get(8001)
            .and_then(CreatureInstance::phase)
    };
    assert_eq!(phase(&state), None, "no line yet is not grounded");
    feed(
        &mut state,
        &format!(
            "A {} plummets toward the ground, sending out a radiating wall of devastation!\n",
            wyrm()
        ),
    );
    assert_eq!(phase(&state), Some(BossPhase::Grounded));
    feed(
        &mut state,
        &format!(
            "A {}'s muscles bunch and she launches herself into the air!\n",
            wyrm()
        ),
    );
    assert_eq!(phase(&state), Some(BossPhase::Airborne));
    feed(
        &mut state,
        &format!(
            "Corruscations of color play along a {}'s scaled hide, disrupting the attack!\n",
            wyrm()
        ),
    );
    assert_eq!(phase(&state), Some(BossPhase::Shielded));
    // A refresh lists the wyrm again and keeps the phase.
    feed(
        &mut state,
        &listed(8001, "wyrm", "silver-scaled cold wyrm", "hostile=\"1\""),
    );
    assert_eq!(phase(&state), Some(BossPhase::Shielded));
    assert!(
        state
            .creatures()
            .get(8001)
            .is_some_and(CreatureInstance::valid_target)
    );
}
