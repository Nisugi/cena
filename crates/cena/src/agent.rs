//! The agent, wired to this binary (`plan/35`): `--agent`'s MCP listener, and
//! `;agent`, the player's control of what an agent may do.
//!
//! **Two switches.** Without `--agent`, nothing listens. With it, what an
//! agent may do with each character is that character's level
//! (`cena_session::agent`), which is `off` until the player raises it with
//! `;agent level`. The level is kept in the character's settings file, so it
//! lasts from run to run (author, 2026-09-24: *"persistent"*), and `;agent`
//! works whether or not the listener runs: the level is the character's, and
//! the listener only the way in.
//!
//! **A fixed port and a kept token**, where Despana's page takes any port and
//! prints its link: an MCP client's configuration names the address and the
//! token once, so both stay the same from run to run. The token is made the
//! first time and kept in `<data>/agent.json`, beside the other stores; delete
//! the file to make a new one. `--agent-port N` picks another port.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use cena_agent::Characters;
use cena_session::agent::{self as level, Level};
use cena_session::command::claimant::Claimed;
use cena_session::{GameState, Notice, NoticeKind, SessionHandle, SessionId, SessionObserver};
use tokio_util::sync::CancellationToken;

use crate::commands::Commands;

/// Where the agent listens unless told otherwise.
const DEFAULT_PORT: u16 = 47_700;

/// The file that keeps the token.
const FILE: &str = "agent.json";

/// Whether `--agent` was given.
pub(crate) fn requested() -> bool {
    std::env::args().skip(1).any(|arg| arg == "--agent")
}

/// `--agent-port N` or `--agent-port=N`, else the default.
///
/// # Errors
///
/// A value that is not a port. It was taken as the default with nothing
/// said, so an agent set up for the port typed found nobody listening (the
/// review of 2026-09-29).
fn port() -> Result<u16, String> {
    port_of(std::env::args().skip(1))
}

/// [`port`] over `args`.
fn port_of(mut args: impl Iterator<Item = String>) -> Result<u16, String> {
    while let Some(arg) = args.next() {
        let value = match arg.strip_prefix("--agent-port=") {
            Some(value) => Some(value.to_owned()),
            None if arg == "--agent-port" => args.next(),
            None => None,
        };
        if let Some(value) = value {
            return value
                .trim()
                .parse()
                .map_err(|_| format!("--agent-port {value}: a port is a number from 1 to 65535."));
        }
    }
    Ok(DEFAULT_PORT)
}

/// The listener, and the characters it can see.
pub(crate) struct Agent {
    characters: Characters,
    stop: CancellationToken,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Agent {
    /// Listen, when `--agent` was given. `None` when it was not, or when the
    /// listener cannot start; the run goes on without it, and says why.
    pub(crate) async fn open(dir: &Path) -> Option<Self> {
        if !requested() {
            return None;
        }
        let token = match token(dir) {
            Ok(token) => token,
            Err(e) => {
                eprintln!("[agent] not started: no token -- {e}");
                return None;
            }
        };
        let port = match port() {
            Ok(port) => port,
            Err(why) => {
                eprintln!("[agent] not started: {why}");
                return None;
            }
        };
        let listener =
            match tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await {
                Ok(listener) => listener,
                Err(e) => {
                    eprintln!(
                        "[agent] not started: port {port} -- {e}. `--agent-port N` picks another."
                    );
                    return None;
                }
            };
        let url = format!("http://127.0.0.1:{port}/mcp");
        if let Err(e) = save(dir, &url, &token) {
            eprintln!("[agent] the address and token were not saved -- {e}");
        }
        eprintln!(
            "[agent] MCP at {url}; its token is in {}. Each character's `;agent level` says what an agent may do.",
            dir.join(FILE).display()
        );
        eprintln!(
            "[agent] Claude Code: claude mcp add --transport http hydra {url} --header \"Authorization: Bearer {token}\""
        );
        let characters = Characters::default();
        let stop = CancellationToken::new();
        let serving = (characters.clone(), stop.clone());
        let task = tokio::spawn(async move {
            if let Err(e) = cena_agent::serve(listener, serving.0, token, serving.1).await {
                eprintln!("[agent] stopped -- {e}");
            }
        });
        Some(Self {
            characters,
            stop,
            task: Mutex::new(Some(task)),
        })
    }

    /// A character started: an agent may reach it, as far as its level
    /// allows. `recording`: whether its database gets combat and loot both
    /// (`setup::Record::everything`).
    pub(crate) fn seat(
        &self,
        id: SessionId,
        (game, character): (&str, &str),
        handle: &SessionHandle,
        observer: SessionObserver,
        (database, recording): (Option<PathBuf>, bool),
    ) {
        self.characters.seat(
            id,
            (game, character),
            observer,
            handle.agent_door(),
            database,
            Some(recording),
        );
    }

    /// A character stopped.
    pub(crate) fn unseat(&self, id: SessionId) {
        self.characters.unseat(id);
    }

    /// Stop listening.
    pub(crate) async fn shutdown(&self) {
        self.stop.cancel();
        let task = self
            .task
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}

/// What `;agent help` says.
const HELP: &[&str] = &[
    "agent                  this character's agent level, and what an agent is waiting on you for",
    "agent level <level>    set it, kept for this character: off (the default), observe, advise, behaviors, commands or takeover",
    "                         off: an agent may do nothing with this character",
    "                         observe: it may read the character, and nothing else",
    "                         advise: it may also put a message in front of you; nothing reaches the game",
    "                         behaviors: it may also start, steer and stop go2, hunt, heal, keep and waggle",
    "                         commands: it may also send game commands, never dropping, giving, selling or destroying",
    "                         takeover: it may also take the character over, stopping what runs, until it gives it back",
    "agent approve <n>      let an agent do the one thing it asked, once",
    "agent deny <n>         refuse it",
    "agent stop             stop everything the agent is doing, and take the character back if it holds it",
    "An agent is a program such as Claude Code, connected to the listener Hydra starts with --agent.",
];

/// The character's agent level, from its settings file, and `;agent` on its
/// command line. Called once the login has said who the character is, as
/// the command symbol is (`travel.rs`).
///
/// A settings file that cannot be read leaves the level `off` and says so:
/// the safe level is the one that allows nothing.
pub(crate) fn control(
    handle: &SessionHandle,
    observer: &SessionObserver,
    commands: &Commands,
    state: &GameState,
) {
    let character = &state.character;
    let who = character.instance.clone().zip(character.name.clone());
    let dir = cena_session::character_store::data_dir();
    if let Some((instance, name)) = who.clone() {
        tokio::spawn(keep_level(
            handle.clone(),
            observer.clone(),
            dir.clone(),
            instance,
            name,
        ));
    }
    if let Some((instance, name)) = &who {
        match load_level(&dir, instance, name) {
            Ok(level) => handle.set_agent_level(level),
            Err(why) => handle.say(Notice::line(
                NoticeKind::Warn,
                format!("Agent: {why} -- the agent level stays off."),
            )),
        }
    }
    let told = handle.clone();
    commands.agent(Arc::new(move |line: &str| {
        let mut words = line.split_whitespace();
        if !words.next()?.eq_ignore_ascii_case("agent") {
            return None;
        }
        // Hydra's words ignore case, as `Level::named` does.
        let words: Vec<String> = words.map(str::to_ascii_lowercase).collect();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let who = who.as_ref().map(|(i, n)| (i.as_str(), n.as_str()));
        told.say(answer(&told, &dir, who, &words).answering());
        Some(Claimed::Done)
    }));
}

/// Keep every change of the level in the settings file, whoever made it:
/// the player's `;agent level`, and the session's own drop to Observe after
/// a run that ended badly (`plan/35` §4), which must not come back at the
/// next login.
async fn keep_level(
    handle: SessionHandle,
    observer: SessionObserver,
    dir: PathBuf,
    instance: String,
    name: String,
) {
    use cena_session::agent::Change;
    use tokio::sync::broadcast::error::RecvError;
    let Ok((_, mut events)) = observer.subscribe().await else {
        return;
    };
    loop {
        match events.recv().await {
            Ok(event) => {
                // The player's own `;agent level` saved before it answered,
                // so the file already holds it: not written twice (the
                // review of 2026-09-29). The session's own drop, or a file
                // that does not read, is saved here.
                if let cena_session::Event::Agent(Change::Level(level)) = event.event
                    && load_level(&dir, &instance, &name) != Ok(level)
                    && let Err(why) = save_level(&dir, &instance, &name, level)
                {
                    handle.say(Notice::line(
                        NoticeKind::Warn,
                        format!("Agent: the level {} was not kept: {why}.", level.word()),
                    ));
                }
            }
            Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

/// What `;agent <words>` does, and what it says.
fn answer(handle: &SessionHandle, dir: &Path, who: Option<(&str, &str)>, words: &[&str]) -> Notice {
    let number =
        |word: Option<&&str>| word.and_then(|w| w.trim_start_matches('#').parse::<u64>().ok());
    match words {
        [] | ["level"] => status(handle),
        ["level", word] => match Level::named(word) {
            Some(level) => {
                handle.set_agent_level(level);
                let saved = match who {
                    Some((instance, name)) => save_level(dir, instance, name, level).map_or_else(
                        |why| format!("Not kept past this run: {why}."),
                        |_| "Kept for this character.".to_owned(),
                    ),
                    None => "Not kept past this run: the game has not said who this is.".to_owned(),
                };
                Notice::line(
                    NoticeKind::Info,
                    format!(
                        "Agent level: {} -- {}. {saved}",
                        level.word(),
                        level.allows()
                    ),
                )
            }
            None => Notice::line(
                NoticeKind::Error,
                format!(
                    "Agent: there is no level {word}; the levels are {}.",
                    levels()
                ),
            ),
        },
        [verb @ ("approve" | "deny"), rest @ ..] => {
            let Some(id) = number(rest.first()).filter(|_| rest.len() == 1) else {
                return Notice::line(
                    NoticeKind::Error,
                    format!("Agent: {verb} takes the request's number, as agent lists them."),
                );
            };
            let approving = *verb == "approve";
            let done = if approving {
                handle.approve_agent(id)
            } else {
                handle.deny_agent(id)
            };
            match done {
                Ok(()) if approving => {
                    Notice::line(NoticeKind::Info, format!("Agent: approved request {id}."))
                }
                Ok(()) => Notice::line(NoticeKind::Info, format!("Agent: denied request {id}.")),
                Err(why) => Notice::line(NoticeKind::Error, format!("Agent: {why}.")),
            }
        }
        ["stop"] => match handle.stop_agent() {
            (0, false) => Notice::line(
                NoticeKind::Info,
                "Agent: nothing of the agent's is running.",
            ),
            (stopped, held) => Notice::line(
                NoticeKind::Info,
                format!(
                    "Agent: {stopped} operation(s) told to stop{}. The level is unchanged; agent level observe keeps it from acting again.",
                    if held {
                        ", and the character taken back"
                    } else {
                        ""
                    }
                ),
            ),
        },
        ["help"] => Notice::table(
            NoticeKind::Info,
            HELP.iter().map(|&l| l.to_owned()).collect(),
        ),
        _ => Notice::line(
            NoticeKind::Error,
            format!(
                "Agent: I do not know agent {}. agent help lists what it does.",
                words.join(" ")
            ),
        ),
    }
}

/// `;agent`: the level, whether anything can connect, and what waits.
fn status(handle: &SessionHandle) -> Notice {
    let level = handle.agent_level();
    let mut lines = vec![format!(
        "Agent level: {} -- {}.",
        level.word(),
        level.allows()
    )];
    lines.push(if requested() {
        match port() {
            Ok(port) => format!("Agents connect at http://127.0.0.1:{port}/mcp."),
            Err(why) => format!("No agent can connect: {why}"),
        }
    } else {
        "No agent can connect: Hydra was not started with --agent.".to_owned()
    });
    if let Some(operation) = handle.agent_holds() {
        lines.push(format!(
            "An agent holds this character (operation {operation}): agent stop takes it back."
        ));
    }
    let requests = handle.agent_requests();
    if requests.is_empty() {
        lines.push("Nothing is waiting on you.".to_owned());
    }
    for request in requests {
        lines.push(format!(
            "Waiting, {}: an agent asks to {}, because: {} -- lapses in {}s; agent approve {} or agent deny {}.",
            request.id,
            request.act,
            request.because,
            request.expires_in.as_secs(),
            request.id,
            request.id
        ));
    }
    Notice::prose(NoticeKind::Info, lines)
}

/// The levels, as a player types them.
fn levels() -> String {
    Level::ALL.map(Level::word).join(", ")
}

/// The level kept in the character's settings file; `off` when none is.
fn load_level(dir: &Path, instance: &str, name: &str) -> Result<Level, String> {
    cena_session::settings_store::load(dir, instance, name)
        .map_err(|e| e.to_string())?
        .section::<level::Settings>(level::SECTION)
        .map(|settings| settings.level)
        .map_err(|e| format!("the {} section is malformed: {e}", level::SECTION))
}

/// Keep `level` in the character's settings file. A file that cannot be read
/// is never overwritten (`settings_store`): the level still holds for this run.
///
/// Read, changed and written under the file's lock (`store::changing`; the
/// crate review of 2026-10-01, SE-C-1): the session drops the level at a
/// moment the player did not choose, perhaps while a page is being changed.
fn save_level(dir: &Path, instance: &str, name: &str, level: Level) -> Result<PathBuf, String> {
    let path = cena_session::settings_store::settings_path(dir, instance, name).unwrap_or_default();
    cena_session::store::changing(&path, || {
        let mut file =
            cena_session::settings_store::load(dir, instance, name).map_err(|e| e.to_string())?;
        file.set_section(level::SECTION, &level::Settings { level })
            .map_err(|e| e.to_string())?;
        cena_session::settings_store::save(dir, &file).map_err(|e| e.to_string())
    })
}

/// The kept token, or a new one, kept.
fn token(dir: &Path) -> Result<String, String> {
    let kept = std::fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.get("token")?.as_str().map(str::to_owned))
        .filter(|token| !token.is_empty());
    match kept {
        Some(token) => Ok(token),
        None => cena_agent::new_token(),
    }
}

fn save(dir: &Path, url: &str, token: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(&serde_json::json!({ "url": url, "token": token }))
        .map_err(std::io::Error::other)?;
    std::fs::write(dir.join(FILE), text)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_port_that_is_not_one_is_refused_not_taken_as_the_default() {
        let of = |args: &[&str]| super::port_of(args.iter().map(|a| (*a).to_owned()));
        assert_eq!(of(&["--agent", "--agent-port", "47701"]), Ok(47701));
        assert_eq!(of(&["--agent-port=47702"]), Ok(47702));
        assert_eq!(of(&["--agent"]), Ok(super::DEFAULT_PORT));
        assert!(of(&["--agent-port", "70000"]).is_err_and(|why| why.contains("70000")));
        assert!(of(&["--agent-port=abc"]).is_err());
        assert!(
            of(&["--agent-port"]).is_ok(),
            "nothing after it: the default, as before"
        );
    }

    use super::*;
    use cena_platform::AnsweringSource;
    use cena_session::agent::{Approval, Call, Denied};
    use cena_session::{Body, Session};

    fn text(notice: &Notice) -> String {
        match &notice.body {
            Body::Lines(lines) | Body::Mono(lines) => lines.join("\n"),
        }
    }

    /// The level is set, kept in the settings file beside the other
    /// sections, and read back; a word that is no level changes nothing.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn the_level_is_set_kept_and_read_back() {
        let dir = std::env::temp_dir().join(format!("cena-agent-level-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (source, _) = AnsweringSource::new(b"<prompt time=\"1\">&gt;</prompt>\n");
        let handle = Session::new(source).handle();
        let who = Some(("GS3", "Nisugi"));
        assert_eq!(
            handle.agent_level(),
            Level::Off,
            "off until the player says"
        );

        let said = text(&answer(&handle, &dir, who, &["level", "Observe"]));
        assert!(said.contains("observe") && said.contains("Kept"), "{said}");
        assert_eq!(handle.agent_level(), Level::Observe);
        assert_eq!(load_level(&dir, "GS3", "Nisugi"), Ok(Level::Observe));

        let said = text(&answer(&handle, &dir, who, &["level", "everything"]));
        assert!(said.contains("no level everything"), "{said}");
        assert_eq!(handle.agent_level(), Level::Observe, "unchanged");
        assert!(text(&answer(&handle, &dir, who, &[])).contains("Nothing is waiting"));
        let said = text(&answer(&handle, &dir, who, &["stop"]));
        assert!(said.contains("nothing of the agent's"), "{said}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What an agent asks above the level waits for `;agent approve`, which
    /// does it once; `;agent deny` refuses; a number twice is gone.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_request_is_approved_once_or_denied() {
        let (source, _) = AnsweringSource::new(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        handle.set_agent_level(Level::Observe);
        let door = handle.agent_door();
        let call = |request| Call {
            request,
            generation: None,
        };
        let Err(Denied::Level(refused)) =
            door.tell_player("the hunt is over", "you asked to know", call("r1"))
        else {
            panic!("told the player at observe");
        };
        let Approval::Asked { id, .. } = refused.approval else {
            panic!("the player was not asked: {refused:?}");
        };
        assert!(text(&answer(&handle, Path::new("."), None, &[])).contains("tell you something"));
        let said = text(&answer(
            &handle,
            Path::new("."),
            None,
            &["approve", &id.to_string()],
        ));
        assert!(said.contains("approved"), "{said}");
        let said = text(&answer(
            &handle,
            Path::new("."),
            None,
            &["approve", &id.to_string()],
        ));
        assert!(said.contains("no request"), "{said}");
        let mut told = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let cena_session::Event::Notice(notice) = event {
                told.push(text(&notice));
            }
        }
        assert_eq!(
            told.iter()
                .filter(|t| t.contains("Agent: the hunt is over"))
                .count(),
            1,
            "done once: {told:?}"
        );

        let Err(Denied::Level(refused)) = door.tell_player("again", "why not", call("r2")) else {
            panic!("told the player at observe");
        };
        let Approval::Asked { id, .. } = refused.approval else {
            panic!("the player was not asked: {refused:?}");
        };
        let said = text(&answer(
            &handle,
            Path::new("."),
            None,
            &["deny", &id.to_string()],
        ));
        assert!(said.contains("denied"), "{said}");
    }
}
