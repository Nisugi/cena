//! What the game says you can attack: the `combat` dialog's `dDBTarget`.
//!
//! > **AUTHOR, 2026-09-21:** *"if you look at lich dDBTarget is part of the
//! > equation how it determines an npc is hostile or not, before the hostile
//! > flag of course."*
//!
//! MEASURED before writing any of this, over one live session:
//!
//! ```sh
//! grep -c "<dialogData id='combat'"        -> 2031   # the noisiest dialog
//! grep -c "dDBTarget"                      -> 1034
//! grep -rn 'dDBTarget' crates/cena-model   -> 0      # read by nothing
//! ```
//!
//! The dropdown line here is verbatim from that session.

use cena_model::GameState;
use cena_model::state::targeting::{Targeting, parse_ids};
use cena_protocol::Parser;

/// A real populated dropdown, from the live log.
const WARG: &str = concat!(
    r#"<dialogData id='combat'><dropDownBox id='dDBTarget' value="giant warg" "#,
    r#"cmd="target %dDBTarget%" tooltip='Select Target' content_text="giant warg" "#,
    r##"content_value="#94076783" anchor_left='cmdTarget' anchor_right='cmdAttack' "##,
    r#"height='20' width='80'  top='93' left='0' align='n'/></dialogData>"#,
    "\n",
);

/// The empty state, also verbatim: `none`, and `content_value` is not an id.
const NO_TARGET: &str = concat!(
    r#"<dialogData id='combat'><dropDownBox id='dDBTarget' value="none" "#,
    r#"cmd="target %dDBTarget%" content_text="none" content_value="target help" "#,
    r#"height='20' width='80'/></dialogData>"#,
    "\n",
);

fn fed(wire: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

#[test]
fn a_real_dropdown_reaches_the_model() {
    // The whole point: 1,034 of these a session, and nothing read them.
    let state = fed(WARG);
    assert!(state.targeting.is_stated());
    assert_eq!(state.targeting.current(), Some(94_076_783));
    assert_eq!(state.targeting.name(), Some("giant warg"));
    assert_eq!(state.targeting.ids(), [94_076_783]);
    assert_eq!(state.targeting.is_targetable(94_076_783), Some(true));
    assert_eq!(state.targeting.is_targetable(1), Some(false));
}

#[test]
fn no_target_is_stated_rather_than_unknown() {
    // §5.2. `value="none"` with `content_value="target help"` -- neither is an
    // id, and the game HAS spoken.
    assert_eq!(GameState::default().targeting.is_targetable(1), None);
    let state = fed(NO_TARGET);
    assert!(state.targeting.is_stated());
    assert!(state.targeting.ids().is_empty());
    assert_eq!(state.targeting.current(), None);
    assert_eq!(
        state.targeting.name(),
        None,
        "`none` is not a creature name"
    );
    assert_eq!(state.targeting.is_targetable(1), Some(false));
}

#[test]
fn content_value_is_a_comma_separated_list() {
    // `xmlparser.rb:776` splits on commas -- and reading it as ONE id was my
    // first mistake, corrected by that source. A single id is the common case
    // on the wire, not the shape.
    assert_eq!(parse_ids("#1,#2,#3"), [1, 2, 3]);
    assert_eq!(parse_ids("#94076783"), [94_076_783]);
    // Lich's pattern is `\#(\-?\d+)`: negative ids are players.
    assert_eq!(parse_ids("#-10202515,#42"), [-10_202_515, 42]);
    // Not ids, and not an error -- the wire's empty state.
    assert_eq!(parse_ids("target help"), [] as [i64; 0]);
    assert_eq!(parse_ids(""), [] as [i64; 0]);
    // A malformed entry is skipped rather than taking the rest with it.
    assert_eq!(parse_ids("#1,rubbish,#2"), [1, 2]);
}

#[test]
fn the_current_target_is_the_first_of_the_list() {
    // `xmlparser.rb:781-785`.
    let mut t = Targeting::default();
    t.read("#7,#8,#9", Some("a kobold"));
    assert_eq!(t.current(), Some(7));
    assert_eq!(t.ids(), [7, 8, 9]);
}

#[test]
fn a_restatement_of_the_same_target_is_not_a_change() {
    // 1,034 rows a session, mostly identical. A consumer must be able to
    // ignore them.
    let mut t = Targeting::default();
    assert!(
        t.read("#94076783", Some("giant warg")),
        "the first is a change"
    );
    assert!(
        !t.read("#94076783", Some("giant warg")),
        "the second is not"
    );
    assert!(t.read("#94076784", Some("giant warg")), "a new id is");
}

#[test]
fn a_targetable_id_in_no_room_list_is_something_you_cannot_see() {
    // The author's point, and Lich's `GameObj.hidden_targets`
    // (`gameobj.rb:1171`). The room shows one creature; the game offers two
    // targets. The second is there and unseen.
    //
    // **The room fixture is the game's full idiom, and it had to be.** A
    // creature joins the roster only when a `<crtrStatus>` vouches for a
    // BOLDED `room objs` link (`state/creatures.rs:11-26`) -- so an unbolded
    // link, and a bolded one with no tag, both leave 111 unregistered and
    // reported as hidden. This test found that by failing twice.
    let wire = concat!(
        "<component id='room objs'>You also see a ",
        "<pushBold/><a exist=\"111\" noun=\"kobold\">kobold</a><popBold/>.",
        "<crtrStatus exist=\"111\" inferior=\"1\"/></component>\n",
        r##"<dialogData id='combat'><dropDownBox id='dDBTarget' value="kobold" content_value="#111,#222"/></dialogData>"##,
        "\n",
    );
    let state = fed(wire);
    assert_eq!(
        state.hidden_targets(),
        [222],
        "the id the room never mentioned"
    );
}

#[test]
fn nothing_is_hidden_when_the_room_accounts_for_every_target() {
    let wire = concat!(
        "<component id='room objs'>You also see a ",
        "<pushBold/><a exist=\"111\" noun=\"kobold\">kobold</a><popBold/>.",
        "<crtrStatus exist=\"111\" inferior=\"1\"/></component>\n",
        r##"<dialogData id='combat'><dropDownBox id='dDBTarget' value="kobold" content_value="#111"/></dialogData>"##,
        "\n",
    );
    assert!(fed(wire).hidden_targets().is_empty());
}

#[test]
fn another_dropdown_in_the_same_dialog_is_not_the_target() {
    // `dDBAmmo` and `dDBStance` arrive in the SAME `combat` dialog -- MEASURED
    // 319 and 102 in one session. Reading any dropdown as the target would
    // make the ammo selection a creature.
    let wire = concat!(
        r#"<dialogData id='combat'><dropDownBox id='dDBAmmo' value="none" "#,
        r#"content_value="clear"/></dialogData>"#,
        "\n",
    );
    let state = fed(wire);
    assert!(
        !state.targeting.is_stated(),
        "the ammo dropdown was read as the target list"
    );
}

#[test]
fn a_reconnect_forgets_what_you_could_attack() {
    let mut state = fed(WARG);
    assert!(state.targeting.is_stated(), "guard");
    state.invalidate_for_reconnect();
    assert!(!state.targeting.is_stated());
    assert_eq!(state.targeting.current(), None);
}

#[test]
fn a_cast_time_is_read_and_is_not_the_action_roundtime() {
    // `xmlparser.rb:770` keeps `@cast_roundtime_end` beside `@roundtime_end`:
    // they run at once and expire independently.
    let state = fed("<castTime value='1788719145'/><roundTime value='1788719100'/>\n");
    assert_eq!(state.cast_time_ends, Some(1_788_719_145));
    assert_eq!(state.roundtime_ends, Some(1_788_719_100));
}

#[test]
fn a_cast_does_not_survive_a_reconnect_though_a_roundtime_does() {
    // The server enforces an action roundtime across the gap; a spell's
    // preparation is gone with the connection.
    let mut state = fed("<castTime value='1788719145'/><roundTime value='1788719100'/>\n");
    state.invalidate_for_reconnect();
    assert_eq!(state.cast_time_ends, None);
    assert_eq!(
        state.roundtime_ends,
        Some(1_788_719_100),
        "the roundtime is the server's and is KEPT"
    );
}
