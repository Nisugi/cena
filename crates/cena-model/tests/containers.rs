//! The stow and ready lists.
//!
//! Every wire line here is reconstructed from the patterns in
//! `infomon/xmlparser.rb:514-527` -- each one is that regex with its optional
//! groups taken, so a line that fails here would have failed Lich too.

use cena_model::{GameState, ReadySlot, StoreMode, StowSlot};
use cena_protocol::Parser;

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    for frame in parser.parse_line("<prompt time=\"1\">&gt;</prompt>") {
        state.apply(&frame);
    }
    state
}

/// An `<a exist= noun=>` link, as both lists carry them.
fn link(id: &str, noun: &str, text: &str) -> String {
    format!("<a exist=\"{id}\" noun=\"{noun}\">{text}</a>")
}

mod stow {
    use super::{GameState, StowSlot, link, state_after};

    const OPENER: &str = "You have the following containers set as stow targets:";

    /// One `stow list` row, per `StowListContainer` (`xmlparser.rb:516`).
    fn row(id: &str, noun: &str, text: &str, slot: &str) -> String {
        format!("  a {} ({slot})", link(id, noun, text))
    }

    fn listed() -> GameState {
        state_after(&[
            OPENER,
            &row("12345", "backpack", "leather backpack", "gem"),
            &row("12346", "pouch", "silk pouch", "herb"),
            &row("12347", "sack", "burlap sack", "default"),
        ])
    }

    #[test]
    fn a_listed_container_is_held_with_its_id() {
        let c = listed().containers;
        let gem = c.stow(StowSlot::Gem).expect("gem container");
        assert_eq!(gem.id, "12345", "what a command targets");
        assert_eq!(gem.noun, "backpack");
        assert_eq!(gem.text, "leather backpack");
    }

    #[test]
    fn reading_the_whole_list_marks_it_checked() {
        assert!(listed().containers.stow_checked());
        assert!(
            !GameState::default().containers.stow_checked(),
            "guard: not checked before anything is read"
        );
    }

    #[test]
    fn a_category_the_list_omits_is_unknown_not_empty() {
        // §5.2. `stow list` prints only the categories that are SET, so an
        // absent row means "nothing set", and there is no row that says so.
        assert_eq!(listed().containers.stow(StowSlot::Box), None);
    }

    #[test]
    fn restating_the_list_forgets_what_it_no_longer_names() {
        // Lich's `reset` at the opener (`xmlparser.rb:589`). A category the
        // game stops listing has been cleared, and keeping it would report a
        // container that is not a stow target any more.
        let mut state = listed();
        assert!(
            state.containers.stow(StowSlot::Herb).is_some(),
            "guard: known first"
        );
        for line in [
            super::stow::OPENER,
            &super::stow::row("12345", "backpack", "leather backpack", "gem"),
        ] {
            let mut parser = cena_protocol::Parser::new();
            for frame in parser.parse_line(line) {
                state.apply(&frame);
            }
            for frame in parser.parse_line("<prompt time=\"2\">&gt;</prompt>") {
                state.apply(&frame);
            }
        }
        assert_eq!(state.containers.stow(StowSlot::Herb), None, "no longer set");
        assert!(state.containers.stow(StowSlot::Gem).is_some(), "still set");
    }

    #[test]
    fn every_one_of_the_fourteen_categories_is_read() {
        // `stowlist.rb:6`, ported whole. A fifteenth category added by the
        // game is a visible failure here rather than a silent miss.
        for slot in StowSlot::ALL {
            let state = state_after(&[&row("-1", "sack", "a sack", slot.as_str())]);
            assert!(
                state.containers.stow(slot).is_some(),
                "{} should be read",
                slot.as_str()
            );
        }
    }

    #[test]
    fn a_stow_set_confirmation_teaches_one_category() {
        // `StowSetContainer1` (`xmlparser.rb:517`), which SHOUTS the category
        // where the list whispers it.
        let state = state_after(&[&format!(
            "Set \"a {}\" to be your STOW GEM container.",
            link("999", "pouch", "velvet pouch")
        )]);
        assert_eq!(
            state.containers.stow(StowSlot::Gem).map(|i| i.id.clone()),
            Some("999".to_owned())
        );
        assert!(
            !state.containers.stow_checked(),
            "one confirmation is not a whole list"
        );
    }

    #[test]
    fn the_default_container_has_its_own_wording() {
        // `StowSetContainer2` (`xmlparser.rb:518`) -- the game says
        // "your default STOW container", not "your STOW DEFAULT container".
        let state = state_after(&[&format!(
            "Set \"a {}\" to be your default STOW container.",
            link("888", "sack", "burlap sack")
        )]);
        assert_eq!(
            state
                .containers
                .stow(StowSlot::Default)
                .map(|i| i.id.clone()),
            Some("888".to_owned())
        );
    }

    #[test]
    fn a_confirmation_replaces_the_category_it_names() {
        let state = state_after(&[
            OPENER,
            &row("12345", "backpack", "leather backpack", "gem"),
            &format!(
                "Set \"a {}\" to be your STOW GEM container.",
                link("999", "pouch", "velvet pouch")
            ),
        ]);
        assert_eq!(
            state.containers.stow(StowSlot::Gem).map(|i| i.id.clone()),
            Some("999".to_owned()),
            "one container per category"
        );
    }
}

mod ready {
    use super::{ReadySlot, StoreMode, link, state_after};

    const OPENER: &str = "Your current settings are:";
    const CLOSER: &str = concat!(
        "To change your default item for a category that is already set, ",
        "clear the category first by clicking on the item in the list above.  ",
        "Click <d cmd=\"ready list\">here</d> to update the list."
    );

    /// One `ready list` row for a slot that IS set, and one for a slot that
    /// is not.
    ///
    /// **Copied from real wire**, not from `ReadyListNormal`'s regex: the two
    /// forms differ in a way the pattern's optional `\(?` hides. MEASURED in
    /// `E:/Gemstone/dev/lich-5/logs`, `2026-09-01_10-04-51`:
    ///
    /// ```text
    ///   weapon: <d cmd="store WEAPON clear">a <a exist="208924336" ...>...</a></d> (<d cmd='store set'>put in sheath</d>)
    ///   shield: (<d cmd='ready SHIELD'>none</d>) (<d cmd='store set'>put in sheath</d>)
    /// ```
    ///
    /// A set row has NO parentheses around its item; an unset one does. An
    /// earlier draft of these tests parenthesised both, which is the shape
    /// the wire never sends.
    fn row(label: &str, cmd: &str, item: &str, store: &str) -> String {
        format!(
            "  {label}: <d cmd=\"store {cmd} clear\">{item}</d> (<d cmd='store set'>{store}</d>)"
        )
    }

    /// An unset row: `shield: (<d cmd='ready SHIELD'>none</d>) (...)`.
    fn empty_row(label: &str, cmd: &str, store: &str) -> String {
        format!("  {label}: (<d cmd='ready {cmd}'>none</d>) (<d cmd='store set'>{store}</d>)")
    }

    #[test]
    fn a_row_teaches_both_the_item_and_where_it_is_stored() {
        let state = state_after(&[
            OPENER,
            &row(
                "weapon",
                "WEAPON",
                &format!("a {}", link("555", "broadsword", "fine broadsword")),
                "stowed",
            ),
            CLOSER,
        ]);
        let c = state.containers;
        assert_eq!(
            c.ready(ReadySlot::Weapon).map(|i| i.id.clone()),
            Some("555".to_owned())
        );
        assert_eq!(c.store_mode(ReadySlot::Weapon), Some(StoreMode::Stowed));
    }

    #[test]
    fn a_row_saying_none_leaves_the_slot_unset() {
        // The game prints the ROW whether or not the slot is filled, and
        // `none` is not an item. Reading it as one would hand a behavior a
        // weapon called "none".
        let state = state_after(&[OPENER, &empty_row("shield", "SHIELD", "stowed"), CLOSER]);
        assert_eq!(state.containers.ready(ReadySlot::Shield), None);
        assert_eq!(
            state.containers.store_mode(ReadySlot::Shield),
            Some(StoreMode::Stowed),
            "the mode is still stated"
        );
    }

    #[test]
    fn the_list_is_checked_only_once_its_closing_line_arrives() {
        // **Lich's rule, and it is the right one** (`xmlparser.rb:609`). A
        // list cut off by a disconnect halfway is not a list you have read,
        // and `checked` is what a consumer asks before trusting a gap.
        let cut_short = state_after(&[
            OPENER,
            &row(
                "weapon",
                "WEAPON",
                &format!("a {}", link("555", "sword", "sword")),
                "stowed",
            ),
        ]);
        assert!(!cut_short.containers.ready_checked());
        assert!(
            cut_short.containers.ready(ReadySlot::Weapon).is_some(),
            "what did arrive is still believed"
        );

        let whole = state_after(&[OPENER, CLOSER]);
        assert!(whole.containers.ready_checked());
    }

    #[test]
    fn a_sheath_row_carries_no_store_mode() {
        // `ReadyListSheathsSet` (`xmlparser.rb:524`) has no `(store)` group,
        // because a sheath IS where something is stored. Asking where to
        // store the sheath is not a question the game offers.
        let state = state_after(&[
            OPENER,
            &format!(
                "  sheath: <d cmd=\"store SHEATH clear\">a {}</d>",
                link("777", "scabbard", "leather scabbard")
            ),
            CLOSER,
        ]);
        assert_eq!(
            state
                .containers
                .ready(ReadySlot::Sheath)
                .map(|i| i.id.clone()),
            Some("777".to_owned())
        );
        assert_eq!(state.containers.store_mode(ReadySlot::Sheath), None);
        assert!(
            !ReadySlot::Sheath.has_store_mode(),
            "and the type says so, per readylist.rb:22"
        );
    }

    #[test]
    fn every_one_of_the_nine_slots_is_read() {
        // `readylist.rb:6`, ported whole.
        for slot in ReadySlot::ALL {
            let state = state_after(&[&format!(
                "  {}: (<d cmd='ready X clear'>a {}</d>) (<d cmd='store set'>stowed</d>)",
                slot.as_str(),
                link("-1", "thing", "a thing")
            )]);
            assert!(
                state.containers.ready(slot).is_some(),
                "{} should be read",
                slot.as_str()
            );
        }
    }

    #[test]
    fn clearing_a_slot_clears_its_store_mode_too() {
        // **A FIX, not a port.** Lich's `ReadyItemClear` handler
        // (`xmlparser.rb:611-613`) writes only the item and leaves the mode
        // behind, so a slot with no item still reports where that item would
        // be stored -- an answer about nothing. A consumer asking "where does
        // my shield go" gets `stowed` for a shield it does not have.
        let state = state_after(&[
            OPENER,
            &row(
                "shield",
                "SHIELD",
                &format!("a {}", link("111", "shield", "a shield")),
                "stowed",
            ),
            CLOSER,
            "Cleared your default shield.",
        ]);
        assert_eq!(state.containers.ready(ReadySlot::Shield), None);
        assert_eq!(
            state.containers.store_mode(ReadySlot::Shield),
            None,
            "no item, so no answer about where it goes"
        );
    }

    #[test]
    fn a_ready_confirmation_teaches_one_slot() {
        // `ReadyItemSet` (`xmlparser.rb:527`).
        let state = state_after(&[&format!(
            "Setting a {} to be your default ranged weapon.",
            link("222", "bow", "composite bow")
        )]);
        assert_eq!(
            state
                .containers
                .ready(ReadySlot::RangedWeapon)
                .map(|i| i.id.clone()),
            Some("222".to_owned())
        );
    }

    #[test]
    fn a_two_word_slot_is_read_whole() {
        let state = state_after(&[&format!(
            "Setting a {} to be your default secondary weapon.",
            link("333", "dagger", "a dagger")
        )]);
        assert_eq!(
            state
                .containers
                .ready(ReadySlot::SecondaryWeapon)
                .map(|i| i.id.clone()),
            Some("333".to_owned())
        );
        assert_eq!(
            state.containers.ready(ReadySlot::Weapon),
            None,
            "not the primary"
        );
    }
}

mod store_mode {
    use super::{ReadySlot, StoreMode, link, state_after};

    #[test]
    fn the_two_vocabularies_read_to_one_value() {
        // **THE DEFECT THIS TYPE EXISTS FOR.** `ready list` and `store set`
        // word the same three states differently -- `ReadyListNormal`
        // (`xmlparser.rb:522`) against `ReadyStoreSet` (`:526`) -- and Lich
        // stores whichever string arrived last (`:605`, `:617`), so a
        // consumer comparing it gets a different answer depending on which
        // message happened to teach it.
        for (from_list, from_confirmation, expected) in [
            (
                "worn if possible, stowed otherwise",
                "worn if possible and stowed if not",
                StoreMode::WornIfPossible,
            ),
            ("stowed", "stowed", StoreMode::Stowed),
            ("put in sheath", "stored in your sheath", StoreMode::Sheath),
            (
                "put in secondary sheath",
                "stored in your secondary sheath",
                StoreMode::SecondarySheath,
            ),
        ] {
            assert_eq!(
                StoreMode::parse(from_list),
                Some(expected),
                "as `ready list` words it"
            );
            assert_eq!(
                StoreMode::parse(from_confirmation),
                Some(expected),
                "as `store set` words it -- same state, same value"
            );
        }
    }

    #[test]
    fn a_secondary_sheath_is_not_a_sheath() {
        // Order matters: "stored in your secondary sheath" ENDS WITH
        // "sheath", so a naive suffix test answers the wrong slot.
        assert_eq!(
            StoreMode::parse("stored in your secondary sheath"),
            Some(StoreMode::SecondarySheath)
        );
    }

    #[test]
    fn a_store_set_confirmation_teaches_the_mode_alone() {
        // `ReadyStoreSet` (`xmlparser.rb:526`) names no item.
        let state = state_after(&[
            &format!(
                "Setting a {} to be your default weapon.",
                link("444", "sword", "a sword")
            ),
            "When storing your weapon, it will be worn if possible and stowed if not.",
        ]);
        assert_eq!(
            state.containers.store_mode(ReadySlot::Weapon),
            Some(StoreMode::WornIfPossible)
        );
        assert!(
            state.containers.ready(ReadySlot::Weapon).is_some(),
            "and the item is untouched"
        );
    }
}

#[test]
fn an_unrelated_line_with_a_link_is_not_a_container_event() {
    let state = state_after(&[&format!(
        "You see a {} lying here.",
        link("-1", "backpack", "leather backpack")
    )]);
    assert_eq!(state.containers, cena_model::Containers::default());
}

#[test]
fn both_lists_survive_a_reconnect() {
    // **The contrast with `group`, which is cleared.** Both hold `exist` ids,
    // but a group member is another player who leaves while we are gone,
    // whereas a stow container is YOUR backpack -- and the list itself is a
    // per-character setting the SERVER holds. Nothing about a dropped socket
    // changes either.
    //
    // The failure this pins is silent: neither list is re-sent by the login
    // burst, so clearing would leave a behavior with no stow container and no
    // event ever coming to restore one.
    let mut state = state_after(&[
        "You have the following containers set as stow targets:",
        &format!(
            "  a {} (gem)",
            link("12345", "backpack", "leather backpack")
        ),
    ]);
    assert!(
        state.containers.stow(StowSlot::Gem).is_some(),
        "guard: known first"
    );
    state.invalidate_for_reconnect();
    assert!(
        state.containers.stow(StowSlot::Gem).is_some(),
        "your backpack is still on your back"
    );
}

mod anchored_as_lich_anchors {
    //! Review finding: the stow-row and ready-row fallbacks matched the
    //! TRIMMED text of any main-window line. Lich's row patterns all open
    //! `^  ` (`xmlparser.rb:516`, `:522-524`), and the ready rows also require
    //! the row's own `<d cmd='store ...'>`/`<d cmd='ready ...'>` link.

    use super::{ReadySlot, StowSlot, link, state_after};

    #[test]
    fn an_unindented_line_ending_in_a_category_is_not_a_stow_row() {
        let state = state_after(&[&format!(
            "You notice a {} (gem)",
            link("12345", "pouch", "velvet pouch")
        )]);
        assert_eq!(state.containers.stow(StowSlot::Gem), None);
    }

    #[test]
    fn a_label_and_a_colon_is_not_a_ready_row() {
        // Unindented, and no slot command either.
        for wire in [
            format!("shield: {}", link("111", "shield", "a shield")),
            format!("  shield: {}", link("111", "shield", "a shield")),
        ] {
            let state = state_after(&[&wire]);
            assert_eq!(state.containers.ready(ReadySlot::Shield), None, "{wire}");
        }
    }

    #[test]
    fn a_row_that_names_no_item_does_not_clear_one() {
        // `unless match[:id].nil?` (`xmlparser.rb:600`): Lich writes a slot
        // only when the row named something. A confirmation taught the shield;
        // a stray `none` row with no opener before it is not evidence enough
        // to forget it.
        let state = state_after(&[
            &format!(
                "Setting a {} to be your default shield.",
                link("111", "shield", "kite shield")
            ),
            "  shield: (<d cmd='ready SHIELD'>none</d>) (<d cmd='store set'>stowed</d>)",
        ]);
        assert_eq!(
            state
                .containers
                .ready(ReadySlot::Shield)
                .map(|i| i.id.clone()),
            Some("111".to_owned())
        );
    }

    #[test]
    fn a_stow_confirmation_must_begin_set() {
        let state = state_after(&[&format!(
            "Bob says, \"a {}\" to be your STOW GEM container.",
            link("999", "pouch", "velvet pouch")
        )]);
        assert_eq!(state.containers.stow(StowSlot::Gem), None);
    }
}
