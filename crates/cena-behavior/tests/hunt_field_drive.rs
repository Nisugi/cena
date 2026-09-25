//! Actual Hunt/travel drivers over a scripted byte source, never the game.
mod drive_support;
mod ready;

use cena_behavior::hunt::{Hunt, Profile, hunt};
use cena_behavior::travel::TravelNotes;
use cena_behavior::watchdog::Heartbeat;
use cena_map::Map;
use cena_platform::AnsweringSource;
use cena_session::{AuthorityToken, CommandId, Session, Vital};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn injury_en_route_cancels_field_trip_and_walks_to_town_without_field_commands() {
    let (source, transcript) = AnsweringSource::logged_in(drive_support::PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let stop_session = session.cancel_token();
    let (mut snapshot, events) = session.subscribe();
    let (_, ready) = session.subscribe();
    // A fully observed synthetic initial character; subsequent changes travel
    // through the real protocol parser, actor, Hunt and travel event streams.
    let state = &mut snapshot.state;
    state.room.id = Some("1001".into());
    state.status.set("standing", true);
    state.status.set("bleeding", false);
    state.character.stance = Some("defensive (100%)".into());
    state.character.observed_body_parts = u16::MAX;
    state.character.encumbrance_percent = Some(0);
    state.character.experience.mind_percent = Some(100);
    state
        .vitals
        .insert("health".into(), Vital::percent_only(100));
    state.vitals.insert("mana".into(), Vital::percent_only(100));
    transcript.answer("north", b"<nav rm='1002'/><dialogData id='injuries'><image id='leftArm' name='Injury1'/></dialogData><prompt time='2'>&gt;</prompt>\n");
    transcript.answer("east", &drive_support::arrival(1004));
    transcript.answer("south", &drive_support::arrival(1003));
    let actor = tokio::spawn(session.into_actor().run());
    let stop = CancellationToken::new();
    let cancel = stop.clone();
    let task = tokio::spawn(async move {
        ready::until_ready(ready).await.unwrap();
        let map = Map::from_rooms(serde_json::from_str(r#"[
          {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
          {"id":2,"uid":[1002],"exits":[{"to":4,"kind":"cardinal","cmd":"east","cost":1},{"to":3,"kind":"cardinal","cmd":"south","cost":1}]},
          {"id":3,"uid":[1003]}, {"id":4,"uid":[1004]}
        ]"#).unwrap()).unwrap();
        let profile = Profile::parse(
            r"
          [rooms]
          allowed=[1,2,4]
          hunting=1
          resting=3
          [rest]
          fried=100
          commands=['town-only']
          [rest.until]
          experience=80
          [rest.field]
          room=4
          commands=['field-only']
          [rest.field.until]
          experience=80
        ",
        )
        .unwrap();
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        handle.claim(AuthorityToken(1)).await.unwrap();
        Box::pin(hunt(
            &handle,
            &cancel,
            ids,
            AuthorityToken(1),
            (snapshot, events.into()),
            &map,
            Hunt::new(profile, 1),
            &Heartbeat::default(),
            TravelNotes::default(),
            |_| {},
        ))
        .await
    });
    let arrived = drive_support::until_written(&transcript, "town-only").await;
    stop.cancel();
    let _ = task.await;
    stop_session.cancel();
    actor.await.unwrap();
    let lines = transcript.lines();
    assert!(arrived, "Town recovery must be reached: {lines:?}");
    assert!(lines.contains(&"north".to_owned()));
    assert!(lines.contains(&"south".to_owned()));
    assert!(
        !lines.contains(&"east".to_owned()),
        "The remaining field leg must be cancelled: {lines:?}"
    );
    assert!(!lines.contains(&"field-only".to_owned()));
}
