//! **M2 step 4: containers and what is in them.**
//!
//! The wire shape, verbatim from `GSIV-Nisugi`:
//!
//! ```text
//! <container id='stow' title="My Cloak" target='#64863904' location='right'/>
//! <clearContainer id="stow"/>
//! <inv id='stow'>In the <a exist="64863904" noun="cloak">cloak</a>:</inv>
//! <inv id='stow'> an <a exist="64863936" noun="dauber">undulant vaelfyren tentacle dauber</a></inv>
//! ```
//!
//! MEASURED over 12 files across 4 characters: **66,996** `<inv>` tags, 1,147
//! `<clearContainer>`, 167 `<container>`, 38 `<deleteContainer>`.
//!
//! # Three line shapes, one rule
//!
//! Of the `<inv>` lines: **45,839** carry an `<a exist=>` link, **817** are the
//! container's own header (`In the cloak:`), and the rest are the literal string
//! ` nothing`.
//!
//! An item is **a line carrying a link that is not the container itself**. That
//! one rule covers all three: the header's only link names the container, so it
//! is excluded by id rather than by matching prose; ` nothing` has no link at
//! all. No string matching, and nothing to break when the game rewords a header.

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

/// A full container listing, in the order and shape the wire sends it.
const CLOAK: &[u8] = b"<container id='stow' title=\"My Cloak\" target='#64863904' location='right'/>\
<clearContainer id=\"stow\"/>\
<inv id='stow'>In the <a exist=\"64863904\" noun=\"cloak\">cloak</a>:</inv>\
<inv id='stow'> an <a exist=\"64863936\" noun=\"dauber\">undulant vaelfyren tentacle dauber</a></inv>\
<inv id='stow'> a <a exist=\"64863935\" noun=\"card\">pass card</a></inv>\n";

fn nouns(state: &GameState, id: &str) -> Vec<String> {
    state
        .inventory
        .container(id)
        .map(|c| c.items.iter().map(|i| i.noun.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn a_container_listing_becomes_its_items() {
    let state = fold(CLOAK);
    assert_eq!(nouns(&state, "stow"), vec!["dauber", "card"]);
}

#[test]
fn the_header_line_is_not_an_item() {
    // `In the <a exist="64863904">cloak</a>:` names the container, and a
    // container is not inside itself. Excluded by ID, not by matching "In the".
    let state = fold(CLOAK);
    assert!(
        !nouns(&state, "stow").contains(&"cloak".to_owned()),
        "the container listed itself as its own contents: {:?}",
        nouns(&state, "stow")
    );
}

#[test]
fn an_empty_container_lists_nothing() {
    // The literal string ` nothing`, which has no link, so it falls out without
    // being recognised.
    let state = fold(
        b"<container id='64863895' title=\"a sack\"/>\
<inv id='64863895'> nothing</inv>\n",
    );
    assert!(nouns(&state, "64863895").is_empty());
    assert!(
        state.inventory.container("64863895").is_some(),
        "an empty container must still be known -- 'it is empty' is a fact"
    );
}

#[test]
fn the_title_is_kept() {
    let state = fold(CLOAK);
    assert_eq!(
        state
            .inventory
            .container("stow")
            .and_then(|c| c.title.as_deref()),
        Some("My Cloak")
    );
}

#[test]
fn a_clear_empties_the_items_but_keeps_the_title() {
    // The wire clears in order to refill. A window that lost its name mid-refresh
    // would flicker.
    let mut wire = CLOAK.to_vec();
    wire.extend_from_slice(b"<clearContainer id=\"stow\"/>\n");
    let state = fold(&wire);

    assert!(nouns(&state, "stow").is_empty());
    assert_eq!(
        state
            .inventory
            .container("stow")
            .and_then(|c| c.title.as_deref()),
        Some("My Cloak"),
        "the clear took the title with it"
    );
}

#[test]
fn a_redeclaration_does_not_drop_the_contents() {
    // `<container>` declares; it does not clear. The wire sends an explicit
    // `<clearContainer>` when it means to replace, so clearing here would make
    // that redundant AND would drop items on a re-declaration carrying none.
    let mut wire = CLOAK.to_vec();
    wire.extend_from_slice(b"<container id='stow' title=\"My Cloak\" location='left'/>\n");
    let state = fold(&wire);
    assert_eq!(nouns(&state, "stow"), vec!["dauber", "card"]);
}

#[test]
fn a_delete_removes_the_container_entirely() {
    let mut wire = CLOAK.to_vec();
    wire.extend_from_slice(b"<deleteContainer id=\"stow\"/>\n");
    let state = fold(&wire);
    assert!(state.inventory.container("stow").is_none());
}

#[test]
fn a_clear_touches_only_the_container_it_names() {
    let mut wire = CLOAK.to_vec();
    wire.extend_from_slice(
        b"<container id='pack' title=\"a pack\"/><inv id='pack'> a <a exist=\"9\" noun=\"rock\">rock</a></inv>\n",
    );
    wire.extend_from_slice(b"<clearContainer id=\"stow\"/>\n");
    let state = fold(&wire);

    assert!(nouns(&state, "stow").is_empty());
    assert_eq!(
        nouns(&state, "pack"),
        vec!["rock"],
        "a clear emptied a container it did not name"
    );
}

#[test]
fn an_inv_line_for_an_undeclared_container_still_lands() {
    // The wire sends `<inv>` for containers it never declared in this session --
    // 66,996 `<inv>` tags against 167 `<container>`. Dropping those would lose
    // almost everything.
    let state = fold(b"<inv id='65019875'> a <a exist=\"1\" noun=\"gem\">blue gem</a></inv>\n");
    assert_eq!(nouns(&state, "65019875"), vec!["gem"]);
    assert_eq!(
        state
            .inventory
            .container("65019875")
            .and_then(|c| c.title.as_deref()),
        None,
        "a container nobody declared has no title, and that is not the same as \
         having an empty one"
    );
}

#[test]
fn an_item_keeps_its_exist_id_for_the_noun_registry() {
    // `plan/18` step 5 keys on this. An item with no id could not be targeted
    // unambiguously when two things share a noun.
    let state = fold(CLOAK);
    let items = &state.inventory.container("stow").expect("declared").items;
    assert_eq!(items[0].id, "64863936");
    assert_eq!(items[0].text, "undulant vaelfyren tentacle dauber");
}

#[test]
fn a_reconnect_forgets_the_containers() {
    // Lich's `Inventory.reset!` exists specifically for "a session reset /
    // reconnect", so "no stale container mirror survives"
    // (`reference/lich-5/lib/common/inventory.rb:1014-1045`).
    let mut state = fold(CLOAK);
    assert!(!state.inventory.is_empty());

    state.invalidate_for_reconnect();

    assert!(state.inventory.is_empty());
}

/// The prose either side of an item's link, which `full_name` joins.
///
/// Three runs per line, VERIFIED by probing the parser with this exact wire:
///
/// ```text
/// Run { text: " a ",          link: None }
/// Run { text: "leather sack", link: Some(Exist { id: "111", noun: "sack" }) }
/// Run { text: " is closed.",  link: None }
/// ```
const QUALIFIED: &[u8] = b"<container id='stow' title=\"My Cloak\" target='#100'/>\
<clearContainer id=\"stow\"/>\
<inv id='stow'>In the <a exist=\"100\" noun=\"cloak\">cloak</a>:</inv>\
<inv id='stow'> a <a exist=\"111\" noun=\"sack\">leather sack</a> is closed.</inv>\
<inv id='stow'> an <a exist=\"112\" noun=\"anklet\">nightshade anklet</a> caught with spiders</inv>\
<inv id='stow'> a <a exist=\"113\" noun=\"rod\">slender wooden rod</a></inv>\
<inv id='stow'> a <a exist=\"114\" noun=\"orb\">shimmering green orb</a> </inv>\n";

#[test]
fn an_item_keeps_the_prose_either_side_of_its_link() {
    // **The Rule 2.2a fix.** `gameobj.rb:227`'s `full_name` joins
    // before/name/after, and `eherbs.lic:1132` matches against that joined
    // form. Keeping only the link text handed a consumer the middle third of
    // what the game said.
    let state = fold(QUALIFIED);
    let items = &state.inventory.container("stow").expect("declared").items;

    assert_eq!(items.len(), 4, "guard: four items, header excluded");

    assert_eq!(items[0].before.as_deref(), Some("a"));
    assert_eq!(items[0].text, "leather sack");
    assert_eq!(
        items[0].after.as_deref(),
        Some("is closed."),
        "the one after-text with a structural meaning (`xmlparser.rb:1264`)"
    );

    // `an`, not `a` -- the article is the game's, not reconstructed.
    assert_eq!(items[1].before.as_deref(), Some("an"));
    assert_eq!(items[1].after.as_deref(), Some("caught with spiders"));
}

#[test]
fn an_item_with_no_trailing_run_has_no_after_text() {
    // The wire emits **two** runs for a line ending at the link -- VERIFIED by
    // probe -- so there is no trailing run to read and `None` comes from the
    // index being off the end.
    let state = fold(QUALIFIED);
    let items = &state.inventory.container("stow").expect("declared").items;

    assert_eq!(items[2].text, "slender wooden rod");
    assert_eq!(
        items[2].before.as_deref(),
        Some("a"),
        "guard: before is read"
    );
    assert_eq!(items[2].after, None);
}

#[test]
fn a_trailing_whitespace_run_is_not_after_text() {
    // **THE INPUT IS THE POINT.** A first draft asserted this over the line
    // above, which emits no trailing run at all -- so the whitespace guard was
    // never reached and a mutant returning `Some("")` for every run passed
    // green. This line ends `</a> </inv>`, which the parser DOES split into a
    // third run of one space.
    //
    // §5.2: `Some("")` is not absence. A consumer joining `full_name` would
    // render it as a trailing space that the game never sent.
    let state = fold(QUALIFIED);
    let items = &state.inventory.container("stow").expect("declared").items;

    assert_eq!(items[3].text, "shimmering green orb");
    assert_eq!(
        items[3].after, None,
        "a whitespace-only run is nothing, not an empty qualifier"
    );
}

#[test]
fn the_containers_own_header_is_still_not_an_item() {
    // The header line `In the <a exist="100">cloak</a>:` has prose either side
    // too -- `In the` and `:`. Reading before/after must not make the header
    // look like an item with a qualifier.
    let state = fold(QUALIFIED);
    let items = &state.inventory.container("stow").expect("declared").items;
    assert!(
        items.iter().all(|item| item.id != "100"),
        "the container listed itself: {items:?}"
    );
}

/// Two links abutting, with no prose between them.
///
/// The wire sends one item per `<inv>` line -- MEASURED at 45,839 linked lines
/// -- so this shape is not expected. It is tested because the *guard against
/// it* is otherwise unenforced: a mutant deleting the link check passed all
/// fifteen other tests, since no fixture put two links next to each other.
const ABUTTING: &[u8] = b"<container id='stow' title=\"My Cloak\" target='#100'/>\
<clearContainer id=\"stow\"/>\
<inv id='stow'> a <a exist=\"115\" noun=\"pin\">silver pin</a><a exist=\"116\" noun=\"gem\">ruby gem</a></inv>\n";

#[test]
fn a_neighbouring_item_is_not_this_items_qualifier() {
    // VERIFIED by probe: this emits three runs, the second and third both
    // links. Without the guard, the pin's `after` becomes `ruby gem` and the
    // gem's `before` becomes `silver pin` -- each item claiming the other's
    // name as its own description, which `full_name` would then render as
    // `a silver pin ruby gem`.
    let state = fold(ABUTTING);
    let items = &state.inventory.container("stow").expect("declared").items;

    assert_eq!(items.len(), 2, "guard: both links became items");
    assert_eq!(items[0].text, "silver pin");
    assert_eq!(items[1].text, "ruby gem");

    assert_eq!(
        items[0].after, None,
        "the next run is the gem's link, not prose about the pin"
    );
    assert_eq!(
        items[1].before, None,
        "the previous run is the pin's link, not prose about the gem"
    );
    // The pin's own `before` still reads, so this is not vacuous.
    assert_eq!(items[0].before.as_deref(), Some("a"));
}
