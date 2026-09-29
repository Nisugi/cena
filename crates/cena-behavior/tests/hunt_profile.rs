//! The profile: read, written back, checked; and the chain a key resolves
//! through (`plan/30` §5, `plan/12` §6a.2).

use std::path::{Path, PathBuf};

use cena_behavior::hunt::chain::{self, LoadError};
use cena_behavior::hunt::{Profile, Step, load};

const EXAMPLE: &str = r#"
prepare = ["ready weapon", "incant 515"]
signs = ["515", "605 evoke"]
targets = [
  { name = "mastodon", routine = "b" },
  { any = true, routine = "f" },
]

[rooms]
hunting = 29902
boundaries = [29900, 30115]
resting = 29877

[stance]
hunting = "offensive"

[rest]
fried = 101
encumbered = 20
until = { experience = 100, mana = 90 }
when = { bleeding = true, health_at_most = 60, cannot_use_ranged = true }
commands = ["store all"]

[routines]
b = ["kweed (expiring \"Tangleweed Vigor\" 5)", "volley", "coupdegrace (thp 20 empowered_below 30)", "fire"]
f = ["hide (!hidden)", "fire (hidden)", { step = "attack(stunned)", held = "guard `stunned` is not built yet" }]

[sequences]
volley = []
"#;

/// A directory of this test's own.
fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-hunt-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Write a level's file, making its directory. An error is a broken fixture,
/// which the test unwraps into a failure.
fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

#[test]
fn the_example_reads_as_written() {
    let profile = Profile::parse(EXAMPLE).unwrap();
    assert_eq!(profile.rooms.hunting, Some(29_902));
    assert_eq!(profile.rooms.boundaries, [29_900, 30_115]);
    assert_eq!(profile.rest.when.health_at_most, Some(60));
    assert!(profile.rest.when.bleeding && profile.rest.when.cannot_use_ranged);
    assert!(!profile.rest.when.cannot_cast, "unset is false");
    assert_eq!(profile.rest.overkill, 0, "the built-in default");
    assert!(
        (profile.wander.wait - 0.3).abs() < f64::EPSILON,
        "the built-in default"
    );
    assert!(profile.targets[1].any);
    assert_eq!(profile.targets[0].routine, "b");

    let b = &profile.routines["b"];
    assert_eq!(b.len(), 4);
    assert_eq!(b[0].send, "kweed");
    assert_eq!(b[0].when.len(), 1);
    assert!(
        !b[0].when[0].negated,
        "expiring runs the refresh; no ! (plan/33 question 6)"
    );
    assert_eq!(b[2].when.len(), 2);
    assert!(b[1].when.is_empty() && b[1].held.is_none());

    let f = &profile.routines["f"];
    assert_eq!(
        f[2].send, "attack(stunned)",
        "a held step keeps bigshot's line whole"
    );
    assert_eq!(
        f[2].held.as_deref(),
        Some("guard `stunned` is not built yet")
    );
    assert!(f[2].when.is_empty(), "and parses no guard from it");

    let held: Vec<String> = profile.held_steps().map(|(where_, _)| where_).collect();
    assert_eq!(held, ["routine f, step 3"]);
    assert_eq!(
        profile.unwritten_sequences().collect::<Vec<_>>(),
        ["volley"]
    );
    assert!(profile.problems().is_empty(), "{:?}", profile.problems());
}

#[test]
fn written_back_it_reads_the_same() {
    let profile = Profile::parse(EXAMPLE).unwrap();
    let text = profile.to_toml().unwrap();
    let again = Profile::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(again, profile);
    assert!(
        text.contains("held = "),
        "a held step is written as a table:\n{text}"
    );
    assert!(
        text.contains("\"coupdegrace (thp 20 empowered_below 30)\""),
        "a step that runs is written as one line:\n{text}"
    );
}

#[test]
fn a_mistyped_key_is_refused_by_name() {
    let err = Profile::parse("[room]\nhunting = 1\n").unwrap_err();
    assert!(err.contains("room"), "{err}");
    let err = Profile::parse("[rooms]\nhutning = 1\n").unwrap_err();
    assert!(err.contains("hutning"), "{err}");
    let err = Profile::parse("[routines]\na = [\"fire (hiden)\"]\n").unwrap_err();
    assert!(
        err.contains("hiden"),
        "a guard Hydra does not know is refused when the profile loads: {err}"
    );
}

#[test]
fn what_reads_cleanly_and_is_still_wrong() {
    let profile = Profile::parse(
        "targets = [{ name = \"rat\", any = true, routine = \"x\" }, { routine = \"a\" }]\n\
         [stance]\nhunting = \"sideways\"\n[routines]\na = []\n",
    )
    .unwrap();
    let problems = profile.problems();
    assert_eq!(problems.len(), 5, "{problems:?}");
    let all = problems.join("\n");
    for expected in [
        "target 1 has both a name and any",
        "target 1 uses routine \"x\"",
        "target 2 names no creature",
        "stance.hunting:",
        "routine a has no steps",
    ] {
        assert!(all.contains(expected), "{expected}\n{all}");
    }
}

#[test]
fn a_step_is_what_to_send_and_its_guards() {
    let plain = Step::parse("fire").unwrap();
    assert_eq!((plain.send.as_str(), plain.when.len()), ("fire", 0));
    assert_eq!(
        Step::parse(" fire (hidden) ").unwrap().to_string(),
        "fire (hidden)"
    );
    let quoted = Step::parse("shout (!expiring \"Empowered (+20)\" 5)").unwrap();
    assert_eq!(
        quoted.send, "shout",
        "a parenthesis inside a quoted name is the name's"
    );
    assert_eq!(
        quoted.to_string(),
        "shout (!expiring \"Empowered (+20)\" 5)"
    );
    assert!(
        Step::parse("(hidden)")
            .unwrap_err()
            .contains("nothing to send")
    );
    assert!(Step::parse("fire hidden)").unwrap_err().contains("no `(`"));
    let held = Step::held(" attack(stunned) ", "why");
    assert_eq!(
        (held.send.as_str(), held.held.as_deref()),
        ("attack(stunned)", Some("why"))
    );
}

#[test]
fn overlay_merges_tables_and_replaces_the_rest() {
    let mut base: toml::Table = "a = 1\n[t]\nx = 1\ny = [1, 2]\n".parse().unwrap();
    let over: toml::Table = "b = 2\n[t]\ny = [3]\nz = 3\n".parse().unwrap();
    chain::overlay(&mut base, over);
    let expected: toml::Table = "a = 1\nb = 2\n[t]\nx = 1\ny = [3]\nz = 3\n"
        .parse()
        .unwrap();
    assert_eq!(base, expected);
}

#[test]
fn a_key_resolves_character_then_profile_then_global_then_default() {
    let dir = temp_dir("chain");
    write(
        &chain::global_path(&dir),
        "[rest]\nfried = 90\nencumbered = 50\n",
    )
    .unwrap();
    write(
        &chain::profile_path(&dir, "archer").unwrap(),
        "[rooms]\nhunting = 1\n[rest]\nencumbered = 20\n[routines]\na = [\"fire\"]\n",
    )
    .unwrap();
    write(
        &chain::character_path(&dir, "GS3", "Nisugi").unwrap(),
        "[rest]\nfried = 80\n[routines]\na = [\"hide (!hidden)\", \"fire (hidden)\"]\n",
    )
    .unwrap();

    let mine = load(&dir, Some("gs3"), Some("NISUGI"), "Archer").unwrap();
    assert_eq!(mine.profile.rest.fried, Some(80), "the character's");
    assert_eq!(
        mine.profile.rest.encumbered,
        Some(20),
        "the profile's, over the global"
    );
    assert_eq!(mine.profile.rooms.hunting, Some(1), "the profile's");
    assert_eq!(mine.profile.rest.overkill, 0, "the built-in default");
    assert_eq!(
        mine.profile.routines["a"].len(),
        2,
        "a list replaces, never appends"
    );
    assert_eq!(mine.sources.len(), 3, "{:?}", mine.sources);

    let theirs = load(&dir, Some("gs3"), Some("Someone"), "archer").unwrap();
    assert_eq!(
        theirs.profile.rest.fried,
        Some(90),
        "no character file: the global's"
    );
    assert_eq!(theirs.sources.len(), 2);

    let nobody = load(&dir, None, None, "archer").unwrap();
    assert_eq!(
        nobody.profile.rest.fried,
        Some(90),
        "no character known: the character level is skipped"
    );
}

#[test]
fn what_cannot_be_loaded_says_which_file() {
    let dir = temp_dir("errors");
    assert!(matches!(
        load(&dir, None, None, "nosuch"),
        Err(LoadError::Missing(_))
    ));
    assert!(matches!(
        load(&dir, None, None, "???"),
        Err(LoadError::BadName(_))
    ));

    let bad = chain::profile_path(&dir, "bad").unwrap();
    write(&bad, "this is not = = toml\n").unwrap();
    match load(&dir, None, None, "bad") {
        Err(LoadError::Malformed {
            path: Some(path), ..
        }) => assert_eq!(path, bad),
        other => panic!("{other:?}"),
    }

    write(
        &chain::profile_path(&dir, "wrongkey").unwrap(),
        "[rooms]\nhutning = 1\n",
    )
    .unwrap();
    match load(&dir, None, None, "wrongkey") {
        Err(LoadError::Malformed { why, .. }) => assert!(why.contains("hutning"), "{why}"),
        other => panic!("{other:?}"),
    }

    write(
        &chain::profile_path(&dir, "invalid").unwrap(),
        "targets = [{ routine = \"a\" }]\n",
    )
    .unwrap();
    assert!(matches!(
        load(&dir, None, None, "invalid"),
        Err(LoadError::Invalid(_))
    ));
}

#[test]
fn names_and_files() {
    assert_eq!(
        chain::file_name(" Ojandhaart "),
        Some("ojandhaart".to_owned())
    );
    assert_eq!(
        chain::file_name("atoll_path-2"),
        Some("atoll_path-2".to_owned())
    );
    assert_eq!(chain::file_name("???"), None);
    assert!(chain::character_path(Path::new("d"), "", "x").is_none());

    let dir = temp_dir("write");
    let path = chain::profile_path(&dir, "once").unwrap();
    chain::write_new(&path, "a = 1\n").unwrap();
    let again = chain::write_new(&path, "b = 2\n").unwrap_err();
    assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "a = 1\n",
        "the first file stands"
    );
}

/// Each setting in effect is named with the level that set it last -- the
/// character's file over the profile over global over the built-in
/// default -- once, in the table's order (`plan/50` §7 step 6).
#[test]
fn every_setting_says_where_it_came_from() {
    use chain::Level;
    let dir = temp_dir("origins");
    write(
        &chain::global_path(&dir),
        "[rooms]\nresting = 5\nhunting = 1\n",
    )
    .unwrap();
    write(
        &chain::profile_path(&dir, "archer").unwrap(),
        "[rooms]\nhunting = 10\n",
    )
    .unwrap();
    write(
        &chain::character_path(&dir, "Prime", "Nisugi").unwrap(),
        "[rooms]\nrally = [3]\n",
    )
    .unwrap();

    let levels = chain::named_levels(&dir, Some("Prime"), Some("Nisugi"), "archer").unwrap();
    let order: Vec<Level> = levels.iter().map(|(level, _)| *level).collect();
    assert_eq!(order, [Level::Global, Level::Profile, Level::Character]);
    let origins = chain::origins(levels).unwrap();
    let from = |name: &str| {
        origins
            .iter()
            .find(|(found, _, _)| found == name)
            .map(|(_, value, level)| (value.to_string(), *level))
    };
    assert_eq!(from("rooms.resting"), Some(("5".to_owned(), Level::Global)));
    assert_eq!(
        from("rooms.hunting"),
        Some(("10".to_owned(), Level::Profile))
    );
    assert_eq!(
        from("rooms.rally"),
        Some(("[3]".to_owned(), Level::Character))
    );
    assert_eq!(from("prepare"), Some(("[]".to_owned(), Level::Default)));
    let mut names: Vec<&str> = origins.iter().map(|(name, _, _)| name.as_str()).collect();
    let all = names.len();
    names.dedup();
    assert_eq!(names.len(), all, "each once");

    let theirs = chain::named_levels(&dir, Some("Prime"), Some("Someone"), "archer").unwrap();
    assert_eq!(theirs.len(), 2, "no file of their own");
}

/// An empty list says nothing of what it holds; asked of the profile, a
/// list of rooms holds numbers and a list of commands words.
#[test]
fn an_empty_list_says_what_it_holds() {
    let merged = chain::merge(Vec::new()).unwrap();
    assert!(chain::holds_numbers(&merged, "rooms.rally"));
    assert!(chain::holds_numbers(&merged, "rooms.boundaries"));
    assert!(!chain::holds_numbers(&merged, "prepare"));
    assert!(!chain::holds_numbers(&merged, "no.such"));
}

/// The profiles are every `.toml` in the folder, by name, in order; none
/// at all when there is no folder yet.
#[test]
fn the_profiles_are_listed_by_name() {
    let dir = temp_dir("names");
    assert!(chain::profile_names(&dir).unwrap().is_empty());
    for name in ["zeta", "alpha"] {
        write(&chain::profile_path(&dir, name).unwrap(), "").unwrap();
    }
    write(&chain::profiles_dir(&dir).join("notes.txt"), "").unwrap();
    assert_eq!(chain::profile_names(&dir).unwrap(), ["alpha", "zeta"]);
}

/// A sequence may hold another; one that holds itself, or two that hold
/// each other, is refused before the hunt begins.
#[test]
fn a_sequence_that_comes_back_to_itself_is_a_problem() {
    let with = |sequences: &str| {
        let text = format!(
            "[[targets]]
any = true
routine = \"a\"

[routines]
a = [\"open\"]

[sequences]
{sequences}"
        );
        Profile::parse(&text).map(|profile| profile.problems())
    };
    assert_eq!(
        with(
            "open = [\"store weapon\", \"close\"]
close = [\"ready weapon\"]
"
        ),
        Ok(Vec::new()),
        "one inside another is no circle"
    );
    assert_eq!(
        with(
            "open = [\"store weapon\", \"open\"]
"
        ),
        Ok(vec!["sequence open names itself".to_owned()])
    );
    assert_eq!(
        with(
            "open = [\"close\"]
close = [\"kick\", \"open\"]
"
        ),
        Ok(vec![
            "sequence close comes back to itself through open".to_owned(),
            "sequence open comes back to itself through close".to_owned(),
        ])
    );
}
