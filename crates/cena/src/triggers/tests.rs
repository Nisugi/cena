use super::*;
use cena_platform::ReplaySource;
use cena_session::trigger::Matcher;
use cena_session::{Event, Session};

/// A data directory holding `file` as its triggers file, if any.
fn dir(test: &str, file: Option<&str>) -> io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("cena-bin-triggers-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    if let Some(file) = file {
        fs::write(triggers::path(&dir), file)?;
    }
    Ok(dir)
}

/// Everything said to the player so far, as text.
fn told(events: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
    std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            Event::Notice(notice) => Some(notice.lines().join(" ")),
            _ => None,
        })
        .collect()
}

/// What `character` is given from `file`, and what it is told.
fn opened(test: &str, file: Option<&str>, character: &str) -> io::Result<(usize, Vec<String>)> {
    let dir = dir(test, file)?;
    let session = Session::new(ReplaySource::from_bytes(b""));
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    open(&handle, &dir, character);
    let _ = fs::remove_dir_all(&dir);
    Ok((handle.triggers().triggers().len(), told(&mut events)))
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_character_is_given_its_triggers_and_told_what_was_left_out() {
    let file = "[trigger.stunned]\ntext = 'You are stunned'\nlook = { bold = true }\n\
                [trigger.bad]\ntext = 'x'\nlook = { color = 'red' }\n\
                [trigger.dicates]\ntext = 'y'\nsquelch = true\ncharacters = ['Dicate']\n";
    let (count, told) = opened("given", Some(file), "Nisugi").unwrap();
    assert_eq!(
        count, 1,
        "stunned; bad is refused and dicates is not Nisugi's"
    );
    assert_eq!(told.len(), 2, "{told:?}");
    assert!(
        told[0].contains("`bad`") && told[0].contains("left out"),
        "{told:?}"
    );
    assert_eq!(told[1], "Triggers: 1 trigger on.");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn no_file_is_no_triggers_and_nothing_said() {
    let (count, told) = opened("none", None, "Nisugi").unwrap();
    assert_eq!(count, 0);
    assert!(told.is_empty(), "{told:?}");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_file_that_is_not_toml_is_said_and_nothing_is_on() {
    let (count, told) = opened("broken", Some("[trigger.x\n"), "Nisugi").unwrap();
    assert_eq!(count, 0);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(
        told[0].contains("is not TOML") && told[0].contains("None are on"),
        "{told:?}"
    );
}

/// `;trigger` typed at a session with no file yet.
struct Typing {
    dir: PathBuf,
    /// The owner, kept so the helper borrows its handle rather than storing
    /// a second one (`single_owner.rs`).
    session: Session<ReplaySource>,
    events: tokio::sync::broadcast::Receiver<Event>,
    commands: Commands,
}

impl Typing {
    fn new(test: &str) -> io::Result<Self> {
        Self::telling(test, Changes::new())
    }

    /// The same, its changes and imports on `changes`.
    fn telling(test: &str, changes: Changes) -> io::Result<Self> {
        let dir = dir(test, None)?;
        let session = Session::new(ReplaySource::from_bytes(b""));
        let handle = session.handle();
        let (_, events) = session.subscribe();
        let commands = Commands::default();
        command(
            &handle,
            &commands,
            dir.clone(),
            "Nisugi".to_owned(),
            changes,
        );
        Ok(Self {
            dir,
            session,
            events,
            commands,
        })
    }

    /// The triggers the session answers its lines with now.
    fn triggers(&self) -> std::sync::Arc<Matcher> {
        self.session.handle().triggers()
    }

    /// Type `line` (without its symbol), and what it said.
    fn typed(&mut self, line: &str) -> Vec<String> {
        assert!(
            self.commands
                .route(line, cena_session::Origin::Manual)
                .is_some(),
            "{line} was not ours"
        );
        told(&mut self.events)
    }

    fn file(&self) -> String {
        file(&self.dir).unwrap_or_default()
    }
}

impl Drop for Typing {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// The author's loop for the hunt, for triggers: make one, change it, and
/// test it, without opening the file. Each change is on for this character
/// at once.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn add_set_and_test_without_writing_the_file() {
    let mut typing = Typing::new("loop").unwrap();
    let added = typing.typed("trigger add stunned You are stunned");
    assert!(added[0].contains("`stunned` added"), "{added:?}");
    assert!(added[0].contains("1 trigger on for Nisugi"), "{added:?}");
    assert_eq!(typing.triggers().triggers().len(), 1);

    let set = typing.typed("trigger set stunned look.color #ff4040");
    assert!(
        set[0].contains("look.color = \"#ff4040\" (was unset)"),
        "{set:?}"
    );
    let tested = typing.typed("trigger test You are stunned!");
    assert!(
        tested
            .iter()
            .any(|line| line.contains("\"You are stunned\": colour #ff4040, bold")),
        "{tested:?}"
    );
    assert!(
        typing.file().contains("[trigger.stunned]"),
        "{}",
        typing.file()
    );
}

/// A change the file cannot use is refused, and the file is as it was.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_refused_change_writes_nothing() {
    let mut typing = Typing::new("refused").unwrap();
    typing.typed("trigger add stunned You are stunned");
    let before = typing.file();
    let refused = typing.typed("trigger set stunned look.color red");
    assert!(
        refused[0].contains("not changed") && refused[0].contains("is not a colour"),
        "{refused:?}"
    );
    assert_eq!(typing.file(), before);
    assert!(
        !typing.dir.join("triggers.toml.new").exists(),
        "nothing left beside it"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn list_off_and_every() {
    let mut typing = Typing::new("list").unwrap();
    assert!(typing.typed("trigger list")[0].contains("none yet"));
    typing.typed("trigger add stunned You are stunned");
    typing.typed("trigger add spam gestures");
    typing.typed("trigger set spam category Ignores");
    typing.typed("trigger set spam squelch on");
    typing.typed("trigger off stunned");
    assert_eq!(
        typing.typed("trigger list"),
        [
            "Triggers: (no category): stunned (off)",
            "Triggers: Ignores: spam"
        ]
    );
    let every = typing.typed("trigger off every squelch");
    assert!(
        every[0].contains("every squelch off") && every[0].contains("1 trigger on"),
        "the look `add` gave spam keeps it: {every:?}"
    );
    assert!(!typing.triggers().triggers()[0].rule.squelch);
    let unknown = typing.typed("trigger frobnicate");
    assert!(unknown[0].contains("`frobnicate`"), "{unknown:?}");
}

/// `;trigger import`: a Wrayth file comes in and is on at once, what is held
/// is said, and the same file again replaces what it brought.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn import_brings_a_wrayth_file_in_and_again_replaces_it() {
    let mut typing = Typing::new("import").unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../cena-behavior/tests/fixtures/wrayth.xml");
    let said = typing.typed(&format!("trigger import \"{}\"", fixture.display()));
    assert!(
        said[0].contains(
            "wrayth.xml imported: 8 highlights (\"Wrayth strings\"), 3 names (\"Wrayth names\"), \
             2 ignores (\"Wrayth ignores\")"
        ) && said[0].contains("13 triggers on for Nisugi"),
        "{said:?}"
    );
    // The fixture's sounds point at a machine this is not: said once, with
    // where to put them.
    assert!(
        said[1].starts_with(r"Triggers: 2 sounds not found: `C:\fx\CPU unit lost.wav`, `C:\fx\data.wav`. Put them in ")
            && said[1].ends_with("sounds."),
        "{said:?}"
    );
    assert!(
        said[2].starts_with("Triggers: 2 triggers play a sound: from the path Wrayth wrote"),
        "{said:?}"
    );
    assert_eq!(typing.triggers().triggers().len(), 13);

    let again = typing.typed(&format!("trigger import {}", fixture.display()));
    assert!(
        again[0].contains("13 from an earlier import of it replaced")
            && again[0].contains("13 triggers on"),
        "{again:?}"
    );

    let missing = typing.typed(r"trigger import C:\no\such.xml");
    assert!(missing[0].contains("such.xml"), "{missing:?}");
    assert_eq!(typing.triggers().triggers().len(), 13, "nothing changed");
}

/// A file broken after a good one changes nothing: the triggers read before
/// stay on, and the reload says so, not that none are (the crate review of
/// 2026-09-28, R4).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_broken_reload_keeps_the_triggers_and_says_so() {
    let mut typing = Typing::new("broken-reload").unwrap();
    fs::write(
        triggers::path(&typing.dir),
        "[trigger.stunned]\ntext = 'You are stunned'\nlook = { bold = true }\n",
    )
    .unwrap();
    let said = typing.typed("trigger reload");
    assert!(
        said.iter().any(|said| said.ends_with("1 trigger on.")),
        "{said:?}"
    );
    fs::write(triggers::path(&typing.dir), "[trigger.x\n").unwrap();
    let said = typing.typed("trigger reload");
    assert_eq!(typing.triggers().triggers().len(), 1, "still on");
    assert!(
        said.iter()
            .any(|said| said.contains("The 1 trigger read before stays on.")),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|said| said.contains("None are on")),
        "{said:?}"
    );
    let _ = fs::remove_dir_all(&typing.dir);
}

/// Two characters' `;trigger` changes at once each keep theirs: one made
/// while another is being made waits for it (the crate review of
/// 2026-09-28, R5).
#[tokio::test(flavor = "current_thread")]
async fn a_change_made_meanwhile_waits_and_both_are_kept() {
    let mut typing = Typing::new("meanwhile").unwrap();
    let _ = typing.typed("trigger add first You are stunned");
    let path = triggers::path(&typing.dir);
    let (dir, handle) = (typing.dir.clone(), typing.session.handle());
    let other = cena_session::store::changing(&path, || {
        let old = fs::read_to_string(&path).unwrap();
        let other = std::thread::spawn(move || {
            let parsed = words::parse("trigger add second You are webbed").unwrap();
            answer(parsed, &handle, &dir, "Dicate", &Changes::new())
        });
        std::thread::sleep(std::time::Duration::from_millis(200));
        let text = edit::add(&old, "third", "You are hit").unwrap();
        fs::write(&path, text).unwrap();
        other
    });
    let said = other.join().unwrap();
    let file = typing.file();
    for name in ["first", "second", "third"] {
        assert!(edit::show(&file, name).is_ok(), "{name}: {file}\n{said:?}");
    }
}

/// Another player's triggers file, two of whose triggers send commands.
const SHARED: &str = "[trigger.stunned]
text = 'You are stunned'
send = 'stand'

                      [trigger.webbed]
text = 'webbed'
send = 'stance defensive'
origin = 'theirs'
approved = 'stance defensive'

                      [trigger.rock]
text = 'a rock'
squelch = true
";

/// A shared file written beside the test's triggers file.
fn shared(typing: &Typing) -> io::Result<PathBuf> {
    let path = typing.dir.join("Maravel.toml");
    fs::write(&path, SHARED)?;
    Ok(path)
}

/// `plan/54` step 5, with no window: another player's file comes in, what
/// made its sends theirs left behind, every command held and named.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_shared_file_with_no_window_comes_in_with_its_commands_held() {
    let mut typing = Typing::new("shared").unwrap();
    let path = shared(&typing).unwrap();
    let said = typing.typed(&format!("trigger import {}", path.display()));
    assert!(
        said[0].contains("Maravel.toml imported: 3 triggers"),
        "{said:?}"
    );
    assert!(
        said.iter()
            .any(|s| s.contains("`webbed` would send \"stance defensive\": held")),
        "their approval did not come with it: {said:?}"
    );
    let loaded = cena_behavior::triggers::read(&typing.file()).unwrap();
    assert_eq!(loaded.held.len(), 2, "both commands held");
}

/// `plan/54` step 5, with a window: nothing is written until the player
/// answers; the commands accepted are approved, the rest held; *Cancel*
/// imports nothing.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_shared_file_with_commands_waits_for_the_answer() {
    let window = cena_gui::Sessions::new(tokio::runtime::Handle::current());
    let changes = Changes::asking_in(Some(window));
    let mut typing = Typing::telling("shared-asked", changes.clone()).unwrap();
    let path = shared(&typing).unwrap();

    let said = typing.typed(&format!("trigger import {}", path.display()));
    assert!(
        said[0].contains("nothing is imported until you answer"),
        "{said:?}"
    );
    assert_eq!(typing.file(), "", "nothing written yet");

    let done = super::import::answer(&changes, 0, Some(vec!["stunned".to_owned()]));
    assert!(done.contains("1 command approved"), "{done}");
    let loaded = cena_behavior::triggers::read(&typing.file()).unwrap();
    let held: Vec<&str> = loaded.held.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(held, ["webbed"], "the one not accepted stays held");

    let said = typing.typed(&format!("trigger import {}", path.display()));
    assert!(said[0].contains("until you answer"), "{said:?}");
    let before = typing.file();
    let cancelled = super::import::answer(&changes, 1, None);
    assert!(cancelled.contains("not imported"), "{cancelled}");
    assert_eq!(typing.file(), before, "a cancel writes nothing");
    assert!(super::import::answer(&changes, 1, None).contains("already answered"));
}
