//! The deeds a crossing hands the driver, through the real driver and a
//! scripted game: taking things back, casting to be carried, a key taken
//! out, and how a walk ends when the controls or the connection are not its
//! own. Mostly the review of 2026-09-23's regression tests, each of which
//! names the defect it was written against and was seen to fail without its
//! fix.
//!
//! Virtual time throughout, as in `travel_drive.rs`, whose harness this
//! shares (`drive_support`).

mod drive_support;

use std::time::Duration;

use cena_behavior::BehaviorError;
use cena_behavior::travel::{CARRIED_WITHIN_MS, Ended, TravelNotes, walker_from};
use cena_platform::AnsweringSource;
use cena_session::hands::Hand;
use cena_session::{
    AuthorityToken, GameState, Gate, NoticeKind, Origin, Session, SkillLine, Vital, spells,
};
use drive_support::*;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// Holding a sword and a shield: both go away for the rope.
fn sword_and_shield(state: &mut GameState) {
    state.left_hand = Hand::Holding {
        id: Some("12".into()),
        noun: Some("shield".into()),
        name: "kite shield".into(),
    };
}

/// A stop while the first of two things is being taken back: **both** are
/// still recorded, both are named, and the stop's one take-back tries both.
///
/// Review finding (2026-09-23): `FillHands` took the whole list out and put
/// each back as it failed, so the error on the first -- the shield -- left
/// the sword off the record. Reproduced before the fix: `still_stored` held
/// only the shield, and `get #11` was never sent.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_stop_while_taking_back_keeps_everything_not_yet_back_on_record() {
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, _) = set_out_as(&stop, ROOMS, None, sword_and_shield);
    transcript.answer("north", &arrival(1002));
    transcript.answer("store right", SWORD_GONE);
    transcript.answer(
        "store left",
        b"<left>Empty</left>\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    transcript.answer("climb rope", &arrival(1003));
    // Answered with no prompt, so the take-back is still waiting when the
    // stop lands.
    transcript.answer("get #12", b"You reach for it.\n");
    assert!(until_written(&transcript, "get #12").await);
    stop.cancel();

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::Cancelled));
    let still: Vec<&str> = travelled
        .still_stored
        .iter()
        .map(|stored| stored.id.as_str())
        .collect();
    assert_eq!(still, ["11", "12"], "in the order they were stored");
    let lines = transcript.lines();
    assert_eq!(
        lines[lines.len() - 2..],
        ["get #12", "get #11"],
        "one each, last stored first: {lines:?}"
    );
    session.cancel();
}

/// The key is out when the stop lands: the walk says so. It is **not** put
/// back -- the author's one exception is for what was stored, and a put is a
/// second kind of command, as the stance is (`drive`'s module docs).
///
/// Review finding (2026-09-23): the key taken mid-crossing was in neither
/// `Travelled` nor the player's notices, so a stop left it in hand silently.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_key_out_when_the_walk_stops_is_reported() {
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, mut told) = set_out_as(&stop, GATE, None, |_| {});
    transcript.answer(
        "get my heavy key",
        b"You remove <a exist=\"77\" noun=\"key\">a heavy iron key</a> from in your \
          <a exist=\"88\" noun=\"cloak\">dark cloak</a>.\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    // The lock is never answered: the stop lands with the key in hand.
    transcript.answer("unlock spiked gate with my heavy key", b"You fumble.\n");
    assert!(until_written(&transcript, "unlock spiked gate with my heavy key").await);
    stop.cancel();

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::Cancelled));
    assert_eq!(travelled.still_out.as_deref(), Some("heavy key"));
    assert_eq!(
        transcript.lines().last().map(String::as_str),
        Some("unlock spiked gate with my heavy key"),
        "nothing is sent to put it back"
    );
    let said = told_so_far(&mut told);
    assert!(
        said.iter()
            .any(|(kind, text)| *kind == NoticeKind::Warn && text.contains("heavy key")),
        "{said:?}"
    );
    session.cancel();
}

/// The connection changes under the walk while a deed's command is out: the
/// actor discards it as an older connection's (`Outcome::Interrupted`). That
/// is a disconnection, and the walk ends as one -- not as the player's stop,
/// which would send its take-back and say nothing of the session.
///
/// Not the test that pins the `Interrupted` arm: `cena-session` now answers
/// a stale command `Disconnected` at admission (`actor/io.rs`), so this
/// reaches that first. MEASURED: it stayed green with the old
/// `Interrupted -> Cancelled` arm restored. `BehaviorError::from_outcome`'s
/// unit test pins the arm; this pins the path end to end.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_from_an_older_connection_ends_the_walk_as_a_disconnection() {
    // Straight to the rope: the first thing sent is `store right`, a deed's
    // exchange.
    const ROPE: &str = r#"[
      {"id":1,"uid":[1001],"exits":[{"to":3,"kind":"scripted","cost":1,
         "steps":[{"empty_hands":null},{"move":"climb rope"}]}]},
      {"id":3,"uid":[1003]}
    ]"#;
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, mut told, cell) =
        set_out_on_a_connection(&stop, ROPE, None, |_| {});
    // What a supervisor does between connections; this actor keeps the old
    // one, so everything the walk sends is stale.
    cell.advance();

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::Disconnected));
    assert_eq!(transcript.written_count(), 0, "discarded, never written");
    let said = told_so_far(&mut told);
    assert!(
        said.iter()
            .any(|(kind, text)| *kind == NoticeKind::Error && text.contains("went away")),
        "{said:?}"
    );
    session.cancel();
}

/// Something else holds the controls when the walk starts: it cannot set out,
/// and it **says so**. The player typed a destination; silence reads as a
/// walk that is happening.
///
/// Review finding (2026-09-23): the refused claim returned before `report`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_walk_that_cannot_take_the_controls_says_so() {
    let stop = CancellationToken::new();
    let (walk, transcript, session, typed, mut told) = set_out_as(&stop, ROOMS, None, |_| {});
    // Claimed first: this reaches the actor before the walk has been polled.
    assert!(typed.claim(AuthorityToken(9)).await.is_ok());

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(
        travelled.ended,
        Ended::Stopped(BehaviorError::AuthorityHeld)
    );
    assert_eq!(transcript.written_count(), 0);
    let said = told_so_far(&mut told);
    assert!(
        said.iter()
            .any(|(kind, text)| *kind == NoticeKind::Error && text.contains("did not set out")),
        "{said:?}"
    );
    session.cancel();
}

/// Phase at an insignia (`mapdb.json`, room 18926's proc), and a long way
/// round by 2.
const INSIGNIA: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
     {"to":3,"kind":"scripted","cost":1,"steps":[{"cast_at":["Phase","insignia"]}]},
     {"to":2,"kind":"cardinal","cmd":"east","cost":50}]},
  {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":50}]},
  {"id":3,"uid":[1003]}
]"#;

const HINDERED: &[u8] = b"[Spell Hindrance for Phase is 45% with current armor worn.]\n\
    Your armor prevents the spell from working correctly.\n<prompt time=\"2\">&gt;</prompt>\n";

/// Upstream casts again while armour hinders it; here `MAX_HINDERED` times,
/// and then the exit is given up **at once** and the walker goes round.
///
/// Review finding (2026-09-23): the cast was sent once, and the step then
/// waited to be carried for `MAX_WAIT_MS` -- thirty minutes -- by a spell that
/// never went off. Reproduced before the fix: one `cast insignia`, and the
/// walk took half an hour to go round.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_hindered_phase_is_cast_again_and_then_given_up_promptly() {
    /// `routines::casting::MAX_HINDERED`.
    const MAX_HINDERED: usize = 20;
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, INSIGNIA);
    for _ in 0..MAX_HINDERED {
        transcript.answer("cast insignia", HINDERED);
    }
    transcript.answer("east", &arrival(1002));
    transcript.answer("north", &arrival(1003));

    let began = Instant::now();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    let lines = transcript.lines();
    let casts = lines.iter().filter(|line| *line == "cast insignia").count();
    assert_eq!(casts, MAX_HINDERED, "{lines:?}");
    assert_eq!(lines[lines.len() - 2..], ["east", "north"]);
    assert!(
        began.elapsed() < Duration::from_mins(1),
        "gave up after {:?}",
        began.elapsed()
    );
    session.cancel();
}

/// Cast, and nothing carried the walker: given up after
/// [`CARRIED_WITHIN_MS`], not the half-hour a ship's voyage is allowed.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_phase_that_carries_nobody_is_given_up_promptly() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, INSIGNIA);
    transcript.answer("east", &arrival(1002));
    transcript.answer("north", &arrival(1003));

    let began = Instant::now();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["prepare 704", "cast insignia", "east", "north"]
    );
    let waited = began.elapsed();
    assert!(
        waited >= Duration::from_millis(CARRIED_WITHIN_MS),
        "{waited:?}"
    );
    assert!(waited < Duration::from_mins(1), "{waited:?}");
    session.cancel();
}

/// A walker who knows the spell and cannot pay for it waits for the mana --
/// ten minutes, `casting`'s bound -- sends nothing meanwhile, and then says
/// why and goes round.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_phase_waits_for_the_mana_and_then_gives_up_saying_why() {
    const DISK: &str = r#"[
      {"id":1,"uid":[1001],"exits":[
         {"to":3,"kind":"scripted","cost":1,"steps":[{"cast_at":["Floating Disk","insignia"]}]},
         {"to":2,"kind":"cardinal","cmd":"east","cost":50}]},
      {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":50}]},
      {"id":3,"uid":[1003]}
    ]"#;
    assert_eq!(
        spells::spell(511).map(|spell| spell.name.as_str()),
        Some("Floating Disk"),
        "the fixture names spell 511 by the table's name"
    );
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, mut told) = set_out_as(&stop, DISK, None, |state| {
        state.character.experience.level = Some("Level 50".into());
        let line = SkillLine::classify("  Major Elemental....................|              25");
        if let Some(line) = line {
            state.character.skills.apply(&line, false);
        }
        let gauge = |current| Vital {
            percent: 0,
            current: Some(current),
            max: Some(100),
        };
        state.vitals.insert("stamina".into(), gauge(100));
        state.vitals.insert("spirit".into(), gauge(100));
        state.vitals.insert("mana".into(), gauge(1));
    });
    transcript.answer("east", &arrival(1002));
    transcript.answer("north", &arrival(1003));

    let began = Instant::now();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(transcript.lines(), ["east", "north"], "nothing was cast");
    let waited = began.elapsed();
    assert!(waited >= Duration::from_mins(10), "{waited:?}");
    assert!(waited < Duration::from_mins(11), "{waited:?}");
    let said = told_so_far(&mut told);
    assert!(
        said.iter().any(|(_, text)| text.contains("mana")),
        "{said:?}"
    );
    session.cancel();
}

/// A cast refused for roundtime has not been cast: it is sent again once the
/// roundtime has passed, as `fput` does.
///
/// Review finding (2026-09-23): `Deed::Cast` sent once and waited for the
/// prompt, so the refusal was taken for the answer and the deed counted done.
/// Reproduced before the fix: `["sigil of resolve", "climb rope"]`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_cast_refused_for_roundtime_is_cast_again() {
    const SIGIL: &str = r#"[
      {"id":1,"uid":[1001],"exits":[{"to":3,"kind":"scripted","cost":1,
         "steps":[{"cast":"Sigil of Resolve"},{"move":"climb rope"}]}]},
      {"id":3,"uid":[1003]}
    ]"#;
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop, SIGIL);
    transcript.answer(
        "sigil of resolve",
        b"...wait 2 seconds.\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    transcript.answer("climb rope", &arrival(1003));

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["sigil of resolve", "sigil of resolve", "climb rope"]
    );
    session.cancel();
}

/// What the walker knows of its spells comes from `Effects::active`, which
/// knows an effect whose clock has not started yet. In the login burst the
/// spells arrive **before the first prompt**: there is no server second to
/// stamp them against, so each duration is held until one comes, and
/// `ends_at` is `None` meanwhile -- the same spelling as a spell that never
/// ends. Read alone, it made a spell stated with no time left look live for
/// ever (cena-model, 2026-09-23).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_spell_the_login_burst_stated_as_over_is_not_taken_for_live() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let session_cancel = session.cancel_token();
    tokio::spawn(session.into_actor().run());
    // The burst, with no prompt anywhere in it.
    transcript.answer(
        "spells",
        b"<dialogData id='Active Spells'>\
          <progressBar id='506' value='90' text=\"Celerity\" time='00:02:00'/>\
          <progressBar id='515' value='0' text=\"Rapid Fire\" time='00:00:00'/>\
          </dialogData>\n",
    );
    let _ = handle.send_now("spells", Origin::Manual, Gate::None).await;
    tokio::time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;

    let (snapshot, _) = observer.subscribe().await.unwrap();
    let state = snapshot.state;
    assert_eq!(state.game_time(), None, "no prompt yet, so no clock");
    let walker = walker_from(&state, &TravelNotes::default(), 0);
    let active = walker.active_spells.unwrap();
    assert!(active.contains("Celerity"), "{active:?}");
    assert!(!active.contains("Rapid Fire"), "stated as over: {active:?}");
    session_cancel.cancel();
}
