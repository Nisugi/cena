//! Explicitly enabled developer correction files; never game/map database writes.

use axum::{
    Json,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const SCHEMA: &str = "hydra-hunting-corrections-v1";
const LIMIT: usize = 5_000_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    area: String,
    hunt: String,
    name: String,
    map_sha256: String,
    base_rooms: Vec<u32>,
    creatures: Vec<String>,
    added: Vec<u32>,
    removed: Vec<u32>,
    notes: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    records: Vec<Record>,
}

impl Document {
    fn validate(&self) -> io::Result<()> {
        if self.schema != SCHEMA || self.records.len() > 2000 {
            return Err(invalid("Unsupported correction document"));
        }
        let mut keys = BTreeSet::new();
        for r in &self.records {
            if !safe_area(&r.area)
                || r.hunt.is_empty()
                || r.hunt.len() > 2000
                || r.name.is_empty()
                || r.name.len() > 2000
                || r.notes.len() > 16000
                || r.map_sha256.len() != 64
                || !r
                    .map_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || !keys.insert((&r.area, &r.hunt))
            {
                return Err(invalid("Invalid correction identity"));
            }
            for ids in [&r.base_rooms, &r.added, &r.removed] {
                if ids.len() > 40000 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
                    return Err(invalid("Invalid room IDs"));
                }
            }
            let base: BTreeSet<_> = r.base_rooms.iter().collect();
            if r.added.iter().any(|id| base.contains(id))
                || r.removed.iter().any(|id| !base.contains(id))
                || r.creatures.len() > 500
                || r.creatures.iter().any(|c| c.len() > 2000)
                || r.creatures.iter().collect::<BTreeSet<_>>().len() != r.creatures.len()
            {
                return Err(invalid("Invalid membership delta"));
            }
        }
        Ok(())
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn safe_area(area: &str) -> bool {
    !area.is_empty()
        && area.len() <= 180
        && area
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
        && area
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && !area.contains("..")
}
fn valid_date(date: &str) -> bool {
    let b = date.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return false;
    }
    let Ok(year) = date[..4].parse::<u32>() else {
        return false;
    };
    let Ok(month) = date[5..7].parse::<usize>() else {
        return false;
    };
    let Ok(day) = date[8..].parse::<u32>() else {
        return false;
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    (2000..=2100).contains(&year)
        && (1..=12).contains(&month)
        && day > 0
        && day
            <= [
                31,
                if leap { 29 } else { 28 },
                31,
                30,
                31,
                30,
                31,
                31,
                30,
                31,
                30,
                31,
            ][month - 1]
}
fn filename(area: &str, date: &str) -> String {
    format!("{area}-{date}.hunting.json")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Save {
    revision: u64,
    date: String,
    record: Record,
}

pub(crate) struct Files {
    directory: PathBuf,
    // OS lock is released even after a crash. The small lock file stays put.
    _lock: Option<File>,
    records: BTreeMap<(String, String), Record>,
    known: BTreeMap<String, Vec<u8>>,
    revision: u64,
}

impl Files {
    pub(crate) fn open(directory: &Path) -> io::Result<Self> {
        Self::read(directory, true)
    }
    fn read(directory: &Path, writable: bool) -> io::Result<Self> {
        if !directory.is_absolute() {
            return Err(invalid("Correction directory must be absolute"));
        }
        if writable {
            fs::create_dir_all(directory)?;
        }
        let directory = directory.canonicalize()?;
        let lock = if writable {
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(directory.join(".hunting-writer.lock"))?;
            lock.try_lock().map_err(|_| {
                io::Error::other("Correction folder is already open in another editor server")
            })?;
            Some(lock)
        } else {
            None
        };
        let mut files = Self {
            directory,
            _lock: lock,
            records: BTreeMap::new(),
            known: BTreeMap::new(),
            revision: 0,
        };
        let mut entries = Vec::new();
        let mut total = 0;
        for entry in fs::read_dir(&files.directory)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".hunting.json") {
                continue;
            }
            if entries.len() >= 2000 || !entry.file_type()?.is_file() {
                return Err(invalid(
                    "Too many correction files or non-regular correction file",
                ));
            }
            let size = entry.metadata()?.len();
            total += size;
            if total > 32_000_000 || size > LIMIT as u64 {
                return Err(invalid("Correction folder exceeds size limit"));
            }
            let bytes = fs::read(entry.path())?;
            let document: Document = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            document.validate()?;
            let Some(first) = document.records.first() else {
                return Err(invalid("Empty correction file"));
            };
            let prefix = format!("{}-", first.area);
            let date = name
                .strip_prefix(&prefix)
                .and_then(|v| v.strip_suffix(".hunting.json"))
                .ok_or_else(|| invalid("Correction filename does not match area"))?;
            if !valid_date(date) || document.records.iter().any(|r| r.area != first.area) {
                return Err(invalid("Invalid dated area file"));
            }
            entries.push((date.to_owned(), name, bytes, document));
        }
        entries.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        for (_, name, bytes, document) in entries {
            for r in document.records {
                files.records.insert((r.area.clone(), r.hunt.clone()), r);
            }
            files.known.insert(name, bytes);
        }
        files.document().validate()?;
        if serde_json::to_vec(&files.document())
            .map_err(io::Error::other)?
            .len()
            > LIMIT
        {
            return Err(invalid("Active correction dataset exceeds size limit"));
        }
        Ok(files)
    }
    fn document(&self) -> Document {
        Document {
            schema: SCHEMA.into(),
            records: self.records.values().cloned().collect(),
        }
    }
    fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"directory":self.directory,"revision":self.revision,"document":self.document()})
    }
    fn save(&mut self, save: Save) -> io::Result<serde_json::Value> {
        if save.revision != self.revision {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Another tab saved changes. Reload before editing further.",
            ));
        }
        if !valid_date(&save.date) {
            return Err(invalid("Invalid correction date"));
        }
        Document {
            schema: SCHEMA.into(),
            records: vec![save.record.clone()],
        }
        .validate()?;
        // A clock moving backwards must not write behind a newer dated snapshot.
        let prefix = format!("{}-", save.record.area);
        if self
            .known
            .keys()
            .filter_map(|name| name.strip_prefix(&prefix))
            .filter_map(|name| name.strip_suffix(".hunting.json"))
            .any(|date| date > save.date.as_str())
        {
            return Err(invalid(
                "A newer dated correction exists; check the system date",
            ));
        }
        let name = filename(&save.record.area, &save.date);
        let path = self.directory.join(&name);
        match (fs::symlink_metadata(&path), self.known.get(&name)) {
            (Ok(meta), Some(old)) if meta.is_file() && fs::read(&path)? == *old => {}
            (Err(e), None) if e.kind() == io::ErrorKind::NotFound => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Correction file changed outside this server. Reload the server before saving.",
                ));
            }
        }
        let mut next = self.records.clone();
        let area = save.record.area.clone();
        next.insert((area.clone(), save.record.hunt.clone()), save.record);
        let aggregate = Document {
            schema: SCHEMA.into(),
            records: next.values().cloned().collect(),
        };
        aggregate.validate()?;
        if serde_json::to_vec(&aggregate)
            .map_err(io::Error::other)?
            .len()
            > LIMIT
        {
            return Err(invalid("Active correction dataset exceeds size limit"));
        }
        let document = Document {
            schema: SCHEMA.into(),
            records: next.values().filter(|r| r.area == area).cloned().collect(),
        };
        let bytes = serde_json::to_vec_pretty(&document).map_err(io::Error::other)?;
        if bytes.len() > LIMIT {
            return Err(invalid("Area correction file exceeds size limit"));
        }
        let history_bytes: usize = self
            .known
            .iter()
            .filter(|(key, _)| *key != &name)
            .map(|(_, value)| value.len())
            .sum();
        if history_bytes + bytes.len() > 32_000_000
            || self.known.len() >= 2000 && !self.known.contains_key(&name)
        {
            return Err(invalid(
                "Correction history exceeds size limit; archive older files first",
            ));
        }
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(io::Error::other)?;
        let temp = self
            .directory
            .join(format!(".hunting-{:032x}.tmp", u128::from_le_bytes(nonce)));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        self.records = next;
        self.known.insert(name.clone(), bytes);
        self.revision += 1;
        Ok(serde_json::json!({"revision":self.revision,"file":name,"path":path}))
    }
}

pub(crate) type Store = Arc<Mutex<Files>>;
pub(crate) fn boundary_document(shared: &crate::server::Shared) -> io::Result<serde_json::Value> {
    let document = if let Some(store) = &shared.hunting_files {
        let files = store
            .lock()
            .map_err(|_| invalid("Correction store unavailable"))?;
        // Read the atomic files, including edits made since this server opened.
        Files::read(&files.directory, false)?.document()
    } else if let Some(directory) = &shared.hunting_corrections {
        Files::read(directory, false)?.document()
    } else {
        Document {
            schema: SCHEMA.into(),
            records: Vec::new(),
        }
    };
    serde_json::to_value(document).map_err(io::Error::other)
}
pub(crate) async fn boundaries(State(shared): State<Arc<crate::server::Shared>>) -> Response {
    match tokio::task::spawn_blocking(move || boundary_document(&shared)).await {
        Ok(Ok(document)) => Json(document).into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
pub(crate) fn limit() -> DefaultBodyLimit {
    DefaultBodyLimit::max(LIMIT)
}
pub(crate) async fn load(State(shared): State<Arc<crate::server::Shared>>) -> Response {
    let Some(store) = shared.hunting_files.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::task::spawn_blocking(move || {
        store
            .lock()
            .map(|s| s.snapshot())
            .map_err(|_| io::Error::other("Correction store unavailable"))
    })
    .await
    {
        Ok(Ok(value)) => Json(value).into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
pub(crate) async fn save(
    State(shared): State<Arc<crate::server::Shared>>,
    headers: HeaderMap,
    Json(save): Json<Save>,
) -> Response {
    let Some(store) = shared.hunting_files.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Unlike public asset GETs, writes always require the exact browser Origin.
    if !crate::server::authorized_headers(&headers, &shared.authority, &shared.origin, true) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match tokio::task::spawn_blocking(move || {
        store
            .lock()
            .map_err(|_| io::Error::other("Correction store unavailable"))?
            .save(save)
    })
    .await
    {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => {
            let status = match error.kind() {
                io::ErrorKind::AlreadyExists => StatusCode::CONFLICT,
                io::ErrorKind::InvalidData => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(serde_json::json!({"error":error.to_string()}))).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> io::Result<Self> {
            let mut bytes = [0_u8; 16];
            getrandom::fill(&mut bytes).map_err(io::Error::other)?;
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/hunting-file-tests")
                .join(format!("{:032x}", u128::from_le_bytes(bytes)));
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn record() -> Record {
        Record {
            area: "area-lysierian-hills".into(),
            hunt: "smokey".into(),
            name: "Smokey Cave".into(),
            map_sha256: "a".repeat(64),
            base_rooms: vec![1, 2],
            creatures: vec!["rat".into()],
            added: vec![3],
            removed: vec![1],
            notes: "review".into(),
        }
    }
    fn request(revision: u64) -> Save {
        Save {
            revision,
            date: "2026-09-24".into(),
            record: record(),
        }
    }

    #[test]
    fn creates_a_dated_area_file_only_on_edit_and_reloads_it() {
        let dir = Scratch::new().expect("scratch");
        let mut files = Files::open(&dir.0).expect("open");
        assert!(files.known.is_empty());
        let saved = files.save(request(0)).expect("save");
        assert_eq!(
            saved["file"],
            "area-lysierian-hills-2026-09-24.hunting.json"
        );
        assert_eq!(files.known.len(), 1);
        assert_eq!(files.revision, 1);
        let reader = Files::read(&dir.0, false).expect("concurrent read-only catalogue");
        assert_eq!(reader.document().records[0], record());
        let mut next = request(1);
        next.record.notes = "second edit".into();
        files.save(next).expect("update");
        assert_eq!(files.known.len(), 1);
        drop(files);
        let reloaded = Files::open(&dir.0).expect("reload");
        assert_eq!(reloaded.document().records[0].notes, "second edit");
    }
    #[test]
    fn preserves_other_hunts_and_days_and_rejects_stale_writers() {
        let dir = Scratch::new().expect("scratch");
        let mut files = Files::open(&dir.0).expect("open");
        assert!(Files::open(&dir.0).is_err(), "exclusive process lock");
        files.save(request(0)).expect("first");
        assert_eq!(
            files.save(request(0)).expect_err("stale").kind(),
            io::ErrorKind::AlreadyExists
        );
        let mut other = request(1);
        other.record.hunt = "blackened".into();
        files.save(other).expect("second hunt");
        let mut tomorrow = request(2);
        tomorrow.date = "2026-09-25".into();
        files.save(tomorrow).expect("next day");
        assert_eq!(files.known.len(), 2);
        assert_eq!(files.document().records.len(), 2);
        assert!(
            files.save(request(3)).is_err(),
            "do not write an older dated state"
        );
    }
    #[test]
    fn rejects_traversal_invalid_dates_and_external_file_changes() {
        let dir = Scratch::new().expect("scratch");
        let mut files = Files::open(&dir.0).expect("open");
        let mut bad = request(0);
        bad.record.area = "../../escape".into();
        assert!(files.save(bad).is_err());
        for date in [
            "2026-02-30",
            "2026-09-24/../evil",
            "2026-13-01",
            "2026-00-01",
            "2026-09-00",
        ] {
            let mut bad = request(0);
            bad.date = date.into();
            assert!(files.save(bad).is_err());
        }
        files.save(request(0)).expect("first");
        fs::write(
            dir.0.join("area-lysierian-hills-2026-09-24.hunting.json"),
            b"external edit",
        )
        .expect("fixture");
        assert_eq!(
            files.save(request(1)).expect_err("changed").kind(),
            io::ErrorKind::AlreadyExists
        );
        drop(files);
        assert!(
            Files::open(&dir.0).is_err(),
            "invalid files never silently disappear"
        );
    }
    #[test]
    fn rejects_extra_fields_and_invalid_deltas() {
        let mut r = record();
        r.added = vec![1];
        assert!(
            Document {
                schema: SCHEMA.into(),
                records: vec![r]
            }
            .validate()
            .is_err()
        );
        let mut json = serde_json::to_value(record()).expect("json");
        json["exits"] = serde_json::json!([]);
        assert!(serde_json::from_value::<Record>(json).is_err());
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2026-02-29"));
    }
}
