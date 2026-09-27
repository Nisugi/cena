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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use cena_agent::scripts::Runners;
use cena_agent::scripts::runner::{self, Start};
use cena_session::command::claimant::Claimed;
use cena_session::script::Door;
use cena_session::{Notice, NoticeKind, SessionHandle, SessionId, SessionObserver};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::commands::Commands;

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
    /// Scripts over Hydra's data folder `dir`. Nothing starts until a
    /// character's first script.
    pub(crate) fn new(dir: &Path) -> Self {
        Self {
            shared: Arc::new(Shared {
                dir: dir.to_owned(),
                runners: Runners::default(),
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
        let (scripts, told) = (self.shared.dir.join("scripts"), handle.clone());
        commands.scripts(Arc::new(move |line: &str| {
            let word = first_word(line);
            if !LICH_WORDS.contains(&word.as_str()) && !names_a_script(&scripts, &word) {
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

/// The first word of a typed line, lowercased: the script or the command.
fn first_word(line: &str) -> String {
    line.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Whether `word` names a script in `dir`, as Lich finds one
/// (`common/script.rb`, `__find_script_file`): in `custom`, its folders, or
/// `dir` itself; the whole name, or the start of one.
fn names_a_script(dir: &Path, word: &str) -> bool {
    if word.is_empty() || word.contains(['/', '\\', '.']) {
        return false;
    }
    let custom = dir.join("custom");
    let mut folders = vec![dir.to_owned(), custom.clone()];
    if let Ok(entries) = std::fs::read_dir(&custom) {
        folders.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir()),
        );
    }
    folders.iter().any(|folder| {
        std::fs::read_dir(folder).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                KINDS.iter().any(|kind| {
                    name.strip_suffix(kind)
                        .and_then(|stem| stem.strip_suffix('.'))
                        .is_some_and(|stem| stem.starts_with(word))
                })
            })
        })
    })
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
    let ruby = runner::find_ruby().ok_or(
        "scripts need Ruby 4.0, which Lich's installer puts in C:\\Ruby4Lich5; none was found there or on the PATH.",
    )?;
    let url = shared
        .url
        .get_or_init(|| listen(shared.runners.clone(), shared.stop.clone()))
        .await
        .clone()?;
    let dir = shared
        .unpacked
        .get_or_init(|| {
            let dir = shared.dir.join("runner").join("ruby");
            runner::unpack(&dir)
                .map(|()| dir)
                .map_err(|e| format!("the runner's files were not written: {e}"))
        })
        .clone()?;
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
mod tests {
    use super::*;

    fn scratch(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cena-scripts-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// Lich's order of finding: a whole name or its start, in the folder or
    /// its `custom` folders; never a Wizard script, and never a path.
    #[test]
    fn a_script_is_found_as_lich_finds_one() {
        let dir = scratch("found");
        std::fs::create_dir_all(dir.join("custom/mine")).unwrap();
        for file in [
            "trollspeak.lic",
            "custom/eloot.rb",
            "custom/mine/wander.lic",
            "old.cmd",
        ] {
            std::fs::write(dir.join(file), "").unwrap();
        }
        for (word, found) in [
            ("trollspeak", true),
            ("troll", true),
            ("eloot", true),
            ("wander", true),
            ("old", false),
            ("nosuch", false),
            ("../trollspeak", false),
            ("", false),
        ] {
            assert_eq!(names_a_script(&dir, word), found, "{word:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    const DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

    /// What the player was told, until a line containing `until`, or a
    /// minute.
    async fn told_until(
        events: &mut tokio::sync::broadcast::Receiver<cena_session::Event>,
        until: &str,
    ) -> Vec<String> {
        let mut told = Vec::new();
        let _ = tokio::time::timeout(std::time::Duration::from_mins(1), async {
            while let Ok(event) = events.recv().await {
                if let cena_session::Event::Notice(notice) = event {
                    told.extend(notice.lines().iter().cloned());
                    if told.iter().any(|line| line.contains(until)) {
                        return;
                    }
                }
            }
        })
        .await;
        told
    }

    /// Through the command line: Lich's words answer while no runner runs;
    /// a word of Hydra's is never a script's, even a family still starting;
    /// the player's script runs on a runner Hydra starts, hearing the game;
    /// and a character leaving the table stops its runner, which stops
    /// hearing.
    #[tokio::test(flavor = "current_thread")]
    async fn a_typed_script_runs_and_leaving_the_table_stops_it() {
        assert!(
            runner::find_ruby().is_some(),
            "no Ruby: scripts need Ruby 4.0"
        );
        let dir = scratch("typed");
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(
            dir.join("scripts/greet.lic"),
            "put 'look'\nwaitfor 'quiet room'\necho 'done'\nwaitfor 'never comes'\n",
        )
        .unwrap();
        std::fs::write(dir.join("scripts/go2.lic"), "echo 'a script'\n").unwrap();
        let (source, transcript) =
            cena_platform::AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
        for _ in 0..2 {
            transcript.answer(
                "look",
                b"You see a quiet room.\n<prompt time=\"2\">&gt;</prompt>\n",
            );
        }
        let session = cena_session::Session::new(source);
        let handle = session.handle();
        let observer = session.observer();
        let (_, mut events) = session.subscribe();
        let commands = Commands::install(&handle);
        tokio::spawn(session.into_actor().run());
        let scripts = Scripts::new(&dir);
        let id = SessionId(1);
        scripts.open(id, "Nisugi", "GS3", &handle, &observer, &commands);
        let generation = handle.generation();

        handle.send_manual_at(generation, ";l", DEADLINE).await;
        let told = told_until(&mut events, "No scripts").await;
        assert!(
            told.iter().any(|line| line == "No scripts are running."),
            "{told:#?}"
        );
        handle
            .send_manual_at(generation, ";go2 bank", DEADLINE)
            .await;
        let told = told_until(&mut events, "still starting").await;
        assert!(
            told.iter().any(|line| line.contains("still starting")),
            "Hydra's word, not the script's: {told:#?}"
        );

        handle.send_manual_at(generation, ";greet", DEADLINE).await;
        let told = told_until(&mut events, "[greet: done]").await;
        assert!(told.iter().any(|line| line == "[greet: done]"), "{told:#?}");
        assert!(told.iter().any(|line| line == "[greet]>look"), "{told:#?}");

        scripts.close(id).await;
        while events.try_recv().is_ok() {}
        handle.send_manual_at(generation, "look", DEADLINE).await;
        let heard = std::iter::from_fn(|| events.try_recv().ok())
            .filter(|event| matches!(event, cena_session::Event::Heard(_)))
            .count();
        assert_eq!(heard, 0, "the runner is gone, and nothing listens");
        assert_eq!(transcript.lines(), ["look", "look"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
