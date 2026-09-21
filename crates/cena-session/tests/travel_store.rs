//! The travel file: `cena_session::travel_store` (`plan/24` §5). One file,
//! a spot in it for each character, and targets every character shares.

use std::path::PathBuf;

use cena_session::travel_store::{
    TRAVEL_SCHEMA_VERSION, TravelFile, TravelLoadError, Whose, legacy_path, load, save,
    save_target, travel_path,
};

/// A directory this test alone owns, named after it.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-travel-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Nobody has travelled yet: no file, and that is ordinary.
#[test]
fn a_missing_file_is_an_empty_spot_not_an_error() {
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
    file.last_room = Some(228);
    let path = save(&dir, &file).unwrap();
    assert_eq!(path, travel_path(&dir));
    assert!(path.ends_with("travel.json"));
    assert_eq!(load(&dir, "GSIV", "Ashryn").unwrap(), file);
    assert!(
        !path.with_extension("json.tmp").exists(),
        "the temp file became the file"
    );
}

/// The author's shape, 2026-09-21: one file, and each character's spot in it.
#[test]
fn every_character_has_a_spot_in_the_one_file() {
    let dir = temp_dir("spots");
    let mut ashryn = TravelFile::new("GSIV", "Ashryn");
    ashryn
        .memories
        .insert("duskruin_origin".into(), "228".into());
    let mut nerten = TravelFile::new("GSIV", "Nerten");
    nerten.settings.insert("use_urchins".into(), "true".into());
    save(&dir, &ashryn).unwrap();
    save(&dir, &nerten).unwrap();

    let files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(files.len(), 1, "one file: {files:?}");
    assert_eq!(load(&dir, "GSIV", "Ashryn").unwrap(), ashryn);
    assert_eq!(load(&dir, "GSIV", "Nerten").unwrap(), nerten);
    // Found however the name is spelt, and saved back to the same spot.
    assert_eq!(
        load(&dir, "gsiv", "ASHRYN").unwrap().memories,
        ashryn.memories
    );
    save(&dir, &TravelFile::new("gsiv", "ASHRYN")).unwrap();
    let text = std::fs::read_to_string(travel_path(&dir)).unwrap();
    // Compared without case: `ASHRYN` must not hide from a count of `shryn`.
    assert_eq!(text.to_lowercase().matches("ashryn").count(), 1, "{text}");
}

/// The reason saves change one spot and never write back what they read: two
/// characters finish their trips holding copies loaded before either saved.
#[test]
fn a_character_saving_a_stale_copy_does_not_erase_anyone_else() {
    let dir = temp_dir("stale");
    let mut ashryn = load(&dir, "GSIV", "Ashryn").unwrap();
    let mut nerten = load(&dir, "GSIV", "Nerten").unwrap();
    save_target(&dir, EVERYONE, "my shop", &[7120]).unwrap();

    ashryn
        .memories
        .insert("duskruin_origin".into(), "228".into());
    save(&dir, &ashryn).unwrap();
    // Nerten's copy knows nothing of Ashryn's memory, nor of the shop.
    nerten.last_room = Some(3668);
    save(&dir, &nerten).unwrap();

    let ashryn = load(&dir, "GSIV", "Ashryn").unwrap();
    assert_eq!(
        ashryn.memories.get("duskruin_origin").map(String::as_str),
        Some("228")
    );
    assert_eq!(ashryn.targets.get("my shop"), Some(&vec![7120]));
    assert_eq!(load(&dir, "GSIV", "Nerten").unwrap().last_room, Some(3668));
}

const EVERYONE: Whose<'_> = Whose::Everyone;
const ASHRYN: Whose<'_> = Whose::Character {
    instance: "GSIV",
    character: "Ashryn",
};

/// `--global` (author, 2026-09-21): the same shortcuts for all their guys,
/// whichever instance each is on.
#[test]
fn a_global_target_is_everyones_on_every_instance() {
    let dir = temp_dir("targets");
    save_target(&dir, EVERYONE, "my shop", &[7120]).unwrap();
    save_target(&dir, EVERYONE, "pond", &[1, 2]).unwrap();
    for (instance, character) in [("GSIV", "Ashryn"), ("GSIV", "Nerten"), ("GSPlat", "Ashryn")] {
        let seen = load(&dir, instance, character).unwrap().targets;
        assert_eq!(seen.get("my shop"), Some(&vec![7120]));
        assert_eq!(seen.get("pond"), Some(&vec![1, 2]));
    }
    // go2's rule (`go2.lic:1149-1157`): a name that means one room is
    // replaced; one that means several gains the new room, once.
    save_target(&dir, EVERYONE, "my shop", &[228]).unwrap();
    save_target(&dir, EVERYONE, "pond", &[3, 2]).unwrap();
    let seen = load(&dir, "GSIV", "Ashryn").unwrap().targets;
    assert_eq!(seen.get("my shop"), Some(&vec![228]));
    assert_eq!(seen.get("pond"), Some(&vec![1, 2, 3]));
    // Naming it nothing forgets it.
    save_target(&dir, EVERYONE, "pond", &[]).unwrap();
    assert_eq!(
        load(&dir, "GSIV", "Ashryn").unwrap().targets.get("pond"),
        None
    );
}

/// Without `--global` (author, 2026-09-21) a name is the character's own:
/// nobody else sees it, it is looked for before everyone's, and forgetting
/// it uncovers the one everyone shares.
#[test]
fn a_characters_own_target_is_its_own_and_comes_first() {
    let dir = temp_dir("own-targets");
    save_target(&dir, EVERYONE, "my shop", &[7120]).unwrap();
    save_target(&dir, ASHRYN, "my shop", &[228]).unwrap();
    save_target(&dir, ASHRYN, "hideout", &[9]).unwrap();

    let ashryn = load(&dir, "GSIV", "Ashryn").unwrap();
    assert_eq!(ashryn.targets.get("my shop"), Some(&vec![228]));
    assert_eq!(ashryn.targets.get("hideout"), Some(&vec![9]));
    let nerten = load(&dir, "GSIV", "Nerten").unwrap().targets;
    assert_eq!(nerten.get("my shop"), Some(&vec![7120]));
    assert_eq!(nerten.get("hideout"), None);

    // A trip's save writes no targets: what Ashryn read was both kinds
    // together, and writing that back would make everyone's her own.
    save(&dir, &ashryn).unwrap();
    save_target(&dir, EVERYONE, "my shop", &[3668]).unwrap();
    save_target(&dir, ASHRYN, "my shop", &[]).unwrap();
    let ashryn = load(&dir, "GSIV", "Ashryn").unwrap().targets;
    assert_eq!(
        ashryn.get("my shop"),
        Some(&vec![3668]),
        "everyone's, uncovered"
    );
    assert_eq!(
        ashryn.get("hideout"),
        Some(&vec![9]),
        "and her own survived her save"
    );
}

/// A target saved before any trip must not make an empty spot: the old
/// file is never read again once a spot exists, so its memories would be
/// stranded -- and so would the character, at whatever event they record.
#[test]
fn a_target_saved_first_still_moves_the_old_file_in() {
    let dir = temp_dir("target-first");
    std::fs::create_dir_all(&dir).unwrap();
    let old = legacy_path(&dir, "GSIV", "Ashryn").unwrap();
    std::fs::write(
        &old,
        r#"{"schema_version":2,"instance":"GSIV","character":"Ashryn",
            "memories":{"duskruin_origin":"228"},"targets":{"home":[228]}}"#,
    )
    .unwrap();
    save_target(&dir, ASHRYN, "hideout", &[9]).unwrap();
    let file = load(&dir, "GSIV", "Ashryn").unwrap();
    assert_eq!(
        file.memories.get("duskruin_origin").map(String::as_str),
        Some("228")
    );
    assert_eq!(file.targets.get("home"), Some(&vec![228]));
    assert_eq!(file.targets.get("hideout"), Some(&vec![9]));
}

/// It is not the snapshot: rewriting one cannot touch the other.
#[test]
fn it_is_not_the_snapshots_file() {
    let dir = temp_dir("beside");
    let snapshot = cena_session::character_store::store_path(&dir, "GSIV", "Ashryn").unwrap();
    assert_ne!(travel_path(&dir), snapshot);
    assert_eq!(travel_path(&dir).parent(), snapshot.parent());
}

/// The snapshot's rule is refuse-and-resync. This file's is the opposite: what
/// cannot be read must not be replaced by nothing -- **and a save refuses
/// too**, since one character's save would otherwise erase everyone.
#[test]
fn a_file_that_cannot_be_trusted_is_refused_and_left_alone() {
    let dir = temp_dir("refused");
    std::fs::create_dir_all(&dir).unwrap();
    let path = travel_path(&dir);

    std::fs::write(&path, "{ not json").unwrap();
    assert!(matches!(
        load(&dir, "GSIV", "Ashryn"),
        Err(TravelLoadError::Unreadable(_))
    ));
    assert!(save(&dir, &TravelFile::new("GSIV", "Ashryn")).is_err());
    assert!(save_target(&dir, EVERYONE, "my shop", &[7120]).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");

    let newer = format!(
        r#"{{"schema_version":{},"characters":{{"GSIV_Ashryn":{{"later":true}}}}}}"#,
        TRAVEL_SCHEMA_VERSION + 1
    );
    std::fs::write(&path, &newer).unwrap();
    assert!(matches!(
        load(&dir, "GSIV", "Ashryn"),
        Err(TravelLoadError::Newer { .. })
    ));
    assert!(save(&dir, &TravelFile::new("GSIV", "Ashryn")).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
}

/// Versions 1 and 2 were a file per character. Moving in is a migration, not
/// a refusal: everything the old file held is kept, and the old file is left.
#[test]
fn a_characters_old_file_is_read_into_its_spot() {
    let dir = temp_dir("migrate");
    std::fs::create_dir_all(&dir).unwrap();
    let old = legacy_path(&dir, "GSIV", "Ashryn").unwrap();
    assert!(old.ends_with("GSIV_Ashryn.travel.json"));
    let text = r#"{"schema_version":2,"instance":"GSIV","character":"Ashryn",
                   "settings":{"ice_mode":"wait"},"memories":{"duskruin_origin":"228"},
                   "targets":{"home":[228,3668],"my shop":[1]},"last_room":228}"#;
    std::fs::write(&old, text).unwrap();
    // Everyone has a shop by that name too: the character's own comes first.
    save_target(&dir, EVERYONE, "my shop", &[7120]).unwrap();

    let file = load(&dir, "GSIV", "Ashryn").unwrap();
    assert_eq!(
        file.settings.get("ice_mode").map(String::as_str),
        Some("wait")
    );
    assert_eq!(
        file.memories.get("duskruin_origin").map(String::as_str),
        Some("228")
    );
    assert_eq!(file.last_room, Some(228));
    assert_eq!(file.targets.get("home"), Some(&vec![228, 3668]));
    assert_eq!(file.targets.get("my shop"), Some(&vec![1]));

    // The first save moves it in -- and its targets stay its own.
    save(&dir, &file).unwrap();
    let nerten = load(&dir, "GSIV", "Nerten").unwrap().targets;
    assert_eq!(nerten.get("home"), None);
    assert_eq!(nerten.get("my shop"), Some(&vec![7120]));
    assert_eq!(
        std::fs::read_to_string(&old).unwrap(),
        text,
        "left as it was"
    );
    // ...and once it has a spot, the old file is never read again.
    std::fs::write(&old, "{ not json").unwrap();
    assert_eq!(load(&dir, "GSIV", "Ashryn").unwrap(), file);
    save(&dir, &file).unwrap();
}

/// A version-1 file, from before targets and the last room, moves in too.
#[test]
fn a_version_one_file_moves_in_with_what_it_had() {
    let dir = temp_dir("v1");
    std::fs::create_dir_all(&dir).unwrap();
    let old = legacy_path(&dir, "GSIV", "Ashryn").unwrap();
    let text = r#"{"schema_version":1,"instance":"GSIV","character":"Ashryn",
                   "memories":{"duskruin_origin":"228"}}"#;
    std::fs::write(&old, text).unwrap();
    let file = load(&dir, "GSIV", "Ashryn").unwrap();
    assert_eq!(file.memories.len(), 1);
    assert!(file.targets.is_empty() && file.last_room.is_none());
}

/// An old file that is someone else's, or broken, is not moved in -- and not
/// papered over with an empty spot, which the next save would make permanent.
#[test]
fn an_old_file_that_cannot_be_trusted_stops_the_move() {
    let dir = temp_dir("bad-legacy");
    std::fs::create_dir_all(&dir).unwrap();
    let old = legacy_path(&dir, "GSIV", "Ashryn").unwrap();
    std::fs::write(
        &old,
        r#"{"schema_version":2,"instance":"GSIV","character":"Someone"}"#,
    )
    .unwrap();
    assert!(matches!(
        load(&dir, "GSIV", "Ashryn"),
        Err(TravelLoadError::WrongCharacter { .. })
    ));
    assert!(save(&dir, &TravelFile::new("GSIV", "Ashryn")).is_err());
    assert!(!travel_path(&dir).exists());
}
