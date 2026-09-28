//! The Ruby runner end to end (`plan/46` §11 step 1): Hydra writes out and
//! starts Lich's own engine in Ruby; the player's command starts a Lich
//! script there; its line reaches the game as a script's, the game's answer
//! reaches the script, and what it says reaches the player, echoed as Lich
//! echoes it.
//!
//! **It needs Ruby 4.0**, which Lich's engine needs: on the `PATH` or where
//! Lich's Windows installer puts it. Without one this fails, saying so,
//! rather than passing over nothing.

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
fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-runner-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The world a test's character plays in.
#[derive(Default)]
struct World<'a> {
    /// The scripted game's answers.
    answers: &'a [(&'a str, &'a [u8])],
    /// The map its runner places it on.
    atlas: Option<Atlas>,
    /// What the player typed before the runner started.
    before: &'a [&'a str],
    /// Whether Hydra's travel is stood in for (`walker`).
    walker: bool,
}

/// How many `go2 far` walks were started, and how many stopped.
#[derive(Default)]
struct Walks {
    started: std::sync::atomic::AtomicUsize,
    stopped: std::sync::atomic::AtomicUsize,
}

impl Walks {
    fn counts(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering::SeqCst;
        (self.started.load(SeqCst), self.stopped.load(SeqCst))
    }
}

/// A stand-in for Hydra's travel, the binary's performer: `go2 229` walks
/// north and arrives; `go2 far` walks until it is stopped, counted in
/// `walks`; `go2 slow` too, taking 300 ms to start, so a script can be
/// killed before its run's number comes back.
fn walker(
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
struct Running {
    handle: cena_session::SessionHandle,
    transcript: TranscriptHandle,
    /// The stand-in travel's far walks, started and stopped.
    walks: Arc<Walks>,
    legacy: Receiver<Event>,
    runners: Runners,
    token: String,
    child: tokio::process::Child,
    errors: Arc<Mutex<Vec<String>>>,
    stop: CancellationToken,
}

/// What the player was told and shown and what went out, until a line told
/// contains `until`, or a minute passed.
struct Heard {
    told: Vec<String>,
    shown: Vec<String>,
    sent: Vec<(String, Origin)>,
    finished: bool,
}

impl Running {
    /// The character plays in `world`; its runner runs the scripts in
    /// `scripts`, with `dir` for the rest.
    async fn start(scripts: &Path, dir: &Path, world: World<'_>) -> Option<Self> {
        let World {
            answers,
            atlas,
            before,
            walker: walking,
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
    fn typed(&self, line: &str) -> bool {
        self.runners.typed(&self.token, line)
    }

    async fn heard_until(&mut self, until: &str) -> Heard {
        self.heard_until_either(until, until).await
    }

    /// Run `;line` and hear it through to `name has exited`. Lich says a
    /// script has exited a moment before it leaves the running list, so a
    /// second run of the same script typed at once can be told it is still
    /// running: that answer is waited out, as a player would type again.
    /// `None` when it never stopped running.
    async fn run(&mut self, line: &str, name: &str) -> Option<Heard> {
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

    async fn heard_until_either(&mut self, until: &str, or: &str) -> Heard {
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
    async fn typed_line(&self, line: &str) -> cena_session::Outcome {
        self.handle
            .send_typed_at(self.handle.generation(), line, Duration::from_secs(5))
            .await
    }

    /// The next `count` lines the player is shown, or fewer after ten
    /// seconds.
    async fn shown(&mut self, count: usize) -> Vec<String> {
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

    fn errors(&self) -> Vec<String> {
        self.errors.lock().map(|e| e.clone()).unwrap_or_default()
    }

    async fn end(mut self) {
        let _ = self.child.kill().await;
        self.runners.dismiss(&self.token);
        self.stop.cancel();
    }
}

/// The game's answer to `look`: a line, and the prompt.
const QUIET_ROOM: &[u8] = b"You see a quiet room.\n<prompt time=\"2\">&gt;</prompt>\n";

/// A room the game describes whole: its number, title and exits; a player,
/// a kobold and a rock; the hands, a bar and a status.
const DESCRIBED_ROOM: &[u8] = b"<nav rm='7000'/>
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
fn scripts_with(dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let scripts = dir.join("scripts");
    std::fs::create_dir_all(&scripts)?;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::copy(fixture, scripts.join(name))?;
    Ok(scripts)
}

#[tokio::test(flavor = "current_thread")]
async fn a_lich_script_runs_against_hydra() {
    assert!(
        find_ruby().is_some(),
        "no Ruby: the runner needs Ruby 4.0, on the PATH or under C:\\Ruby4Lich5 (Lich's installer)"
    );
    let dir = temp_dir("lich-script");
    let scripts = scripts_with(&dir, "hydratest.lic").unwrap();
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: &[("look", QUIET_ROOM)],
            ..World::default()
        },
    )
    .await
    .unwrap();

    assert!(running.typed("hydratest one \"two three\""));
    let heard = running.heard_until("hydratest has exited").await;
    let errors = running.errors();
    assert!(
        heard.finished,
        "the script did not finish.\ntold: {:#?}\nrunner's errors: {errors:#?}",
        heard.told
    );
    for expected in [
        "--- Lich: hydratest active.",
        "[hydratest: args: one|two three]",
        "[hydratest]>look",
        "[hydratest: heard: You see a quiet room.]",
        "a table",
        "  row one",
        "--- Lich: hydratest has exited.",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    assert_eq!(heard.sent, [("look".to_owned(), Origin::Script)]);
    assert_eq!(running.transcript.lines(), ["look"]);
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The reads** (`plan/46` §11 step 2): Lich's own `XMLData` readers,
/// `GameObj`, `Char` and the check family, answered from the local copy
/// that the game's description of the room filled.
#[tokio::test(flavor = "current_thread")]
async fn a_script_reads_its_character_as_lich_does() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("reads");
    let scripts = scripts_with(&dir, "readstest.lic").unwrap();
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: &[("look", DESCRIBED_ROOM)],
            ..World::default()
        },
    )
    .await
    .unwrap();
    let heard = running.run("readstest", "readstest").await.unwrap();
    let errors = running.errors();
    for expected in [
        "[readstest: room: [Quiet Glade] 7000 [\"n\", \"e\"] true]",
        "[readstest: npcs: kobold loot: rock pcs: [\"Kiyna\"]]",
        "[readstest: hands: sword nil]",
        "[readstest: char: Nisugi 50/100 standing=true stunned=false]",
        "[readstest: spell: Heroism circle=2 known=true active=true checkspell=true]",
        "[readstest: costs: mana=15.0 on another=1.0 minutes affordable=true]",
        "[readstest: others: false 215 nil]",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two rooms of a map: the Quiet Glade (228, the game's 7000), and the North
/// Glade (229, the game's 7001) north of it.
const TWO_ROOMS: &str = r#"[
    {"id":228,"uid":[7000],"title":["[Quiet Glade]"],"exits":[{"to":229,"kind":"cardinal","cmd":"north","cost":0.2}]},
    {"id":229,"uid":[7001],"title":["[North Glade]"],"exits":[{"to":228,"kind":"cardinal","cmd":"south","cost":0.2}]}
]"#;

/// The Quiet Glade, as `look` shows it.
const QUIET_GLADE: &[u8] = b"<nav rm='7000'/>
<streamWindow id='room' title='Room' subtitle=\" - Quiet Glade\"/>
<compass><dir value=\"n\"/></compass>
<component id='room players'></component>
<component id='room objs'></component>
You see a quiet glade.
<prompt time=\"1001\">&gt;</prompt>
";

/// The North Glade, walked into, with a kobold in it.
const NORTH_GLADE: &[u8] = b"<nav rm='7001'/>
<streamWindow id='room' title='Room' subtitle=\" - North Glade\"/>
<compass><dir value=\"s\"/></compass>
<component id='room players'></component>
<component id='room objs'>You also see <pushBold/>a <a exist=\"42\" noun=\"kobold\">kobold</a><popBold/>.</component>
<crtrStatus exist=\"42\" hostile=\"1\"/>
You walk north.
<prompt time=\"1002\">&gt;</prompt>
";

/// The game's answers in the two glades.
const IN_THE_GLADES: &[(&str, &[u8])] = &[
    ("look", QUIET_GLADE),
    ("north", NORTH_GLADE),
    (
        "target random",
        b"You are now targeting a kobold.\n<prompt time=\"1003\">&gt;</prompt>\n",
    ),
];

/// Travel's way of naming a room, cut down for a test: by the game's number.
fn by_number(map: &Map, state: &GameState, _: cena_map::Origin) -> Option<RoomId> {
    let uid: i64 = state.room.id.as_deref()?.parse().ok()?;
    match map.ids_for_uid(Uid(uid)) {
        [room] => Some(*room),
        _ => None,
    }
}

/// The two glades as an atlas.
fn two_rooms() -> Option<Atlas> {
    let rooms = serde_json::from_str(TWO_ROOMS).ok()?;
    Some(Atlas {
        map: Arc::new(Map::from_rooms(rooms).ok()?),
        locate: by_number,
    })
}

/// A running character in the Quiet Glade, its runner on the two glades'
/// map, running `scripts`. It looked before its runner started, so the
/// runner's first copy has the room.
async fn in_the_quiet_glade(scripts: &Path, dir: &Path) -> Option<Running> {
    Running::start(
        scripts,
        dir,
        World {
            answers: IN_THE_GLADES,
            atlas: two_rooms(),
            before: &["look"],
            walker: true,
        },
    )
    .await
}

/// **The map and the stores** (`plan/46` §11 step 2): `Room.current` named
/// by Hydra, its `wayto` walked with Lich's own `move`, and `CharSettings`
/// kept in `lich.db3` from one runner to the next: the second runner is a
/// new process, so what it finds can only have come from the file.
#[tokio::test(flavor = "current_thread")]
async fn a_script_walks_the_map_and_keeps_its_settings() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("walk");
    let scripts = scripts_with(&dir, "walktest.lic").unwrap();
    let mut running = in_the_quiet_glade(&scripts, &dir).await.unwrap();
    let heard = running.run("walktest", "walktest").await.unwrap();
    let errors = running.errors();
    for expected in [
        "[walktest: from 228 [Quiet Glade] north to 229]",
        "[walktest: moved true, now 229 [North Glade]]",
        "[walktest: seen [229]]",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    assert_eq!(heard.sent, [("north".to_owned(), Origin::Script)]);
    running.end().await;

    let mut running = in_the_quiet_glade(&scripts, &dir).await.unwrap();
    let heard = running.run("walktest", "walktest").await.unwrap();
    assert!(
        heard
            .told
            .iter()
            .any(|line| line == "[walktest: seen [229, 229]]"),
        "the first runner's setting, from lich.db3: {:#?}
runner's errors: {:#?}",
        heard.told,
        running.errors()
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Hydra's built-ins, started as Lich's scripts** (`plan/46` §11 step 3):
/// `Script.run('go2', ...)` walks by Hydra's travel and waits, Lich's go2
/// settings left behind, and the room it arrived in is read after it;
/// `start_script('go2', ...)` is seen running by Lich's own `running?` and
/// `exists?`, and killing it stops travel's walk.
#[tokio::test(flavor = "current_thread")]
async fn a_script_runs_go2_as_hydras_travel() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("builtins");
    let scripts = scripts_with(&dir, "builtintest.lic").unwrap();
    let mut running = in_the_quiet_glade(&scripts, &dir).await.unwrap();
    let heard = running.run("builtintest", "builtintest").await.unwrap();
    let errors = running.errors();
    for expected in [
        "[builtintest: walked true: now 229]",
        "[builtintest: far: running=true exists=true]",
        "[builtintest: stopped: running=false]",
        "[builtintest: stopped at once]",
        "[builtintest: stopped while starting]",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    // Every far walk a killed script started is stopped: the one it waited
    // on, the one killed at once, and the slow one killed before its run's
    // number came back. Over HTTP from the runner's own thread, which may still be
    // starting the second when the script has gone: the counts must hold
    // equal for 300 ms, within ten seconds for a busy machine.
    let mut counts = running.walks.counts();
    let mut steady = 0;
    for _ in 0..500 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let now = running.walks.counts();
        steady = if now == counts && now.0 >= 2 && now.0 == now.1 {
            steady + 1
        } else {
            0
        };
        counts = now;
        if steady == 15 {
            break;
        }
    }
    assert!(
        counts.0 >= 2 && counts.0 == counts.1,
        "killing the script stops travel's walk: {counts:?} (started, stopped)"
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// A room of two lines, the second a breeze.
const BREEZY_ROOM: &[u8] =
    b"You see a quiet room.\nA breeze blows.\n<prompt time=\"3\">&gt;</prompt>\n";

/// **Hooks** (`plan/46` §11 step 4): Lich's own `DownstreamHook` and
/// `UpstreamHook`, asked by Hydra. While the script runs, the breeze is
/// hidden from the player and still heard by the script, the room shown
/// changed (its markup not drawn), and then `tt` sent as `target`; killed,
/// its hooks go with it, and the room is shown as the game sent it.
#[tokio::test(flavor = "current_thread")]
async fn a_scripts_hooks_change_what_is_shown_and_typed() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("hooks");
    let scripts = scripts_with(&dir, "hooktest.lic").unwrap();
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: &[
                (
                    "glance",
                    b"You glance about.\n<prompt time=\"2\">&gt;</prompt>\n",
                ),
                ("look", BREEZY_ROOM),
                ("look", BREEZY_ROOM),
            ],
            // The first text builds the session's classifiers, which a debug
            // build takes past the hooks' half second over.
            before: &["glance"],
            ..World::default()
        },
    )
    .await
    .unwrap();
    assert!(running.typed("hooktest"));
    let heard = running.heard_until("[hooktest: hooked]").await;
    assert!(
        heard.finished,
        "not hooked: {:#?}\nrunner's errors: {:#?}",
        heard.told,
        running.errors()
    );

    // The display hook alone, first.
    running.typed_line("look").await;
    let heard = running.heard_until("[hooktest: typing hooked]").await;
    assert!(
        heard
            .told
            .iter()
            .any(|told| told == "[hooktest: heard the breeze]"),
        "the script hears what its hook hides: {:#?}",
        heard.told
    );
    let mut shown = heard.shown;
    if shown.is_empty() {
        shown = running.shown(1).await;
    }
    assert_eq!(
        shown,
        ["You see a loud room."],
        "runner's errors: {:#?}",
        running.errors()
    );
    running.typed_line("tt").await;
    assert_eq!(running.transcript.lines(), ["glance", "look", "target"]);

    // Its hooks go with it, told to Hydra by Lich's cleanup, which runs
    // before Lich says it was killed.
    assert!(running.typed("k hooktest"));
    let killed = running
        .heard_until_either("hooktest was killed", "hooktest has exited")
        .await;
    assert!(killed.told.iter().any(|told| told.contains("hooktest")));
    running.typed_line("look").await;
    assert_eq!(
        running.shown(2).await,
        ["You see a quiet room.", "A breeze blows."]
    );
    assert_eq!(
        running.transcript.lines(),
        ["glance", "look", "target", "look"]
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The second real script end to end** (`plan/46` §11 step 2): Tillmen's
/// `wander`, unchanged, from `CENA_LICH_SCRIPTS`. It walks out of the Quiet
/// Glade by the map, finds the kobold, targets it, and stops.
#[tokio::test(flavor = "current_thread")]
#[ignore = "Tier 2: needs CENA_LICH_SCRIPTS, a folder holding wander.lic; run with --ignored"]
async fn wander_runs_unchanged() {
    let scripts = PathBuf::from(
        std::env::var_os("CENA_LICH_SCRIPTS").expect("CENA_LICH_SCRIPTS names no folder"),
    );
    assert!(scripts.join("wander.lic").is_file(), "no wander.lic");
    let dir = temp_dir("wander");
    let mut running = in_the_quiet_glade(&scripts, &dir).await.unwrap();
    let heard = running.run("wander", "wander").await.unwrap();
    let errors = running.errors();
    assert!(heard.finished, "{:#?}\n{errors:#?}", heard.told);
    let sent: Vec<&str> = heard.sent.iter().map(|(line, _)| line.as_str()).collect();
    assert_eq!(
        sent,
        ["north", "target random"],
        "{:#?}\n{errors:#?}",
        heard.told
    );
    assert_eq!(
        running.transcript.lines(),
        ["look", "north", "target random"]
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The first real script end to end** (`plan/46` §11 step 1): Tillmen's
/// `trollspeak`, from the player's own Lich scripts, unchanged. It is not
/// ours to commit, so this reads it from the folder `CENA_LICH_SCRIPTS`
/// names (the author's: `reference/lich_repo_mirror/lib`).
#[tokio::test(flavor = "current_thread")]
#[ignore = "Tier 2: needs CENA_LICH_SCRIPTS, a folder holding trollspeak.lic; run with --ignored"]
async fn trollspeak_runs_unchanged() {
    let scripts = PathBuf::from(
        std::env::var_os("CENA_LICH_SCRIPTS").expect("CENA_LICH_SCRIPTS names no folder"),
    );
    assert!(
        scripts.join("trollspeak.lic").is_file(),
        "no trollspeak.lic"
    );
    let dir = temp_dir("trollspeak");
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: &[("look", QUIET_ROOM)],
            ..World::default()
        },
    )
    .await
    .unwrap();

    let heard = running
        .run("trollspeak say hello there, friend", "trollspeak")
        .await
        .unwrap();
    assert!(heard.finished, "{:#?}\n{:#?}", heard.told, running.errors());
    let [(said, Origin::Script)] = heard.sent.as_slice() else {
        panic!("{:?}", heard.sent);
    };
    assert!(
        said.starts_with("say ") && said != "say hello there, friend",
        "{said}"
    );
    assert!(
        heard
            .told
            .iter()
            .any(|line| *line == format!("[trollspeak]>{said}")),
        "{:#?}",
        heard.told
    );

    let heard = running
        .run("trollspeak echo hello", "trollspeak")
        .await
        .unwrap();
    assert!(heard.sent.is_empty(), "echo sends nothing");
    assert!(
        heard
            .told
            .iter()
            .any(|line| line.starts_with("[trollspeak: ")),
        "{:#?}",
        heard.told
    );

    let heard = running.run("trollspeak", "trollspeak").await.unwrap();
    assert!(
        heard
            .told
            .iter()
            .any(|line| line.starts_with("Usage: ;trollspeak")),
        "{:#?}",
        heard.told
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}
