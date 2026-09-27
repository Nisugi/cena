//! Scripts (`plan/46`, M7b): `;name args` runs the player's Lich script
//! `name` on Lich's own engine, in a Ruby runner of the character's own
//! (`cena_agent::scripts`, `bridges/ruby`).
//!
//! **Hydra's words come first** (§10, question 5, the author: *"yep"*): the
//! command line hears a script only after every family of Hydra's own, and
//! after the check that tells a word of a family still starting to wait, so
//! `;go2` typed during the login is never someone's `go2.lic`.
//!
//! **A runner starts on the character's first script** (§10, question 9):
//! the player's Ruby, found as a Lich player's is; one listener for every
//! character's runner, opened with the first. The player's own `;` words for
//! running scripts (`;k`, `;l`, `;p` and the rest, `inventory/13` §1.5) go
//! to the runner too, which answers them with Lich's own command table;
//! those that only act on running scripts are answered here while no runner
//! runs. **Leaving the table stops the runner**, and its scripts with it
//! (§8).
//!
//! Scripts are found in `scripts` in Hydra's data folder (and its `custom`
//! folders, as Lich's are), and a runner keeps its data in `lich` beside it.
//! `;scripts import <Lich folder>` brings a Lich player's scripts and their
//! settings there once ([`import`]); `;scripts check <script>` says which of
//! a script's lines will not work under Hydra, and why ([`check`]);
//! `;scripts` says where they are.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cena_agent::scripts::Runners;
use cena_agent::scripts::local::Atlas;
use cena_agent::scripts::runner::{self, Start};
use cena_session::command::claimant::Claimed;
use cena_session::script::Door;
use cena_session::{Notice, NoticeKind, SessionHandle, SessionId, SessionObserver};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::commands::Commands;

mod check;
mod import;

/// Lich's own `;` words over its scripts, which the runner answers
/// (`bridges/ruby/lich/lib/common/client_commands/builtins.rb`).
const LICH_WORDS: &[&str] = &[
    "k",
    "kill",
    "stop",
    "ka",
    "killall",
    "stopall",
    "kd",
    "p",
    "pause",
    "pa",
    "pauseall",
    "u",
    "unpause",
    "ua",
    "unpauseall",
    "l",
    "la",
    "list",
    "listall",
    "force",
    "send",
    "s",
    "e",
    "eq",
    "exec",
    "execq",
    "en",
    "execname",
];

/// Of [`LICH_WORDS`], the ones that start a script, so start the runner.
const STARTING: &[&str] = &["force", "e", "eq", "exec", "execq", "en", "execname"];

/// The script files a runner runs: Lich's own kinds, less the Wizard's
/// `.cmd` and `.wiz`, which the bridge does not take (`plan/46` §5).
const KINDS: &[&str] = &["lic", "rb", "lic.gz", "rb.gz"];

/// What is said when the player has no Ruby.
const NO_RUBY: &str = "scripts need Ruby 4.0, which Lich's installer puts in C:\\Ruby4Lich5; none was found there or on the PATH.";

/// How many typed commands may wait for a runner to start.
const WAITING: usize = 32;

/// Every character's scripts: their runners, and the one listener.
pub(crate) struct Scripts {
    shared: Arc<Shared>,
    desks: Mutex<BTreeMap<SessionId, Desk>>,
}

/// One character's desk: how to stop it, and its task, over when its runner
/// is.
struct Desk {
    stop: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

struct Shared {
    /// Hydra's data folder.
    dir: PathBuf,
    runners: Runners,
    /// The listener's address, once the first runner opened it.
    url: tokio::sync::OnceCell<Result<String, String>>,
    /// Where the runner's files were written, once.
    unpacked: OnceLock<Result<PathBuf, String>>,
    stop: CancellationToken,
}

/// One character, as its runner is started for it: its script door, which
/// is how the runner acts, and how the player is told.
struct Seat {
    character: String,
    game: String,
    door: Door,
    observer: SessionObserver,
}

/// A runner that is running.
struct Running {
    child: tokio::process::Child,
    token: String,
    /// The last lines it wrote to its standard error, for when it stops.
    last_words: Arc<Mutex<Vec<String>>>,
}

impl Scripts {
    /// Scripts over Hydra's data folder `dir`, their characters placed on
    /// `map` when there is one. Nothing starts until a character's first
    /// script.
    pub(crate) fn new(dir: &Path, map: &crate::map_context::ConfiguredMap) -> Self {
        let runners = match map {
            Ok(context) => Runners::with_atlas(Atlas {
                map: Arc::clone(&context.map),
                locate: cena_behavior::travel::room_of,
            }),
            Err(_) => Runners::default(),
        };
        Self {
            shared: Arc::new(Shared {
                dir: dir.to_owned(),
                runners,
                url: tokio::sync::OnceCell::new(),
                unpacked: OnceLock::new(),
                stop: CancellationToken::new(),
            }),
            desks: Mutex::default(),
        }
    }

    /// A character joined the table: its scripts and `;` words run from its
    /// command line from now on.
    pub(crate) fn open(
        &self,
        id: SessionId,
        character: &str,
        game: &str,
        handle: &SessionHandle,
        observer: &SessionObserver,
        commands: &Commands,
    ) {
        let (typed, waiting) = mpsc::channel(WAITING);
        let (shared, told) = (Arc::clone(&self.shared), handle.clone());
        let scripts = shared.dir.join("scripts");
        commands.scripts(Arc::new(move |line: &str| {
            let word = first_word(line);
            if word == "scripts" {
                answer(line, &shared, &told);
                return Some(Claimed::Done);
            }
            if !LICH_WORDS.contains(&word.as_str()) && find_script(&scripts, &word).is_none() {
                return None;
            }
            let said = match typed.try_send(line.trim().to_owned()) {
                Ok(()) => None,
                Err(mpsc::error::TrySendError::Full(_)) => Some(
                    "Scripts: too many commands already wait for the script runner; not taken.",
                ),
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    Some("Scripts have stopped for this character.")
                }
            };
            if let Some(said) = said {
                told.say(Notice::line(NoticeKind::Warn, said));
            }
            Some(Claimed::Done)
        }));
        let stop = CancellationToken::new();
        let seat = Seat {
            character: character.to_owned(),
            game: game.to_owned(),
            door: handle.script_door(),
            observer: observer.clone(),
        };
        let task = tokio::spawn(desk(Arc::clone(&self.shared), seat, waiting, stop.clone()));
        if let Some(old) = self.lock().insert(id, Desk { stop, task }) {
            old.stop.cancel();
        }
    }

    /// A character left the table: its runner stops, and its scripts, by the
    /// time this returns.
    pub(crate) async fn close(&self, id: SessionId) {
        let desk = self.lock().remove(&id);
        if let Some(desk) = desk {
            desk.stop.cancel();
            let _ = desk.task.await;
        }
    }

    /// Hydra is stopping: every runner, then the listener.
    pub(crate) async fn shutdown(&self) {
        let desks = std::mem::take(&mut *self.lock());
        for desk in desks.values() {
            desk.stop.cancel();
        }
        for (_, desk) in desks {
            let _ = desk.task.await;
        }
        self.shared.stop.cancel();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<SessionId, Desk>> {
        self.desks.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// `;scripts`: where they are, `check <script>`, and `import <Lich
/// folder>`, the last two run off the command line and said when done.
fn answer(line: &str, shared: &Arc<Shared>, told: &SessionHandle) {
    let hydra = shared.dir.as_path();
    let rest = line
        .trim()
        .split_once(char::is_whitespace)
        .map_or("", |(_, rest)| rest.trim());
    if let Some(script) = rest
        .strip_prefix("check")
        .filter(|script| script.is_empty() || script.starts_with(char::is_whitespace))
    {
        check::check(script, shared, told);
        return;
    }
    let Some(folder) = rest
        .strip_prefix("import")
        .filter(|folder| folder.is_empty() || folder.starts_with(char::is_whitespace))
    else {
        told.say(Notice::table(
            NoticeKind::Info,
            vec![
                format!(
                    "Your Lich scripts run from {}, and their settings are kept in {}.",
                    hydra.join("scripts").display(),
                    hydra.join("lich").join("lich.db3").display()
                ),
                "scripts import <Lich folder>   bring your Lich scripts and their settings here: C:\\Lich5"
                    .to_owned(),
                "scripts check <script>   which of its lines will not work under Hydra, and why"
                    .to_owned(),
                "<script> [args]   run one; k, l, p, u as in Lich".to_owned(),
            ],
        ));
        return;
    };
    let folder = folder.trim().trim_matches('"');
    if folder.is_empty() {
        told.say(Notice::line(
            NoticeKind::Error,
            "Scripts: import from where? Name your Lich folder, the one with data and scripts in it.",
        ));
        return;
    }
    let (from, into, told) = (PathBuf::from(folder), hydra.to_owned(), told.clone());
    tokio::task::spawn_blocking(move || {
        let said = match import::import(&from, &into) {
            Ok(imported) => Notice::line(NoticeKind::Info, imported.said(&from)),
            Err(why) => Notice::line(NoticeKind::Error, format!("Scripts: {why}")),
        };
        told.say(said);
    });
}

/// The first word of a typed line, lowercased: the script or the command.
fn first_word(line: &str) -> String {
    line.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The script `word` names in `dir`, as Lich finds one
/// (`common/script.rb`, `__find_script_file`): in `custom` and its folders,
/// then `dir` itself, each in order of name; the whole name first, then the
/// first that starts with it. `word` is lowercase.
fn find_script(dir: &Path, word: &str) -> Option<PathBuf> {
    if word.is_empty() || word.contains(['/', '\\', '.']) {
        return None;
    }
    let custom = dir.join("custom");
    let mut folders = vec![custom.clone()];
    if let Ok(entries) = std::fs::read_dir(&custom) {
        let mut inside: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        inside.sort();
        folders.extend(inside);
    }
    folders.push(dir.to_owned());
    let scripts: Vec<(String, PathBuf)> = folders
        .iter()
        .flat_map(|folder| {
            let mut found: Vec<(String, PathBuf)> = std::fs::read_dir(folder)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                    let stem = KINDS.iter().find_map(|kind| {
                        name.strip_suffix(kind)
                            .and_then(|stem| stem.strip_suffix('.'))
                    })?;
                    Some((stem.to_owned(), entry.path()))
                })
                .collect();
            found.sort();
            found
        })
        .collect();
    let whole = scripts.iter().find(|(stem, _)| stem == word);
    whole
        .or_else(|| scripts.iter().find(|(stem, _)| stem.starts_with(word)))
        .map(|(_, path)| path.clone())
}

/// Where the runner's files are, written the first time they are asked
/// for.
fn unpacked(shared: &Shared) -> Result<PathBuf, String> {
    shared
        .unpacked
        .get_or_init(|| {
            let dir = shared.dir.join("runner").join("ruby");
            runner::unpack(&dir)
                .map(|()| dir)
                .map_err(|e| format!("the runner's files were not written: {e}"))
        })
        .clone()
}

/// One character's runner: started on the first command that needs it,
/// given each typed command in order, told of when it stops by itself, and
/// stopped when the character leaves the table.
async fn desk(
    shared: Arc<Shared>,
    seat: Seat,
    mut waiting: mpsc::Receiver<String>,
    stop: CancellationToken,
) {
    let mut running: Option<Running> = None;
    loop {
        tokio::select! {
            () = stop.cancelled() => break,
            line = waiting.recv() => {
                let Some(line) = line else { break };
                if running.is_none() {
                    let word = first_word(&line);
                    if LICH_WORDS.contains(&word.as_str()) && !STARTING.contains(&word.as_str()) {
                        seat.door.say(Notice::line(NoticeKind::Info, "No scripts are running."));
                        continue;
                    }
                    match start(&shared, &seat).await {
                        Ok(started) => running = Some(started),
                        Err(why) => {
                            seat.door.say(Notice::line(NoticeKind::Error, format!("Scripts: {why}")));
                            continue;
                        }
                    }
                }
                if let Some(runner) = &running
                    && !shared.runners.typed(&runner.token, &line)
                {
                    seat.door.say(Notice::line(NoticeKind::Error, "Scripts: the runner is gone."));
                }
            }
            status = ended(&mut running) => {
                if let Some(runner) = running.take() {
                    shared.runners.dismiss(&runner.token);
                    let last = runner.last_words.lock().map(|w| w.join(" | ")).unwrap_or_default();
                    seat.door.say(Notice::line(
                        NoticeKind::Warn,
                        format!("Scripts: the script runner stopped ({status}), and its scripts with it. {last}"),
                    ));
                }
            }
        }
    }
    if let Some(mut runner) = running {
        let _ = runner.child.kill().await;
        shared.runners.dismiss(&runner.token);
    }
}

/// When the running runner exits, how; never while none runs.
async fn ended(running: &mut Option<Running>) -> String {
    match running {
        Some(runner) => match runner.child.wait().await {
            Ok(status) => status.to_string(),
            Err(e) => e.to_string(),
        },
        None => std::future::pending().await,
    }
}

/// Start a runner for `seat`: the listener and the runner's files the first
/// time, then the player's Ruby.
async fn start(shared: &Shared, seat: &Seat) -> Result<Running, String> {
    let ruby = runner::find_ruby().ok_or(NO_RUBY)?;
    let url = shared
        .url
        .get_or_init(|| listen(shared.runners.clone(), shared.stop.clone()))
        .await
        .clone()?;
    let dir = unpacked(shared)?;
    let (scripts, data) = (shared.dir.join("scripts"), shared.dir.join("lich"));
    for folder in [&scripts, &data] {
        std::fs::create_dir_all(folder)
            .map_err(|e| format!("{} could not be made: {e}", folder.display()))?;
    }
    let token = shared
        .runners
        .admit(&seat.character, seat.door.clone(), &seat.observer)
        .await?;
    let started = runner::start(&Start {
        ruby: &ruby,
        dir: &dir,
        url: &url,
        token: &token,
        character: &seat.character,
        game: &seat.game,
        scripts: &scripts,
        data: &data,
        symbol: seat.door.symbol(),
    });
    let mut child = match started {
        Ok(child) => child,
        Err(e) => {
            shared.runners.dismiss(&token);
            return Err(format!("Ruby ({}) did not start: {e}", ruby.display()));
        }
    };
    let last_words = Arc::new(Mutex::new(Vec::new()));
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(keep_words(
            stderr,
            format!("[{}]", seat.character),
            Arc::clone(&last_words),
        ));
    }
    Ok(Running {
        child,
        token,
        last_words,
    })
}

/// Open the listener every character's runner reaches Hydra through: on
/// loopback, a port the system chooses.
async fn listen(runners: Runners, stop: CancellationToken) -> Result<String, String> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|e| format!("the scripts' listener did not open: {e}"))?;
    let address = listener
        .local_addr()
        .map_err(|e| format!("the scripts' listener has no address: {e}"))?;
    tokio::spawn(async move {
        if let Err(e) = cena_agent::scripts::serve(listener, runners, stop).await {
            eprintln!("[scripts] the listener stopped -- {e}");
        }
    });
    Ok(format!("http://{address}/mcp"))
}

/// A runner's standard error, to the terminal as it comes, and its last
/// few lines kept for when it stops.
async fn keep_words(
    stderr: tokio::process::ChildStderr,
    who: String,
    last_words: Arc<Mutex<Vec<String>>>,
) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        eprintln!("{who} [ruby] {line}");
        if let Ok(mut words) = last_words.lock() {
            words.push(line);
            let excess = words.len().saturating_sub(3);
            words.drain(..excess);
        }
    }
}

#[cfg(test)]
mod command_tests;
