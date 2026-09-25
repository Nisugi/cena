//! bigshot's guard words through the importer (`plan/33`,
//! `hunt/import/words.rs`): what each becomes, read against bigshot's own
//! branch for when the step runs, and what holds a step and why.

use cena_behavior::hunt::import::import;

/// `fire(<token>)` imported: the step as Hydra writes it, or why it holds.
fn carried(token: &str) -> Result<String, String> {
    let brought = import("t", &format!("hunting_commands: fire({token})\n"))?;
    let step = brought
        .profile
        .routines
        .get("a")
        .and_then(|steps| steps.first())
        .ok_or("no step")?;
    step.held.clone().map_or_else(|| Ok(step.to_string()), Err)
}

/// Each token becomes this Hydra step.
fn becomes(cases: &[(&str, &str)]) {
    for (token, want) in cases {
        assert_eq!(carried(token).as_deref(), Ok(*want), "`{token}`");
    }
}

/// Each token holds its step, and says this.
fn holds(cases: &[(&str, &str)]) {
    for (token, why) in cases {
        let result = carried(token);
        assert!(
            matches!(&result, Err(held) if held.contains(why)),
            "`{token}`: {result:?}"
        );
    }
}

#[test]
fn amount_words_answer_skip_so_they_run_at_or_above_the_amount() {
    // `h50` skips below 50 (`:3207`), so it runs at 50 or more; and `e20`,
    // read the same way, runs at 20% encumbrance or more (`plan/33`'s
    // correction of its `e` row).
    becomes(&[
        ("h50", "fire (health_at_least 50)"),
        ("!h50", "fire (!health_at_least 50)"),
        ("m30", "fire (mana_at_least 30)"),
        ("s20", "fire (stamina_at_least 20)"),
        ("v5", "fire (spirit_at_least 5)"),
        ("e20", "fire (encumbrance_at_least 20)"),
        ("!e20", "fire (!encumbrance_at_least 20)"),
        ("k1", "fire (self_kneeling)"),
        ("!k1", "fire (!self_kneeling)"),
        ("mob3", "fire (targets_at_least 3)"),
        ("!mob3", "fire (targets_at_most 3)"),
        ("valid2", "fire (targets_at_least 2)"),
        ("thp20", "fire (thp 20)"),
        ("empowered30", "fire (empowered_below 30)"),
        ("repeatdelay10", "fire (every 10)"),
        ("!repeatdelay10", "fire (every 10)"),
    ]);
    holds(&[
        ("h", "never held this step back"),
        ("k", "never held this step back"),
        ("essence5", "does not capture yet"),
    ]);
}

#[test]
fn tier_is_two_checks_at_once() {
    // `tier2` passes the amount check (skip below 2) and the `tier2` branch
    // (skip unless 2): it runs at exactly 2. `!tier2` runs below 2.
    becomes(&[
        ("tier2", "fire (position 2)"),
        ("!tier2", "fire (!position_at_least 2)"),
        ("tier5", "fire (position_at_least 5)"),
        ("!tier5", "fire (!position_at_least 6)"),
        ("ucsdecent", "fire (position 1)"),
        ("!ucsexcellent", "fire (!position 3)"),
        ("ucstierup", "fire (ucstierup)"),
    ]);
}

#[test]
fn the_four_inverted_words_are_flipped() {
    becomes(&[
        ("frozen", "fire (!immobilized)"),
        ("!frozen", "fire (immobilized)"),
        ("prone", "fire (!down)"),
        ("!prone", "fire (down)"),
        ("rooted", "fire (!rooted)"),
        ("!rooted", "fire (rooted)"),
        ("voidweaver", "fire (!buff \"Voidweaver\")"),
        ("!voidweaver", "fire (buff \"Voidweaver\")"),
    ]);
}

#[test]
fn words_kept_or_renamed_carry_their_bang() {
    becomes(&[
        ("stunned", "fire (stunned)"),
        ("!webbed", "fire (!webbed)"),
        ("mini_boss", "fire (mini_boss)"),
        ("!undead", "fire (!undead)"),
        ("pcs", "fire (alone)"),
        ("!pcs", "fire (!alone)"),
        ("room", "fire (once_here)"),
        ("once", "fire (once)"),
        ("wounded", "fire (thp 25)"),
        ("!wounded", "fire (!thp 25)"),
        ("splashy disease", "fire (splashy disease)"),
    ]);
    holds(&[
        ("!once", "never held this step back"),
        ("!room", "never held this step back"),
        ("justice", "does not capture yet"),
        ("lying", "not a guard bigshot knows"),
    ]);
}

#[test]
fn effect_words_become_the_effect_they_named() {
    becomes(&[
        ("rapid", "fire (buff \"Rapid Fire\")"),
        ("!flurry", "fire (!buff \"Slashing Strikes\")"),
        ("506", "fire (spell \"Celerity\")"),
        ("burst", "fire (buff \"Enh. Dexterity\")"),
        ("!burst", "fire (!cooldown \"Burst of Swiftness\")"),
        ("!surge", "fire (!cooldown \"Surge of Strength\")"),
        ("coupdegrace", "fire (buff \"Empowered\")"),
        ("reflex", "fire (buff \"Nature's Touch Arcane Ref\")"),
    ]);
    holds(&[
        ("!506", "Celerity's last three seconds"),
        ("!celerity", "Celerity's last three seconds"),
    ]);
}

#[test]
fn a_quoted_effect_is_read_as_a_name() {
    becomes(&[
        ("EB\"Rapid Fire\"", "fire (buff \"Rapid Fire\")"),
        ("!ES\"Celerity\"", "fire (!spell \"Celerity\")"),
        ("EC\"Surge\"", "fire (cooldown \"Surge\")"),
        ("ED\"Confused\"", "fire (debuff \"Confused\")"),
        // A bare dot is a dot, and an escape is the character escaped.
        ("EB\"Enh. Strength\"", "fire (buff \"Enh. Strength\")"),
        (
            r#"EB"Enh\. Strength \(\+10\)""#,
            "fire (buff \"Enh. Strength (+10)\")",
        ),
    ]);
    holds(&[(r#"EB"Empow.*30""#, "is a pattern")]);
}

#[test]
fn censer_is_no_guard_but_the_whole_routines_policy() {
    let brought = import("t", "hunting_commands: fire(censer), incant 302(censer)\n").unwrap();
    let steps: Vec<String> = brought.profile.routines["a"]
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        steps,
        ["fire", "incant 302"],
        "the word is dropped from each step"
    );
    assert!(brought.profile.censer_between_actions);
    let noted = brought
        .notes
        .iter()
        .filter(|note| note.contains("censer_between_actions"))
        .count();
    assert_eq!(noted, 1, "said once: {:?}", brought.notes);
    holds(&[("!censer", "never held this step back")]);
    let plain = import("t", "hunting_commands: fire\n").unwrap();
    assert!(!plain.profile.censer_between_actions);
}
