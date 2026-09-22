//! A character's settings file: one file, a section per system.
//!
//! The property that makes SHARING safe is the one tested hardest: a system
//! saving its own section must not cost another system its settings.

use std::path::PathBuf;

use cena_session::settings_store::{self, SettingsFile, SettingsLoadError};
use serde::{Deserialize, Serialize};

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-settings-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Mine {
    #[serde(default)]
    loud: bool,
}

#[test]
fn it_sits_beside_the_snapshot_under_the_same_name() {
    let dir = PathBuf::from("data");
    let snapshot = cena_session::character_store::store_path(&dir, "Prime", "Nisugi");
    let settings = settings_store::settings_path(&dir, "Prime", "Nisugi");
    // Lowercased by `store::safe_component`, so one character cannot get two
    // files on a case-sensitive filesystem. The guard that matters is that
    // both stores agree on the stem, and it still does.
    assert_eq!(snapshot, Some(dir.join("prime_nisugi.json")), "guard");
    assert_eq!(settings, Some(dir.join("prime_nisugi.settings.json")));
}

#[test]
fn a_character_with_no_file_has_every_default_and_nothing_is_written() {
    let dir = temp_dir("missing");
    let file = settings_store::load(&dir, "Prime", "Nisugi").expect("missing is not an error");
    assert_eq!(file, SettingsFile::new("Prime", "Nisugi"));
    assert_eq!(
        file.section::<Mine>("mine").expect("default"),
        Mine::default()
    );
    assert!(!dir.exists(), "loading created something");
}

#[test]
fn saving_one_section_keeps_a_section_this_build_has_never_heard_of() {
    // The hazard a typed struct-of-all-sections would have: a build that does
    // not know `highlights` reads the file, saves its own section, and the
    // player's highlights are gone.
    let dir = temp_dir("foreign");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = settings_store::settings_path(&dir, "Prime", "Nisugi").expect("path");
    std::fs::write(
        &path,
        r#"{"schema_version":1,"instance":"Prime","character":"Nisugi",
            "sections":{"highlights":{"rules":[{"match":"kobold","colour":"red"}]}}}"#,
    )
    .expect("write");

    let mut file = settings_store::load(&dir, "Prime", "Nisugi").expect("load");
    file.set_section("mine", &Mine { loud: true }).expect("set");
    settings_store::save(&dir, &file).expect("save");

    let again = settings_store::load(&dir, "Prime", "Nisugi").expect("reload");
    assert_eq!(
        again.section::<Mine>("mine").expect("mine"),
        Mine { loud: true }
    );
    assert_eq!(
        again.sections["highlights"]["rules"][0]["match"], "kobold",
        "another system's settings were lost by saving ours"
    );
}

#[test]
fn a_malformed_section_is_an_error_not_the_default() {
    // A player who mistyped a setting should be told, not silently given the
    // behaviour they were trying to change.
    let mut file = SettingsFile::new("Prime", "Nisugi");
    file.sections
        .insert("mine".to_owned(), serde_json::json!({"loud": "very"}));
    assert!(file.section::<Mine>("mine").is_err());
}

#[test]
fn a_file_from_a_newer_build_is_refused() {
    let dir = temp_dir("newer");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = settings_store::settings_path(&dir, "Prime", "Nisugi").expect("path");
    std::fs::write(
        &path,
        r#"{"schema_version":99,"instance":"Prime","character":"Nisugi"}"#,
    )
    .expect("write");
    assert!(matches!(
        settings_store::load(&dir, "Prime", "Nisugi"),
        Err(SettingsLoadError::Newer { found: 99 })
    ));
}

#[test]
fn a_file_describing_someone_else_is_refused() {
    let dir = temp_dir("wrong");
    settings_store::save(&dir, &SettingsFile::new("Prime", "Nisugi")).expect("save");
    let from = settings_store::settings_path(&dir, "Prime", "Nisugi").expect("path");
    let to = settings_store::settings_path(&dir, "Prime", "Other").expect("path");
    std::fs::rename(from, to).expect("rename");
    assert!(matches!(
        settings_store::load(&dir, "Prime", "Other"),
        Err(SettingsLoadError::WrongCharacter { .. })
    ));
}
