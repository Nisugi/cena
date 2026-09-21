//! The travel file: `cena_session::travel_store` (`plan/24` §5).

use std::path::PathBuf;

use cena_session::travel_store::{
    TRAVEL_SCHEMA_VERSION, TravelFile, TravelLoadError, load, save, travel_path,
};

/// A directory this test alone owns, named after it.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-travel-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A character who has never travelled has no file, and that is ordinary.
#[test]
fn a_missing_file_is_an_empty_one_not_an_error() {
    let dir = temp_dir("missing");
    let file = load(&dir, "GSIV", "Ashryn").unwrap();
    assert_eq!(file, TravelFile::new("GSIV", "Ashryn"));
    assert!(!dir.exists(), "loading writes nothing");
}

#[test]
fn a_memory_survives_being_written_and_read() {
    let dir = temp_dir("round-trip");
    let mut file = TravelFile::new("GSIV", "Ashryn");
    file.memories.insert("duskruin_origin".into(), "228".into());
    file.settings.insert("ice_mode".into(), "wait".into());
    let path = save(&dir, &file).unwrap();
    assert_eq!(path, travel_path(&dir, "GSIV", "Ashryn").unwrap());
    assert!(path.ends_with("GSIV_Ashryn.travel.json"));
    assert_eq!(load(&dir, "GSIV", "Ashryn").unwrap(), file);
    assert!(
        !path.with_extension("json.tmp").exists(),
        "the temp file became the file"
    );
}

/// It lives beside the snapshot and is not the snapshot: the two names differ,
/// so rewriting one cannot touch the other.
#[test]
fn it_is_not_the_snapshots_file() {
    let dir = temp_dir("beside");
    let travel = travel_path(&dir, "GSIV", "Ashryn").unwrap();
    let snapshot = cena_session::character_store::store_path(&dir, "GSIV", "Ashryn").unwrap();
    assert_ne!(travel, snapshot);
    assert_eq!(travel.parent(), snapshot.parent());
}

/// The snapshot's rule is refuse-and-resync. This file's is the opposite: what
/// cannot be read must not be replaced by nothing.
#[test]
fn a_file_that_cannot_be_trusted_is_refused_and_left_alone() {
    let dir = temp_dir("refused");
    std::fs::create_dir_all(&dir).unwrap();
    let path = travel_path(&dir, "GSIV", "Ashryn").unwrap();

    std::fs::write(&path, "{ not json").unwrap();
    assert!(matches!(
        load(&dir, "GSIV", "Ashryn"),
        Err(TravelLoadError::Unreadable(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");

    let mut newer = TravelFile::new("GSIV", "Ashryn");
    newer.schema_version = TRAVEL_SCHEMA_VERSION + 1;
    save(&dir, &newer).unwrap();
    assert!(matches!(
        load(&dir, "GSIV", "Ashryn"),
        Err(TravelLoadError::Newer { .. })
    ));

    save(&dir, &TravelFile::new("GSIV", "Someone")).unwrap();
    std::fs::rename(travel_path(&dir, "GSIV", "Someone").unwrap(), &path).unwrap();
    assert!(matches!(
        load(&dir, "GSIV", "Ashryn"),
        Err(TravelLoadError::WrongCharacter { .. })
    ));
}

/// A file written before a field existed still loads.
#[test]
fn a_file_without_memories_yet_is_one_with_none() {
    let dir = temp_dir("sparse");
    std::fs::create_dir_all(&dir).unwrap();
    let path = travel_path(&dir, "GSIV", "Ashryn").unwrap();
    let text = r#"{"schema_version":1,"instance":"GSIV","character":"Ashryn"}"#;
    // A version-1 file, which is also one from before `targets` existed.
    std::fs::write(&path, text).unwrap();
    assert_eq!(
        load(&dir, "GSIV", "Ashryn").unwrap(),
        TravelFile::new("GSIV", "Ashryn")
    );
}

/// Version 1 to 2 is a migration, not a refusal: what was remembered is kept,
/// what is new starts empty, and the file says 2 from then on.
#[test]
fn a_version_one_file_is_migrated_and_keeps_what_it_held() {
    let dir = temp_dir("migrate");
    std::fs::create_dir_all(&dir).unwrap();
    let path = travel_path(&dir, "GSIV", "Ashryn").unwrap();
    let text = r#"{"schema_version":1,"instance":"GSIV","character":"Ashryn",
                   "memories":{"duskruin_origin":"228"}}"#;
    std::fs::write(&path, text).unwrap();

    let mut file = load(&dir, "GSIV", "Ashryn").unwrap();
    assert_eq!(file.schema_version, TRAVEL_SCHEMA_VERSION);
    assert_eq!(
        file.memories.get("duskruin_origin").map(String::as_str),
        Some("228")
    );
    assert!(file.targets.is_empty() && file.last_room.is_none());

    file.targets.insert("home".into(), vec![228, 3668]);
    file.last_room = Some(228);
    save(&dir, &file).unwrap();
    assert_eq!(load(&dir, "GSIV", "Ashryn").unwrap(), file);
}
