//! **M2 step 3: experience, injuries, stance and encumbrance.**
//!
//! All four are wire-driven attributes with no regex anywhere, which is the
//! finding `plan/18` §1 records: Lich scrapes experience and injuries out of
//! prose *and* reads the dialog (`common/xmlparser.rb:725-735`), so reading its
//! directory names would have deferred both to the text-scraped half. Reading the
//! wire puts them here.
//!
//! MEASURED over 24 files across 6 characters, `<dialogData id=>`: `combat`
//! 5,869, `minivitals` 5,235, `expr` 2,439, `injuries` 1,892, `encum` 941,
//! `stance` 377. The effect dialogs are `plan/17`'s; `combat` and `quick*` are UI
//! affordances (`plan/18` §2b defers them).
//!
//! # `plan/18` guessed the injuries shape wrong, and the wire corrected it
//!
//! §2b said *"per-body-part injury and scar, 16 parts, named on the wire"* and
//! assumed `<progressBar>`, the shape that carries everything else. MEASURED: the
//! `injuries` dialog carries only a `health2` bar. The body parts arrive as
//! `<image id="leftArm" name="Injury1"/>`, where **`name` equal to `id` means
//! unhurt**.
//!
//! VERIFIED against Lich, which reads it identically and supplies two facts the
//! wire alone does not (`lib/common/xmlparser.rb:809-822`): a scar CLEARS the
//! wound, because a scar is what a healed wound leaves; and `nsys` -- the nervous
//! system -- is one of the parts rather than a separate mechanism.

use cena_model::GameState;
use cena_protocol::Parser;

fn fold(wire: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

/// The `expr` dialog, verbatim from `GSIV-Zoleta`.
const EXPR: &[u8] = b"<dialogData id='expr'>\
<label id='yourLvl' value='Level 100' top='0' left='0' align='n' width='160' height='15'/>\
<progressBar id='mindState' value='0' text='clear as a bell' top='45' left='3' width='160'/>\
<progressBar id='nextLvlPB' value='100' text='11999265 experience' top='20' left='3'/>\
</dialogData>\n";

#[test]
fn the_expr_dialog_fills_experience() {
    let state = fold(EXPR);
    let exp = &state.character.experience;

    assert_eq!(exp.level.as_deref(), Some("Level 100"));
    assert_eq!(exp.mind_state.as_deref(), Some("clear as a bell"));
    assert_eq!(exp.mind_percent, Some(0));
    assert_eq!(exp.next_level.as_deref(), Some("11999265 experience"));
    assert_eq!(exp.next_level_percent, Some(100));
}

#[test]
fn the_level_is_kept_as_the_wire_wrote_it() {
    // `Level 100`, not `100`. Parsing a number out is a guess about a format
    // the game may change, and every consumer that displays it wants the string
    // back anyway.
    let state = fold(EXPR);
    assert_eq!(
        state.character.experience.level.as_deref(),
        Some("Level 100")
    );
}

#[test]
fn an_expr_bar_does_not_become_a_vital() {
    // The routing hazard this step introduces: one `<progressBar>` shape carries
    // gauges, stance, encumbrance AND advancement. `mindState` landing in
    // `vitals` would show up as a health-like gauge that nothing owns.
    let state = fold(EXPR);
    assert!(
        !state.vitals.contains_key("mindState"),
        "an expr bar leaked into vitals: {:?}",
        state.vitals
    );
    assert!(!state.vitals.contains_key("nextLvlPB"));
}

#[test]
fn the_players_own_vitals_still_land_in_vitals() {
    // The falsifying pair. Without it, routing EVERY dialog bar away from vitals
    // would satisfy the test above while emptying the thing vitals is for.
    let state = fold(
        b"<dialogData id='minivitals'><progressBar id='health' value='97' text='health 213/223'/></dialogData>\n",
    );
    assert_eq!(state.vitals.get("health"), Some(&97));
}

#[test]
fn a_bar_with_the_right_id_in_the_wrong_dialog_is_not_the_players() {
    // **The test the first fifteen did not have.** Dropping the dialog check
    // from bar routing -- matching on `id` alone -- passed all of them, because
    // every one sends its bar inside the correct dialog. Review pattern (C) in
    // `plan/19`, caught by falsifying.
    //
    // The hazard is real and the model already fights it elsewhere: an appraisal
    // opens `<dialogData id="injuries-{existID}">` carrying its OWN bars (wiki
    // `:243`), which is how a target's health once overwrote the player's.
    let state = fold(
        b"<dialogData id='injuries-12345'><progressBar id='mindState' value='99' text='someone else'/></dialogData>
",
    );

    assert_eq!(
        state.character.experience.mind_state, None,
        "a bar from a third party's dialog set the character's mind state"
    );
    assert!(
        !state.vitals.contains_key("mindState"),
        "...and rejecting it must not simply push it into vitals instead"
    );
}

#[test]
fn the_stance_bar_is_the_stance_from_either_dialog_that_carries_it() {
    // **A test premise the corpus corrected.** This file first asserted that a
    // `pbarStance` outside the `stance` dialog must be ignored. MEASURED over 15
    // files: `pbarStance` arrives in BOTH `stance` and `combat`, 130 times each,
    // with identical values -- one fact shown in two windows.
    //
    // Rejecting the `combat` copy did not drop it; it fell through to `vitals`,
    // where a real capture showed `pbarStance: 0` sitting beside health. So this
    // routes by ID, which is safe because nothing else on the wire is named
    // `pbarStance`, and the end-to-end check on a real capture is what caught it.
    let state = fold(
        b"<dialogData id='combat'><progressBar id='pbarStance' value='80' text='guarded (80%)' tooltip='Percent of stance contributing to defense'/></dialogData>
",
    );

    assert_eq!(state.character.stance.as_deref(), Some("guarded (80%)"));
    assert_eq!(state.character.stance_percent, Some(80));
    assert!(
        !state.vitals.contains_key("pbarStance"),
        "the stance bar leaked into vitals: {:?}",
        state.vitals
    );
}

#[test]
fn the_stance_dialog_fills_stance() {
    let state = fold(
        b"<dialogData id='stance'><progressBar id='pbarStance' value='100' text='defensive (100%)' tooltip='Percent of stance contributing to defense'/></dialogData>\n",
    );
    assert_eq!(state.character.stance.as_deref(), Some("defensive (100%)"));
    assert_eq!(state.character.stance_percent, Some(100));
}

#[test]
fn the_encum_dialog_fills_both_its_halves() {
    // Two elements, two facts: the bar is the level, the label is the prose a
    // player reads.
    let state = fold(
        b"<dialogData id='encum'>\
<progressBar id='encumlevel' value='0' text='None' top='5' left='-5' width='160'/>\
<label id='encumblurb' value='You are not encumbered enough to notice.' top='10'/>\
</dialogData>\n",
    );
    assert_eq!(state.character.encumbrance.as_deref(), Some("None"));
    assert_eq!(state.character.encumbrance_percent, Some(0));
    assert_eq!(
        state.character.encumbrance_detail.as_deref(),
        Some("You are not encumbered enough to notice.")
    );
}

#[test]
fn an_injury_image_records_a_wound() {
    // The real wire shape. `height='0' width='0'` and all.
    let state = fold(
        b"<dialogData id='injuries'><image id=\"leftArm\" name=\"Injury1\" height=\"0\" width=\"0\"/></dialogData>\n",
    );
    let hurt = state.character.injuries.get("leftArm").copied();
    assert_eq!(hurt.map(|i| (i.wound, i.scar)), Some((1, 0)));
}

#[test]
fn a_healthy_part_is_absent_rather_than_zero() {
    // `name` equal to `id` means whole. Storing it as a zero would make
    // `injuries` a list of every body part on every refresh, and `is_empty()`
    // would stop answering "is this character unhurt".
    let state =
        fold(b"<dialogData id='injuries'><image id=\"neck\" name=\"neck\"/></dialogData>\n");
    assert!(
        state.character.injuries.is_empty(),
        "a healthy part was recorded as an injury: {:?}",
        state.character.injuries
    );
}

#[test]
fn healing_removes_the_part_from_the_map() {
    // The refresh case, and the one a naive insert-only model gets wrong: the
    // game re-sends every part, so a healed arm arrives as `name="leftArm"`.
    let state = fold(
        b"<dialogData id='injuries'><image id=\"leftArm\" name=\"Injury2\"/></dialogData>\n\
          <dialogData id='injuries'><image id=\"leftArm\" name=\"leftArm\"/></dialogData>\n",
    );
    assert!(
        state.character.injuries.is_empty(),
        "a healed part was still reported hurt: {:?}",
        state.character.injuries
    );
}

#[test]
fn a_scar_clears_the_wound() {
    // Lich's rule (`xmlparser.rb:813-816`), and it is not obvious from the wire:
    // a scar is what a HEALED wound leaves behind, so reporting rank 2 wound AND
    // rank 1 scar would double-count one injury.
    let state = fold(
        b"<dialogData id='injuries'><image id=\"back\" name=\"Injury2\"/></dialogData>\n\
          <dialogData id='injuries'><image id=\"back\" name=\"Scar1\"/></dialogData>\n",
    );
    let scarred = state
        .character
        .injuries
        .get("back")
        .copied()
        .expect("still tracked");
    assert_eq!(
        (scarred.wound, scarred.scar),
        (0, 1),
        "a scar left the old wound in place, double-counting one injury"
    );
}

#[test]
fn the_nervous_system_is_a_body_part_like_any_other() {
    // `nsys` appears 29 times in the sample as an ordinary body-part image.
    // Lich gives it special handling for its own nerve tracker; the fact itself
    // is the same shape.
    let state =
        fold(b"<dialogData id='injuries'><image id=\"nsys\" name=\"Injury3\"/></dialogData>\n");
    assert_eq!(
        state.character.injuries.get("nsys").map(|i| i.wound),
        Some(3)
    );
}

#[test]
fn an_image_outside_the_injuries_dialog_is_not_an_injury() {
    // 1,313 of 2,155 `<image>` tags in the sample are `nomap.jpg` map tiles.
    // Before `Frame::InjuryImage` carried its dialog, every one of them was
    // reported as a body part.
    let state = fold(
        b"<image id='nomap' name='nomap.jpg'/>\n\
          <dialogData id='mapViewMain'><image id='head' name='Injury3'/></dialogData>\n",
    );
    assert!(
        state.character.injuries.is_empty(),
        "an image outside the injuries dialog was taken for a wound: {:?}",
        state.character.injuries
    );
}

#[test]
fn a_label_outside_a_dialog_is_ignored() {
    // `<label id='yourLvl'>` means a character level inside `expr`. A bare one,
    // or one in another dialog, is something else entirely.
    let state = fold(
        b"<label id='yourLvl' value='Level 1'/>\n\
          <dialogData id='mapViewMain'><label id='yourLvl' value='Ta Vaalor'/></dialogData>\n",
    );
    assert_eq!(
        state.character.experience.level, None,
        "a label from another dialog set the character's level"
    );
}

#[test]
fn a_partial_dialog_leaves_the_rest_unknown() {
    // The dialogs are partial: a burst may carry the level and not the mind
    // state. `None` is "not yet told", which `plan/12` §5.2 makes a first-class
    // value rather than a default.
    let state =
        fold(b"<dialogData id='expr'><label id='yourLvl' value='Level 42'/></dialogData>\n");
    assert_eq!(
        state.character.experience.level.as_deref(),
        Some("Level 42")
    );
    assert_eq!(state.character.experience.mind_state, None);
    assert_eq!(state.character.experience.mind_percent, None);
}

/// **The character model survives a reconnect**, except the shroud flag.
///
/// This test was `a_reconnect_forgets_all_four`, and its comment read:
/// *"MEASURED (`plan/15` §2b): NONE of the four dialogs is in the login
/// burst."* §2b is about `crtrStatus` gaining health and says nothing about
/// the burst -- a citation that resolves to a real section not supporting the
/// claim, which is `CLAUDE.md`'s `styleIfClosed` hazard in its harder form. The
/// claim is false -- MEASURED 2026-09-20 from `<app>` to the first client
/// command, `dialogData id='expr'`, `id='injuries'`, `encumlevel` and
/// `encumblurb` are all present with real values. Only `pbarStance` is absent.
///
/// The rule was wrong too:
///
/// > **AUTHOR, 2026-09-20:** *"time stops for 99.9% of things when you're
/// > offline ... you can't really change rooms when you're logged off."*
///
/// A logged-off character is out of the world, so nothing re-stances them,
/// wounds them, or changes what they carry. Absence from the burst means the
/// server had no need to restate a fact that never stopped being true.
#[test]
fn the_character_model_survives_a_reconnect() {
    let mut wire = EXPR.to_vec();
    wire.extend_from_slice(
        b"<dialogData id='injuries'><image id=\"head\" name=\"Injury1\"/></dialogData>
",
    );
    wire.extend_from_slice(
        b"<dialogData id='stance'><progressBar id='pbarStance' value='80' text='forward'/></dialogData>
",
    );
    let mut state = fold(&wire);

    // Guards, so the assertions below cannot pass on a default.
    assert!(state.character.experience.level.is_some());
    assert!(!state.character.injuries.is_empty());
    assert!(state.character.stance.is_some());

    state.invalidate_for_reconnect();

    assert!(
        state.character.experience.level.is_some(),
        "the burst carries `expr`, and experience is the one thing that moves          offline -- so the burst's value is the authority, not a default"
    );
    assert!(
        !state.character.injuries.is_empty(),
        "wounds do not heal while the character is out of the world"
    );
    assert!(
        state.character.stance.is_some(),
        "stance is the one dialog genuinely absent from the burst, and that          is not a reason to forget it -- nobody re-stances a logged-off          character"
    );
}

/// **The shroud flag is the one thing a reconnect drops, and not because of
/// elapsed time.**
///
/// It is a *suppression* flag: while true, an `info` report's identity fields
/// are refused, because Shroud of Deception falsifies them. A suppression flag
/// that outlives the evidence for it is the one shape where keeping is worse
/// than forgetting -- it silently discards good data on the strength of an
/// effect nobody has re-observed.
///
/// A stale fact is corrected by the next observation. A stale refusal to
/// observe is not.
#[test]
fn the_shroud_flag_does_not_outlive_its_effect_list() {
    let mut state = fold(EXPR);
    state.character.shrouded = true;
    assert!(state.character.shrouded, "guard");

    state.invalidate_for_reconnect();

    assert!(!state.character.shrouded);
}
