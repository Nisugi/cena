//! The player's own Lich for a character, switched on and off (`plan/51`
//! §7, step 5): `;lich`.
//!
//! `lich on` starts the character's Lich through the relay
//! (`cena_agent::lich`) and keeps it on for the character, so it starts
//! again whenever the character does; `lich off` stops it and keeps it off.
//! Off until asked (`plan/51` §5). Kept in the character's settings file, as
//! the command symbol is.
//!
//! Lich is the player's own install: `lich folder <folder>` says where, the
//! folder with `lich.rbw` in it, once for every character (`lich.json` in
//! Hydra's data folder). It runs on the Ruby Lich's installer put in, found
//! as the script runner finds it.
//!
//! Registered once the login has said who the character is, as `;agent` is:
//! the switch is the character's, and the file is named by it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use cena_agent::lich::{Launch, run};
use cena_agent::scripts::runner::find_ruby;
use cena_session::command::claimant::Claimed;
use cena_session::{Notice, NoticeKind, SessionHandle, SessionId, SessionObserver, State};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::commands::Commands;

/// The character's settings section: whether its Lich is on.
const SECTION: &str = "lich";

/// Where Lich is, for every character, in Hydra's data folder.
const PLACE_FILE: &str = "lich.json";

/// What `;lich help` says.
const HELP: &[&str] = &[
    "lich                 whether this character's own Lich runs, and where Lich is",
    "lich on              start it, and start it whenever this character starts",
    "lich off             stop it, and leave it off",
    "lich folder <folder> where your Lich is: the folder with lich.rbw in it, for every character",
    "With Lich on, a line you type goes to Lich, and one starting with ; is Lich's own command. \
     Give Hydra another command symbol, such as ., to reach Hydra's.",
];

/// The character's switch, in its settings file.
#[derive(Debug, Default, Deserialize, Serialize)]
struct Switch {
    #[serde(default)]
    on: bool,
}

/// Where Lich is, for every character.
#[derive(Debug, Default, Deserialize, Serialize)]
struct Place {
    folder: Option<PathBuf>,
}

/// Every character's Lich that runs, by session.
#[derive(Clone, Default)]
pub(crate) struct Lichs(Arc<Mutex<BTreeMap<SessionId, Running>>>);

/// One character's Lich: how to stop it, and the relay's task.
struct Running {
    stop: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

/// One character, as `;lich` acts for it.
#[derive(Clone)]
struct Seat {
    id: SessionId,
    dir: PathBuf,
    instance: String,
    name: String,
}

impl Lichs {
    /// Once `id`'s login has said who it is: `;lich` on its command line,
    /// and its Lich started if it is kept on. Waits for that login.
    pub(crate) async fn at_login(
        self,
        id: SessionId,
        handle: SessionHandle,
        observer: SessionObserver,
        commands: Commands,
    ) {
        let Some((instance, name)) = logged_in(&observer).await else {
            return;
        };
        self.seat(
            &handle,
            &Seat {
                id,
                dir: cena_session::character_store::data_dir(),
                instance,
                name,
            },
            &commands,
        );
    }

    /// `;lich` on `seat`'s command line, and its Lich started if it is kept
    /// on.
    fn seat(&self, handle: &SessionHandle, seat: &Seat, commands: &Commands) {
        let lichs = self.clone();
        let (told, answering) = (handle.clone(), seat.clone());
        commands.lich(Arc::new(move |line: &str| {
            let mut words = line.split_whitespace();
            if !words.next()?.eq_ignore_ascii_case("lich") {
                return None;
            }
            let rest: Vec<&str> = words.collect();
            lichs.answer(&told, &answering, &rest);
            Some(Claimed::Done)
        }));
        match switch(seat) {
            Ok(true) => self.start(handle, seat, false),
            Ok(false) => {}
            Err(why) => handle.say(Notice::line(
                NoticeKind::Warn,
                format!("Lich: {why} -- Lich stays off."),
            )),
        }
    }

    /// A character left the table: its Lich stops, by the time this
    /// returns.
    pub(crate) async fn close(&self, id: SessionId) {
        let running = self.lock().remove(&id);
        if let Some(running) = running {
            running.stop.cancel();
            let _ = running.task.await;
        }
    }

    /// Hydra is stopping: every Lich.
    pub(crate) async fn shutdown(&self) {
        let running = std::mem::take(&mut *self.lock());
        for one in running.values() {
            one.stop.cancel();
        }
        for (_, one) in running {
            let _ = one.task.await;
        }
    }

    /// Answer `;lich` with `words` after it.
    fn answer(&self, handle: &SessionHandle, seat: &Seat, words: &[&str]) {
        let lower: Vec<String> = words.iter().map(|w| w.to_ascii_lowercase()).collect();
        let said = match lower.first().map(String::as_str) {
            None => Ok(status(handle, seat)),
            Some("help") => {
                let lines = HELP.iter().map(|&line| line.to_owned()).collect();
                handle.say(Notice::table(NoticeKind::Info, lines).answering());
                return;
            }
            Some("on") => keep_switch(seat, true).map(|()| {
                self.start(handle, seat, true);
                String::new()
            }),
            Some("off") => keep_switch(seat, false).map(|()| {
                if self.stop(seat.id) {
                    "Lich is stopping, and stays off for this character.".to_owned()
                } else {
                    "Lich stays off for this character.".to_owned()
                }
            }),
            Some("folder") if words.len() > 1 => keep_folder(&seat.dir, &words[1..].join(" ")),
            Some(_) => Err("say lich on, lich off, lich folder <folder>, or lich help".to_owned()),
        };
        match said {
            Ok(said) if said.is_empty() => {}
            Ok(said) => handle.say(Notice::line(NoticeKind::Info, said).answering()),
            Err(why) => {
                handle.say(Notice::line(NoticeKind::Warn, format!("Lich: {why}")).answering());
            }
        }
    }

    /// Start the character's Lich, unless it runs; say why not when it
    /// cannot, `answering` the player's `lich on` or on Hydra's own at
    /// login. The relay says how it ends.
    fn start(&self, handle: &SessionHandle, seat: &Seat, answering: bool) {
        let say = |notice: Notice| {
            handle.say(if answering {
                notice.answering()
            } else {
                notice
            });
        };
        let mut running = self.lock();
        if running
            .get(&seat.id)
            .is_some_and(|one| !one.task.is_finished())
        {
            say(Notice::line(
                NoticeKind::Info,
                "Lich already runs for this character.",
            ));
            return;
        }
        let launch = match launch(&seat.dir) {
            Ok(launch) => launch,
            Err(why) => {
                say(Notice::line(
                    NoticeKind::Warn,
                    format!("Lich: {why} -- Lich was not started."),
                ));
                return;
            }
        };
        say(Notice::line(
            NoticeKind::Info,
            format!("Lich is starting, from {}.", launch.lich.display()),
        ));
        let stop = CancellationToken::new();
        let relay = run(handle.lich_door(), launch, stop.clone());
        let task = tokio::spawn(async move {
            let _ = relay.await;
        });
        running.insert(seat.id, Running { stop, task });
    }

    /// Stop the character's Lich: whether one ran.
    fn stop(&self, id: SessionId) -> bool {
        self.lock().remove(&id).is_some_and(|one| {
            one.stop.cancel();
            !one.task.is_finished()
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<SessionId, Running>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Whether Lich runs, is kept on, and where it is.
fn status(handle: &SessionHandle, seat: &Seat) -> String {
    let runs = if handle.lich_running() {
        "Lich runs for this character"
    } else {
        "Lich is not running for this character"
    };
    let kept = match switch(seat) {
        Ok(true) => ", and starts with it",
        Ok(false) => ", and is off",
        Err(_) => "",
    };
    let place = match folder(&seat.dir) {
        Ok(Some(folder)) => format!("Lich is at {}.", folder.display()),
        Ok(None) => "Where Lich is has not been said: lich folder <folder>.".to_owned(),
        Err(why) => why,
    };
    format!("{runs}{kept}. {place}")
}

/// The character's instance and name, once its login has said them: `None`
/// if the session ends first.
async fn logged_in(observer: &SessionObserver) -> Option<(String, String)> {
    let (snapshot, mut events) = observer.subscribe().await.ok()?;
    if snapshot.lifecycle != State::Ready {
        loop {
            match events.recv().await {
                Ok(event)
                    if matches!(event.event, cena_session::Event::StateChanged(State::Ready)) =>
                {
                    break;
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
    let (snapshot, _) = observer.subscribe().await.ok()?;
    let character = snapshot.state.character;
    character.instance.zip(character.name)
}

/// Whether the character's Lich is kept on.
fn switch(seat: &Seat) -> Result<bool, String> {
    cena_session::settings_store::load(&seat.dir, &seat.instance, &seat.name)
        .map_err(|e| e.to_string())?
        .section::<Switch>(SECTION)
        .map(|switch| switch.on)
        .map_err(|e| format!("the {SECTION} section is malformed: {e}"))
}

/// Keep the character's Lich `on`, or off. A settings file that cannot be
/// read is never overwritten (`settings_store`).
fn keep_switch(seat: &Seat, on: bool) -> Result<(), String> {
    let mut file = cena_session::settings_store::load(&seat.dir, &seat.instance, &seat.name)
        .map_err(|e| e.to_string())?;
    file.set_section(SECTION, &Switch { on })
        .map_err(|e| e.to_string())?;
    cena_session::settings_store::save(&seat.dir, &file)
        .map(drop)
        .map_err(|e| e.to_string())
}

/// Where Lich is, if that has been said.
fn folder(dir: &Path) -> Result<Option<PathBuf>, String> {
    match std::fs::read_to_string(dir.join(PLACE_FILE)) {
        Ok(text) => serde_json::from_str::<Place>(&text)
            .map(|place| place.folder)
            .map_err(|e| format!("{PLACE_FILE} is malformed: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{PLACE_FILE} could not be read: {e}")),
    }
}

/// Keep `folder` as where Lich is, for every character, once it holds
/// `lich.rbw`.
fn keep_folder(dir: &Path, folder: &str) -> Result<String, String> {
    let folder = PathBuf::from(folder.trim().trim_matches('"'));
    if !folder.join("lich.rbw").is_file() {
        return Err(format!("{} has no lich.rbw in it", folder.display()));
    }
    let place = Place {
        folder: Some(folder.clone()),
    };
    let path = dir.join(PLACE_FILE);
    cena_session::store::save_json(dir, &path, &place).map_err(|e| e.to_string())?;
    Ok(format!(
        "Lich is at {}, for every character.",
        folder.display()
    ))
}

/// How to start the player's Lich: its folder, and Lich's Ruby.
fn launch(dir: &Path) -> Result<Launch, String> {
    let folder = folder(dir)?.ok_or("where Lich is has not been said: lich folder <folder>")?;
    let lich = folder.join("lich.rbw");
    if !lich.is_file() {
        return Err(format!("{} has no lich.rbw in it", folder.display()));
    }
    let ruby = find_ruby().ok_or(
        "Lich needs Ruby, which Lich's installer puts in C:\\Ruby4Lich5; none was found there \
         or on the PATH",
    )?;
    Ok(Launch {
        ruby,
        lich,
        args: Vec::new(),
        env: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use cena_platform::AnsweringSource;
    use cena_session::{Session, SessionHandle, SessionId};

    use super::{Lichs, Seat, folder, keep_folder, switch};
    use crate::commands::Commands;

    const DEADLINE: Duration = Duration::from_secs(90);

    /// A data folder of its own, with a Lich in it: the relay's stand-in,
    /// as `lich.rbw`.
    fn data_with_lich(name: &str) -> Option<PathBuf> {
        let dir = std::env::temp_dir().join(format!("cena-lich-{name}-{}", std::process::id()));
        let lich = dir.join("Lich5");
        std::fs::create_dir_all(&lich).ok()?;
        let standin = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cena-agent/tests/fixtures/standin_lich.rb");
        std::fs::copy(standin, lich.join("lich.rbw")).ok()?;
        keep_folder(&dir, &lich.display().to_string()).ok()?;
        Some(dir)
    }

    /// Whether `handle`'s Lich runs, once it has settled to `running`.
    async fn settles(handle: &SessionHandle, running: bool) -> bool {
        tokio::time::timeout(DEADLINE, async {
            while handle.lich_running() != running {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .is_ok()
    }

    /// `;lich on` starts the character's Lich and keeps it on; `;lich off`
    /// stops it and keeps it off; a character kept on starts its Lich when
    /// it is seated again.
    #[tokio::test(flavor = "multi_thread")]
    async fn lich_on_and_off_are_kept_for_the_character() {
        let dir = data_with_lich("switch").expect("a data folder");
        let (source, _transcript) =
            AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        tokio::spawn(session.into_actor().run());
        let commands = Commands::install(&handle);
        let seat = Seat {
            id: SessionId::FIRST,
            dir: dir.clone(),
            instance: "GS3".to_owned(),
            name: "Tester".to_owned(),
        };
        let lichs = Lichs::default();
        lichs.seat(&handle, &seat, &commands);
        assert!(!handle.lich_running(), "off until asked");

        let typed = |line: &'static str| {
            let handle = handle.clone();
            async move {
                handle
                    .send_typed_at(handle.generation(), line, DEADLINE)
                    .await
            }
        };
        typed(".lich on").await;
        assert!(settles(&handle, true).await, "started");
        assert_eq!(switch(&seat), Ok(true), "kept on");
        typed(".lich off").await;
        assert!(settles(&handle, false).await, "stopped");
        assert_eq!(switch(&seat), Ok(false), "kept off");

        super::keep_switch(&seat, true).unwrap();
        lichs.seat(&handle, &seat, &Commands::default());
        assert!(settles(&handle, true).await, "started with the character");
        lichs.shutdown().await;
        assert!(!handle.lich_running());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Where Lich is is kept once it holds `lich.rbw`, and read back.
    #[test]
    fn the_folder_is_kept_once_it_holds_lich() {
        let dir = std::env::temp_dir().join(format!("cena-lich-folder-{}", std::process::id()));
        let lich = dir.join("Lich5");
        std::fs::create_dir_all(&lich).unwrap();
        assert_eq!(folder(&dir), Ok(None));
        assert!(keep_folder(&dir, &lich.display().to_string()).is_err());
        std::fs::write(lich.join("lich.rbw"), "").unwrap();
        assert!(keep_folder(&dir, &format!("\"{}\"", lich.display())).is_ok());
        assert_eq!(folder(&dir), Ok(Some(lich)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
