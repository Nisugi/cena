//! The character store: round-trip, staleness, and the four ways a load fails.
//!
//! M3 step 8. These write real files, into a per-test temporary directory
//! under the OS temp dir -- not into the workspace, and not a shared one, so
//! the tests do not race each other.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use cena_model::state::character::snapshot::{CharacterSnapshot, Group, SCHEMA_VERSION};
use cena_model::{SkillKind, SkillLine, SkillSet, StatKind, StatValue};
use cena_session::character_store::{LoadError, load, save, store_path};

/// A directory this test alone owns.
///
/// Named after the test, so a failure leaves behind a directory that says
/// which test wrote it.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-store-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A snapshot with something in every group, so a round-trip proves more than
/// that an empty struct survives.
fn populated() -> CharacterSnapshot {
    let mut snapshot = CharacterSnapshot::new("GameInstance", "Ashryn");
    snapshot.stats.insert(
        StatKind::Strength,
        cena_model::Stat {
            normal: Some(StatValue {
                value: 110,
                bonus: 30,
            }),
            ascended: Some(StatValue {
                value: 115,
                bonus: 32,
            }),
            enhanced: None,
            enhanced_is_bolded: false,
        },
    );
    snapshot.identity.race = Some("Half-Elf".to_owned());
    snapshot.identity.profession = Some("Ranger".to_owned());

    let mut skills = SkillSet::default();
    if let Some(line) =
        SkillLine::classify("  Two Weapon Combat..................|     312     212")
    {
        skills.apply(&line, true);
    }
    snapshot.skills = skills;
    snapshot.touch(
        Group::Stats,
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
    );
    snapshot
}

/// Write, read back, and get the same thing.
///
/// The plan's own requirement for this step. Equality is on the whole struct,
/// so a field added later that serde cannot round-trip fails here rather than
/// silently reading back as `Default`.
#[test]
fn a_snapshot_round_trips() {
    let dir = temp_dir("round-trip");
    let want = populated();

    let path = save(&dir, &want).expect("save should succeed");
    assert!(path.exists(), "save must write the file it names");

    let got = load(&dir, "GameInstance", "Ashryn").expect("load should succeed");
    assert_eq!(got, want);
}

/// The file is JSON a person can read.
///
/// The author's *"way to reset and refresh it"* starts with looking at what is
/// stored, so this is a product requirement rather than a formatting
/// preference.
#[test]
fn the_file_is_readable_json() {
    let dir = temp_dir("readable");
    let _ = save(&dir, &populated()).expect("save");
    let path = store_path(&dir, "GameInstance", "Ashryn").expect("path");
    let text = std::fs::read_to_string(path).expect("read");

    assert!(text.contains('\n'), "pretty-printed, not one line");
    assert!(text.contains("\"schema_version\""));
    assert!(text.contains("\"Ashryn\""));
}

/// **A snapshot from another schema version is refused, not migrated.**
///
/// A snapshot is the output of a parser, so one written by a different parser
/// may have read a column differently. Lich makes the same call for the same
/// reason (`cli.rb:73-80`). The character re-syncs.
#[test]
fn an_older_schema_is_refused() {
    let dir = temp_dir("old-schema");
    let mut snapshot = populated();
    let _ = save(&dir, &snapshot).expect("save");

    // Guard: it loads while current.
    assert!(load(&dir, "GameInstance", "Ashryn").is_ok(), "guard");

    snapshot.schema_version = SCHEMA_VERSION - 1;
    let _ = save(&dir, &snapshot).expect("save");

    match load(&dir, "GameInstance", "Ashryn") {
        Err(LoadError::WrongVersion { found, expected }) => {
            assert_eq!(found, SCHEMA_VERSION - 1);
            assert_eq!(expected, SCHEMA_VERSION);
        }
        other => panic!("expected WrongVersion, got {other:?}"),
    }
}

/// A NEWER schema is refused too.
///
/// Not `>=`. A file from a newer build was written by a classifier this one
/// does not have, and reading its fields as if they meant what they mean here
/// is the same error as reading an older one.
#[test]
fn a_newer_schema_is_also_refused() {
    let dir = temp_dir("new-schema");
    let mut snapshot = populated();
    snapshot.schema_version = SCHEMA_VERSION + 1;
    let _ = save(&dir, &snapshot).expect("save");

    assert!(matches!(
        load(&dir, "GameInstance", "Ashryn"),
        Err(LoadError::WrongVersion { .. })
    ));
}

/// A character with no file is `Missing`, not an error to report.
#[test]
fn an_unstored_character_is_missing() {
    let dir = temp_dir("missing");
    assert!(matches!(
        load(&dir, "GameInstance", "Nobody"),
        Err(LoadError::Missing)
    ));
}

/// **The instance is part of the identity.**
///
/// Two characters of the same name on different instances are different
/// people. Loading one into the other would look like a character who had
/// silently lost training, which is why Lich keys its table the same way
/// (`infomon.rb:86`).
#[test]
fn the_same_name_on_two_instances_is_two_characters() {
    let dir = temp_dir("two-instances");

    let mut prime = CharacterSnapshot::new("InstanceOne", "Ashryn");
    prime.identity.profession = Some("Ranger".to_owned());
    let mut other = CharacterSnapshot::new("InstanceTwo", "Ashryn");
    other.identity.profession = Some("Wizard".to_owned());

    let prime_path = save(&dir, &prime).expect("save");
    let other_path = save(&dir, &other).expect("save");
    assert_ne!(prime_path, other_path, "two files, not one");

    let loaded = load(&dir, "InstanceOne", "Ashryn").expect("load");
    assert_eq!(loaded.identity.profession.as_deref(), Some("Ranger"));
    let loaded = load(&dir, "InstanceTwo", "Ashryn").expect("load");
    assert_eq!(loaded.identity.profession.as_deref(), Some("Wizard"));
}

/// A file whose contents name someone else is refused.
///
/// Reachable only by renaming a file by hand or copying it between machines.
/// Checked because the failure it prevents -- one character's skills loaded
/// into another -- is silent.
#[test]
fn a_renamed_file_is_refused() {
    let dir = temp_dir("renamed");
    let snapshot = CharacterSnapshot::new("GameInstance", "Ashryn");
    let from = save(&dir, &snapshot).expect("save");
    let to = store_path(&dir, "GameInstance", "Baelor").expect("path");
    std::fs::rename(&from, &to).expect("rename");

    match load(&dir, "GameInstance", "Baelor") {
        Err(LoadError::WrongCharacter { found }) => assert!(found.contains("Ashryn")),
        other => panic!("expected WrongCharacter, got {other:?}"),
    }
}

/// A file copied from ANOTHER INSTANCE is refused.
///
/// Found by mutation: deleting the instance half of `describes` left the suite
/// green, because `the_same_name_on_two_instances_is_two_characters` is
/// satisfied by the *path* separating them -- the contents check is never
/// reached on that route.
///
/// This is the route that reaches it: same character name, same filename, and
/// the file's own `instance` field says otherwise. It happens when a store is
/// copied between machines or a directory is shared, and the failure it
/// prevents -- one character's skills loaded into another's session -- is
/// silent.
#[test]
fn a_file_from_another_instance_is_refused() {
    let dir = temp_dir("wrong-instance");
    let mut snapshot = CharacterSnapshot::new("InstanceOne", "Ashryn");
    snapshot.identity.profession = Some("Ranger".to_owned());
    let _ = save(&dir, &snapshot).expect("save");

    // Move it to where InstanceTwo's Ashryn would live, as a copy would.
    let from = store_path(&dir, "InstanceOne", "Ashryn").expect("path");
    let to = store_path(&dir, "InstanceTwo", "Ashryn").expect("path");
    std::fs::rename(&from, &to).expect("rename");

    match load(&dir, "InstanceTwo", "Ashryn") {
        Err(LoadError::WrongCharacter { found }) => {
            assert!(
                found.contains("InstanceOne"),
                "names the real owner: {found}"
            );
        }
        other => panic!("expected WrongCharacter, got {other:?}"),
    }
}

/// Corrupt JSON is `Unreadable`, not a panic.
#[test]
fn a_corrupt_file_is_unreadable() {
    let dir = temp_dir("corrupt");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = store_path(&dir, "GameInstance", "Ashryn").expect("path");
    std::fs::write(&path, "{ this is not json").expect("write");

    assert!(matches!(
        load(&dir, "GameInstance", "Ashryn"),
        Err(LoadError::Unreadable(_))
    ));
}

/// The name is case-insensitive, because the wire is not consistent.
#[test]
fn the_name_matches_regardless_of_case() {
    let dir = temp_dir("case");
    let _ = save(&dir, &CharacterSnapshot::new("GameInstance", "Ashryn")).expect("save");
    assert!(load(&dir, "gameinstance", "ASHRYN").is_ok());
}

/// **The case rule is in the FILENAME, not only in the comparison.**
///
/// The test above passed on Windows and failed on Linux for a reason neither
/// platform's result explained: `describes` compares case-insensitively
/// (`snapshot.rs:358`) but the filename preserved case, so `save` wrote
/// `GameInstance_Ashryn.json` and a differently-capitalised `load` opened a
/// different path. NTFS is case-insensitive and hid it; ext4 is not.
///
/// So this asserts on the **path**, which is platform-independent, rather than
/// on a load succeeding, which is not. A test that only round-trips cannot tell
/// the two filesystems apart -- which is precisely how the bug survived.
#[test]
fn the_path_itself_does_not_depend_on_case() {
    let dir = temp_dir("case-path");
    assert_eq!(
        store_path(&dir, "GameInstance", "Ashryn"),
        store_path(&dir, "gameinstance", "ASHRYN"),
        "one character must have one file on every filesystem"
    );
    let path = store_path(&dir, "GameInstance", "Ashryn").expect("path");
    let name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("a filename");
    assert_eq!(
        name, "gameinstance_ashryn.json",
        "the stored name is lowercased, so no filesystem gets a choice"
    );
}

/// A name that sanitises to nothing has no path, rather than a shared one.
///
/// A name of only punctuation gets **no file** instead of a default one that a
/// second such character would then share.
#[test]
fn an_unusable_name_has_no_path() {
    let dir = temp_dir("unusable");
    assert_eq!(store_path(&dir, "GameInstance", "---"), None);
    assert_eq!(store_path(&dir, "", "Ashryn"), None);
}

/// **Filtering IS lossy, and the sink's comment overstates what it buys.**
///
/// `sink/writer.rs:278` says characters are filtered rather than substituted
/// "so two names cannot collide through substitution". That is true of
/// substitution specifically and false as a general claim: filtering collides
/// too, and `A-B` and `A_B` both reduce to `AB`.
///
/// It does not matter here, for a reason about the game rather than the code:
/// MEASURED against a live capture, character names are alphanumeric only --
/// no spaces, hyphens or apostrophes -- so two names differing only in
/// punctuation cannot both exist.
///
/// Recorded rather than hidden, so that if the name vocabulary ever widens
/// (`DragonRealms` is deferred, not ruled out) the assumption is written where
/// someone will find it.
#[test]
fn filtering_collides_and_the_game_is_why_that_is_safe() {
    let dir = temp_dir("collide");
    assert_eq!(
        store_path(&dir, "GameInstance", "A-B"),
        store_path(&dir, "GameInstance", "A_B"),
        "filtering is lossy; both reduce to `AB`"
    );
    // The names that actually occur do not collide.
    assert_ne!(
        store_path(&dir, "GameInstance", "Ashryn"),
        store_path(&dir, "GameInstance", "Baelor")
    );
}

/// A group never taught is stale, which is what syncs a new character.
#[test]
fn an_untaught_group_is_always_stale() {
    let snapshot = CharacterSnapshot::new("GameInstance", "Ashryn");
    let stale = snapshot.stale_groups(SystemTime::UNIX_EPOCH, Duration::from_mins(1));
    assert_eq!(stale, Group::ALL.to_vec(), "nothing is known yet");
    assert!(!snapshot.is_known(Group::Skills));
}

/// A group taught recently is fresh; one taught long ago is not.
#[test]
fn staleness_is_per_group() {
    let mut snapshot = CharacterSnapshot::new("GameInstance", "Ashryn");
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
    snapshot.touch(Group::Stats, now - Duration::from_secs(10));
    snapshot.touch(Group::Skills, now - Duration::from_secs(5_000));

    let stale = snapshot.stale_groups(now, Duration::from_mins(1));
    assert!(!stale.contains(&Group::Stats), "taught 10s ago");
    assert!(stale.contains(&Group::Skills), "taught 5000s ago");
    assert!(stale.contains(&Group::Psms), "never taught");
}

/// **A clock that went backwards does not make data fresh.**
///
/// `duration_since` fails when `now` precedes the stamp -- a corrected system
/// clock, or a file copied from a machine whose clock ran ahead. Treating that
/// as fresh would trust a timestamp we have just proved wrong.
#[test]
fn a_future_timestamp_is_stale() {
    let mut snapshot = CharacterSnapshot::new("GameInstance", "Ashryn");
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    snapshot.touch(Group::Stats, now + Duration::from_secs(10_000));

    assert!(
        snapshot
            .stale_groups(now, Duration::from_mins(1))
            .contains(&Group::Stats),
        "a stamp in the future is not evidence of freshness"
    );
}

/// Every group names the command that refreshes it.
///
/// So "reset and refresh" can issue them without the caller knowing the
/// commands by heart.
#[test]
fn every_group_names_its_refresh_command() {
    for group in Group::ALL {
        let command = group.refresh_command();
        assert!(!command.is_empty(), "{group:?} has no refresh command");
    }
    assert_eq!(Group::Skills.refresh_command(), "skills full");
    assert_eq!(
        Group::Enhancives.refresh_command(),
        "inventory enhancive totals"
    );
}

/// Saving twice leaves one file and the newer contents.
///
/// The write is atomic -- temp file, then rename -- so the second save must not
/// leave a `.tmp` behind or a half-written target.
#[test]
fn saving_twice_leaves_one_clean_file() {
    let dir = temp_dir("atomic");
    let mut snapshot = populated();
    let _ = save(&dir, &snapshot).expect("first save");
    snapshot.identity.race = Some("Sylvankind".to_owned());
    let _ = save(&dir, &snapshot).expect("second save");

    let entries: Vec<String> = std::fs::read_dir(&dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries.len(), 1, "one file, no leftover temp: {entries:?}");

    let got = load(&dir, "GameInstance", "Ashryn").expect("load");
    assert_eq!(got.identity.race.as_deref(), Some("Sylvankind"));
}

/// A successful save leaves no temp file behind.
///
/// This is the part of the atomic write that IS testable. See
/// `character_store.rs`'s `save` for why the atomicity itself is not, and what
/// is done instead.
#[test]
fn a_successful_save_leaves_no_temp_file() {
    let dir = temp_dir("no-temp");
    let _ = save(&dir, &populated()).expect("save");
    let leftovers: Vec<String> = std::fs::read_dir(&dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("tmp"))
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
}

/// The store lands in the directory it was given.
///
/// `rename` is atomic only within one filesystem, so the temp file must share
/// the target's directory. This asserts the target's location, which is the
/// observable half of that.
#[test]
fn the_store_lands_in_the_given_directory() {
    let dir = temp_dir("location");
    let path = save(&dir, &populated()).expect("save");
    assert_eq!(path.parent(), Some(dir.as_path()));
}

/// The skills group survives the round trip with its bold flag.
///
/// A spot-check on the nested types: `SkillSet` is a map of maps, and the
/// `enhanced` flag is the one field a lazy `Default` would silently drop.
#[test]
fn nested_skill_state_survives() {
    let dir = temp_dir("skills");
    let _ = save(&dir, &populated()).expect("save");
    let got = load(&dir, "GameInstance", "Ashryn").expect("load");

    let skill = got.skills.get(SkillKind::TwoWeaponCombat).copied();
    assert_eq!(skill.and_then(|s| s.ranks), Some(212));
    assert_eq!(skill.and_then(|s| s.bonus), Some(312));
    assert!(skill.is_some_and(|s| s.enhanced), "the bold flag survives");
}
