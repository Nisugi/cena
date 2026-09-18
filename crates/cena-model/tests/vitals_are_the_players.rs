//! `GameState.vitals` holds the CHARACTER's bars, not everyone's.
//!
//! `<progressBar>` is also how the game ships other creatures' health: an
//! appraisal opens `<dialogData id="injuries-{existID}">` carrying its own
//! `health2` bar (wiki `:243`). Keying vitals on `bar.id` alone let an
//! appraised target's health overwrite the player's, so the model would report
//! the character at 12% because something they looked at was.

use cena_model::GameState;
use cena_protocol::Parser;

fn health_after(lines: &[&str]) -> Option<u32> {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    state.vitals.get("health2").copied()
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
