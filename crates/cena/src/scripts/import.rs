//! `;scripts import <Lich folder>`: the player's Lich scripts and their
//! settings, brought into Hydra's data folder once (`plan/46` §5, §10
//! question 6: *"lich script settings? would be saved in lich no? so data
//! folder?"*).
//!
//! **The script tables, not the file.** Lich's `lich.db3` holds a login's
//! cache too (`simu_game_entry`) and Lich's own settings; a script needs
//! three of its tables -- `script_setting`, `script_auto_settings`
//! (`Settings`, `CharSettings`, `GameSettings`) and `uservars` (`Vars`,
//! `UserVars`) -- and nothing else is copied. Their rows go into Hydra's
//! `lich.db3` as they are, Lich's Marshal blobs untouched, **replacing** a
//! row Hydra has under the same key: the player asked for Lich's. Rows are
//! merged into the file a running runner holds open, so nothing need stop;
//! a script already running keeps what it has read.
//!
//! **Scripts are copied, never over one Hydra has**: the player's edits in
//! Hydra's folder stay theirs. `.lic` and `.rb`, in the folder and its
//! `custom` folders, as the runner finds them. And Lich's own
//! `gameobj-data.xml`, which `GameObj` classifies items by, when Hydra has
//! none.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

/// The tables a script's stores are, as Lich makes them (`lich.rb`,
/// `Lich.init_db`), so a runner started later finds them as Lich left them.
const TABLES: &[(&str, &str)] = &[
    (
        "script_setting",
        "CREATE TABLE IF NOT EXISTS script_setting (script TEXT NOT NULL, name TEXT NOT NULL, value BLOB, PRIMARY KEY(script, name));",
    ),
    (
        "script_auto_settings",
        "CREATE TABLE IF NOT EXISTS script_auto_settings (script TEXT NOT NULL, scope TEXT, hash BLOB, PRIMARY KEY(script, scope));",
    ),
    (
        "uservars",
        "CREATE TABLE IF NOT EXISTS uservars (scope TEXT NOT NULL, hash BLOB, PRIMARY KEY(scope));",
    ),
];

/// What an import brought.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Imported {
    /// Rows copied, by table.
    pub(crate) rows: Vec<(&'static str, usize)>,
    /// Scripts copied.
    pub(crate) scripts: usize,
    /// Scripts Hydra already had, and kept.
    pub(crate) kept: usize,
    /// Whether `gameobj-data.xml` was copied.
    pub(crate) gameobj: bool,
}

impl Imported {
    /// What it brought, in a line for the player.
    pub(crate) fn said(&self, from: &Path) -> String {
        let rows: usize = self.rows.iter().map(|(_, n)| n).sum();
        let mut said = format!(
            "Scripts: imported from {}: {rows} settings rows, {} scripts",
            from.display(),
            self.scripts
        );
        if self.kept > 0 {
            let _ = write!(said, " ({} Hydra already had, kept)", self.kept);
        }
        if self.gameobj {
            said.push_str(", and Lich's item types");
        }
        said.push_str(". Scripts already running keep what they read.");
        said
    }
}

/// Import from `lich`, Lich's own folder (its `data` and `scripts`), into
/// Hydra's data folder `hydra`: the stores into `hydra/lich/lich.db3`,
/// the scripts into `hydra/scripts`.
///
/// # Errors
///
/// `lich` is not a Lich folder, or a file could not be read or written.
pub(crate) fn import(lich: &Path, hydra: &Path) -> Result<Imported, String> {
    let (data, scripts) = (lich.join("data"), lich.join("scripts"));
    if !data.join("lich.db3").is_file() && !scripts.is_dir() {
        return Err(format!(
            "{} is not a Lich folder: it has no data\\lich.db3 and no scripts folder",
            lich.display()
        ));
    }
    let mut imported = Imported::default();
    let stores = hydra.join("lich");
    std::fs::create_dir_all(&stores).map_err(|e| format!("{}: {e}", stores.display()))?;
    if data.join("lich.db3").is_file() {
        imported.rows = settings(&data.join("lich.db3"), &stores.join("lich.db3"))
            .map_err(|e| format!("the settings were not imported: {e}"))?;
    }
    let gameobj = stores.join("gameobj-data.xml");
    if data.join("gameobj-data.xml").is_file() && !gameobj.exists() {
        std::fs::copy(data.join("gameobj-data.xml"), &gameobj)
            .map_err(|e| format!("{}: {e}", gameobj.display()))?;
        imported.gameobj = true;
    }
    if scripts.is_dir() {
        copy_scripts(&scripts, &hydra.join("scripts"), &mut imported)?;
    }
    Ok(imported)
}

/// Copy the script tables' rows from Lich's database into Hydra's.
fn settings(from: &Path, into: &Path) -> rusqlite::Result<Vec<(&'static str, usize)>> {
    let lich = Connection::open_with_flags(from, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut hydra = Connection::open(into)?;
    // A runner may hold the file: wait for its writes as Lich's own
    // connections do (`lich.rb`, `DEFAULT_SQLITE_BUSY_TIMEOUT_MS`).
    hydra.busy_timeout(std::time::Duration::from_secs(5))?;
    let transaction = hydra.transaction()?;
    let mut copied = Vec::new();
    for (table, create) in TABLES {
        transaction.execute_batch(create)?;
        let exists: bool = lich.query_row(
            "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            continue;
        }
        let mut read = lich.prepare(&format!("SELECT * FROM {table}"))?;
        let columns = read.column_count();
        let marks = vec!["?"; columns].join(", ");
        let mut write =
            transaction.prepare(&format!("INSERT OR REPLACE INTO {table} VALUES ({marks})"))?;
        let mut rows = read.query([])?;
        let mut count = 0;
        while let Some(row) = rows.next()? {
            let values: Vec<rusqlite::types::Value> = (0..columns)
                .map(|i| row.get(i))
                .collect::<rusqlite::Result<_>>()?;
            write.execute(rusqlite::params_from_iter(values))?;
            count += 1;
        }
        copied.push((*table, count));
    }
    transaction.commit()?;
    Ok(copied)
}

/// Copy the scripts in `from`, and in its `custom` folders, into `into`,
/// keeping each one's place; never over a file Hydra has.
fn copy_scripts(from: &Path, into: &Path, imported: &mut Imported) -> Result<(), String> {
    let mut folders: Vec<PathBuf> = vec![PathBuf::new(), PathBuf::from("custom")];
    if let Ok(entries) = std::fs::read_dir(from.join("custom")) {
        folders.extend(
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .map(|entry| PathBuf::from("custom").join(entry.file_name())),
        );
    }
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(from.join(&folder)) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            // What `;name` would run, and no less: `foo.lic.gz` was left
            // behind (the review of 2026-09-29).
            let script = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| {
                    let lower = name.to_ascii_lowercase();
                    super::KINDS.iter().any(|kind| {
                        lower.len() > kind.len() + 1 && lower.ends_with(&format!(".{kind}"))
                    })
                });
            if !script || !path.is_file() {
                continue;
            }
            let target = into.join(&folder).join(entry.file_name());
            if target.exists() {
                imported.kept += 1;
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            std::fs::copy(&path, &target).map_err(|e| format!("{}: {e}", target.display()))?;
            imported.scripts += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cena-import-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A Lich folder: its database, as Lich makes it, with a script's
    /// settings, a user variable and a login's cache; two scripts, one in
    /// `custom`; its item types.
    fn lich_folder(root: &Path) -> rusqlite::Result<()> {
        std::fs::create_dir_all(root.join("data")).ok();
        std::fs::create_dir_all(root.join("scripts/custom")).ok();
        let db = Connection::open(root.join("data/lich.db3"))?;
        for (_, create) in TABLES {
            db.execute_batch(create)?;
        }
        db.execute_batch(
            "CREATE TABLE simu_game_entry (character TEXT NOT NULL, game_code TEXT NOT NULL, data BLOB, PRIMARY KEY(character, game_code));
             INSERT INTO simu_game_entry VALUES ('Nisugi', 'GS3', X'00');
             INSERT INTO script_auto_settings VALUES ('wander', 'GSIV:Nisugi', X'04087b00');
             INSERT INTO script_setting VALUES ('old', 'x', X'01');
             INSERT INTO uservars VALUES ('GSIV:Nisugi', X'04087b06');",
        )?;
        std::fs::write(root.join("scripts/wander.lic"), "echo 'lich'").ok();
        std::fs::write(root.join("scripts/notes.txt"), "not a script").ok();
        std::fs::write(root.join("scripts/packed.lic.gz"), "gz").ok();
        std::fs::write(root.join("scripts/custom/mine.rb"), "echo 'mine'").ok();
        std::fs::write(root.join("data/gameobj-data.xml"), "<data/>").ok();
        Ok(())
    }

    #[test]
    fn a_lich_folder_is_imported_its_login_cache_left_behind() {
        let root = scratch("whole");
        let (lich, hydra) = (root.join("Lich5"), root.join("hydra"));
        lich_folder(&lich).unwrap();
        // Hydra has its own wander already, and a setting of its own.
        std::fs::create_dir_all(hydra.join("scripts")).unwrap();
        std::fs::write(hydra.join("scripts/wander.lic"), "echo 'edited'").unwrap();
        std::fs::create_dir_all(hydra.join("lich")).unwrap();
        let mine = Connection::open(hydra.join("lich/lich.db3")).unwrap();
        mine.execute_batch(TABLES[1].1).unwrap();
        mine.execute(
            "INSERT INTO script_auto_settings VALUES ('wander', 'GSIV:Nisugi', X'ff')",
            [],
        )
        .unwrap();
        drop(mine);

        let imported = import(&lich, &hydra).unwrap();
        assert_eq!(
            imported.rows,
            [
                ("script_setting", 1),
                ("script_auto_settings", 1),
                ("uservars", 1)
            ]
        );
        assert_eq!((imported.scripts, imported.kept), (2, 1));
        assert!(imported.gameobj);
        assert_eq!(
            std::fs::read_to_string(hydra.join("scripts/wander.lic")).unwrap(),
            "echo 'edited'",
            "Hydra's own is kept"
        );
        assert!(hydra.join("scripts/custom/mine.rb").is_file());
        assert!(!hydra.join("scripts/notes.txt").exists());
        assert!(
            hydra.join("scripts/packed.lic.gz").is_file(),
            "a packed script runs, so it is brought"
        );

        let db = Connection::open(hydra.join("lich/lich.db3")).unwrap();
        let wander: Vec<u8> = db
            .query_row(
                "SELECT hash FROM script_auto_settings WHERE script = 'wander'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(wander, [0x04, 0x08, 0x7b, 0x00], "Lich's, as it was");
        let login: i64 = db
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'simu_game_entry'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(login, 0, "a login's cache is not a script's");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A real Lich folder, the player's own (`CENA_LICH_FOLDER`), into a
    /// folder of this test's that is gone again afterwards: its settings
    /// come across, and no login's cache with them.
    #[test]
    #[ignore = "Tier 2: needs CENA_LICH_FOLDER, a Lich install; run with --ignored"]
    fn a_players_own_lich_folder_is_imported() {
        let lich = PathBuf::from(
            std::env::var_os("CENA_LICH_FOLDER").expect("CENA_LICH_FOLDER names no folder"),
        );
        let hydra = scratch("players");
        let imported = import(&lich, &hydra).unwrap();
        eprintln!("{}", imported.said(&lich));
        let db = Connection::open(hydra.join("lich/lich.db3")).unwrap();
        let tables: Vec<String> = db
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        drop(db);
        let _ = std::fs::remove_dir_all(&hydra);
        assert_eq!(
            tables,
            ["script_auto_settings", "script_setting", "uservars"]
        );
        assert!(imported.rows.iter().map(|(_, n)| n).sum::<usize>() > 0);
    }

    #[test]
    fn a_folder_that_is_not_lichs_is_refused() {
        let root = scratch("not");
        std::fs::create_dir_all(&root).unwrap();
        let refused = import(&root, &root.join("hydra")).unwrap_err();
        assert!(refused.contains("is not a Lich folder"), "{refused}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
