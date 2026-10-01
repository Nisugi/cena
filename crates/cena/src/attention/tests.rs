use super::*;

fn call(character: &str, trigger: &str, sound: Option<&str>, notify: Option<&str>) -> Called {
    Called {
        character: character.into(),
        attention: Arc::new(Attention {
            trigger: trigger.into(),
            sound: sound.map(Into::into),
            notify: notify.map(Into::into),
            alert: None,
            cooldown: 3,
        }),
    }
}

/// Three characters hear one shout: one sound, one notification.
#[test]
fn one_occurrence_seen_by_several_characters_passes_once() {
    let mut once = Once::default();
    let start = Instant::now();
    let shout = |who| call(who, "shout", Some("ding.wav"), Some("Help!"));
    assert!(once.admit(&shout("Nisugi"), start).is_some());
    assert!(once.admit(&shout("Dicate"), start).is_none());
    let later = start + Duration::from_millis(900);
    assert!(once.admit(&shout("Sugiin"), later).is_none());
    // A second on, it is another occurrence.
    assert!(once.admit(&shout("Dicate"), start + SAME).is_some());
}

/// The same character calling again is its own again: its session's
/// cooldown already let it through.
#[test]
fn the_same_character_again_is_another_occurrence() {
    let mut once = Once::default();
    let now = Instant::now();
    let shout = call("Nisugi", "shout", Some("ding.wav"), None);
    assert!(once.admit(&shout, now).is_some());
    assert!(once.admit(&shout, now).is_some());
}

/// Two characters' different calls are two occurrences, and a banner alone
/// is not the desk's.
#[test]
fn a_different_call_passes_and_a_banner_alone_does_not_reach_the_desk() {
    let mut once = Once::default();
    let now = Instant::now();
    assert!(
        once.admit(&call("Nisugi", "shout", Some("a.wav"), None), now)
            .is_some()
    );
    assert!(
        once.admit(&call("Dicate", "shout", Some("b.wav"), None), now)
            .is_some()
    );
    assert!(
        once.admit(&call("Dicate", "other", Some("a.wav"), None), now)
            .is_some()
    );
    assert_eq!(once.admit(&call("Dicate", "banner", None, None), now), None);
}

/// A sound is found by its file name in the sounds folder, as written or
/// with one of the four extensions; a path is never followed (BI-B-1).
#[test]
fn a_sound_is_found_where_vellum_finds_one() {
    let root = std::env::temp_dir().join(format!("cena-attention-found-{}", std::process::id()));
    let sounds = root.join("sounds");
    fs_setup(&sounds).unwrap();
    let elsewhere = root.join("elsewhere.wav");
    std::fs::write(&elsewhere, b"").unwrap();

    let at = |named: &str| found(&sounds, named);
    assert_eq!(
        at(&elsewhere.display().to_string()),
        None,
        "a file outside the sounds folder is not played, though it is there"
    );
    assert_eq!(at("ding.wav"), Some(sounds.join("ding.wav")));
    assert_eq!(
        at("ding"),
        Some(sounds.join("ding.wav")),
        "an extension tried"
    );
    assert_eq!(at("chime"), Some(sounds.join("chime.ogg")));
    // A path is refused before anything is looked for, though its file name
    // is here: a network path would reach out to the host it names.
    for path in [
        r"C:\Users\Someone\Desktop\fx\ding.wav",
        r"\\203.0.113.9\s\ding.wav",
        r"\\?\UNC\203.0.113.9\s\ding.wav",
        "../sounds/ding.wav",
    ] {
        assert_eq!(at(path), None, "{path}");
    }
    assert_eq!(
        at("ding.mp3"),
        None,
        "an extension written is the one looked for"
    );
    assert_eq!(at("nothing"), None);
    let _ = std::fs::remove_dir_all(&root);
}

fn fs_setup(sounds: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_dir_all(sounds);
    std::fs::create_dir_all(sounds)?;
    std::fs::write(sounds.join("ding.wav"), b"")?;
    std::fs::write(sounds.join("chime.ogg"), b"")
}
