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
    assert_eq!(
        stated,
        Some(all().count()),
        "one row per spell in the table"
    );
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
    assert!(
        repair
            .extras
            .costs
            .iter()
            .any(|c| c.kind == "mana" && c.text.contains("Wounds.limbs"))
    );
    assert!(
        spell(520)
            .and_then(|s| s.extras.cast_proc.as_deref())
            .is_some_and(|p| p.contains("incant 520"))
    );
}

#[test]
fn every_message_is_kept_with_its_type_not_one_per_type() {
    // MEASURED: the file has 628 <message> elements (REXML, every parent a
    // <spell>); the duplicated 9052's second copy holds 2, and the first copy
    // wins. inventory/12 §1.1 found the table keeps one per type.
    let messages: usize = all().map(|s| s.extras.messages.len()).sum();
    assert_eq!(messages, 626);
    let stated = include_str!("../data/spell_extras.tsv")
        .lines()
        .find_map(|l| l.strip_prefix("# messages	"))
        .and_then(|n| n.parse::<usize>().ok());
    assert_eq!(stated, Some(626));
    let more_than_one_of_a_type = all()
        .filter(|s| {
            let mut kinds: Vec<&str> = s.extras.messages.iter().map(|(k, _)| k.as_str()).collect();
            let n = kinds.len();
            kinds.sort_unstable();
            kinds.dedup();
            kinds.len() < n
        })
        .count();
    assert_eq!(
        more_than_one_of_a_type, 43,
        "the 43 spells whose extra messages the table lost"
    );
}

/// A society power's line is its cast proc's, not its name (the crate
/// review of 2026-10-01, MO-F-2): Kai's Smite is `smite`, eight symbols
/// append the target, two procs send an empty line and one refuses.
#[test]
fn a_powers_line_is_read_off_its_cast_proc() {
    use cena_model::spells::ProcLine;
    let line = |n: u16| spell(n).and_then(|s| s.extras.proc_line());
    let sends = |text: &str, targeted| {
        Some(ProcLine::Sends {
            line: text.to_owned(),
            targeted,
        })
    };
    assert_eq!(line(9708), sends("sigil of offense", false));
    assert_eq!(line(9805), sends("symbol of courage", false));
    assert_eq!(line(9701), sends("sigil of recognition", false), "`fput`");
    assert_eq!(line(9911), sends("sign of hypnosis", false), "no comma");
    assert_eq!(line(9918), sends("sign of wracking", false), "in a loop");
    assert_eq!(line(9821), sends("smite", true));
    for (n, name) in [
        (9802, "symbol of blessing"),
        (9804, "symbol of diminishment"),
        (9807, "symbol of submission"),
        (9809, "symbol of holiness"),
        (9811, "symbol of sleep"),
        (9812, "symbol of transcendence"),
        (9814, "symbol of sight"),
        (9822, "symbol of turning"),
    ] {
        assert_eq!(line(n), sends(name, true), "{n}");
    }
    assert_eq!(line(9803), Some(ProcLine::Nothing));
    assert_eq!(line(9808), Some(ProcLine::Nothing));
    assert_eq!(line(9920), Some(ProcLine::Refuses));
    assert_eq!(line(9725), None, "a timer, with no proc");
    // Every number from 9700 up: none read as something the proc does not say.
    let unread: Vec<u16> = all()
        .filter(|s| s.number >= 9700)
        .filter(|s| matches!(s.extras.proc_line(), Some(ProcLine::Refuses)))
        .map(|s| s.number)
        .collect();
    assert_eq!(unread, [9920]);
}
