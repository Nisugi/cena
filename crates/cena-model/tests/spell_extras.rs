//! What the spell table dropped, joined back by number (`plan/37` Stage 1).
//! The counts are the source's, measured with `grep -o` over the same
//! `effect-list.xml` the extractor read.

use cena_model::spells::{CastType, Span, all, spell};

#[test]
fn every_spell_has_its_extras_and_the_counts_are_the_files() {
    let stated = include_str!("../data/spell_extras.tsv")
        .lines()
        .find_map(|l| l.strip_prefix("# spells\t"))
        .and_then(|n| n.parse::<usize>().ok());
    assert_eq!(stated, Some(all().count()), "one row per spell in the table");
    let count = |f: &dyn Fn(&cena_model::Spell) -> bool| all().filter(|s| f(s)).count();
    assert_eq!(count(&|s| !s.extras.incant), 17, "incant='no'");
    assert_eq!(count(&|s| s.extras.stance), 16, "stance='yes'");
    assert_eq!(count(&|s| s.extras.channel), 15, "channel='yes'");
    assert_eq!(count(&|s| s.extras.cast_proc.is_some()), 109, "<cast-proc>");
    let spans = |span| {
        all()
            .flat_map(|s| s.extras.shapes.iter())
            .filter(|shape| shape.span == Some(span))
            .count()
    };
    assert_eq!(spans(Span::Stackable), 116);
    assert_eq!(spans(Span::Refreshable), 57, "one of them double-quoted");
}

#[test]
fn elemental_defense_stacks_and_multicasts_on_self_and_others() {
    let ed = spell(401).expect("401");
    for cast in [CastType::SelfCast, CastType::Target] {
        let shape = ed.extras.shape(cast).expect("both forms");
        assert_eq!(shape.span, Some(Span::Stackable));
        assert_eq!(shape.multicastable, Some(true));
    }
    let costs: Vec<(&str, &str)> = ed
        .extras
        .costs
        .iter()
        .map(|c| (c.kind.as_str(), c.text.as_str()))
        .collect();
    assert_eq!(costs, [("mana", "1")]);
}

#[test]
fn a_cost_the_table_could_not_read_is_kept_as_written() {
    let repair = spell(1102).expect("1102");
    assert_eq!(repair.mana, None, "an expression is not a number");
    assert!(repair
        .extras
        .costs
        .iter()
        .any(|c| c.kind == "mana" && c.text.contains("Wounds.limbs")));
    assert!(spell(520)
        .and_then(|s| s.extras.cast_proc.as_deref())
        .is_some_and(|p| p.contains("incant 520")));
}
