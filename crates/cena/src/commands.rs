//! Hydra's own command line: every line that starts with the command symbol.
//!
//! **Installed when the session is built, before it connects** (author,
//! 2026-09-23: the symbol is "going to be for more than just travelling, so it
//! shouldn't be tied to a map, it should be something loaded on startup by
//! default"). It used to be opened by travel, and only when travel had a map
//! -- so with no map nothing claimed `;`, and `;go2 bank` was said in the room.
//!
//! The symbol decides and the word routes: a line marked as Hydra's goes to
//! the first handler that knows its word, and a word nobody knows is answered
//! "I do not know" by the session. It never reaches the game either way.
//!
//! Handlers join once the session knows enough to run them. Travel needs the
//! login's own state, so it registers after the login burst; a travel command
//! typed before then is told to wait rather than dropped silently.
//!
//! # Knowing when a command is over
//!
//! `;multi 2,get gem,;sc 401,put gem in sack` waits for `;sc 401` to finish
//! before the `put` (`crate::batch`). So a family whose commands start
//! something that goes on -- a walk, a hunt-desk run, a batch -- is a
//! [`Starter`], and hands back the task it started ([`Took::Started`]);
//! [`Commands::route`] gives that to whoever asked. Typed at the prompt, the
//! task is let go and runs on its own, as it always did.
//!
//! # Who may give which command
//!
//! A line reaches this command line from the player's typing, and also from
//! a trigger's send (`plan/45` Stage 5), a script's `put` (`plan/46`) and
//! the player's Lich (`plan/51`), each told apart by its [`Origin`]. The
//! author's design lets those run Hydra's commands as the player would
//! (`.go2`, `.hunt`, `.multi`, `.sorter`, `.sc`, a script by name), and
//! they still do. **One table refuses the rest**, [`PLAYERS_OWN`]: the
//! commands that decide what may act on the character -- `agent` (its level
//! and approvals), `trigger` (approving and changing a send), `lich` (start
//! it), and the relay's `to` and `all` (another character's typing) -- run
//! only for [`Origin::Manual`], the player's own typing. Anything else that
//! names one is refused and said (the crate review of 2026-10-01: BI-D-1,
//! BI-D-2, BI-B-2). Before, a trigger whose send held another player's
//! words, or any Lich script, could raise the agent to `takeover`.
//!
//! A `.multi` or `.foreach` runs each of its own Hydra commands with the
//! origin of the line that started it, so a batch is no way round the
//! table: `.multi 1;look;.all stand` from a trigger stops at the `.all`.

use cena_session::command::claimant::{Claimed, Desk, Runner};
use cena_session::{Notice, NoticeKind, Origin, SessionHandle};
use std::sync::{Arc, OnceLock};

/// A family of commands: `Some` when the line was its own, `None` when the
/// word is not one it knows.
pub(crate) type Handler = Arc<dyn Fn(&str) -> Option<Claimed> + Send + Sync>;

/// A family whose commands may start something that goes on: `Some` when
/// the line was its own, with the task when it started one.
pub(crate) type Starter = Arc<dyn Fn(&str) -> Option<Took> + Send + Sync>;

/// `;multi` and `;foreach`: a [`Starter`] told who sent the line, since the
/// commands a batch runs are run as that sender's.
pub(crate) type Batch = Arc<dyn Fn(&str, Origin) -> Option<Took> + Send + Sync>;

/// The words only the player's own typing may give: the commands that decide
/// what may act on the character. See the module docs.
pub(crate) const PLAYERS_OWN: &[&str] = &["agent", "trigger", "triggers", "lich", "to", "all"];

/// The word of `line` (without its symbol) when it is one of [`PLAYERS_OWN`]
/// and `origin` is not the player's typing.
pub(crate) fn refused(line: &str, origin: Origin) -> Option<&'static str> {
    if origin == Origin::Manual {
        return None;
    }
    let word = line.split_whitespace().next()?;
    PLAYERS_OWN
        .iter()
        .copied()
        .find(|own| word.eq_ignore_ascii_case(own))
}

/// Who sent a line, as the player is told it.
fn sender(origin: Origin) -> &'static str {
    match origin {
        Origin::Trigger => "a trigger",
        Origin::Script => "a script",
        Origin::Lich => "your Lich",
        Origin::Agent(_) => "an agent",
        Origin::Manual | Origin::Behavior(_) | Origin::Hydra => "Hydra",
    }
}

/// How a family stops what it started, for `;stop`: `true` when something
/// was running.
pub(crate) type Stopper = Arc<dyn Fn() -> bool + Send + Sync>;

/// What a family did with a line of its own.
pub(crate) enum Took {
    /// Done, or answered, by the time it returned.
    Done,
    /// Started something that is over when this task is.
    Started(tokio::task::JoinHandle<()>),
    /// Started something that goes on -- a hunt, a walk, a batch -- over
    /// when this task is, and stopped, it and nothing else, by the stopper.
    /// A `;multi` that is stopped stops what it started (the review of
    /// 2026-09-29: the batch let go of it and it kept sending).
    Stoppable(tokio::task::JoinHandle<()>, Stopper),
    /// Not run: its word, one of [`PLAYERS_OWN`], is only the player's to
    /// type, and someone else sent it.
    Refused(&'static str),
}

/// The controls of a run a family started, put here once the run has them
/// (it subscribes to the session first), and a stop asked for before then
/// kept and made as they arrive.
#[derive(Clone, Default)]
pub(crate) struct Controls(
    Arc<std::sync::Mutex<(Option<cena_behavior::operation::Steering>, bool)>>,
);

impl Controls {
    /// The run has begun: these stop it.
    pub(crate) fn set(&self, steering: cena_behavior::operation::Steering) {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if held.1 {
            steering.stop();
        }
        held.0 = Some(steering);
    }

    /// What stops the run, whenever it is asked: `true` when it had begun.
    pub(crate) fn stopper(&self) -> Stopper {
        let held = Arc::clone(&self.0);
        Arc::new(move || {
            let mut held = held
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            held.1 = true;
            held.0
                .as_ref()
                .map(cena_behavior::operation::Steering::stop)
                .is_some()
        })
    }
}

/// What `;help` says: every family, and the word that says more of it
/// (`plan/44` Q04). Answered before any family is ready, since it needs
/// nothing but this list.
pub(crate) const HELP: &[&str] = &[
    "Hydra's commands start with your command symbol: . unless you changed it. None of them reaches the game.",
    "hunt help        hunt on a profile, see and change its settings, lead a group, stop",
    "heal help        heal with herbs: name the herb container, heal, stock it",
    "waggle help      cast a list of spells on people",
    "keep list        spells kept up: keep add <spell>, then keep; hunt stop ends it",
    "sc help          cast one spell as set up: sc 401; beside a hunt, never in its place",
    "trigger help     triggers: what they watch and what they do; the Triggers window edits them",
    "go2 help         walk to a place or a room, save places, stop",
    "loot help        loot the room, sell, the locksmith pool, the bank; loot summary and the other reports",
    "combat           reports on the fights recorded: combat hunts, combat attacks",
    "history help     read back what you saw: history tail, history last 15m, history search <text>",
    "sorter           show a container's contents one line per category: sorter on, off or status",
    "targetid         a tag after each creature's name, tk <tag> and the rest as the script's: targetid on, off, status, slot or health",
    "doll import <folder>   your VellumFE injury dolls into Hydra, each calibration kept in its picture",
    "theme help       themes: list them, this character's own and its accent, a Wrayth file's presets as one",
    "multi help, foreach help   run commands several times, or once for each item",
    "agent help       what an agent (a program such as Claude Code) may do with this character",
    "lich help        run your own Lich for this character, and keep it on",
    "stop             stop everything Hydra is doing on this character: a hunt, a walk, a batch",
    "keys import <file>   in a play window: a Wrayth key set into this character's keys (keys import global <file>: every character's)",
    "to <name> <command>, all <command>   send a command on another character, or on every one; all -Dicate,Maravel <command> leaves some out, all +Nisugi,Dicate <command> sends on only those",
    "<script> [args]  run one of your Lich scripts; k, l, p, u as in Lich. scripts: where they are; scripts import, scripts check",
];

/// Whether a line, without its symbol, asks for [`HELP`].
fn asks_for_help(line: &str) -> bool {
    let line = line.trim();
    line.eq_ignore_ascii_case("help") || line == "?"
}

/// Whether a line, without its symbol, is `;stop`: everything Hydra is doing
/// on this character. The play window's Stop sends it (`plan/47` step 4).
fn asks_to_stop(line: &str) -> bool {
    line.trim().eq_ignore_ascii_case("stop")
}

/// The handlers the command line routes to, filled as each becomes ready.
#[derive(Clone, Default)]
pub(crate) struct Commands {
    travel: Arc<OnceLock<Starter>>,
    hunt: Arc<OnceLock<Starter>>,
    loot: Arc<OnceLock<Handler>>,
    combat: Arc<OnceLock<Handler>>,
    history: Arc<OnceLock<Handler>>,
    /// The injury doll's import (`crate::doll`).
    doll: Arc<OnceLock<Handler>>,
    /// The themes (`crate::theme_command`).
    theme: Arc<OnceLock<Handler>>,
    sorter: Arc<OnceLock<Handler>>,
    /// Creature tags (`crate::targetid`).
    targetid: Arc<OnceLock<Handler>>,
    trigger: Arc<OnceLock<Handler>>,
    batch: Arc<OnceLock<Batch>>,
    agent: Arc<OnceLock<Handler>>,
    /// The player's own Lich (`crate::lich`).
    lich: Arc<OnceLock<Handler>>,
    relay: Arc<OnceLock<Starter>>,
    /// The player's own scripts (`crate::scripts`): heard last of all.
    scripts: Arc<OnceLock<Handler>>,
    /// What `;stop` stops: each family that starts something that goes on,
    /// by the word the player knows it by.
    stoppers: Arc<std::sync::Mutex<Vec<(&'static str, Stopper)>>>,
}

/// A family's handler, set once: a second registration is said and the
/// first kept. The one body of the thirteen methods below (the review of
/// 2026-09-29).
fn once<T>(slot: &OnceLock<T>, family: &str, handler: T) {
    if slot.set(handler).is_err() {
        eprintln!("  !! [commands] {family} was registered twice; keeping the first");
    }
}

impl Commands {
    /// Put the command line on `handle`, with the default symbol until the
    /// character's own is known (`SessionHandle::set_command_symbol`).
    pub(crate) fn install(handle: &SessionHandle) -> Self {
        let commands = Self::default();
        let routes = commands.clone();
        let told = handle.clone();
        let runner: Runner = Arc::new(move |line: &str, origin: Origin| {
            if asks_for_help(line) {
                let lines = HELP.iter().map(|&line| line.to_owned()).collect();
                told.say(Notice::table(NoticeKind::Info, lines).answering());
                return Claimed::Done;
            }
            if asks_to_stop(line) {
                let stopped = routes.stop();
                told.say(
                    Notice::line(
                        NoticeKind::Info,
                        if stopped.is_empty() {
                            "Nothing was running.".to_owned()
                        } else {
                            format!("Stopped: {}.", stopped.join(", "))
                        },
                    )
                    .answering(),
                );
                return Claimed::Done;
            }
            // A task it started runs on by itself: nobody typing waits.
            match routes.route(line, origin) {
                Some(Took::Refused(word)) => {
                    let symbol = told
                        .command_symbol()
                        .unwrap_or(cena_session::command::claimant::DEFAULT_SYMBOL);
                    told.say(Notice::line(
                        NoticeKind::Warn,
                        format!(
                            "`{symbol}{}` was not run: {symbol}{word} is only for you to type, and {} sent it.",
                            line.trim(),
                            sender(origin),
                        ),
                    ));
                    return Claimed::Done;
                }
                Some(_) => return Claimed::Done,
                None => {}
            }
            let starting = if cena_behavior::travel::parse_command(line).is_some() {
                Some("Travel")
            } else if cena_behavior::hunt::parse_command(line).is_some() {
                Some("Hunt")
            } else if line.split_whitespace().next() == Some("agent") {
                Some("Agent")
            } else if line.split_whitespace().next() == Some("lich") {
                Some("Lich")
            } else {
                None
            };
            if let Some(family) = starting {
                told.say(Notice::line(
                    NoticeKind::Warn,
                    format!(
                        "{family} is still starting; nothing was sent. Try again once logged in."
                    ),
                ).answering());
                return Claimed::Done;
            }
            // The player's own scripts come last: a word of Hydra's, or of a
            // family still starting, is never a script's (`plan/46` §10,
            // question 5).
            if let Some(scripts) = routes.scripts.get()
                && scripts(line).is_some()
            {
                return Claimed::Done;
            }
            Claimed::Unknown
        });
        if !handle.set_desk(Desk::new(None, runner)) {
            eprintln!("  !! [commands] something already runs this session's commands");
        }
        commands
    }

    /// The family that knows `line` (without its symbol) runs it, and says
    /// what it did; `None` when no family knows the word. Each family
    /// answers `Some` for its own words and `None` for the rest, so the
    /// first to answer has the line.
    ///
    /// `origin` sent it: a word of [`PLAYERS_OWN`] from anyone but the
    /// player is [`Took::Refused`], before any family sees it -- **the one
    /// place that refuses**, for a typed line and a batch's alike.
    pub(crate) fn route(&self, line: &str, origin: Origin) -> Option<Took> {
        if let Some(word) = refused(line, origin) {
            return Some(Took::Refused(word));
        }
        for family in [&self.travel, &self.hunt] {
            if let Some(starter) = family.get()
                && let Some(took) = starter(line)
            {
                return Some(took);
            }
        }
        if let Some(batch) = self.batch.get()
            && let Some(took) = batch(line, origin)
        {
            return Some(took);
        }
        if let Some(relay) = self.relay.get()
            && let Some(took) = relay(line)
        {
            return Some(took);
        }
        for family in [
            &self.loot,
            &self.combat,
            &self.history,
            &self.doll,
            &self.theme,
            &self.sorter,
            &self.targetid,
            &self.trigger,
            &self.agent,
            &self.lich,
        ] {
            if let Some(handler) = family.get()
                && handler(line).is_some()
            {
                return Some(Took::Done);
            }
        }
        None
    }

    /// `;stop` stops `name`'s work with `stopper` from now on.
    pub(crate) fn stops(&self, name: &'static str, stopper: Stopper) {
        self.stoppers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((name, stopper));
    }

    /// Stop every family's work; the names of those that had some.
    fn stop(&self) -> Vec<&'static str> {
        self.stoppers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|(_, stopper)| stopper())
            .map(|(name, _)| *name)
            .collect()
    }

    /// Route travel's words to `handler` from now on. Once: a second call is
    /// ignored, as the session's own `set_desk` is.
    pub(crate) fn travel(&self, handler: Starter) {
        once(&self.travel, "travel", handler);
    }

    /// Route hunt's words to `handler` from now on. Once, as for travel.
    pub(crate) fn hunt(&self, handler: Starter) {
        once(&self.hunt, "hunt", handler);
    }

    /// Route `;loot` to `handler` from now on. Once, as for travel.
    pub(crate) fn loot(&self, handler: Handler) {
        once(&self.loot, "loot", handler);
    }

    /// Route `;combat` to `handler` from now on. Once, as for travel.
    pub(crate) fn combat(&self, handler: Handler) {
        once(&self.combat, "combat", handler);
    }

    /// Route `;history` to `handler` from now on. Once, as for travel.
    pub(crate) fn history(&self, handler: Handler) {
        once(&self.history, "history", handler);
    }

    /// Route `;doll` to `handler` from now on. Once, as for travel.
    pub(crate) fn doll(&self, handler: Handler) {
        once(&self.doll, "doll", handler);
    }

    /// Route `;theme` to `handler` from now on. Once, as for travel.
    pub(crate) fn theme(&self, handler: Handler) {
        once(&self.theme, "theme", handler);
    }

    /// Route `;sorter` to `handler` from now on. Once, as for travel.
    pub(crate) fn sorter(&self, handler: Handler) {
        once(&self.sorter, "sorter", handler);
    }

    /// Route `.targetid` to `handler` from now on. Once, as for travel.
    pub(crate) fn targetid(&self, handler: Handler) {
        once(&self.targetid, "targetid", handler);
    }

    /// Route `;agent` to `handler` from now on. Once, as for travel.
    pub(crate) fn agent(&self, handler: Handler) {
        once(&self.agent, "agent", handler);
    }

    /// Route `;lich` to `handler` from now on. Once, as for travel.
    pub(crate) fn lich(&self, handler: Handler) {
        once(&self.lich, "lich", handler);
    }

    /// Route `;trigger` to `handler` from now on. Once, as for travel.
    pub(crate) fn trigger(&self, handler: Handler) {
        once(&self.trigger, "trigger", handler);
    }

    /// Route `;to` and `;all` to `handler` from now on. Once, as for travel.
    pub(crate) fn relay(&self, handler: Starter) {
        once(&self.relay, "relay", handler);
    }

    /// Route the player's scripts to `handler` from now on, after every
    /// other word. Once, as for travel.
    pub(crate) fn scripts(&self, handler: Handler) {
        once(&self.scripts, "scripts", handler);
    }

    /// Route `;multi` and `;foreach` to `handler` from now on. Once, as for
    /// travel.
    pub(crate) fn batch(&self, handler: Batch) {
        once(&self.batch, "batch", handler);
    }
}

/// Stand-ins for the families of [`PLAYERS_OWN`], each keeping the line it
/// ran, for the tests of who may reach them.
#[cfg(test)]
pub(crate) fn stand_ins(commands: &Commands) -> Arc<std::sync::Mutex<Vec<String>>> {
    /// `Some` when `line`'s word is one of `words`, kept in `ran`.
    fn keeps(line: &str, words: &[&str], ran: &std::sync::Mutex<Vec<String>>) -> Option<()> {
        let word = line.split_whitespace().next()?;
        words.iter().any(|w| word.eq_ignore_ascii_case(w)).then(|| {
            ran.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(line.to_owned());
        })
    }
    let ran = Arc::new(std::sync::Mutex::new(Vec::new()));
    let families: [(&str, &'static [&'static str]); 3] = [
        ("agent", &["agent"]),
        ("trigger", &["trigger", "triggers"]),
        ("lich", &["lich"]),
    ];
    for (family, words) in families {
        let ran = Arc::clone(&ran);
        let handler: Handler =
            Arc::new(move |line: &str| keeps(line, words, &ran).map(|()| Claimed::Done));
        match family {
            "agent" => commands.agent(handler),
            "trigger" => commands.trigger(handler),
            _ => commands.lich(handler),
        }
    }
    let relayed = Arc::clone(&ran);
    commands.relay(Arc::new(move |line: &str| {
        keeps(line, &["to", "all"], &relayed).map(|()| Took::Done)
    }));
    ran
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_platform::AnsweringSource;
    use cena_session::{Event, Outcome, Session};
    use std::time::Duration;

    const DEADLINE: Duration = Duration::from_secs(5);

    /// What the player was told, as text.
    fn told(events: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
        let mut said = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let Event::Notice(notice) = event {
                said.push(format!("{:?}", notice.body));
            }
        }
        said
    }

    /// The author's live run: `;go2 bank` reached the game because nothing
    /// had claimed `;`. Installed at startup, it cannot: before travel is
    /// ready, after it, and for a word nobody knows.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn nothing_marked_as_hydras_reaches_the_game() {
        let (source, transcript) = AnsweringSource::new(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        let generation = handle.generation();
        let commands = Commands::install(&handle);
        tokio::spawn(session.into_actor().run());

        // Before travel registers: its word is told to wait.
        let early = handle
            .send_manual_at(generation, ".go2 bank", DEADLINE)
            .await;
        assert_eq!(early, Outcome::Handled);
        assert!(
            told(&mut events)
                .iter()
                .any(|s| s.contains("still starting")),
            "a travel command before travel is ready is answered"
        );

        // A word nobody knows, before and after.
        assert_eq!(
            handle.send_manual_at(generation, ".nosuch", DEADLINE).await,
            Outcome::Handled
        );
        assert!(
            told(&mut events).iter().any(|s| s.contains(".help")),
            "a word nobody knows points at .help"
        );

        // Help, before anything else is ready, and nothing sent.
        assert_eq!(
            handle.send_manual_at(generation, ".help", DEADLINE).await,
            Outcome::Handled
        );
        assert!(
            told(&mut events).iter().any(|s| s.contains("hunt help")),
            ".help lists the families"
        );

        commands.travel(Arc::new(|line: &str| {
            line.starts_with("go2").then_some(Took::Done)
        }));
        assert_eq!(
            handle
                .send_manual_at(generation, ".go2 bank", DEADLINE)
                .await,
            Outcome::Handled
        );
        assert_eq!(
            handle.send_manual_at(generation, ".nosuch", DEADLINE).await,
            Outcome::Handled
        );

        // The game's own lines still go to the game.
        handle.send_manual_at(generation, "look", DEADLINE).await;
        assert_eq!(
            transcript.lines(),
            ["look"],
            "only the game's line was sent"
        );
    }

    /// The one table: the player's typing may give every command; no one
    /// else may give the player's own, and everyone may give the rest.
    #[test]
    fn only_the_players_typing_gives_the_players_own_commands() {
        for line in [
            "agent level takeover",
            "AGENT approve 3",
            "trigger approve theirs",
            "triggers set x send look",
            "lich on",
            "to Baelor look",
            "all stand",
            "all -Dicate stand",
        ] {
            assert_eq!(refused(line, Origin::Manual), None, "{line}");
            for origin in [Origin::Trigger, Origin::Script, Origin::Lich] {
                assert!(refused(line, origin).is_some(), "{line} from {origin:?}");
            }
        }
        for line in [
            "go2 bank",
            "hunt stop",
            "multi 2,look",
            "sorter on",
            "alls",
            "together",
        ] {
            assert_eq!(refused(line, Origin::Trigger), None, "{line}");
        }
    }

    /// A script's line and every line the player's Lich writes reach Hydra's
    /// command line as theirs: refused the player's own commands, and given
    /// the rest (the crate review of 2026-10-01, BI-B-2).
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_script_and_lich_never_give_the_players_own_commands() {
        let (source, transcript) =
            AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        let commands = Commands::install(&handle);
        let ran = stand_ins(&commands);
        let walked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let walks = Arc::clone(&walked);
        commands.travel(Arc::new(move |line: &str| {
            line.starts_with("go2").then(|| {
                walks.lock().unwrap().push(line.to_owned());
                Took::Done
            })
        }));
        tokio::spawn(session.into_actor().run());
        let script = handle.script_door();
        let lich = handle.lich_door();
        for line in [
            ".agent level takeover",
            ".all .agent level takeover",
            ".lich off",
        ] {
            script.send(line).await;
            for from in [
                cena_session::script::lich::LineFrom::Lich,
                cena_session::script::lich::LineFrom::Player,
            ] {
                lich.send(line, from).await;
            }
        }
        assert!(ran.lock().unwrap().is_empty(), "{:?}", ran.lock().unwrap());
        assert!(
            told(&mut events)
                .iter()
                .any(|said| said.contains("a script sent it")),
            "the refusal is said"
        );
        script.send(".go2 bank").await;
        lich.send(".go2 bank", cena_session::script::lich::LineFrom::Lich)
            .await;
        assert_eq!(*walked.lock().unwrap(), ["go2 bank", "go2 bank"]);
        assert!(transcript.lines().is_empty(), "{:?}", transcript.lines());
    }

    /// `;stop` stops every family that has something running, says which,
    /// and says so when nothing was; the game hears none of it.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn stop_stops_whatever_is_running_and_says_what() {
        let (source, transcript) = AnsweringSource::new(
            b"<prompt time=\"1\">&gt;</prompt>
",
        );
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        let generation = handle.generation();
        let commands = Commands::install(&handle);
        tokio::spawn(session.into_actor().run());
        let hunting = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let running = Arc::clone(&hunting);
        commands.stops(
            "hunt",
            Arc::new(move || running.swap(false, std::sync::atomic::Ordering::Relaxed)),
        );
        commands.stops("go2", Arc::new(|| false));

        for expected in ["Stopped: hunt.", "Nothing was running."] {
            assert_eq!(
                handle.send_manual_at(generation, ".stop", DEADLINE).await,
                Outcome::Handled
            );
            let said = told(&mut events);
            assert!(said.iter().any(|s| s.contains(expected)), "{said:?}");
        }
        assert!(transcript.lines().is_empty(), "nothing reached the game");
    }
}
