use super::*;
use cena_platform::ReplaySource;
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
    handle: SessionHandle,
    events: tokio::sync::broadcast::Receiver<Event>,
    commands: Commands,
}

impl Typing {
    fn new(test: &str) -> io::Result<Self> {
        let dir = dir(test, None)?;
        let session = Session::new(ReplaySource::from_bytes(b""));
        let handle = session.handle();
        let (_, events) = session.subscribe();
        let commands = Commands::default();
        command(&handle, &commands, dir.clone(), "Nisugi".to_owned());
        Ok(Self {
            dir,
            handle,
            events,
            commands,
        })
    }

    /// Type `line` (without its symbol), and what it said.
    fn typed(&mut self, line: &str) -> Vec<String> {
        assert!(self.commands.route(line).is_some(), "{line} was not ours");
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
    assert_eq!(typing.handle.triggers().triggers().len(), 1);

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
    assert!(!typing.handle.triggers().triggers()[0].rule.squelch);
    let unknown = typing.typed("trigger frobnicate");
    assert!(unknown[0].contains("`frobnicate`"), "{unknown:?}");
}
