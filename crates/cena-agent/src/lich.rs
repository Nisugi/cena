//! The Lich relay (`plan/51`): the player's own Lich, for one character,
//! with Hydra keeping the game's connection.
//!
//! [`run`] starts Lich in pipe mode, pointed at a port it holds:
//!
//! ```text
//! ruby lich.rbw --pipe --stormfront -g 127.0.0.1:<port>
//! ```
//!
//! Lich reads a key and a version line from its standard input, as a
//! frontend sends them, and writes both to that port before anything else
//! (`reference/lich-5/lib/main/main.rb:837-843`). The key is a new token,
//! and only a connection that says it first is taken: until Lich connects,
//! anything on this machine could reach the port, and what comes through it
//! goes to the game. It is not the game's key, which Hydra has already
//! given. **Lich never holds a credential.**
//!
//! From then on Lich takes the port as its game:
//!
//! - the game's bytes, as they came, go to it ([`LichDoor::wire`]);
//! - what it writes back goes to the game ([`LichDoor::send`]). A line with
//!   Lich's `<c>` is one of its scripts' (`$cmd_prefix`,
//!   `reference/lich-5/lib/main/main.rb:57`); one without is what the player
//!   typed, passed on after Lich's own hooks (`plan/51` §4, item 3);
//! - the player's typing goes to its standard input;
//! - what it would show a frontend comes out of its standard output, which
//!   is to be the character's text (`plan/51` §7, step 3). Until then it is
//!   read and let go, so Lich never waits to write it.
//!
//! Closing its standard input stops it
//! (`reference/lich-5/lib/common/pipe_io.rb`).

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use cena_session::script::Sending;
use cena_session::script::lich::{LichDoor, LineFrom, WIRE_CHUNKS, Wire};
use cena_session::{Notice, NoticeKind};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::process::{Child, ChildStderr, ChildStdin, Command};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// How long Lich has to connect once started. The spike measured 3.3 to
/// 4.8 s without its windows (`plan/51` §3); with them, and on a slower
/// machine, it takes longer. Lich allows itself 30 s to reach a game.
pub const CONNECT_DEADLINE: Duration = Duration::from_mins(1);

/// How long a connection to the port has to say the key before it is let
/// go. Lich says it as it connects.
const KEY_DEADLINE: Duration = Duration::from_secs(5);

/// How long Lich has to exit once its standard input closes before it is
/// killed. The spike measured 0.65 to 0.71 s.
const STOP_DEADLINE: Duration = Duration::from_secs(5);

/// How many of Lich's last lines of standard error are kept to say why it
/// ended.
const ERRORS_KEPT: usize = 20;

/// The version line a frontend sends after the key. Lich passes it on, and
/// the relay reads it and lets it go: the game has had Hydra's.
const VERSION: &str = "/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML";

/// Pipe mode tells Lich its frontend is `unknown`
/// (`reference/lich-5/lib/main/main.rb:569`), so it would hand a script's
/// text to Hydra's parser as raw markup (`plan/51` §4, item 1). Hydra is
/// `stormfront` (the author, §6 question 2): this puts it back whenever
/// Lich sets `unknown`. A Lich that keeps `--stormfront` in pipe mode
/// (`spike/lich-relay/lich-pipe-frontend.patch`) never sets it, and this
/// does nothing.
const STORMFRONT: &str =
    r#"trace_var(:$frontend) { |name| $frontend = "stormfront" if name == "unknown" }"#;

/// Runs `lich.rbw` as Ruby would run it, after [`STORMFRONT`]: `$0` names
/// it, as Lich finds its folders by it (`reference/lich-5/lib/constants.rb:1`).
const LOAD: &str = "$0 = ARGV.shift; load $0";

/// The player's Lich, and how to start it.
#[derive(Clone, Debug)]
pub struct Launch {
    /// Ruby: [`crate::scripts::runner::find_ruby`].
    pub ruby: PathBuf,
    /// The player's `lich.rbw`. Lich runs in its folder.
    pub lich: PathBuf,
    /// More of Lich's own flags, after Hydra's: a test's `--home`.
    pub args: Vec<OsString>,
    /// Lich's environment beyond Hydra's: a test keeps Lich off the network.
    pub env: Vec<(OsString, OsString)>,
}

/// How a relay ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ended {
    /// Stopped as asked: its standard input closed, and it exited.
    Stopped,
    /// Lich exited on its own.
    Exited {
        /// How, in words.
        status: String,
        /// Its last lines of standard error.
        errors: Vec<String>,
    },
    /// Lich did not connect within [`CONNECT_DEADLINE`], and was stopped.
    NeverConnected {
        /// Its last lines of standard error.
        errors: Vec<String>,
    },
    /// Lich had [`WIRE_CHUNKS`] of the game's chunks unread, and was stopped.
    FellBehind,
    /// The character's session ended, and Lich was stopped.
    SessionGone,
    /// Another Lich already runs for this character.
    Busy,
    /// Lich could not be started: why.
    Failed(String),
}

impl Ended {
    /// What the player is told, if anything: a session that is gone has
    /// nobody to tell.
    #[must_use]
    pub fn notice(&self) -> Option<Notice> {
        let (kind, first, errors) = match self {
            Self::Stopped => (NoticeKind::Info, "Lich stopped.".to_owned(), &[][..]),
            Self::Exited { status, errors } => (
                NoticeKind::Warn,
                format!("Lich exited ({status})."),
                &errors[..],
            ),
            Self::NeverConnected { errors } => (
                NoticeKind::Error,
                format!(
                    "Lich did not connect within {} seconds, and was stopped.",
                    CONNECT_DEADLINE.as_secs()
                ),
                &errors[..],
            ),
            Self::FellBehind => (
                NoticeKind::Error,
                format!(
                    "Lich fell {WIRE_CHUNKS} chunks behind the game, and was stopped: \
                     a stream with a hole in it would mislead its scripts."
                ),
                &[][..],
            ),
            Self::SessionGone => return None,
            Self::Busy => (
                NoticeKind::Warn,
                "Lich already runs for this character.".to_owned(),
                &[][..],
            ),
            Self::Failed(why) => (
                NoticeKind::Error,
                format!("Lich could not start: {why}"),
                &[][..],
            ),
        };
        let mut lines = vec![first];
        lines.extend(errors.iter().cloned());
        Some(Notice::table(kind, lines))
    }
}

/// Run the player's Lich for the character `door` reaches, until `stop`, or
/// until Lich or the session ends; then tell the player how it ended, and
/// say so.
///
/// The game's bytes are copied for Lich from the moment this is called, not
/// when the future is first polled, so a Lich started before the game speaks
/// misses nothing of the login. `typing` is what the player types for Lich.
pub fn run(
    door: LichDoor,
    launch: Launch,
    mut typing: mpsc::Receiver<String>,
    stop: CancellationToken,
) -> impl Future<Output = Ended> + Send {
    let wire = door.wire();
    async move {
        let ended = match wire {
            Some(wire) => relay(&door, &launch, wire, &mut typing, &stop).await,
            None => Ended::Busy,
        };
        if let Some(notice) = ended.notice() {
            door.say(notice);
        }
        ended
    }
}

async fn relay(
    door: &LichDoor,
    launch: &Launch,
    mut wire: Wire,
    typing: &mut mpsc::Receiver<String>,
    stop: &CancellationToken,
) -> Ended {
    let key = match crate::new_token() {
        Ok(key) => key,
        Err(why) => return Ended::Failed(why),
    };
    let listener = match TcpListener::bind(("127.0.0.1", 0)).await {
        Ok(listener) => listener,
        Err(why) => return Ended::Failed(why.to_string()),
    };
    let port = match listener.local_addr() {
        Ok(address) => address.port(),
        Err(why) => return Ended::Failed(why.to_string()),
    };
    let mut child = match spawn(launch, port) {
        Ok(child) => child,
        Err(why) => return Ended::Failed(format!("{}: {why}", launch.ruby.display())),
    };
    let (Some(mut stdin), Some(mut stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Ended::Failed("its standard streams were not piped".to_owned());
    };
    tokio::spawn(async move { tokio::io::copy(&mut stdout, &mut tokio::io::sink()).await });
    let errors = keep_errors(stderr);
    if let Err(why) = stdin
        .write_all(format!("{key}\n{VERSION}\n").as_bytes())
        .await
    {
        finish(child, stdin).await;
        return Ended::Failed(why.to_string());
    }

    let taken = tokio::select! {
        taken = accept_keyed(&listener, &key) => taken,
        () = tokio::time::sleep(CONNECT_DEADLINE) => {
            finish(child, stdin).await;
            return Ended::NeverConnected { errors: tail(&errors) };
        }
        status = child.wait() => return exited(status, &errors),
        () = stop.cancelled() => {
            finish(child, stdin).await;
            return Ended::Stopped;
        }
    };
    let (reader, mut writer) = match taken {
        Ok(taken) => taken,
        Err(why) => {
            finish(child, stdin).await;
            return Ended::Failed(why.to_string());
        }
    };
    drop(listener);
    let mut lines = read_lines(reader);
    let (mut lines_open, mut typing_open) = (true, true);
    loop {
        tokio::select! {
            chunk = wire.next() => {
                let Some(chunk) = chunk else {
                    let ended = if wire.fell_behind() { Ended::FellBehind } else { Ended::SessionGone };
                    finish(child, stdin).await;
                    return ended;
                };
                // A Lich that hung up is going: its exit says why.
                let _ = writer.write_all(&chunk).await;
            }
            line = lines.recv(), if lines_open => match line {
                Some(line) => pass_on(door, &line).await,
                None => lines_open = false,
            },
            typed = typing.recv(), if typing_open => match typed {
                Some(line) => {
                    let _ = stdin.write_all(format!("{line}\n").as_bytes()).await;
                }
                None => typing_open = false,
            },
            status = child.wait() => return exited(status, &errors),
            () = stop.cancelled() => {
                finish(child, stdin).await;
                return Ended::Stopped;
            }
        }
    }
}

fn spawn(launch: &Launch, port: u16) -> std::io::Result<Child> {
    let mut command = Command::new(&launch.ruby);
    command
        .arg("-e")
        .arg(STORMFRONT)
        .arg("-e")
        .arg(LOAD)
        .arg(&launch.lich)
        .args(["--pipe", "--stormfront", "-g"])
        .arg(format!("127.0.0.1:{port}"))
        .args(&launch.args)
        .envs(launch.env.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(folder) = launch.lich.parent().filter(|f| !f.as_os_str().is_empty()) {
        command.current_dir(folder);
    }
    command.spawn()
}

/// Takes the first connection to `listener` that says `key` on its first
/// line, and reads its second, the version line, and lets it go. Any other
/// is dropped.
async fn accept_keyed(
    listener: &TcpListener,
    key: &str,
) -> std::io::Result<(BufReader<OwnedReadHalf>, OwnedWriteHalf)> {
    loop {
        let (stream, _) = listener.accept().await?;
        let (read, write) = stream.into_split();
        let mut reader = BufReader::new(read);
        if let Ok(Ok(true)) = tokio::time::timeout(KEY_DEADLINE, said_key(&mut reader, key)).await {
            return Ok((reader, write));
        }
    }
}

async fn said_key(reader: &mut BufReader<OwnedReadHalf>, key: &str) -> std::io::Result<bool> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.trim_end() != key {
        return Ok(false);
    }
    line.clear();
    reader.read_line(&mut line).await?;
    Ok(true)
}

/// Each line Lich writes to its game, without its line ending.
fn read_lines(mut reader: BufReader<OwnedReadHalf>) -> mpsc::Receiver<String> {
    let (lines, receiver) = mpsc::channel(64);
    tokio::spawn(async move {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf);
                    if lines
                        .send(line.trim_end_matches(['\r', '\n']).to_owned())
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }
    });
    receiver
}

/// Send a line Lich wrote to its game, and tell the player when it did not
/// go.
async fn pass_on(door: &LichDoor, line: &str) {
    let (line, from) = line
        .strip_prefix("<c>")
        .map_or((line, LineFrom::Player), |line| (line, LineFrom::Lich));
    let why = match door.send(line, from).await {
        Sending::Sent { .. } | Sending::Ran => return,
        Sending::Unknown => "Hydra has no such command",
        Sending::Refused(refusal) => crate::scripts::tools::why(refusal),
        Sending::Lost => "not connected to the game",
    };
    door.say(Notice::line(
        NoticeKind::Warn,
        format!("Lich's line was not sent ({why}): {line}"),
    ));
}

/// Keeps Lich's last [`ERRORS_KEPT`] lines of standard error.
fn keep_errors(stderr: ChildStderr) -> Arc<Mutex<VecDeque<String>>> {
    let kept = Arc::new(Mutex::new(VecDeque::new()));
    let keeping = Arc::clone(&kept);
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut kept = keeping.lock().unwrap_or_else(PoisonError::into_inner);
            if kept.len() == ERRORS_KEPT {
                kept.pop_front();
            }
            kept.push_back(line);
        }
    });
    kept
}

fn tail(errors: &Mutex<VecDeque<String>>) -> Vec<String> {
    errors
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .cloned()
        .collect()
}

fn exited(status: std::io::Result<ExitStatus>, errors: &Mutex<VecDeque<String>>) -> Ended {
    let status = match status {
        Ok(status) => status.to_string(),
        Err(why) => why.to_string(),
    };
    Ended::Exited {
        status,
        errors: tail(errors),
    }
}

/// Stop Lich: close its standard input, and kill it if it has not exited
/// within [`STOP_DEADLINE`].
async fn finish(mut child: Child, stdin: ChildStdin) {
    drop(stdin);
    if tokio::time::timeout(STOP_DEADLINE, child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::accept_keyed;
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, TcpStream};

    /// Something on this machine that reaches the port first, without the
    /// key, is let go, and Lich, with it, is taken.
    #[tokio::test]
    async fn only_a_connection_that_says_the_key_is_taken() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let clients = tokio::spawn(async move {
            let mut stranger = TcpStream::connect(address).await.unwrap();
            stranger.write_all(b"guess\nlook\n").await.unwrap();
            let mut lich = TcpStream::connect(address).await.unwrap();
            lich.write_all(b"the-key\n/FE:WRAYTH\n<c>look\n")
                .await
                .unwrap();
            (stranger, lich)
        });
        let (mut reader, _writer) = accept_keyed(&listener, "the-key").await.unwrap();
        let mut first = String::new();
        tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut first)
            .await
            .unwrap();
        assert_eq!(
            first, "<c>look\n",
            "the key and version are read, and nothing after"
        );
        drop(clients.await.unwrap());
    }
}
