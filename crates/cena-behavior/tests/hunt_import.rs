//! The bigshot importer, against the acceptance profile (`plan/30` §4, Q7:
//! Nisugi's `ojandhaart.yaml`, copied to `tests/fixtures/` on 2026-09-24)
//! and against what it must hold rather than lose.

use cena_behavior::hunt::{Profile, import};

/// The fixture. An error is a broken fixture, which the test unwraps into a
/// failure.
fn ojandhaart() -> std::io::Result<String> {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ojandhaart.yaml"
    ))
}

fn lines(steps: &[cena_behavior::hunt::Step]) -> Vec<String> {
    steps.iter().map(ToString::to_string).collect()
}

#[test]
fn nisugis_profile_comes_across_key_by_key() {
    let brought = import("ojandhaart", &ojandhaart().unwrap()).unwrap();
    let p = &brought.profile;

    assert_eq!(p.rooms.hunting, Some(29_902));
    assert_eq!(p.rooms.boundaries, [29_900, 30_115]);
    assert_eq!(p.rooms.resting, Some(29_877));

    assert_eq!(p.stance.hunting.as_deref(), Some("offensive"));
    assert_eq!(p.stance.wander.as_deref(), Some("defensive"));
    assert_eq!(p.stance.stand.as_deref(), Some("offensive"));

    assert_eq!(p.rest.fried, Some(101));
    assert_eq!(p.rest.overkill, 2);
    assert_eq!(p.rest.encumbered, Some(20));
    assert_eq!(p.rest.mana_below, None, "oom was blank");
    let until = &p.rest.until;
    assert_eq!(
        (until.experience, until.mana, until.spirit, until.stamina),
        (Some(100), Some(90), Some(9), Some(90))
    );
    assert!(p.rest.when.bleeding && p.rest.when.cannot_use_ranged && p.rest.when.cannot_cast);
    assert_eq!(p.rest.when.health_at_most, Some(60));
    assert_eq!(p.rest.commands, ["store all"]);

    assert_eq!(p.prepare, ["ready weapon", "incant 515"]);
    assert_eq!(p.signs.len(), 7);
    assert_eq!(p.signs[6], "650 panther evoke", "a sign is sent as written");
    assert!(p.ignore.is_empty() && p.flee.from.is_empty());

    assert_eq!(p.loot.script.as_deref(), Some("eloot"));
    assert!(p.loot.delay && p.loot.defensive && p.loot.box_in_hand);
    assert_eq!(p.flee.count, Some(100));
    assert!((p.wander.wait - 0.3).abs() < f64::EPSILON);

    assert_eq!(p.targets.len(), 6);
    assert_eq!(
        (p.targets[0].name.as_deref(), p.targets[0].routine.as_str()),
        (Some("mastodon"), "b")
    );
    assert_eq!(
        (p.targets[2].name.as_deref(), p.targets[2].routine.as_str()),
        (Some("shield-maiden"), "e")
    );
    assert!(
        p.targets[5].any && p.targets[5].name.is_none(),
        "(?:.+?) is the catch-all"
    );
    assert_eq!(p.targets[5].routine, "f");

    assert_eq!(p.routines.len(), 10, "hunting_commands and _b to _j");
    assert_eq!(
        lines(&p.routines["a"]),
        [
            "kweed (!expiring \"Tangleweed Vigor\" 5)",
            "volley",
            "coupdegrace (thp 20 empowered_below 30)",
            "fire",
        ]
    );
    assert_eq!(
        lines(&p.routines["e"])[3],
        "incant 611 (immobilized)",
        "bigshot's (!frozen) runs when the target is frozen (`bigshot.lic:4418`)"
    );
    assert_eq!(
        &lines(&p.routines["f"])[3..],
        ["incant 608 (!hidden)", "hide (!hidden)", "fire (hidden)"]
    );

    assert!(
        p.held_steps().next().is_none(),
        "every guard Nisugi uses is built"
    );
    assert_eq!(p.unwritten_sequences().collect::<Vec<_>>(), ["volley"]);
    assert!(p.problems().is_empty(), "{:?}", p.problems());

    let notes = brought.notes.join("\n");
    assert!(
        notes.contains("sequence `volley` stands in for `script volley`"),
        "{notes}"
    );
    assert!(
        notes.contains("not imported: monitor_strings = "),
        "{notes}"
    );
    assert!(
        !notes.contains("hunting_commands"),
        "nothing in the routines was dropped: {notes}"
    );
}

#[test]
fn what_is_written_reads_back_the_same() {
    let brought = import("ojandhaart", &ojandhaart().unwrap()).unwrap();
    let text = brought.render().unwrap();
    assert!(
        text.starts_with("# Hydra hunt profile \"ojandhaart\""),
        "{text}"
    );
    assert!(
        text.contains("#   sequence `volley`"),
        "the notes head the file:\n{text}"
    );
    let again = Profile::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(again, brought.profile);
}

#[test]
fn what_cannot_be_carried_is_held_and_named() {
    let yaml = "hunting_commands: attack(stunned), kweed(buff5 hiden), coupdegrace(buff5), \
                fire(x2), kweed and fire, incant 911(!frozen once)\n\
                targets: (?:.+?)(a)\n\
                wounded_eval: bleeding? || Char.percent_health < 40 || foo? || (a && b)\n\
                hunting_room_id: u1234\n\
                hunting_stance: sideways\n\
                resting_commands: stow all and sit(x2)\n";
    let brought = import("odd", yaml).unwrap();
    let p = &brought.profile;

    let a = &p.routines["a"];
    assert_eq!(a.len(), 7, "{:?}", lines(a));
    let held = |i: usize| {
        a[i].held
            .as_deref()
            .unwrap_or_else(|| panic!("step {i} runs: {}", a[i]))
    };
    assert_eq!(a[0].send, "attack(stunned)");
    assert!(
        held(0).contains("guard `stunned` is not built yet"),
        "{}",
        held(0)
    );
    assert!(
        held(1).contains("`hiden` is not a guard bigshot knows either"),
        "{}",
        held(1)
    );
    assert!(
        held(2).contains("pattern"),
        "coupdegrace's buff is a pattern: {}",
        held(2)
    );
    assert_eq!(
        (a[3].to_string(), a[4].to_string()),
        ("fire".to_owned(), "fire".to_owned()),
        "(x2)"
    );
    assert!(held(5).contains("`and`"), "{}", held(5));
    assert!(
        held(6).contains("guard `once` is not built yet"),
        "one guard held holds the step, whatever else translated: {}",
        held(6)
    );
    assert_eq!(p.held_steps().count(), 5);

    assert!(p.targets[0].any);
    assert_eq!(p.rest.when.health_at_most, Some(39), "< 40 is at most 39");
    assert!(p.rest.when.bleeding);
    assert_eq!(p.rooms.hunting, None, "a u-id needs the map");
    assert_eq!(p.stance.hunting, None);
    assert_eq!(
        p.rest.commands,
        ["stow all", "sit", "stow all", "sit"],
        "(x2) on a pair"
    );

    let notes = brought.notes.join("\n");
    for expected in [
        "rest.when: `foo?` is not a shape",
        "joins conditions with &&",
        "hunting_room_id: `u1234`",
        "hunting_stance: ",
        "resting_commands: `stow all and sit`",
    ] {
        assert!(notes.contains(expected), "{expected}\n{notes}");
    }
}

#[test]
fn not_the_dialect_is_refused() {
    assert!(import("x", "just words\n").is_err());
}
