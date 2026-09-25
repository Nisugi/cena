//! Composition of a native Hunt configuration handler with a session observer.
//! It receives no `SessionHandle`, so configuration cannot send game commands.

use crate::map_context::MapContext;
use cena_behavior::hunt::setup;
use cena_session::{SessionObserver, State};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Message {
    Inspect {},
    Configure {
        generation: String,
        config: Box<setup::Request>,
    },
}

pub(crate) fn native(
    observer: SessionObserver,
    context: Arc<MapContext>,
    dir: PathBuf,
) -> cena_web::HuntSetup {
    Arc::new(move |value| {
        let (observer, context, dir) = (observer.clone(), Arc::clone(&context), dir.clone());
        Box::pin(async move {
            let (snapshot, _) = observer
                .subscribe()
                .await
                .map_err(|e| format!("Session unavailable: {e:?}"))?;
            if snapshot.lifecycle != State::Ready {
                return Err("Character is not Ready; reconnect before setup".into());
            }
            let character = &snapshot.state.character;
            let name = character
                .name
                .clone()
                .ok_or("Native character identity is unknown")?;
            let instance = character
                .instance
                .clone()
                .ok_or("Native instance identity is unknown")?;
            let generation = snapshot.generation.0.to_string();
            tokio::task::spawn_blocking(move || {
                answer(value, &context, &dir, &instance, &name, &generation, false)
            })
            .await
            .map_err(|e| e.to_string())?
        })
    })
}

/// The offline harness supplies a fixture identity and an isolated directory.
pub(crate) fn answer(
    value: Value,
    context: &MapContext,
    dir: &std::path::Path,
    instance: &str,
    name: &str,
    generation: &str,
    offline: bool,
) -> Result<Value, String> {
    match serde_json::from_value::<Message>(value).map_err(|e| e.to_string())? {
        Message::Inspect {} => Ok(
            json!({"instance":instance,"character":name,"generation":generation,"map_sha256":context.sha256,"offline":offline}),
        ),
        Message::Configure {
            generation: asked,
            config,
        } => {
            if asked != generation {
                return Err("Session generation changed; reload setup before saving".into());
            }
            serde_json::to_value(setup::configure(
                dir,
                (instance, name),
                &context.map,
                &context.sha256,
                &config,
            )?)
            .map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_platform::AnsweringSource;
    use cena_session::{Generation, Outcome, Session};
    use std::time::Duration;

    #[tokio::test]
    async fn native_observer_binds_identity_and_configuration_sends_nothing() {
        let (source, transcript) = AnsweringSource::logged_in(
            b"<app char='SetupFixture' game='Test'/>\n<prompt time='1'>&gt;</prompt>\n",
        );
        let session = Session::new(source);
        let handle = session.handle();
        let observer = session.observer();
        let stop = session.cancel_token();
        let actor = tokio::spawn(session.into_actor().run());
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if observer.subscribe().await.unwrap().0.lifecycle == State::Ready {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // Only the scripted transport receives this fixture stimulus, never a
        // real connector. Capture its baseline before invoking configuration.
        assert!(matches!(
            handle
                .send_manual_at(
                    Generation::FIRST,
                    "fixture identity",
                    Duration::from_secs(2)
                )
                .await,
            Outcome::Confirmed(_)
        ));
        let baseline = transcript.lines();
        let context = Arc::new(MapContext {
            map: Arc::new(
                cena_behavior::travel::Map::from_rooms(
                    serde_json::from_str(r#"[{"id":1},{"id":2}]"#).unwrap(),
                )
                .unwrap(),
            ),
            sha256: "a".repeat(64),
        });
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/native-host-setup/{stamp}"));
        let callback = native(observer, context, dir);
        let identity = callback(json!({"action":"inspect"})).await.unwrap();
        assert_eq!(identity["character"], "SetupFixture");
        assert_eq!(identity["instance"], "Test");
        let mut message = json!({"action":"configure","generation":identity["generation"],"config":{
            "operation":"preview","name":"fixture","map_sha256":"a".repeat(64),"acknowledged":true,
            "allowed":[1],"start":1,"town":2,"town_commands":[],"field":null,
            "targets":["warg"],"attacks":["attack"],"until":{"experience":80,"mana":90},"preview":null
        }});
        let preview = callback(message.clone()).await.unwrap();
        message["config"]["operation"] = json!("save");
        message["config"]["preview"] = preview["toml"].clone();
        let saved = callback(message.clone()).await.unwrap();
        assert_eq!(saved["saved"], true);
        assert_eq!(saved["started"], false);
        message["generation"] = json!("stale");
        assert!(
            callback(message)
                .await
                .unwrap_err()
                .contains("generation changed")
        );
        assert!(
            callback(json!({"action":"inspect","character":"SomeoneElse"}))
                .await
                .is_err()
        );
        assert_eq!(
            transcript.lines(),
            baseline,
            "Setup must have no game sends"
        );
        stop.cancel();
        actor.await.unwrap();
        assert!(callback(json!({"action":"inspect"})).await.is_err());
    }
}
