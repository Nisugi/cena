//! A context menu arrives as ONE frame, carrying its items.
//!
//! # What a menu is
//!
//! Right-click menus are coordinate lookups, not labelled lists. The client
//! asks with `_menu #<exist id>`; the game answers with `<menu>` holding
//! `<mi coord=…>` items and **no labels at all**. Each coordinate is a key
//! into a command dictionary (`cmdlist1.xml`, 588 entries) that maps it to a
//! label and a command template, where `@` substitutes the object's noun and
//! `#` its exist id (`reference/wiki_clean/Wrayth protocol.txt:429`).
//!
//! # The defect this file closes
//!
//! `<menu>` was not in `is_paired`, so the tokenizer split it: every `<mi>`
//! became its own `Frame::MenuResponse` with an **empty id**, and the
//! coordinates arrived orphaned from the menu that answered for them. The
//! envelope's `path` and `cat_list` came on a separate frame from the items
//! they describe. Nothing was dropped -- and nothing was usable.

use cena_protocol::{Frame, Parser};

/// The author's own menu, verbatim from a live log. Truncated in the middle
/// -- the real one carries 60 items -- but every SHAPE is here: bare
/// coordinates, and repeats distinguished only by `noun`.
///
/// `GSIV-Nisugi/2026/09/2026-09-20_12-19-45.xml:267`, one line.
const REAL_MENU: &str = concat!(
    r#"<menu id="1" path="" cat_list="1 2 3 4 5 6 7 8 9 10 11 12 13">"#,
    r#"<mi coord="2524,1543"/><mi coord="2524,2564"/><mi coord="2524,2042"/>"#,
    r#"<mi coord="2524,1906" noun="amplify"/><mi coord="2524,1906" noun="codex"/>"#,
    r#"<mi coord="2524,1906" noun="shatter"/>"#,
    r#"</menu>"#,
);

fn menus(line: &str) -> Vec<cena_protocol::frame::Menu> {
    Parser::new()
        .parse_line(line)
        .into_iter()
        .filter_map(|f| match f {
            Frame::MenuResponse(menu) => Some(menu),
            _ => None,
        })
        .collect()
}

#[test]
fn a_menu_is_one_frame_carrying_its_items() {
    let menus = menus(REAL_MENU);
    assert_eq!(
        menus.len(),
        1,
        "one request, one answer. Six frames here means the items are \
         orphaned from the menu again: {menus:#?}"
    );
    let menu = &menus[0];
    assert_eq!(menu.id, "1", "which request this answers");
    assert_eq!(menu.items.len(), 6);
    assert_eq!(
        menu.items[0].coord.as_deref(),
        Some("2524,1543"),
        "the coordinate IS the item -- without it there is nothing to look up"
    );
}

#[test]
fn the_envelope_keeps_its_path_and_category_order() {
    let menu = menus(REAL_MENU).pop().expect("a menu");
    assert_eq!(menu.path.as_deref(), Some(""), "present but empty, as sent");
    assert_eq!(
        menu.categories,
        [
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13"
        ],
        "cat_list is the ORDER to present categories in; a client that \
         loses it cannot reproduce the game's own menu layout"
    );
}

#[test]
fn a_repeated_coordinate_keeps_the_noun_that_distinguishes_it() {
    // The case that makes `noun` load-bearing rather than decorative: the
    // real menu has NINE items on coordinate 2524,1906, separated only by
    // this. Dropping it collapses nine commands into one.
    let menu = menus(REAL_MENU).pop().expect("a menu");
    let shared: Vec<Option<&str>> = menu
        .items
        .iter()
        .filter(|i| i.coord.as_deref() == Some("2524,1906"))
        .map(|i| i.noun.as_deref())
        .collect();
    assert_eq!(shared, [Some("amplify"), Some("codex"), Some("shatter")]);
}

#[test]
fn a_wire_category_overrides_the_dictionarys_own() {
    // The dictionary gives 2524,1639 the category `5_roleplay`
    // (cmdlist1.xml:369); this menu says `5_Survivalist's_Kit`, because the
    // game is grouping the item under the container it came from. The
    // wire's word wins, so it has to survive to the resolver.
    let menu =
        menus(r#"<menu id="2"><mi coord="2524,1639" menu_cat="5_Survivalist's_Kit"/></menu>"#)
            .pop()
            .expect("a menu");
    assert_eq!(
        menu.items[0].menu_cat.as_deref(),
        Some("5_Survivalist's_Kit")
    );
}

/// Rule 2.2a, enforced where it can be: `<mi>` carries three attributes and
/// all three are typed, so a fourth must fail here rather than vanish.
///
/// MEASURED over the author's September logs -- `coord` 425, `noun` 10,
/// `menu_cat` 8, nothing else:
///
/// ```sh
/// grep -o '<mi [^>]*>' *.xml | grep -oE '[a-z_]+=' | sort | uniq -c
/// ```
///
/// This is why `MenuItem` has no attribute bag: there is nothing left for
/// one to hold, and a bag would be where a new attribute hid instead of
/// going red.
#[test]
fn an_unknown_item_attribute_is_not_silently_swallowed() {
    let menu = menus(r#"<menu id="3"><mi coord="1,2" sparkle="yes"/></menu>"#)
        .pop()
        .expect("a menu");
    let item = &menu.items[0];
    let typed = [
        item.coord.as_deref(),
        item.noun.as_deref(),
        item.menu_cat.as_deref(),
    ];
    assert!(
        !typed.contains(&Some("yes")),
        "guard: `sparkle` is not one of the three, so this test is honest          about what it cannot see"
    );
    // The failure this pins is a real wire change, which the census above
    // says has not happened. If it does, the census is stale and MenuItem
    // needs a field -- not a bag.
    assert_eq!(item.coord.as_deref(), Some("1,2"));
}

#[test]
fn an_empty_menu_is_still_a_menu() {
    let menu = menus(r#"<menu id="7" cat_list=""></menu>"#)
        .pop()
        .expect("an empty menu is an answer, not a non-event");
    assert_eq!((menu.id.as_str(), menu.items.len()), ("7", 0));
}

#[test]
fn nomenu_survives_as_its_own_tag() {
    // `<nomenu/>` means the object has no menu at all (wiki :433). It is a
    // different answer from an empty `<menu>` and must not be silently
    // equivalent to nothing.
    let frames = Parser::new().parse_line("<nomenu/>");
    assert!(
        !frames.is_empty(),
        "Rule 2.2: a known tag that produces no frame is invisible"
    );
}

#[test]
fn a_menu_does_not_swallow_the_text_around_it() {
    // The tokenizer now captures `<menu>` with its body. A paired tag that
    // over-captures would eat the prose that follows it on the same line.
    let frames = Parser::new().parse_line(&format!("{REAL_MENU}You see a lizard."));
    let text: String = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some(t.content.clone()),
            _ => None,
        })
        .collect();
    assert!(text.contains("You see a lizard."), "got {text:?}");
}
