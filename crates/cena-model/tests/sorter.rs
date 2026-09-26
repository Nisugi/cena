//! `;sorter` over real container looks, as the model finishes them.
//!
//! `fixtures/container_looks.xml` is five looks from the author's logs
//! (`E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi\2026\09\xml`), each cut whole
//! with the prompt after it and Lich's timestamp prefix removed. No player is
//! named in any of them.
//!
//! | Fixture line | Look | Source |
//! |---|---|---|
//! | 1 | a mahogany box | `2026-09-04_01-30-58.xml:1550` |
//! | 3 | a plumille cloak | `2026-09-01_20-24-11.xml:9859` |
//! | 5 | a dwarf skin backpack | `2026-09-01_20-24-11.xml:9869` |
//! | 7 | a shop's wooden shelf | `2026-09-02_22-34-52.xml:23595` |
//! | 9 | the herb kit | `2026-09-04_00-48-57.xml:358` |
//!
//! Each look goes through the real parser and `GameState`, so the model
//! decides the line's runs: the `<container>` and `<inv>` preamble before
//! each look, which holds its own `In the <a ...>box</a>:`, stays off the main
//! stream, and the look's links reach the sorter with their nouns. The same
//! bytes go through a session and the web pump in
//! `crates/cena/tests/web_sorter.rs`.
//!
//! The categories asserted are `gameobj`'s answers for these items, not
//! chosen ones: between them the looks file items under a single type, under
//! two joined (`armor,uncommon`), and under `other`.

use cena_model::GameState;
use cena_model::sorter::{MAX_SORTED_RUNS, is_container_look, sort};
use cena_protocol::frame::LinkKind;
use cena_protocol::runs::Runs;

const WIRE: &str = include_str!("fixtures/container_looks.xml");

/// The main-stream line the model finishes from `wire`.
fn finished(wire: &str) -> Option<Runs> {
    let mut parser = cena_protocol::Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(format!("{wire}\n").as_bytes()) {
        state.apply(&frame);
    }
    state.stream("").last().cloned()
}

/// Fixture line `index`'s look, counting from 0.
fn look(index: usize) -> Option<Runs> {
    finished(WIRE.lines().nth(index)?)
}

/// Fixture line `index`'s look, sorted.
fn sorted(index: usize) -> Option<Vec<Runs>> {
    sort(&look(index)?)
}

fn texts(lines: &[Runs]) -> Vec<String> {
    lines.iter().map(Runs::plain).collect()
}

#[test]
fn a_box_files_each_item_under_its_type() {
    let lines = sorted(0).unwrap();
    assert_eq!(
        texts(&lines),
        [
            "In the mahogany box:",
            "  valuable (1): bright gold ingot",
            "  other (1): some silver coins",
            "  wand (1): smooth amber wand",
            "  gem (3): pinch of electrum dust, piece of brown jade, tar black tourmaline",
            "  lockpick (1): steel lockpick",
        ]
    );
    // The label is bold, as sorter.lic's monsterbold; the items are not.
    assert_eq!(lines[4].runs[0].text, "  gem (3): ");
    assert!(lines[4].runs[0].style.bold_depth > 0);
    assert!(
        lines[4].runs[1..]
            .iter()
            .all(|run| run.style.bold_depth == 0)
    );
}

#[test]
fn sorted_items_keep_their_links() {
    // The view types this was ported from had no links; the model's runs do,
    // so a sorted item is as clickable as the look it came from.
    let lines = sorted(0).unwrap();
    let nouns = |line: &Runs| -> Vec<String> {
        line.objects()
            .filter_map(|link| match &link.kind {
                LinkKind::Exist { noun, .. } => Some(noun.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(nouns(&lines[0]).len(), 1, "the header keeps its container");
    assert_eq!(nouns(&lines[4]).len(), 3, "each gem keeps its link");
}

#[test]
fn duplicates_collapse_to_a_count_in_last_word_order() {
    assert_eq!(
        texts(&sorted(2).unwrap()),
        [
            "In the plumille cloak:",
            "  other (8): slender wooden rod (2), material swatch (5), strand of veniom thread",
            "  magic (1): small statue",
        ]
    );
}

#[test]
fn an_item_of_two_types_is_filed_under_both_joined() {
    assert_eq!(
        texts(&sorted(4).unwrap()),
        [
            "In the dwarf skin backpack:",
            "  other (1): tumbler of black cherry whiskey",
            "  uncommon (3): nacreous disir feather (2), stygian valravn quill",
            "  gem (1): carved basalt teardrop",
            "  magic (1): shimmering green orb",
            "  armor,uncommon (1): scratched spiked vultite greathelm",
            "  jewelry (1): alexandrite inset mithril circlet",
        ]
    );
}

#[test]
fn a_surface_sorts_and_an_items_own_text_stays_with_it() {
    let lines = texts(&sorted(6).unwrap());
    assert_eq!(lines[0], "On the wooden shelf:");
    assert!(
        lines.iter().any(|line| line
            .contains("smooth turquoise leather journal embossed with a peacock feather")),
        "the journal keeps what follows its link: {lines:?}"
    );
    // Nothing is lost: the labels count all sixteen things on the shelf.
    let counted: usize = lines[1..]
        .iter()
        .filter_map(|line| {
            line.split_once(" (")?
                .1
                .split_once(')')?
                .0
                .parse::<usize>()
                .ok()
        })
        .sum();
    assert_eq!(counted, 16, "{lines:?}");
}

/// **The look `VellumFE` would have broken.** It passes `VellumFE`'s gate --
/// asserted, so this test cannot pass by the line never reaching the list
/// check -- and goes on past its list, so it is shown whole, dose counts and
/// all.
#[test]
fn the_herb_kit_is_shown_whole() {
    let kit = look(8).unwrap();
    assert!(is_container_look(&kit.plain()));
    assert!(kit.plain().contains("wolifrew (143)"));
    assert_eq!(sort(&kit), None);
}

/// A sentence after the list is not the last item's own text. SYNTHETIC: no
/// look among the 475 measured has one, so this is the guard's only input;
/// without the guard the rod would read `slender wooden rod.  It glows`.
#[test]
fn a_sentence_after_the_list_leaves_the_line_alone() {
    let line = finished(
        "In the <a exist=\"1\" noun=\"box\">box</a> you see a \
         <a exist=\"2\" noun=\"rod\">slender wooden rod</a>.  It glows.",
    )
    .unwrap();
    assert!(is_container_look(&line.plain()));
    assert_eq!(sort(&line), None);
}

/// `VellumFE`'s `categorizes_counts_and_keeps_links` (`sorter.rs:285-306`),
/// its synthetic look unchanged. Its lockpick lands in `other` there because
/// that test's data pack has two types; Hydra's table has `lockpick`.
#[test]
fn vellumfe_categorizes_and_counts() {
    let line = finished(
        "In the <a exist=\"77\" noun=\"backpack\">backpack</a> you see a \
         <a exist=\"1\" noun=\"sapphire\">blue sapphire</a>, a \
         <a exist=\"2\" noun=\"crystal\">quartz crystal</a>, a \
         <a exist=\"3\" noun=\"sapphire\">blue sapphire</a> and a \
         <a exist=\"4\" noun=\"lockpick\">copper lockpick</a>.",
    )
    .unwrap();
    assert_eq!(
        texts(&sort(&line).unwrap()),
        [
            "In the backpack:",
            "  gem (3): quartz crystal, blue sapphire (2)",
            "  lockpick (1): copper lockpick",
        ]
    );
}

/// `VellumFE`'s `ignores_non_look_lines_and_dual_surface`
/// (`sorter.rs:308-328`): a line that is no look, one naming both an `In`
/// and an `On` surface, and a look with nothing in it.
#[test]
fn vellumfe_non_looks_pass_through() {
    for wire in [
        "You pick up a rock.",
        "In the <a exist=\"5\" noun=\"counter\">counter</a> you see a \
         <a exist=\"6\" noun=\"rock\">rock</a>. On the \
         <a exist=\"5\" noun=\"counter\">counter</a> you see a \
         <a exist=\"7\" noun=\"rock\">rock</a>.",
        "In the <a exist=\"9\" noun=\"pouch\">pouch</a> you see nothing.",
    ] {
        let line = finished(wire).unwrap();
        assert_eq!(sort(&line), None, "{wire}");
    }
}

/// Past [`MAX_SORTED_RUNS`] the look is shown as it came; a shorter one of
/// the same shape sorts, so the bound is what decided.
#[test]
fn a_look_too_long_to_sort_is_shown_as_it_came() {
    let wire = |items: usize| {
        let mut line = String::from("In the <a exist=\"1\" noun=\"locker\">locker</a> you see ");
        for index in 0..items {
            let separator = match index {
                0 => "a ",
                last if last + 1 == items => " and a ",
                _ => ", a ",
            };
            line.push_str(separator);
            // The sorter reads nouns, never ids, so one id serves.
            line.push_str("<a exist=\"2\" noun=\"rock\">rock</a>");
        }
        line.push('.');
        line
    };
    let short = finished(&wire(100)).unwrap();
    assert_eq!(
        texts(&sort(&short).unwrap())[1],
        "  other (100): rock (100)"
    );
    let long = finished(&wire(600)).unwrap();
    assert!(long.runs.len() > MAX_SORTED_RUNS, "{}", long.runs.len());
    assert_eq!(sort(&long), None);
}
