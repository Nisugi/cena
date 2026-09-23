//! `GameState.vitals` holds the CHARACTER's bars, not everyone's.
//!
//! `<progressBar>` is also how the game ships other creatures' health: an
//! appraisal opens `<dialogData id="injuries-{existID}">` carrying its own
//! `health2` bar (wiki `:243`). Keying vitals on `bar.id` alone let an
//! appraised target's health overwrite the player's, so the model would report
//! the character at 12% because something they looked at was.

use cena_model::{GameState, Vital};
use cena_protocol::Parser;

fn health_after(lines: &[&str]) -> Option<u32> {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    state.vitals.get("health2").map(|v| v.percent)
}

#[test]
fn an_appraised_targets_health_does_not_become_the_players() {
    // The player's own injuries dialog: this IS the character.
    let own = health_after(&[
        r"<dialogData id='injuries'><progressBar id='health2' value='99'/></dialogData>",
    ]);
    assert_eq!(
        own,
        Some(99),
        "a bar inside the player's own `injuries` dialog is the player's"
    );

    // Then an appraisal of something else, which must NOT overwrite it.
    let after_appraisal = health_after(&[
        r"<dialogData id='injuries'><progressBar id='health2' value='99'/></dialogData>",
        r#"<dialogData id="injuries-10070682"><progressBar id='health2' value='12'/></dialogData>"#,
    ]);
    assert_eq!(
        after_appraisal,
        Some(99),
        "an `injuries-{{existID}}` dialog belongs to an APPRAISED TARGET, not \
         the character. Keying on `bar.id` alone reported the player at 12% \
         because something they appraised was hurt."
    );
}

/// A gauge keeps the two numbers the wire stated, not just the percentage.
///
/// The percent alone cannot answer "can I afford this spell" or "how many
/// points of healing is that", which is what a Heal or Hunt behavior asks.
/// The parser has parsed `current/max` out of `text=` since M2; the model
/// used to drop both on the floor.
mod amounts {
    use super::*;

    fn minivitals(bars: &str) -> GameState {
        let mut parser = Parser::new();
        let mut state = GameState::default();
        let line = format!("<dialogData id='minivitals'>{bars}</dialogData>");
        for frame in parser.parse_line(&line) {
            state.apply(&frame);
        }
        state
    }

    #[test]
    fn current_and_max_survive_into_the_model() {
        let state = minivitals(
            "<progressBar id='health' value='95' text='health 213/223'/>\
             <progressBar id='mana' value='50' text='mana 60/120'/>",
        );
        let health = state.health().expect("health");
        assert_eq!(
            (health.percent, health.current, health.max),
            (95, Some(213), Some(223))
        );
        assert_eq!(health.amount(), Some((213, 223)));
        // the named accessor and the wire id agree
        assert_eq!(state.mana().and_then(Vital::amount), Some((60, 120)));
        assert_eq!(state.vital("mana"), state.mana());
    }

    #[test]
    fn the_games_own_percent_is_kept_not_recomputed() {
        // 213/223 is 95.5%: the wire says 95, and a bar renders what the
        // game said rather than what we would have rounded to.
        let state = minivitals("<progressBar id='health' value='95' text='health 213/223'/>");
        assert_eq!(state.health().map(|v| v.percent), Some(95));
    }

    #[test]
    fn negative_health_keeps_its_sign() {
        // The interesting case: a character bleeding out reads as -10, not
        // +10. `payload::Amount` records Vellum getting this wrong.
        let state = minivitals("<progressBar id='health' value='0' text='health -10/125'/>");
        assert_eq!(state.health().and_then(Vital::amount), Some((-10, 125)));
    }

    #[test]
    fn a_label_only_bar_states_no_amount() {
        // `mindState` is a label, not a pair. Fabricating (percent, 100)
        // here is exactly what `payload::Amount`'s doc refuses.
        let state = minivitals("<progressBar id='mindState' value='34' text='clear'/>");
        let mind = state.vital("mindState").expect("the bar landed");
        assert_eq!((mind.percent, mind.amount()), (34, None));
    }

    #[test]
    fn an_absent_gauge_is_none_rather_than_zero() {
        let state = minivitals("<progressBar id='health' value='95' text='health 213/223'/>");
        assert_eq!(state.spirit(), None, "never observed, not empty");
        assert_eq!(state.stamina(), None);
    }
}
