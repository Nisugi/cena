//! The herb table cut from eherbs (`plan/36` Stage 1), and the dose monitor
//! read from the wire.

use cena_model::GameState;
use cena_model::herbs::{self, Area, HerbKind, Hurt, Severity, Source};
use cena_protocol::Parser;

/// The count the extractor wrote in the table's header.
fn header_count() -> Option<usize> {
    include_str!("../data/herbs.tsv")
        .lines()
        .find_map(|line| line.strip_prefix("# herbs\t"))
        .and_then(|n| n.parse().ok())
}

#[test]
fn every_herb_in_the_table_reads_and_every_kind_is_known() {
    // A row whose kind does not parse would be dropped silently: the count
    // is pinned to the extractor's own.
    assert_eq!(Some(herbs::herbs().len()), header_count());
    assert_eq!(herbs::herbs().len(), 247);
    let kinds: std::collections::BTreeSet<HerbKind> =
        herbs::herbs().iter().map(|h| h.kind).collect();
    assert_eq!(kinds.len(), 23, "{kinds:?}");
    for kind in HerbKind::all() {
        assert_eq!(HerbKind::parse(&kind.to_string()), Some(kind));
    }
}

#[test]
fn a_herb_is_found_by_the_name_an_item_goes_by() {
    let acantha = herbs::herb_for("acantha leaf").expect("acantha");
    assert_eq!(acantha.name, "some acantha leaf");
    assert_eq!(acantha.kind, HerbKind::Blood);
    assert_eq!(acantha.store_doses, 10);
    assert!(!acantha.is_drinkable());
    let bolmara = herbs::herb_for("bolmara potion").expect("bolmara");
    assert!(bolmara.is_drinkable());
    assert_eq!(
        bolmara.kind,
        HerbKind::Injury {
            severity: Severity::Major,
            area: Area::Nerve,
            hurt: Hurt::Wound
        }
    );
    assert!(herbs::herb_for("tincture of yabathilium").is_some_and(herbs::Herb::is_major_blood));
    assert!(!acantha.is_major_blood());
}

#[test]
fn what_a_town_sells_and_the_herbs_no_shop_does() {
    assert!(herbs::location_stocked("Wehnimer's Landing"));
    assert!(herbs::location_stocked("the city of Ta'Vaalor"));
    assert!(!herbs::location_stocked("Nowhere At All"));
    assert!(!herbs::location_stocked(""));
    let vaalor: Vec<&str> = herbs::sold_in("Ta'Vaalor").map(|h| h.name).collect();
    assert!(vaalor.contains(&"tincture of acantha"), "{vaalor:?}");
    assert!(!vaalor.contains(&"some acantha leaf"));
    let yaba = herbs::herb_for("yabathilium fruit").expect("yaba");
    assert_eq!(yaba.sources, [Source::Forageable]);
    let backroom = herbs::herb_for("bunch of acantha leaf").expect("bunch");
    assert_eq!(backroom.sources, [Source::DoNotBuy]);
}

#[test]
fn eherbs_drinkable_and_bundling_rules() {
    assert!(herbs::is_drinkable("tincture of basal"));
    assert!(herbs::is_drinkable("large Bloody Krolvin ale"));
    assert!(!herbs::is_drinkable("some tealeaf"), "a whole word only");
    assert!(herbs::bundles("some acantha leaf"));
    assert!(!herbs::bundles("yabathilium fruit"));
    assert!(!herbs::bundles("hot bowl of soup"));
}

/// Fold wire, a prompt closing the chunk.
fn fold(state: &mut GameState, wire: &str) {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
}

const PROMPT: &str = "<prompt time=\"1790045835\">&gt;</prompt>\n";

#[test]
fn eating_says_what_is_left_and_measure_settles_the_vague_ones() {
    let mut state = GameState::default();
    fold(
        &mut state,
        &format!(
            "You take a bite of your <a exist=\"101\" noun=\"leaf\">acantha leaf</a>.\nYou have 3 bites left.\n{PROMPT}"
        ),
    );
    assert_eq!(state.doses.left("101"), Some(3));
    fold(
        &mut state,
        &format!(
            "The <a exist=\"101\" noun=\"leaf\">acantha leaf</a> looks like it has a few bites left.\n{PROMPT}"
        ),
    );
    assert_eq!(state.doses.left("101"), Some(3), "3 is a few: kept");
    fold(
        &mut state,
        &format!(
            "The <a exist=\"102\" noun=\"potion\">bolmara potion</a> has several doses left.\n{PROMPT}"
        ),
    );
    assert_eq!(
        state.doses.left("102"),
        Some(7),
        "unmeasured: eherbs' seven"
    );
    fold(
        &mut state,
        &format!(
            "You take a drink from your <a exist=\"102\" noun=\"potion\">bolmara potion</a>.\nThat was the last drop.\n{PROMPT}"
        ),
    );
    assert_eq!(state.doses.left("102"), Some(0));
    fold(
        &mut state,
        &format!("The <a exist=\"103\" noun=\"moss\">basal moss</a> has 12 bites left.\n{PROMPT}"),
    );
    assert_eq!(state.doses.left("103"), Some(12));
    assert_eq!(state.doses.left("104"), None, "never measured is not zero");
}

#[test]
fn a_line_that_is_not_after_a_bite_does_not_count() {
    let mut state = GameState::default();
    fold(&mut state, &format!("You have 3 bites left.\n{PROMPT}"));
    fold(
        &mut state,
        &format!(
            "You take a bite of your <a exist=\"101\" noun=\"leaf\">acantha leaf</a>.\n{PROMPT}You have 9 bites left.\n{PROMPT}"
        ),
    );
    assert_eq!(
        state.doses.left("101"),
        None,
        "the count must follow the bite within the prompt"
    );
}
