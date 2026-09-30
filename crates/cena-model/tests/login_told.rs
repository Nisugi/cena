//! `GameState::login` (`plan/51` §7, step 4): a login built from what a
//! state knows, for the player's Lich started late. A fresh model told it
//! must know what the original did, piece by piece, for everything it tells.
//!
//! The original reads real traffic: the committed login, vitals, effects and
//! room fixtures. Each comparison first checks the original knew the thing,
//! so none passes by comparing two blanks.

use cena_model::GameState;
use cena_protocol::Parser;

/// What the indicators Lich keeps are called on the wire.
const INDICATORS: [&str; 13] = [
    "IconBLEEDING",
    "IconPOISONED",
    "IconDISEASED",
    "IconSTANDING",
    "IconKNEELING",
    "IconSITTING",
    "IconPRONE",
    "IconSTUNNED",
    "IconHIDDEN",
    "IconINVISIBLE",
    "IconDEAD",
    "IconWEBBED",
    "IconJOINED",
];

fn read(markup: &str, state: &mut GameState) {
    for frame in Parser::new().push_bytes(markup.as_bytes()) {
        state.apply(&frame);
    }
}

/// The state real traffic leaves, and a fresh one told its login.
fn original_and_told() -> (GameState, GameState, String) {
    let mut original = GameState::default();
    read(
        concat!(
            include_str!("../../cena-protocol/tests/fixtures/login_setup.xml"),
            include_str!("../../cena-protocol/tests/fixtures/login_burst_full.xml"),
            include_str!("../../cena-protocol/tests/fixtures/vitals.xml"),
            include_str!("../../cena-protocol/tests/fixtures/vitals_secondary.xml"),
            include_str!("../../cena-protocol/tests/fixtures/effect_dialogs.xml"),
            include_str!("../../cena-protocol/tests/fixtures/room_populated.xml"),
            "<compass><dir value=\"n\"/><dir value=\"out\"/></compass>
",
            "<component id='room objs'>You also see <pushBold/>a <a exist=\"71\" noun=\"kobold\">kobold</a><popBold/> and a <a exist=\"72\" noun=\"table\">low table</a>.</component>
",
            "<component id='room players'>Also here: <a exist=\"-73\" noun=\"Ashryn\">Ashryn</a>.</component>
",
            "<dialogData id='injuries'><image id='leftArm' name='Scar2'/><image id='leftArm' name='Injury1'/><image id='head' name='Injury2'/><image id='nsys' name='Nsys2'/></dialogData>\n",
            "<spell exist='spell'>Tangleweed</spell>\n",
            "<prompt time=\"1790000000\">&gt;</prompt>\n",
        ),
        &mut original,
    );
    let login = original.login();
    let mut told = GameState::default();
    read(&login, &mut told);
    (original, told, login)
}

#[test]
fn who_the_character_is() {
    let (original, told, login) = original_and_told();
    let (was, is) = (&original.character, &told.character);
    assert!(was.name.is_some() && was.player_id.is_some() && was.game_code.is_some());
    assert_eq!(
        (&is.name, &is.player_id, &is.instance, &is.game_code),
        (&was.name, &was.player_id, &was.instance, &was.game_code),
        "{login}"
    );
}

#[test]
fn the_gauges() {
    let (original, told, login) = original_and_told();
    assert!(original.vitals.len() >= 4, "{:?}", original.vitals);
    assert_eq!(told.vitals, original.vitals, "{login}");
    let (was, is) = (&original.character, &told.character);
    assert!(was.stance.is_some() && was.encumbrance.is_some());
    assert_eq!(
        (&is.stance, is.stance_percent),
        (&was.stance, was.stance_percent)
    );
    assert_eq!(
        (
            &is.encumbrance,
            is.encumbrance_percent,
            &is.encumbrance_detail
        ),
        (
            &was.encumbrance,
            was.encumbrance_percent,
            &was.encumbrance_detail
        )
    );
    let (was, is) = (&was.experience, &is.experience);
    assert!(was.mind_state.is_some() && was.next_level.is_some() && was.level.is_some());
    assert_eq!(
        (
            &is.level,
            &is.mind_state,
            is.mind_percent,
            &is.next_level,
            is.next_level_percent
        ),
        (
            &was.level,
            &was.mind_state,
            was.mind_percent,
            &was.next_level,
            was.next_level_percent
        )
    );
}

#[test]
fn the_indicators_spell_hands_and_injuries() {
    let (original, told, login) = original_and_told();
    assert!(INDICATORS.iter().any(|id| original.status.get(id)));
    for id in INDICATORS {
        assert_eq!(told.status.get(id), original.status.get(id), "{id}");
    }
    assert_eq!(original.prepared.as_deref(), Some("Tangleweed"));
    assert_eq!(told.prepared, original.prepared);
    // The spell hand an object, `#spell`, as the game sends it (the
    // author, 2026-09-29: *"spell hand is a link as well"*).
    assert_eq!(original.prepared_id.as_deref(), Some("spell"));
    assert_eq!(told.prepared_id, original.prepared_id, "{login}");
    assert_eq!(
        (&told.left_hand, &told.right_hand),
        (&original.left_hand, &original.right_hand),
        "{login}"
    );
    assert!(original.character.injuries.len() >= 3);
    assert_eq!(told.character.injuries, original.character.injuries);
    // The nerves as the window gives them: a rank, its kind unsaid.
    assert_eq!(original.character.nerves.rank, 2);
    assert_eq!(told.character.nerves.rank, 2);
}

/// No spell prepared, the game's `<spell>None</spell>` carries no id, and
/// the one kept from the spell before goes.
#[test]
fn no_spell_prepared_is_no_object() {
    let mut state = GameState::default();
    read("<spell exist='spell'>Breeze</spell>\n", &mut state);
    assert_eq!(state.prepared_id.as_deref(), Some("spell"));
    read("<spell>None</spell>\n", &mut state);
    assert_eq!(state.prepared.as_deref(), Some("None"));
    assert_eq!(state.prepared_id, None);
}

#[test]
fn the_room() {
    let (original, told, login) = original_and_told();
    let (was, is) = (&original.room, &told.room);
    assert!(was.id.is_some() && was.title.is_some() && was.meta.is_some());
    assert!(was.exits.is_some() && !was.creatures.is_empty() && !was.players.is_empty());
    assert_eq!(
        (&is.id, &is.title, &is.exits, &is.meta),
        (&was.id, &was.title, &was.exits, &was.meta),
        "{login}"
    );
    assert_eq!(is.description, was.description);
    let named = |items: &[cena_model::RoomItem]| -> Vec<(String, String, String)> {
        items
            .iter()
            .map(|item| (item.id.clone(), item.noun.clone(), item.text.clone()))
            .collect()
    };
    assert_eq!(named(&is.creatures), named(&was.creatures));
    assert_eq!(named(&is.objects), named(&was.objects));
    assert_eq!(named(&is.players), named(&was.players));
}

#[test]
fn the_effects_with_the_time_they_have_left() {
    let (original, told, login) = original_and_told();
    let now = original.game_time_now().expect("a prompt was read");
    assert!(original.effects.len() >= 3);
    assert_eq!(told.effects.len(), original.effects.len(), "{login}");
    for (id, effect) in original.effects.iter() {
        let again = told.effects.get(id).expect(id);
        assert_eq!(
            (&again.category, &again.text, again.percent),
            (&effect.category, &effect.text, effect.percent)
        );
        let (was, is) = (
            original.effects.remaining(id, now),
            told.effects.remaining(id, now),
        );
        assert!(
            was.zip(is)
                .map_or(was == is, |(was, is)| was.abs_diff(is) <= 1),
            "{id}: {was:?} then {is:?}"
        );
    }
}

#[test]
fn what_it_wears_and_its_prompt() {
    let (original, told, login) = original_and_told();
    assert!(original.worn.items().is_some_and(|items| !items.is_empty()));
    assert_eq!(told.worn.items(), original.worn.items(), "{login}");
    assert_eq!(told.prompt, original.prompt);
}

/// Before the game has named the character, there is nothing to tell.
#[test]
fn nothing_before_the_character_is_named() {
    assert_eq!(GameState::default().login(), "");
}
