//! The binary's scripts, tested through its command line: Lich's script
//! finding, `;scripts` and its import, and a script run on a runner that
//! leaving the table stops.

use super::*;

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-scripts-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Lich's order of finding: a whole name or its start, in the folder or
/// its `custom` folders; never a Wizard script, and never a path.
#[test]
fn a_script_is_found_as_lich_finds_one() {
    let dir = scratch("found");
    std::fs::create_dir_all(dir.join("custom/mine")).unwrap();
    for file in [
        "trollspeak.lic",
        "custom/eloot.rb",
        "custom/mine/wander.lic",
        "old.cmd",
    ] {
        std::fs::write(dir.join(file), "").unwrap();
    }
    for (word, found) in [
        ("trollspeak", true),
        ("troll", true),
        ("eloot", true),
        ("wander", true),
        ("old", false),
        ("nosuch", false),
        ("../trollspeak", false),
        ("", false),
    ] {
        assert_eq!(names_a_script(&dir, word), found, "{word:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

const DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// What the player was told, until a line containing `until`, or a
/// minute.
async fn told_until(
    events: &mut tokio::sync::broadcast::Receiver<cena_session::Event>,
    until: &str,
) -> Vec<String> {
    let mut told = Vec::new();
    let _ = tokio::time::timeout(std::time::Duration::from_mins(1), async {
        while let Ok(event) = events.recv().await {
            if let cena_session::Event::Notice(notice) = event {
                told.extend(notice.lines().iter().cloned());
                if told.iter().any(|line| line.contains(until)) {
                    return;
                }
            }
        }
    })
    .await;
    told
}

/// `;scripts` says where scripts are; `;scripts import` imports and
/// says what it brought; neither needs a runner.
#[tokio::test(flavor = "current_thread")]
async fn scripts_import_is_answered_on_the_command_line() {
    let dir = scratch("import-typed");
    let lich = dir.join("Lich5");
    std::fs::create_dir_all(lich.join("scripts")).unwrap();
    std::fs::write(lich.join("scripts/wander.lic"), "echo 'lich'").unwrap();
    let (source, _transcript) = cena_platform::AnsweringSource::logged_in(
        b"<prompt time=\"1\">&gt;</prompt>
",
    );
    let session = cena_session::Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let (_, mut events) = session.subscribe();
    let commands = Commands::install(&handle);
    tokio::spawn(session.into_actor().run());
    let hydra = dir.join("hydra");
    let scripts = Scripts::new(&hydra, &Err("no map".to_owned()));
    scripts.open(SessionId(1), "Nisugi", "GS3", &handle, &observer, &commands);
    let generation = handle.generation();

    handle
        .send_manual_at(generation, ";scripts", DEADLINE)
        .await;
    let told = told_until(&mut events, "run from").await;
    assert!(
        told.iter().any(|l| l.contains("scripts import")),
        "{told:#?}"
    );
    let typed = format!(";scripts import {}", lich.display());
    handle.send_manual_at(generation, &typed, DEADLINE).await;
    let told = told_until(&mut events, "imported from").await;
    assert!(
        told.iter()
            .any(|l| l.contains("0 settings rows, 1 scripts")),
        "{told:#?}"
    );
    assert!(hydra.join("scripts/wander.lic").is_file());
    handle
        .send_manual_at(generation, ";scripts import", DEADLINE)
        .await;
    let told = told_until(&mut events, "from where").await;
    assert!(told.iter().any(|l| l.contains("from where")), "{told:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Through the command line: Lich's words answer while no runner runs;
/// a word of Hydra's is never a script's, even a family still starting;
/// the player's script runs on a runner Hydra starts, hearing the game;
/// and a character leaving the table stops its runner, which stops
/// hearing.
#[tokio::test(flavor = "current_thread")]
async fn a_typed_script_runs_and_leaving_the_table_stops_it() {
    assert!(
        runner::find_ruby().is_some(),
        "no Ruby: scripts need Ruby 4.0"
    );
    let dir = scratch("typed");
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::write(
        dir.join("scripts/greet.lic"),
        "put 'look'\nwaitfor 'quiet room'\necho 'done'\nwaitfor 'never comes'\n",
    )
    .unwrap();
    std::fs::write(dir.join("scripts/go2.lic"), "echo 'a script'\n").unwrap();
    let (source, transcript) =
        cena_platform::AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    for _ in 0..2 {
        transcript.answer(
            "look",
            b"You see a quiet room.\n<prompt time=\"2\">&gt;</prompt>\n",
        );
    }
    let session = cena_session::Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let (_, mut events) = session.subscribe();
    let commands = Commands::install(&handle);
    tokio::spawn(session.into_actor().run());
    let scripts = Scripts::new(&dir, &Err("no map".to_owned()));
    let id = SessionId(1);
    scripts.open(id, "Nisugi", "GS3", &handle, &observer, &commands);
    let generation = handle.generation();

    handle.send_manual_at(generation, ";l", DEADLINE).await;
    let told = told_until(&mut events, "No scripts").await;
    assert!(
        told.iter().any(|line| line == "No scripts are running."),
        "{told:#?}"
    );
    handle
        .send_manual_at(generation, ";go2 bank", DEADLINE)
        .await;
    let told = told_until(&mut events, "still starting").await;
    assert!(
        told.iter().any(|line| line.contains("still starting")),
        "Hydra's word, not the script's: {told:#?}"
    );

    handle.send_manual_at(generation, ";greet", DEADLINE).await;
    let told = told_until(&mut events, "[greet: done]").await;
    assert!(told.iter().any(|line| line == "[greet: done]"), "{told:#?}");
    assert!(told.iter().any(|line| line == "[greet]>look"), "{told:#?}");

    scripts.close(id).await;
    while events.try_recv().is_ok() {}
    handle.send_manual_at(generation, "look", DEADLINE).await;
    let heard = std::iter::from_fn(|| events.try_recv().ok())
        .filter(|event| matches!(event, cena_session::Event::Heard(_)))
        .count();
    assert_eq!(heard, 0, "the runner is gone, and nothing listens");
    assert_eq!(transcript.lines(), ["look", "look"]);
    let _ = std::fs::remove_dir_all(&dir);
}
