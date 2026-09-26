//! The spell table's Ruby, evaluated (`plan/37` Stage 2).

use cena_model::spells::expr::{Name, Value, evaluate};
use cena_model::spells::{Duration, all};

/// A character with 30 ranks in every list, level 60, society rank 10,
/// every spell known and none active.
fn someone(name: &Name) -> Option<Value> {
    match name {
        Name::Path(module, _) if module == "Spells" || module == "Skills" => Some(Value::Num(30.0)),
        Name::Path(module, method) if module == "Stats" && method == "level" => {
            Some(Value::Num(60.0))
        }
        Name::Path(module, method) if module == "Society" && method == "rank" => {
            Some(Value::Num(10.0))
        }
        Name::Spell(_, method) if method == "known?" => Some(Value::Bool(true)),
        Name::Spell(_, method) if method == "active?" => Some(Value::Bool(false)),
        Name::Spell(_, method) if method == "timeleft" => Some(Value::Num(0.0)),
        _ => None,
    }
}

#[test]
fn the_tables_common_shapes() {
    let at = |source: &str| evaluate(source, &someone);
    assert_eq!(
        at("(Spell[401].known? ? 120 : 20) + Spells.minorelemental"),
        Some(150.0)
    );
    assert_eq!(at("20 + Spells.minorspiritual"), Some(50.0));
    assert_eq!(at("1 + Stats.level / 6.0"), Some(11.0));
    assert_eq!(
        at("Society.rank / 6.0").map(|m| (m * 100.0).round()),
        Some(167.0)
    );
    assert_eq!(
        at("if Skills.slreligion >= 40; 480; elsif Skills.slreligion >= 20; 720; else 1440; end"),
        Some(720.0)
    );
    assert_eq!(at("[Skills.emc, 25].max.to_i"), Some(30.0));
    assert_eq!(
        at("if Spell[9010].active?; Spell[9010].timeleft; else; 5; end"),
        Some(5.0)
    );
}

#[test]
fn what_it_does_not_know_is_none_never_a_guess() {
    let at = |source: &str| evaluate(source, &someone);
    assert_eq!(at("Spellsong.timeleft"), None, "a name nobody resolved");
    assert_eq!(
        at("s = Spell['Core Tap']; s.timeleft"),
        None,
        "an assignment"
    );
    assert_eq!(at("history.find { |l| l =~ /x/ }"), None, "a block");
    assert_eq!(at("20 +"), None, "cut short");
    assert_eq!(at("1 / 0"), None);
}

#[test]
fn most_derived_durations_evaluate_and_the_rest_are_named() {
    let derived: Vec<(u16, &str)> = all()
        .flat_map(|s| s.durations.iter().map(move |(_, d)| (s.number, d)))
        .filter_map(|(n, d)| match d {
            Duration::Derived(source) => Some((n, source.as_str())),
            _ => None,
        })
        .collect();
    let evaluated = derived
        .iter()
        .filter(|(_, source)| evaluate(source, &someone).is_some())
        .count();
    // MEASURED 2026-09-25: all 120 of the table's derived durations parse
    // and evaluate. The table's extractor had already filed the others --
    // `Spellsong.timeleft`, `CMan`, a scrollback scrape -- as unknown (40),
    // and those stay unevaluated. A drop here is the parser losing a shape.
    assert_eq!(derived.len(), 120);
    assert_eq!(evaluated, 120);
}
