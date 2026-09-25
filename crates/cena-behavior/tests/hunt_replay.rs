//! **The hunt engine driven by real wire**: Nisugi's own hunt in Ojandhaart,
//! replayed frame by frame through a session, with the engine asked at every
//! prompt what it would do, and what it says held against what Nisugi's
//! bigshot actually did on those same frames.
//!
//! `tests/hunt_engine.rs` drives every arm with frames built by hand. That
//! tests the shape remembered; this tests the shape that exists
//! (`cena-model/tests/golden_model.rs` makes the same argument), and the
//! first run found three defects the hand-built frames had passed over:
//!
//! 1. **Assume Aspect was cast before any effects list had been seen.** The
//!    maintain arm already held its other signs until a list arrived; the
//!    `650` branch ran ahead of that gate, so a hunt starting on a fresh
//!    state cast the aspect that was already up.
//! 2. **A corpse was never read as dead.** [`CreatureInstance::dead`] is
//!    hit points at zero, and this pegasus never had a `health=` sent: the
//!    room list said `dead="1"` four times while the engine saw it alive,
//!    so `loot` could never fire on a real kill.
//! 3. **`loot.delay` deferred looting for as long as any target stood**,
//!    where bigshot's `time_between(:need_to_loot?, 15)` lets the first
//!    corpse be looted at once and spaces the rest fifteen seconds apart.
//!    Nisugi's client searched the corpse two seconds after the kill with
//!    the engineer standing there.
//!
//! The author then named a newer log, from after the server began sending
//! `health=` on every hostile creature's status, and its replay found a
//! fourth:
//!
//! 4. **The author's own battle mastodon was targeted.** Its status carries
//!    health and nothing else, and the `(?:.+?)` any-creature rule took it.
//!    [`CreatureInstance::hostile`] now reads the flag once a status has
//!    been seen, and the engine leaves a creature known not to be hostile
//!    alone.
//!
//! # Provenance
//!
//! All three fixtures are cut from logs the author named for this purpose
//! on 2026-09-24 (the Ojandhaart village, so the *"nothing from
//! hinterwilds"* condition was lifted by the author for them). Raw wire, no
//! Lich line prefixes, no `vellumImg`; passed through
//! `cena_protocol::scrub::Scrubber` (`cargo run -p cena-protocol --example
//! scrub_fixture -- <path>`), which pseudonymised one passing player in the
//! third and changed nothing in the first two.
//! `cena-protocol/tests/fixtures_are_scrubbed.rs` scans them with its own.
//!
//! | Fixture | Source (`C:\Gemstone\lich-5\logs\GSIV-Nisugi\2026\09\`) | Lines | Bytes | What it holds |
//! |---|---|---|---|---|
//! | `smithy_engage.xml` | `2026-09-13_14-51-55.xml` | 574-872 | 59,220 | arriving at the Smithy, a goliath diviner already targeted, Tangleweed and Camouflage cast, three shots, the diviner riding off, the sign casts between fights |
//! | `smithy_kill.xml` | `2026-09-13_14-51-55.xml` | 873-1150 | 70,502 | inside the smithy: a pegasus fought and dropped dead, searched, the engineer taken up next |
//! | `arch_kill.xml` | `2026-09-21_21-49-41.xml` | 2700-3048 | 59,250 | the health era: a mastodon and a shield-maiden killed at the Runed Arch with health on every status, the author's own mastodon beside them, a player passing through |
//!
//! The cut starts on a room, so the state starts empty: the first prompts
//! of each fixture are ticked before the buffs, the stance and the room's
//! creatures have been stated, which a live session would already know.
//! The assertions below say so where it matters.
//!
//! [`CreatureInstance::dead`]: cena_session::CreatureInstance::dead
//! [`CreatureInstance::hostile`]: cena_session::CreatureInstance::hostile

use cena_behavior::hunt::{Here, Hunt, Profile, Said, import};
use cena_platform::ReplaySource;
use cena_session::{Event, Frame, GameState, Session, State};
use tokio::sync::broadcast::error::RecvError;

/// The goliath diviner Nisugi was fighting when the first cut begins.
const DIVINER: i64 = 242_471_597;
/// The pegasus killed in the second cut, and the engineer taken up next.
const PEGASUS: i64 = 242_512_530;
const ENGINEER: i64 = 242_508_963;

/// The game second of the prompt in `smithy_engage.xml` at which the
/// character is first known to be hidden (Camouflage had just landed).
const HIDDEN_KNOWN: u32 = 1_789_329_136;
/// The game second at which the diviner rode off and the room was empty.
const DIVINER_GONE: u32 = 1_789_329_145;
/// The game second of the prompt after "drops dead" in `smithy_kill.xml`.
const PEGASUS_DEAD: u32 = 1_789_329_159;
/// The game second at which the dropdown first showed the engineer.
const ENGINEER_TARGETED: u32 = 1_789_329_162;

/// `arch_kill.xml`: the battle mastodon and the shield-maiden Nisugi
/// killed, and Nisugi's own battle mastodon, which fought beside them.
const BATTLE_MASTODON: i64 = 407_474_348;
const SHIELD_MAIDEN: i64 = 407_446_374;
const OWN_MASTODON: i64 = 407_520_392;
/// The game seconds of the prompts after each death's `dead="1"`.
const MASTODON_DEAD: u32 = 1_790_045_828;
const SHIELD_MAIDEN_DEAD: u32 = 1_790_045_834;

fn fixture(name: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// Nisugi's profile, as `;hunt import` brings it in.
fn ojandhaart() -> Result<Profile, String> {
    let yaml = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ojandhaart.yaml"
    ))
    .map_err(|e| e.to_string())?;
    Ok(import("ojandhaart", &yaml)?.profile)
}

/// One prompt of the replay: when, what the engine said, and what the
/// state knew at that moment.
#[derive(Debug)]
struct Tick {
    now: Option<u32>,
    said: Said,
    hidden: Option<bool>,
    creatures: usize,
}

impl Tick {
    fn line(&self) -> Option<&str> {
        match &self.said {
            Said::Send { line, .. } => Some(line),
            _ => None,
        }
    }

    fn at(&self, second: u32) -> bool {
        self.now == Some(second)
    }
}

/// Replay the fixture through a session, folding every event as the hunt
/// driver does, and tick the engine at each prompt. The session's event
/// ring holds 2,048 events and each fixture yields fewer, so a lag would be
/// a broken premise, returned rather than papered over.
async fn replay(bytes: &[u8], hunt: &mut Hunt) -> Result<Vec<Tick>, RecvError> {
    let session = Session::new(ReplaySource::from_bytes(bytes));
    let (snapshot, mut events) = session.subscribe();
    let cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());
    let mut state: GameState = snapshot.state;
    let mut ticks = Vec::new();
    loop {
        match events.recv().await {
            Ok(Event::Frame(frame)) => {
                state.apply(&frame);
                if let Frame::Prompt { .. } = *frame {
                    let now = state.game_time_now();
                    let here = Here {
                        room: None,
                        exits: &[],
                        tags: &[],
                    };
                    let said = hunt.tick(&state, here, now);
                    ticks.push(Tick {
                        now,
                        said,
                        hidden: state.status.known().hidden(),
                        creatures: state.creatures().in_room().count(),
                    });
                }
            }
            Ok(Event::StateChanged(State::Closed)) | Err(RecvError::Closed) => break,
            Ok(_) => {}
            Err(lagged) => return Err(lagged),
        }
    }
    cancel.cancel();
    let _ = actor.await;
    Ok(ticks)
}

/// Every spell sent: when, the line, and how many creatures were here.
fn spell_lines(ticks: &[Tick]) -> Vec<(Option<u32>, String, usize)> {
    ticks
        .iter()
        .filter_map(|tick| {
            let line = tick.line()?;
            ["incant ", "prep ", "assume "]
                .iter()
                .any(|verb| line.starts_with(verb))
                .then(|| (tick.now, line.to_owned(), tick.creatures))
        })
        .collect()
}

/// The lines sent, each run of repeats collapsed to one.
fn distinct_lines<'a>(ticks: impl Iterator<Item = &'a Tick>) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for tick in ticks {
        if let Some(line) = tick.line()
            && lines.last().is_none_or(|last| last != line)
        {
            lines.push(line.to_owned());
        }
    }
    lines
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn nothing_is_cast_until_an_effects_list_has_been_seen() {
    let mut hunt = Hunt::new(ojandhaart().unwrap(), 1);
    let ticks = replay(&fixture("smithy_engage.xml").unwrap(), &mut hunt)
        .await
        .unwrap();
    assert_eq!(ticks.len(), 31, "one tick per prompt in the cut");
    let spells = spell_lines(&ticks);
    // Aspect of the Panther was up (Buffs, id 142262), but the list arrives
    // five prompts into the cut. Before the fix, every one of those five
    // prompts said `incant 650 evoke`.
    assert!(
        spells
            .iter()
            .all(|(now, _, _)| now.is_some_and(|now| now >= DIVINER_GONE)),
        "a sign was cast before the lists were seen, or with a target here: {spells:?}"
    );
    assert!(
        spells.iter().all(|(_, _, creatures)| *creatures == 0),
        "maintain casts only when nothing is here to fight: {spells:?}"
    );
    // 9708, 9715 and 9711 are in no list the game sent, so they are down and
    // are cast; 515, 506, 605 and 650 are in the Buffs list and are not.
    let cast: Vec<&str> = spells.iter().map(|(_, line, _)| line.as_str()).collect();
    assert_eq!(cast, ["incant 9708", "incant 9715", "incant 9711"]);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_routine_fires_from_hiding_and_skips_what_bigshot_skipped() {
    let mut hunt = Hunt::new(ojandhaart().unwrap(), 1);
    let ticks = replay(&fixture("smithy_engage.xml").unwrap(), &mut hunt)
        .await
        .unwrap();
    let routine: Vec<&Tick> = ticks
        .iter()
        .filter(|tick| {
            matches!(
                tick.said,
                Said::Send {
                    target: Some(DIVINER),
                    ..
                }
            )
        })
        .collect();
    // Routine f: kweed while Tangleweed Vigor is about to lapse, Camouflage
    // and hide while not hidden, fire while hidden. Tangleweed Vigor had
    // 1:45 left and Camouflage had just landed, so only `fire` holds --
    // which is the three `You fire` lines in the log.
    assert!(
        !routine.is_empty() && routine.iter().all(|tick| tick.line() == Some("fire")),
        "with the buff up and hidden, the routine is `fire` and nothing else: {routine:?}"
    );
    // Hidden is unknown until the game says so, and an unknown guard holds
    // the step rather than running it: nothing of the routine was sent
    // before the first prompt at which hidden was known.
    let first = routine.first().unwrap();
    assert!(
        first.at(HIDDEN_KNOWN) && first.hidden == Some(true),
        "{first:?}"
    );
    assert!(
        ticks
            .iter()
            .filter(|tick| tick.now.is_some_and(|now| now < HIDDEN_KNOWN))
            .all(|tick| tick.hidden.is_none()),
        "the premise: hidden was unknown at every earlier prompt"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_real_corpse_is_looted_at_once_though_a_target_still_stands() {
    let profile = ojandhaart().unwrap();
    assert!(profile.loot.delay, "the premise: Nisugi delays looting");
    let mut hunt = Hunt::new(profile, 1);
    let ticks = replay(&fixture("smithy_kill.xml").unwrap(), &mut hunt)
        .await
        .unwrap();
    assert_eq!(ticks.len(), 28, "one tick per prompt in the cut");
    let loot = format!("loot #{PEGASUS}");
    let looted: Vec<&Tick> = ticks
        .iter()
        .filter(|tick| tick.line() == Some(&loot))
        .collect();
    // The pegasus never had a `health=` sent; the room list's `dead="1"` is
    // all the wire says. Before the fix this list was empty.
    assert_eq!(looted.len(), 1, "looted once: {looted:?}");
    let when = looted[0];
    assert!(
        when.at(PEGASUS_DEAD) && when.creatures == 2,
        "at the prompt after `drops dead`, with the engineer here: {when:?}"
    );
    assert!(
        ticks
            .iter()
            .filter(|tick| tick.now.is_some_and(|now| now < PEGASUS_DEAD))
            .all(|tick| !tick.line().is_some_and(|line| line.starts_with("loot"))),
        "nothing was looted while the pegasus lived"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn after_the_kill_the_engineer_is_taken_up_in_bigshots_order() {
    let mut hunt = Hunt::new(ojandhaart().unwrap(), 1);
    let ticks = replay(&fixture("smithy_kill.xml").unwrap(), &mut hunt)
        .await
        .unwrap();
    let from_kill = || {
        ticks
            .iter()
            .filter(|tick| tick.now.is_some_and(|now| now >= PEGASUS_DEAD))
    };
    // The last shot, the loot, then `target` until the dropdown shows the
    // engineer first, then the hunting stance (a cast had forced it down to
    // guarded), then the first routine step that holds: Camouflage, since
    // hiding had lapsed.
    assert_eq!(
        distinct_lines(from_kill()),
        [
            "fire".to_owned(),
            format!("loot #{PEGASUS}"),
            format!("target #{ENGINEER}"),
            "stance offensive".to_owned(),
            "incant 608".to_owned(),
        ]
    );
    let stance = from_kill()
        .find(|tick| tick.line() == Some("stance offensive"))
        .unwrap();
    assert!(
        stance.at(ENGINEER_TARGETED) && stance.hidden == Some(false),
        "{stance:?}"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn in_the_health_era_both_corpses_are_looted_and_the_companion_is_left_alone() {
    let mut hunt = Hunt::new(ojandhaart().unwrap(), 1);
    let ticks = replay(&fixture("arch_kill.xml").unwrap(), &mut hunt)
        .await
        .unwrap();
    assert_eq!(ticks.len(), 30, "one tick per prompt in the cut");
    for (corpse, when) in [
        (BATTLE_MASTODON, MASTODON_DEAD),
        (SHIELD_MAIDEN, SHIELD_MAIDEN_DEAD),
    ] {
        let loot = format!("loot #{corpse}");
        let looted: Vec<&Tick> = ticks
            .iter()
            .filter(|tick| tick.line() == Some(&loot))
            .collect();
        // Here `dead="1"` comes with `health="-N"`: both readings of a
        // corpse agree, and it is looted at the prompt after it fell.
        assert_eq!(looted.len(), 1, "{loot} once: {looted:?}");
        assert!(
            looted[0].at(when),
            "{loot} at the prompt after the death: {:?}",
            looted[0]
        );
    }
    // Nisugi's own mastodon has a status with health and no `hostile`, and
    // the profile's last target rule is any creature. Before the fix the
    // engine said `target #407520392` six times.
    let own = format!("#{OWN_MASTODON}");
    assert!(
        ticks
            .iter()
            .all(|tick| !tick.line().is_some_and(|line| line.contains(&own))),
        "the companion was targeted or looted"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_shield_maiden_gets_routine_e_with_its_guards_read_off_real_statuses() {
    let mut hunt = Hunt::new(ojandhaart().unwrap(), 1);
    let ticks = replay(&fixture("arch_kill.xml").unwrap(), &mut hunt)
        .await
        .unwrap();
    let after_mastodon = ticks
        .iter()
        .filter(|tick| tick.now.is_some_and(|now| now > MASTODON_DEAD));
    // Routine e: kweed while Tangleweed Vigor is about to lapse (it had
    // over a minute), volley (unwritten), coupdegrace at 20% health (she
    // stood at 738 of 900), `incant 611` unless immobilized (her status
    // read `rooted`), then fire. So: target her, the hunting stance, fire
    // until she falls, loot, and the three signs in no list once the room
    // holds nothing to fight.
    assert_eq!(
        distinct_lines(after_mastodon),
        [
            format!("target #{SHIELD_MAIDEN}"),
            "stance offensive".to_owned(),
            "fire".to_owned(),
            format!("loot #{SHIELD_MAIDEN}"),
            "incant 9708".to_owned(),
            "incant 9715".to_owned(),
            "incant 9711".to_owned(),
        ]
    );
}
