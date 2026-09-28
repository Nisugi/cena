//! What the runner's end-to-end tests share (`tests/runner.rs`): a scripted
//! character with a Ruby runner started for it, the world it plays in, and
//! Hydra's travel stood in for. Moved down out of `runner.rs` at its cap
//! (`plan/05` Rule 4.1).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cena_agent::scripts::local::Atlas;
use cena_agent::scripts::runner::{Start, find_ruby, start, unpack};
use cena_agent::scripts::{Runners, serve};
use cena_map::{Map, RoomId, Uid};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{Event, GameState, Origin, Session};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;

/// A folder of this test's own.
pub fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-runner-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The world a test's character plays in.
#[derive(Default)]
pub struct World<'a> {
    /// The scripted game's answers.
    pub answers: &'a [(&'a str, &'a [u8])],
    /// The map its runner places it on.
    pub atlas: Option<Atlas>,
    /// What the player typed before the runner started.
    pub before: &'a [&'a str],
    /// Whether Hydra's travel is stood in for (`walker`).
    pub walker: bool,
    /// Whether the runner may open windows (Gtk).
    pub windows: bool,
}

/// How many `go2 far` walks were started, and how many stopped.
#[derive(Default)]
pub struct Walks {
    pub started: std::sync::atomic::AtomicUsize,
    pub stopped: std::sync::atomic::AtomicUsize,
}

impl Walks {
    pub fn counts(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering::SeqCst;
        (self.started.load(SeqCst), self.stopped.load(SeqCst))
    }
}

/// A stand-in for Hydra's travel, the binary's performer: `go2 229` walks
/// north and arrives; `go2 far` walks until it is stopped, counted in
/// `walks`; `go2 slow` too, taking 300 ms to start, so a script can be
/// killed before its run's number comes back.
pub fn walker(
    handle: &cena_session::SessionHandle,
    walks: Arc<Walks>,
) -> cena_session::operation::Performer {
    use cena_session::operation::{Ended, Performer, Started, Work};
    let walking = handle.clone();
    Performer {
        allowed: "go2".to_owned(),
        allows: Arc::new(|line: &str| {
            if line.starts_with("go2 ") {
                Ok(line.to_owned())
            } else {
                Err(format!("`{line}` is not travel's"))
            }
        }),
        start: Arc::new(move |line: &str, _reporter| {
            let (walking, walks) = (walking.clone(), Arc::clone(&walks));
            let halt = Arc::new(tokio::sync::Notify::new());
            let heard = Arc::clone(&halt);
            let far = line == "go2 far" || line == "go2 slow";
            if line == "go2 slow" {
                std::thread::sleep(Duration::from_millis(300));
            }
            if far {
                walks
                    .started
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            Started {
                ended: Box::pin(async move {
                    if far {
                        heard.notified().await;
                        walks
                            .stopped
                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        return Ended::plainly(Work::Interrupted, "stopped");
                    }
                    walking
                        .send_manual_at(walking.generation(), "north", Duration::from_secs(5))
                        .await;
                    Ended::plainly(Work::Completed, "arrived")
                }),
                steer: Arc::new(move |_| {
                    halt.notify_one();
                    Ok(())
                }),
                token: None,
            }
        }),
        halt: Arc::new(|| {}),
    }
}

/// A scripted character with a runner started for it over `scripts`.
pub struct Running {
    pub handle: cena_session::SessionHandle,
    pub transcript: TranscriptHandle,
    /// The stand-in travel's far walks, started and stopped.
    pub walks: Arc<Walks>,
    pub legacy: Receiver<Event>,
    pub runners: Runners,
    pub token: String,
    pub child: tokio::process::Child,
    pub errors: Arc<Mutex<Vec<String>>>,
    pub stop: CancellationToken,
}

/// What the player was told and shown and what went out, until a line told
/// contains `until`, or a minute passed.
pub struct Heard {
    pub told: Vec<String>,
    pub shown: Vec<String>,
    pub sent: Vec<(String, Origin)>,
    pub finished: bool,
}

impl Running {
    /// The character plays in `world`; its runner runs the scripts in
    /// `scripts`, with `dir` for the rest.
    pub async fn start(scripts: &Path, dir: &Path, world: World<'_>) -> Option<Self> {
        let World {
            answers,
            atlas,
            before,
            walker: walking,
            windows,
        } = world;
        let ruby = find_ruby()?;
        let (source, transcript) =
            AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
        for (line, reply) in answers {
            transcript.answer(line, reply);
        }
        let session = Session::new(source);
        let handle = session.handle();
        let observer = session.observer();
        let (_, legacy) = session.subscribe();
        tokio::spawn(session.into_actor().run());
        let walks = Arc::new(Walks::default());
        if walking {
            handle
                .set_performer(walker(&handle, Arc::clone(&walks)))
                .then_some(())?;
        }
        for line in before {
            handle
                .send_manual_at(handle.generation(), line, Duration::from_secs(5))
                .await;
        }

        let runners = atlas.map_or_else(Runners::default, Runners::with_atlas);
        let stop = CancellationToken::new();
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.ok()?;
        let url = format!("http://{}/mcp", listener.local_addr().ok()?);
        tokio::spawn(serve(listener, runners.clone(), stop.clone()));
        let token = runners
            .admit("Nisugi", handle.script_door(), &observer)
            .await
            .ok()?;
        let runner_dir = dir.join("runner");
        unpack(&runner_dir).ok()?;
        let data = dir.join("data");
        std::fs::create_dir_all(&data).ok()?;
        let mut child = start(&Start {
            ruby: &ruby,
            dir: &runner_dir,
            url: &url,
            token: &token,
            character: "Nisugi",
            game: "GS3",
            scripts,
            data: &data,
            symbol: ';',
            windows,
        })
        .ok()?;
        let errors = Arc::new(Mutex::new(Vec::<String>::new()));
        let kept = Arc::clone(&errors);
        let stderr = child.stderr.take()?;
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut errors) = kept.lock() {
                    errors.push(line);
                }
            }
        });
        Some(Self {
            handle,
            transcript,
            walks,
            legacy,
            runners,
            token,
            child,
            errors,
            stop,
        })
    }

    /// The player types `;line` for the runner.
    pub fn typed(&self, line: &str) -> bool {
        self.runners.typed(&self.token, line)
    }

    pub async fn heard_until(&mut self, until: &str) -> Heard {
        self.heard_until_either(until, until).await
    }

    /// Run `;line` and hear it through to `name has exited`. Lich says a
    /// script has exited a moment before it leaves the running list, so a
    /// second run of the same script typed at once can be told it is still
    /// running: that answer is waited out, as a player would type again.
    /// `None` when it never stopped running.
    pub async fn run(&mut self, line: &str, name: &str) -> Option<Heard> {
        let exited = format!("{name} has exited");
        for _ in 0..20 {
            assert!(self.typed(line));
            let heard = self.heard_until_either(&exited, "is already running").await;
            if !heard
                .told
                .iter()
                .any(|told| told.contains("is already running"))
            {
                return Some(heard);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        None
    }

    pub async fn heard_until_either(&mut self, until: &str, or: &str) -> Heard {
        let mut heard = Heard {
            told: Vec::new(),
            shown: Vec::new(),
            sent: Vec::new(),
            finished: false,
        };
        let waited = tokio::time::timeout(Duration::from_mins(1), async {
            loop {
                match self.legacy.recv().await {
                    Ok(Event::Notice(notice)) => heard.told.extend(notice.lines().iter().cloned()),
                    Ok(Event::Line(line)) => heard.shown.push(line.text()),
                    // A script's own, never the player's typing before it.
                    Ok(Event::Sent { line, origin }) if origin != Origin::Manual => {
                        heard.sent.push((line, origin));
                    }
                    Ok(_) => {}
                    Err(_) => return,
                }
                if heard
                    .told
                    .iter()
                    .any(|line| line.contains(until) || line.contains(or))
                {
                    return;
                }
            }
        })
        .await;
        heard.finished = waited.is_ok() && heard.told.iter().any(|line| line.contains(until));
        heard
    }

    /// The player types `line` at a frontend: the runner's input hooks
    /// first, then Hydra's commands or the game.
    pub async fn typed_line(&self, line: &str) -> cena_session::Outcome {
        self.handle
            .send_typed_at(self.handle.generation(), line, Duration::from_secs(5))
            .await
    }

    /// The next `count` lines the player is shown, or fewer after ten
    /// seconds.
    pub async fn shown(&mut self, count: usize) -> Vec<String> {
        let mut shown = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(10), async {
            while shown.len() < count {
                match self.legacy.recv().await {
                    Ok(Event::Line(line)) => shown.push(line.text()),
                    Ok(_) => {}
                    Err(_) => return,
                }
            }
        })
        .await;
        shown
    }

    pub fn errors(&self) -> Vec<String> {
        self.errors.lock().map(|e| e.clone()).unwrap_or_default()
    }

    pub async fn end(mut self) {
        let _ = self.child.kill().await;
        self.runners.dismiss(&self.token);
        self.stop.cancel();
    }
}

/// The game's answer to `look`: a line, and the prompt.
pub const QUIET_ROOM: &[u8] = b"You see a quiet room.\n<prompt time=\"2\">&gt;</prompt>\n";

/// A room the game describes whole: its number, title and exits; a player,
/// a kobold and a rock; the hands, a bar and a status.
pub const DESCRIBED_ROOM: &[u8] = b"<nav rm='7000'/>
<streamWindow id='room' title='Room' subtitle=\" - Quiet Glade\"/>
<compass><dir value=\"n\"/><dir value=\"e\"/></compass>
<component id='room players'>Also here: <a exist=\"-99\" noun=\"Kiyna\">Kiyna</a>.</component>
<component id='room objs'>You also see <pushBold/>a <a exist=\"42\" noun=\"kobold\">kobold</a><popBold/> and <a exist=\"43\" noun=\"rock\">a rock</a>.</component>
<crtrStatus exist=\"42\" hostile=\"1\"/>
<right exist=\"11\" noun=\"sword\">broadsword</right><left>Empty</left>
<dialogData id='minivitals'><progressBar id='health' value='50' text='health 50/100'/></dialogData>
<indicator id=\"IconSTANDING\" visible=\"y\"/>
<clearStream id=\"Spells\"/>
<stream id=\"Spells\">Major Spiritual:</stream>
<stream id=\"Spells\">  <a exist=\"-1\" coord=\"1,1\" noun=\"215\">Heroism</a></stream>
<dialogData id='minivitals'><progressBar id='mana' value='50' text='mana 20/40'/></dialogData>
<dialogData id='Active Spells'><progressBar id='215' value='90' text=\"Heroism\" time='00:20:00'/></dialogData>
You see a quiet room.
<prompt time=\"1001\">&gt;</prompt>
";

/// A folder with one of this test's own scripts in it.
pub fn scripts_with(dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let scripts = dir.join("scripts");
    std::fs::create_dir_all(&scripts)?;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::copy(fixture, scripts.join(name))?;
    Ok(scripts)
}

/// Two rooms of a map: the Quiet Glade (228, the game's 7000), and the North
/// Glade (229, the game's 7001) north of it.
pub const TWO_ROOMS: &str = r#"[
    {"id":228,"uid":[7000],"title":["[Quiet Glade]"],"exits":[{"to":229,"kind":"cardinal","cmd":"north","cost":0.2}]},
    {"id":229,"uid":[7001],"title":["[North Glade]"],"exits":[{"to":228,"kind":"cardinal","cmd":"south","cost":0.2}]}
]"#;

/// The Quiet Glade, as `look` shows it.
pub const QUIET_GLADE: &[u8] = b"<nav rm='7000'/>
<streamWindow id='room' title='Room' subtitle=\" - Quiet Glade\"/>
<compass><dir value=\"n\"/></compass>
<component id='room players'></component>
<component id='room objs'></component>
You see a quiet glade.
<prompt time=\"1001\">&gt;</prompt>
";

/// The North Glade, walked into, with a kobold in it.
pub const NORTH_GLADE: &[u8] = b"<nav rm='7001'/>
<streamWindow id='room' title='Room' subtitle=\" - North Glade\"/>
<compass><dir value=\"s\"/></compass>
<component id='room players'></component>
<component id='room objs'>You also see <pushBold/>a <a exist=\"42\" noun=\"kobold\">kobold</a><popBold/>.</component>
<crtrStatus exist=\"42\" hostile=\"1\"/>
You walk north.
<prompt time=\"1002\">&gt;</prompt>
";

/// The game's answers in the two glades.
pub const IN_THE_GLADES: &[(&str, &[u8])] = &[
    ("look", QUIET_GLADE),
    ("north", NORTH_GLADE),
    (
        "target random",
        b"You are now targeting a kobold.\n<prompt time=\"1003\">&gt;</prompt>\n",
    ),
];

/// Travel's way of naming a room, cut down for a test: by the game's number.
pub fn by_number(map: &Map, state: &GameState, _: cena_map::Origin) -> Option<RoomId> {
    let uid: i64 = state.room.id.as_deref()?.parse().ok()?;
    match map.ids_for_uid(Uid(uid)) {
        [room] => Some(*room),
        _ => None,
    }
}

/// The two glades as an atlas.
pub fn two_rooms() -> Option<Atlas> {
    let rooms = serde_json::from_str(TWO_ROOMS).ok()?;
    Some(Atlas {
        map: Arc::new(Map::from_rooms(rooms).ok()?),
        locate: by_number,
        walker: Arc::new(|_, _| cena_map::Walker::default()),
    })
}

/// A running character in the Quiet Glade, its runner on the two glades'
/// map, running `scripts`. It looked before its runner started, so the
/// runner's first copy has the room.
pub async fn in_the_quiet_glade(scripts: &Path, dir: &Path) -> Option<Running> {
    Running::start(
        scripts,
        dir,
        World {
            answers: IN_THE_GLADES,
            atlas: two_rooms(),
            before: &["look"],
            walker: true,
            windows: false,
        },
    )
    .await
}
