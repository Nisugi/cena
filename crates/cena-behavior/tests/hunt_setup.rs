//! Native configuration and rest decisions with no game connection.
use cena_behavior::hunt::setup::{self, Operation, Request};
use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::{Map, RoomId};
use cena_session::{Frame, GameState, Vital};

fn map() -> Result<Map, Box<dyn std::error::Error>> {
    Ok(Map::from_rooms(serde_json::from_str(
        r#"[
      {"id":1,"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1},{"to":3,"kind":"cardinal","cmd":"east","cost":0.1}]},
      {"id":2,"exits":[{"to":1,"kind":"cardinal","cmd":"south","cost":1}]},
      {"id":3,"exits":[{"to":2,"kind":"cardinal","cmd":"west","cost":0.1}]}
    ]"#,
    )?)?)
}
fn request() -> Result<Request, serde_json::Error> {
    serde_json::from_value(
        serde_json::json!({"operation":"preview","name":"test-hunt","map_sha256":"a".repeat(64),"acknowledged":true,
     "allowed":[1,2],"start":1,"town":3,"town_commands":["town command"],"field":{"room":2,"commands":["field command","second field command"],"until":{"experience":80,"mana":90}},
     "targets":["warg"],"attacks":["attack"],"until":{"experience":80,"mana":90},"preview":null}),
    )
}
fn dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/hunt-setup-tests/{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}
fn healthy() -> GameState {
    let mut s = GameState::default();
    s.status.set("standing", true);
    s.status.set("bleeding", false);
    s.character.encumbrance_percent = Some(0);
    s.character.experience.mind_percent = Some(100);
    s.vitals.insert("health".into(), Vital::percent_only(100));
    s.vitals.insert("mana".into(), Vital::percent_only(100));
    for part in [
        "leftEye",
        "rightEye",
        "head",
        "neck",
        "back",
        "chest",
        "abdomen",
        "leftArm",
        "rightArm",
        "rightHand",
        "leftHand",
        "leftLeg",
        "rightLeg",
        "leftFoot",
        "rightFoot",
        "nsys",
    ] {
        s.apply(&Frame::InjuryImage {
            id: part.into(),
            name: part.into(),
            dialog: Some("injuries".into()),
            attrs: Vec::new(),
        });
    }
    s
}
fn here(id: u32) -> Here<'static> {
    Here {
        room: Some(RoomId(id)),
        exits: &[],
    }
}

#[test]
fn preview_save_reload_are_native_and_never_overwrite() {
    let (map, dir, mut request) = (map().unwrap(), dir().unwrap(), request().unwrap());
    let hash = "a".repeat(64);
    let preview = setup::configure(&dir, ("test", "Ada"), &map, &hash, &request).unwrap();
    assert!(!preview.saved && !preview.started);
    assert!(!dir.join("hunt").exists());
    let profile = Profile::parse(&preview.toml).unwrap();
    assert_eq!(profile.rooms.allowed, Some(vec![1, 2]));
    assert!(profile.rooms.boundaries.is_empty());
    request.operation = Operation::Save;
    assert!(setup::configure(&dir, ("test", "Ada"), &map, &hash, &request).is_err());
    request.preview = Some(preview.toml.clone());
    let saved = setup::configure(&dir, ("test", "Ada"), &map, &hash, &request).unwrap();
    assert!(saved.saved && !saved.started);
    assert_eq!(preview.toml, saved.toml);
    assert!(
        setup::configure(&dir, ("test", "Ada"), &map, &hash, &request)
            .unwrap_err()
            .contains("not overwritten")
    );
    request.operation = Operation::Load;
    assert_eq!(
        setup::configure(&dir, ("test", "Ada"), &map, &hash, &request)
            .unwrap()
            .toml,
        preview.toml
    );
}

#[test]
fn stale_empty_missing_unacknowledged_and_conflicting_choices_are_refused() {
    let (map, dir, good) = (map().unwrap(), dir().unwrap(), request().unwrap());
    let hash = "a".repeat(64);
    let mut cases = Vec::new();
    let mut r = good.clone();
    r.allowed.clear();
    cases.push(r);
    let mut r = good.clone();
    r.start = 3;
    cases.push(r);
    let mut r = good.clone();
    r.town = 999;
    cases.push(r);
    let mut r = good.clone();
    r.acknowledged = false;
    cases.push(r);
    let mut r = good.clone();
    r.map_sha256 = "b".repeat(64);
    cases.push(r);
    let mut r = good.clone();
    r.attacks = vec![";bigshot".into()];
    cases.push(r);
    let mut r = good.clone();
    r.until.experience = Some(100);
    cases.push(r);
    let mut r = good.clone();
    r.until.mana = Some(101);
    cases.push(r);
    for r in cases {
        assert!(setup::configure(&dir, ("test", "Ada"), &map, &hash, &r).is_err());
    }
    let path = cena_behavior::hunt::chain::character_path(&dir, "test", "Ada").unwrap();
    cena_behavior::hunt::chain::write_new(&path, "[rooms]\nhunting=3\n").unwrap();
    assert!(
        setup::configure(&dir, ("test", "Ada"), &map, &hash, &good)
            .unwrap_err()
            .contains("overrides")
    );
    assert!(
        setup::configure(&dir, ("test", "Bea"), &map, &hash, &good).is_ok(),
        "Other character overrides must not leak"
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "[rooms]\nhunting=3\n"
    );
}

#[test]
fn setup_preserves_unmanaged_inheritance_and_detects_preview_changes() {
    use cena_behavior::hunt::chain;
    let (map, dir, mut r) = (map().unwrap(), dir().unwrap(), request().unwrap());
    let global = chain::global_path(&dir);
    chain::write_new(&global, "prepare=['look']\n[routines]\nother=['attack']\n").unwrap();
    let hash = "a".repeat(64);
    let preview = setup::configure(&dir, ("test", "Ada"), &map, &hash, &r).unwrap();
    let parsed = Profile::parse(&preview.toml).unwrap();
    assert_eq!(parsed.prepare, vec!["look"]);
    assert!(parsed.routines.contains_key("other"));
    r.operation = Operation::Save;
    r.preview = Some(preview.toml);
    std::fs::write(
        &global,
        "prepare=['look','glance']\n[routines]\nother=['attack']\n",
    )
    .unwrap();
    assert!(
        setup::configure(&dir, ("test", "Ada"), &map, &hash, &r)
            .unwrap_err()
            .contains("Preview changed")
    );
    r.operation = Operation::Preview;
    r.preview = Some(
        setup::configure(&dir, ("test", "Ada"), &map, &hash, &r)
            .unwrap()
            .toml,
    );
    r.operation = Operation::Save;
    setup::configure(&dir, ("test", "Ada"), &map, &hash, &r).unwrap();
    let text = std::fs::read_to_string(chain::profile_path(&dir, &r.name).unwrap()).unwrap();
    assert!(!text.contains("prepare"));
    assert!(!text.contains("other"));
    assert_eq!(
        chain::load(&dir, Some("test"), Some("Ada"), &r.name)
            .unwrap()
            .profile
            .prepare,
        vec!["look", "glance"]
    );
}

#[test]
fn field_and_town_commands_are_separate_and_new_injury_escalates() {
    let (map, dir, r) = (map().unwrap(), dir().unwrap(), request().unwrap());
    let p = Profile::parse(
        &setup::configure(&dir, ("test", "Ada"), &map, &"a".repeat(64), &r)
            .unwrap()
            .toml,
    )
    .unwrap();
    let mut state = healthy();
    assert_eq!(state.character.observed_body_parts, u16::MAX);
    let mut hunt = Hunt::new(p.clone(), 1);
    assert_eq!(hunt.tick(&state, here(1), Some(1)), Said::Walk(RoomId(2)));
    assert_eq!(
        hunt.tick(&state, here(2), Some(2)),
        Said::Send {
            line: "field command".into(),
            target: None
        }
    );
    state.character.encumbrance_percent = Some(1);
    assert_eq!(
        hunt.tick(&state, here(2), Some(3)),
        Said::Walk(RoomId(3)),
        "Do not run the second field command"
    );
    assert_eq!(
        hunt.tick(&state, here(3), Some(4)),
        Said::Send {
            line: "town command".into(),
            target: None
        }
    );
    for missing in ["body", "health", "weight", "bleeding"] {
        let mut s = healthy();
        match missing {
            "body" => s.character.observed_body_parts = 0,
            "health" => {
                s.vitals.remove("health");
            }
            "weight" => s.character.encumbrance_percent = None,
            _ => s.status = cena_session::GameState::default().status,
        }
        assert_eq!(
            Hunt::new(p.clone(), 1).tick(&s, here(1), Some(1)),
            Said::Walk(RoomId(3)),
            "{missing}"
        );
    }
    let mut s = healthy();
    s.invalidate_for_reconnect();
    assert_eq!(s.character.observed_body_parts, 0);
    assert_eq!(
        Hunt::new(p, 1).tick(&s, here(1), Some(1)),
        Said::Walk(RoomId(3))
    );
}

#[test]
fn membership_is_not_exclusion_and_routes_cannot_detour_outside_it() {
    let (map, dir, r) = (map().unwrap(), dir().unwrap(), request().unwrap());
    let mut p = Profile::parse(
        &setup::configure(&dir, ("test", "Ada"), &map, &"a".repeat(64), &r)
            .unwrap()
            .toml,
    )
    .unwrap();
    let restricted = setup::hunting_map(&p, &map).unwrap().unwrap();
    assert!(restricted.room(RoomId(3)).is_none());
    assert_eq!(restricted.room(RoomId(1)).unwrap().exits.len(), 1);
    assert_eq!(restricted.room(RoomId(1)).unwrap().exits[0].to, RoomId(2));
    let mut s = healthy();
    s.character.experience.mind_percent = Some(0);
    assert_eq!(
        Hunt::new(p.clone(), 1).tick(&s, here(3), Some(1)),
        Said::Walk(RoomId(1))
    );
    assert_eq!(
        Hunt::new(p.clone(), 1).tick(
            &s,
            Here {
                room: Some(RoomId(1)),
                exits: &[RoomId(2), RoomId(3)]
            },
            Some(1)
        ),
        Said::Walk(RoomId(2))
    );
    p.rooms.allowed = Some(vec![]);
    assert!(!p.problems().is_empty());
    p.rooms.allowed = None;
    assert!(
        setup::hunting_map(&p, &map).unwrap().is_none(),
        "Legacy behavior remains unrestricted by an absent allow-list"
    );
}

#[test]
fn native_desk_refuses_stale_or_unverified_pinned_map_before_starting() {
    use cena_behavior::hunt::{Command, Desk, chain};
    use cena_session::{AuthorityToken, Session};
    use std::sync::Arc;
    let (map, dir, r) = (map().unwrap(), dir().unwrap(), request().unwrap());
    let text = setup::configure(&dir, ("test", "Ada"), &map, &"a".repeat(64), &r)
        .unwrap()
        .toml;
    chain::write_new(&chain::profile_path(&dir, &r.name).unwrap(), &text).unwrap();
    let map = Arc::new(map);
    for desk in [
        Desk::new(Arc::clone(&map), dir.clone(), AuthorityToken(1)),
        Desk::with_map_sha256(
            Arc::clone(&map),
            dir.clone(),
            AuthorityToken(1),
            "b".repeat(64),
        ),
    ] {
        let (source, transcript) =
            cena_platform::AnsweringSource::logged_in(b"<prompt time='1'>&gt;</prompt>");
        let session = Session::new(source);
        assert!(
            desk.run(
                &session.handle(),
                session.subscribe(),
                Command::Run(r.name.clone())
            )
            .is_none()
        );
        assert_eq!(transcript.written_count(), 0);
    }
}
