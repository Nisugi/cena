//! The whole-inventory snapshot reaches the model and answers the questions
//! the passive container model cannot.
//!
//! `<inventoryManager>` was an untyped attribute bag carrying only its two
//! envelope attributes, and every item row fell out separately as
//! `Frame::Structural` -- because `<i>` sat in the parser's STYLING arm.
//! `<inventoryViewItem>` was a second bag whose `<result>` sections landed in
//! `Frame::WindowHints`, the window placement bag.

use cena_model::GameState;
use cena_protocol::Parser;

/// A real snapshot, trimmed from the author's September logs: a worn bow, a
/// worn cloak that is a container, two items inside it, and a closed purse.
const SNAPSHOT: &str = concat!(
    r"<inventoryManager id='im9445b1acb' room='7503206'>",
    r#"<i id='698' loc='worn,player' name="a scorched,glowbark long,bow" weight='3'/>"#,
    r#"<i id='706' loc='worn,player' name="a nacreous,plumille,cloak" weight='4' in_max='2000'/>"#,
    r#"<i id='758' loc='in,706' name="a sapphire-set,platinum,crown" weight='2'/>"#,
    r#"<i id='818' loc='in,706' name="a,coal black,purse" weight='3' flags='closed' in_max='50'/>"#,
    r"</inventoryManager>",
);

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    state
}

#[test]
fn the_whole_tree_reaches_the_model() {
    let state = state_after(&[SNAPSHOT]);
    let inv = &state.inventory_snapshot;
    assert_eq!(inv.len(), 4);
    assert!(inv.is_known());
    assert_eq!(inv.room(), Some("7503206"));
    assert_eq!(
        inv.get("698").map(|i| i.name.as_str()),
        Some("a scorched glowbark long bow")
    );
}

#[test]
fn an_unasked_inventory_is_not_an_empty_one() {
    // §5.2: "nobody has asked" and "you carry nothing" are different facts,
    // and a behavior that confuses them would conclude the character is
    // empty-handed and act on it.
    let unasked = GameState::default();
    assert!(!unasked.inventory_snapshot.is_known());
    assert!(unasked.inventory_snapshot.is_empty());

    let answered = state_after(&[r"<inventoryManager id='im1' room='1'></inventoryManager>"]);
    assert!(answered.inventory_snapshot.is_known());
    assert!(answered.inventory_snapshot.is_empty());
}

#[test]
fn the_tree_answers_what_is_inside_a_container() {
    // The question the passive model cannot answer unless the container's
    // window happens to be open.
    let state = state_after(&[SNAPSHOT]);
    let inside: Vec<&str> = state
        .inventory_snapshot
        .contents_of("706")
        .map(|i| i.noun.as_str())
        .collect();
    assert_eq!(inside, ["crown", "purse"]);
}

#[test]
fn the_tree_answers_what_is_on_the_person() {
    let state = state_after(&[SNAPSHOT]);
    let worn: Vec<&str> = state
        .inventory_snapshot
        .on_person()
        .map(|i| i.noun.as_str())
        .collect();
    assert_eq!(worn, ["bow", "cloak"], "nested items are not on the person");
}

#[test]
fn find_anywhere_reaches_into_a_closed_container() {
    let state = state_after(&[SNAPSHOT]);
    let found: Vec<&str> = state
        .inventory_snapshot
        .find("crown")
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(found, ["758"]);
}

#[test]
fn total_weight_excludes_what_cannot_be_carried() {
    // A `-1` weight is the wire's sentinel for a fixture, so summing it
    // would SUBTRACT from the carried total.
    let state = state_after(&[concat!(
        r"<inventoryManager id='im1' room='1'>",
        r#"<i id='1' loc='worn,player' name="a,heavy,pack" weight='10'/>"#,
        r#"<i id='2' loc='room' name="a,stone,bench" weight='-1'/>"#,
        r"</inventoryManager>",
    )]);
    assert_eq!(state.inventory_snapshot.total_weight(), 10);
}

#[test]
fn a_snapshot_replaces_rather_than_merges() {
    // The snapshot IS the inventory: anything absent was dropped, sold or
    // stored. A merge would keep a sold item forever.
    let state = state_after(&[
        SNAPSHOT,
        concat!(
            r"<inventoryManager id='im2' room='7503206'>",
            r#"<i id='698' loc='worn,player' name="a scorched,glowbark long,bow" weight='3'/>"#,
            r"</inventoryManager>",
        ),
    ]);
    assert_eq!(state.inventory_snapshot.len(), 1);
    assert!(
        state.inventory_snapshot.get("706").is_none(),
        "the cloak is gone"
    );
}

#[test]
fn a_failed_response_does_not_erase_a_good_snapshot() {
    // `state='stale'` is the server reporting a failed load. Replacing a
    // good tree with a failure would lose the whole inventory on a transient
    // error -- but the marker must be visible so a caller can re-request.
    let state = state_after(&[
        SNAPSHOT,
        r"<inventoryManager id='im2' room='7503206' state='stale'></inventoryManager>",
    ]);
    assert_eq!(state.inventory_snapshot.len(), 4, "the good tree survived");
    assert_eq!(state.inventory_snapshot.state(), Some("stale"));
}

#[test]
fn a_continuation_marks_the_snapshot_incomplete() {
    // Absent from the author's logs -- 0 across 36 snapshots -- but a
    // truncated snapshot that claims to be whole is worse than one that says
    // so: "find anywhere" would answer "not found" from a partial tree.
    let state = state_after(&[concat!(
        r"<inventoryManager id='im1' room='1'>",
        r#"<i id='1' loc='worn,player' name="a,real,item" weight='1'/>"#,
        r"<continuation root='706' last='758'/>",
        r"</inventoryManager>",
    )]);
    assert!(!state.inventory_snapshot.is_complete());
    assert_eq!(state.inventory_snapshot.pending().len(), 1);
    assert_eq!(state.inventory_snapshot.pending()[0].root, "706");

    let whole = state_after(&[SNAPSHOT]);
    assert!(whole.inventory_snapshot.is_complete());
}

mod details {
    use super::*;

    const DETAIL: &[&str] = &[
        r#"<inventoryViewItem id='iv1' exist='706'><result command='look'>A <a exist="706" noun="cloak">cloak</a> of plumille."#,
        r"</result><result command='analyze'>Largely free from restrictions.",
        r"</result></inventoryViewItem>",
    ];

    #[test]
    fn a_detail_response_attaches_to_its_item() {
        let mut lines = vec![SNAPSHOT];
        lines.extend_from_slice(DETAIL);
        let state = state_after(&lines);
        let inv = &state.inventory_snapshot;
        assert_eq!(inv.details("706").map(<[_]>::len), Some(2));
        assert_eq!(
            inv.detail("706", "analyze").map(|d| d.text.as_str()),
            Some("Largely free from restrictions.")
        );
        assert_eq!(inv.detail("706", "recall"), None, "never asked");
    }

    #[test]
    fn a_detail_section_keeps_its_links() {
        let mut lines = vec![SNAPSHOT];
        lines.extend_from_slice(DETAIL);
        let state = state_after(&lines);
        let look = state
            .inventory_snapshot
            .detail("706", "look")
            .expect("the look section");
        assert_eq!(look.links.len(), 1);
        assert_eq!(look.links[0].text, "cloak");
    }

    #[test]
    fn a_torn_response_does_not_replace_a_complete_one() {
        let mut lines = vec![SNAPSHOT];
        lines.extend_from_slice(DETAIL);
        // A block the prompt tore mid-send.
        lines.push(r"<inventoryViewItem id='iv2' exist='706'><result command='look'>Half a des");
        lines.push(r"<prompt time='1'>&gt;</prompt>");
        let state = state_after(&lines);
        assert_eq!(
            state.inventory_snapshot.details("706").map(<[_]>::len),
            Some(2),
            "the complete description survived the torn one"
        );
    }

    #[test]
    fn details_for_an_item_that_left_are_dropped_with_it() {
        // A description describes an item. Keeping it after the item is gone
        // means answering `look` for something no longer carried.
        let mut lines = vec![SNAPSHOT];
        lines.extend_from_slice(DETAIL);
        lines.push(r"<inventoryManager id='im2' room='1'></inventoryManager>");
        let state = state_after(&lines);
        assert!(state.inventory_snapshot.details("706").is_none());
    }
}

#[test]
fn the_snapshot_survives_a_reconnect() {
    // A logged-off character gains and loses nothing, and the snapshot is
    // NOT in the login burst -- clearing it would leave `is_known()` false
    // with no way back until the user asked again by hand.
    //
    // Contrast `inventory`, the passive container model, which IS cleared:
    // it mirrors windows the server reopens on login.
    let mut state = state_after(&[SNAPSHOT]);
    assert_eq!(state.inventory_snapshot.len(), 4, "guard: known first");
    state.invalidate_for_reconnect();
    assert_eq!(state.inventory_snapshot.len(), 4);
    assert!(state.inventory_snapshot.is_known());
}
