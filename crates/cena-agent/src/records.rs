//! `records`: a read-only question put to a character's database, the combat
//! recorder and the loot ledger in one file (`plan/35` §6, "Records").
//!
//! The author, 2026-09-24: *"I guess you could ask it to give you stats on
//! database info that we haven't built reports for yet to see if a report is
//! worth it."* So it takes SQL, not a vocabulary: the point is the question
//! nobody anticipated.
//!
//! **Read-only by construction**, not by a check of ours: its own connection,
//! opened read-only, with `PRAGMA query_only`, so a write fails in `SQLite`.
//! **One statement**: `rusqlite` refuses a string with more. **Bounded**: at
//! most [`MAX_ROWS`] rows, and interrupted after [`TIME_LIMIT`]. **This file
//! only**: `ATTACH` is refused by `SQLite`'s own limit, set to zero, so a
//! question cannot open another database on the machine (issue #19, point 7).
//!
//! **An answer says what it stands on**, so an empty answer is not mistaken
//! for missing data: the schema's version, when the file was last written,
//! and whether this run records into it at all (the caller's to fill in).

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

/// The most rows one question returns.
pub const MAX_ROWS: usize = 500;
/// How long one question may run.
pub const TIME_LIMIT: Duration = Duration::from_secs(5);

/// What a question answered.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Answer {
    /// The columns, in order.
    pub columns: Vec<String>,
    /// The rows, each in column order.
    pub rows: Vec<Vec<serde_json::Value>>,
    /// More rows matched than [`MAX_ROWS`].
    pub truncated: bool,
    /// The database's `user_version`: which schema the rows are in.
    pub schema_version: i64,
    /// When the database (or its write-ahead log) was last written, in Unix
    /// milliseconds: what the rows are current to.
    pub written_unix_ms: Option<u64>,
    /// Whether this run records combat and loot into the database; `null`
    /// when not known. Off, an empty answer may only mean nothing was kept.
    pub recording: Option<bool>,
}

/// One table or view, as `SQLite` defines it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Table {
    /// Its name.
    pub name: String,
    /// Its `CREATE` statement.
    pub sql: String,
}

/// Ask `sql` of the database at `path`. Blocking: run it off the runtime.
///
/// # Errors
///
/// No such database, a question `SQLite` refuses (a write among them), more
/// than one statement, or [`TIME_LIMIT`] passed.
pub fn ask(path: &Path, sql: &str) -> Result<Answer, String> {
    let connection = open(path)?;
    let (done, finished) = mpsc::channel::<()>();
    let interrupt = connection.get_interrupt_handle();
    let timer = std::thread::spawn(move || {
        if finished.recv_timeout(TIME_LIMIT) == Err(mpsc::RecvTimeoutError::Timeout) {
            interrupt.interrupt();
        }
    });
    let answer = run(&connection, sql);
    drop(done);
    let _ = timer.join();
    let mut answer = answer?;
    answer.schema_version = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    answer.written_unix_ms = written(path);
    Ok(answer)
}

/// The latest write to the database or its write-ahead log.
fn written(path: &Path) -> Option<u64> {
    let wal = path.with_extension("db-wal");
    [path, wal.as_path()]
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok()?.modified().ok())
        .max()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_millis()).ok())
}

/// The database's tables and views, for `capabilities`.
///
/// # Errors
///
/// No such database, or it cannot be read.
pub fn schema(path: &Path) -> Result<Vec<Table>, String> {
    let connection = open(path)?;
    let mut statement = connection
        .prepare(
            "SELECT name, sql FROM sqlite_master WHERE type IN ('table', 'view') AND sql IS NOT NULL ORDER BY name",
        )
        .map_err(|e| e.to_string())?;
    let tables = statement
        .query_map([], |row| {
            Ok(Table {
                name: row.get(0)?,
                sql: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(tables)
}

fn open(path: &Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err(format!(
            "there is no database at {} yet: it is made when combat or loot is first recorded",
            path.display()
        ));
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|e| e.to_string())?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)
        .map_err(|e| e.to_string())?;
    Ok(connection)
}

fn run(connection: &Connection, sql: &str) -> Result<Answer, String> {
    let mut statement = connection.prepare(sql).map_err(|e| e.to_string())?;
    let columns: Vec<String> = statement
        .column_names()
        .iter()
        .map(|&c| c.to_owned())
        .collect();
    let mut rows = statement.query([]).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut truncated = false;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if out.len() == MAX_ROWS {
            truncated = true;
            break;
        }
        let mut values = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            values.push(value(row.get_ref(i).map_err(|e| e.to_string())?));
        }
        out.push(values);
    }
    Ok(Answer {
        columns,
        rows: out,
        truncated,
        schema_version: 0,
        written_unix_ms: None,
        recording: None,
    })
}

fn value(value: ValueRef<'_>) -> serde_json::Value {
    match value {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(n) => n.into(),
        ValueRef::Real(f) => {
            serde_json::Number::from_f64(f).map_or(serde_json::Value::Null, Into::into)
        }
        ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned().into(),
        ValueRef::Blob(bytes) => format!("<{} bytes>", bytes.len()).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_ROWS, ask, schema};

    /// A database with one table of `rows` rows, in a folder of its own,
    /// named by the process and the calling test.
    fn database(test: &str, rows: u32) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cena-agent-records-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x_combat.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE kills (id INTEGER PRIMARY KEY, creature TEXT);")
            .unwrap();
        for i in 0..rows {
            connection
                .execute(
                    "INSERT INTO kills (creature) VALUES (?1)",
                    [format!("troll {i}")],
                )
                .unwrap();
        }
        path
    }

    #[test]
    fn a_question_is_answered_and_capped() {
        let path = database("capped", 600);
        let answer = ask(&path, "SELECT id, creature FROM kills ORDER BY id").unwrap();
        assert_eq!(answer.columns, ["id", "creature"]);
        assert_eq!(answer.rows.len(), MAX_ROWS);
        assert!(answer.truncated);
        assert_eq!(answer.rows[0][1], "troll 0");
        let count = ask(&path, "SELECT count(*) AS n FROM kills").unwrap();
        assert_eq!(count.rows, [[serde_json::json!(600)]]);
        assert!(!count.truncated);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A write fails in `SQLite`, and two statements are refused.
    #[test]
    fn nothing_can_be_written() {
        let path = database("unwritable", 1);
        assert!(ask(&path, "DELETE FROM kills").is_err());
        assert!(ask(&path, "INSERT INTO kills (creature) VALUES ('x')").is_err());
        assert!(ask(&path, "SELECT 1; DELETE FROM kills").is_err());
        // A real database beside it: a read-only connection could open it,
        // so only the limit stops the ATTACH.
        let elsewhere = path.with_file_name("other.db");
        rusqlite::Connection::open(&elsewhere)
            .unwrap()
            .execute_batch("CREATE TABLE secrets (x TEXT);")
            .unwrap();
        let attach = format!("ATTACH DATABASE '{}' AS other", elsewhere.display());
        let refused = ask(&path, &attach).unwrap_err();
        assert!(refused.contains("too many attached"), "{refused}");
        assert_eq!(
            ask(&path, "SELECT count(*) FROM kills").unwrap().rows,
            [[serde_json::json!(1)]],
            "still there"
        );
        let tables = schema(&path).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "kills");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn no_database_is_said() {
        let missing = std::env::temp_dir().join("cena-agent-records-none/none.db");
        let why = ask(&missing, "SELECT 1").unwrap_err();
        assert!(why.contains("no database"), "{why}");
    }
}
