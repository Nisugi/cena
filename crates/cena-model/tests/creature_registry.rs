//! The creature registry fed from the wire: `<crtrStatus>` held until its
//! link, the roster rebuilt per `room objs`, cleared on `<nav>`, and the
//! reconciliation rule for the tag's flags.

use cena_model::state::creature::status::Classification;
use cena_model::{GameState, StatusName};
use cena_protocol::Parser;

fn feed(state: &mut GameState, parser: &mut Parser, wire: &str) {
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
}

const OBJS_TWO: &str = "<component id='room objs'>  You also see<crtrStatus exist=\"2418562\" hostile=\"1\" immobile=\"1\" stunned=\"1\" rooted=\"1\" prone=\"1\"/><b> <pushBold/>a <a exist=\"2418562\" noun=\"servant\">horned kobold servant</a><popBold/></b>,<crtrStatus exist=\"1049293\" inferior=\"1\"/><b> <pushBold/>a <a exist=\"1049293\" noun=\"wolf\">great white wolf</a><popBold/></b> and a <a exist=\"2404205\" noun=\"briar\">violently lashing emerald briar</a>.</component>\n";

#[test]
fn a_room_objs_body_registers_its_flagged_creatures_and_builds_the_roster() {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    feed(&mut state, &mut parser, OBJS_TWO);
    let reg = state.creatures();
    assert_eq!(reg.room_ids(), [2_418_562, 1_049_293]);
    assert_eq!(
        reg.len(),
        2,
        "the unflagged briar is an object, not a creature"
    );
    assert_eq!(reg.pending_status_count(), 0);
    let servant = reg.get(2_418_562).expect("servant");
    assert_eq!(
        (servant.name.as_str(), servant.noun.as_deref()),
        ("horned kobold servant", Some("servant"))
    );
    assert!(servant.flag(Classification::Hostile));
    assert!(
        servant.has_status(StatusName::Immobilized, None),
        "immobile -> immobilized"
    );
    assert!(servant.has_status(StatusName::Prone, None));
    assert!(servant.muckled(None));
    let wolf = reg.get(1_049_293).expect("wolf");
    assert!(wolf.flag(Classification::Inferior) && !wolf.flag(Classification::Hostile));
    assert_eq!(reg.targets().map(|c| c.id).collect::<Vec<_>>(), [2_418_562]);
}

#[test]
fn a_refresh_that_omits_a_creature_drops_it_from_the_roster_but_not_the_registry() {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    feed(&mut state, &mut parser, OBJS_TWO);
    feed(
        &mut state,
        &mut parser,
        "<component id='room objs'>  You also see<crtrStatus exist=\"1049293\" inferior=\"1\"/><b> <pushBold/>a <a exist=\"1049293\" noun=\"wolf\">great white wolf</a><popBold/></b>.</component>\n",
    );
    let reg = state.creatures();
    assert_eq!(reg.room_ids(), [1_049_293]);
    assert_eq!(reg.len(), 2, "the servant is still known, just not here");
}

#[test]
fn a_tags_snapshot_clears_absent_known_flags_but_leaves_message_statuses() {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    feed(&mut state, &mut parser, OBJS_TWO);
    // a message-only status the tag never carries
    feed(
        &mut state,
        &mut parser,
        "You blinded <pushBold/>a <a exist=\"2418562\" noun=\"servant\">horned kobold servant</a><popBold/>!\n<prompt time=\"5\">&gt;</prompt>\n",
    );
    assert!(
        state
            .creatures()
            .get(2_418_562)
            .is_some_and(|c| c.has_status(StatusName::Blind, None))
    );
    // the next tag says only hostile: stunned/prone/... are gone, blind stays
    feed(
        &mut state,
        &mut parser,
        "<component id='room objs'>  You also see<crtrStatus exist=\"2418562\" hostile=\"1\"/><b> <pushBold/>a <a exist=\"2418562\" noun=\"servant\">horned kobold servant</a><popBold/></b>.</component>\n",
    );
    let servant = state.creatures().get(2_418_562).expect("servant");
    assert!(!servant.has_status(StatusName::Stunned, None));
    assert!(!servant.has_status(StatusName::Prone, None));
    assert!(servant.has_status(StatusName::Blind, None));
    assert!(servant.flag(Classification::Hostile));
}

#[test]
fn a_standalone_tag_for_a_known_creature_applies_at_once() {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    feed(&mut state, &mut parser, OBJS_TWO);
    feed(
        &mut state,
        &mut parser,
        "<crtrStatus exist=\"1049293\" hostile=\"1\" stunned=\"1\"/>\n",
    );
    let wolf = state.creatures().get(1_049_293).expect("wolf");
    assert!(wolf.has_status(StatusName::Stunned, None));
    assert!(wolf.flag(Classification::Hostile));
    assert_eq!(state.creatures().pending_status_count(), 0);
}

#[test]
fn a_nav_clears_the_roster_and_any_held_tag() {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    feed(&mut state, &mut parser, "<nav rm='100'/>\n");
    feed(&mut state, &mut parser, OBJS_TWO);
    feed(
        &mut state,
        &mut parser,
        "<crtrStatus exist=\"777\" hostile=\"1\"/>\n",
    );
    assert_eq!(
        state.creatures().pending_status_count(),
        1,
        "guard: a tag is held"
    );
    assert_eq!(
        state.creatures().room_ids().len(),
        2,
        "guard: the roster is full"
    );
    feed(&mut state, &mut parser, "<nav rm='101'/>\n");
    assert!(state.creatures().room_ids().is_empty());
    assert_eq!(state.creatures().pending_status_count(), 0);
    assert_eq!(
        state.creatures().len(),
        2,
        "the registry survives a room change"
    );
}

#[test]
fn a_reconnect_forgets_the_roster_and_keeps_the_creatures() {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    feed(&mut state, &mut parser, OBJS_TWO);
    assert_eq!(state.creatures().room_ids().len(), 2, "guard");
    state.invalidate_for_reconnect();
    assert!(state.creatures().room_ids().is_empty());
    assert_eq!(state.creatures().len(), 2);
}

#[test]
fn housekeeping_drops_a_creature_unseen_for_ten_minutes_unless_it_is_in_the_room() {
    use cena_model::state::creatures::CLEANUP_MAX_AGE;
    let mut state = GameState::default();
    let mut parser = Parser::new();
    feed(
        &mut state,
        &mut parser,
        "<prompt time=\"1000\">&gt;</prompt>\n",
    );
    feed(&mut state, &mut parser, OBJS_TWO);
    // leave: the servant and wolf are now out of the room
    feed(
        &mut state,
        &mut parser,
        "<nav rm='200'/>\n<nav rm='201'/>\n",
    );
    let later = 1000 + CLEANUP_MAX_AGE + 1;
    feed(
        &mut state,
        &mut parser,
        &format!("<prompt time=\"{later}\">&gt;</prompt>\n"),
    );
    // a registration triggers housekeeping
    feed(
        &mut state,
        &mut parser,
        "<component id='room objs'>  You also see<crtrStatus exist=\"555\" hostile=\"1\"/><b> <pushBold/>a <a exist=\"555\" noun=\"rat\">giant rat</a><popBold/></b>.</component>\n",
    );
    let reg = state.creatures();
    assert!(reg.get(555).is_some());
    assert!(
        reg.get(2_418_562).is_none(),
        "unseen for > {CLEANUP_MAX_AGE}s and not here"
    );
    assert_eq!(reg.evicted(), 2);
}
