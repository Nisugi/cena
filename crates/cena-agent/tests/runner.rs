//! The Ruby runner end to end (`plan/46` §11 step 1): Hydra writes out and
//! starts Lich's own engine in Ruby; the player's command starts a Lich
//! script there; its line reaches the game as a script's, the game's answer
//! reaches the script, and what it says reaches the player, echoed as Lich
//! echoes it.
//!
//! **It needs Ruby 4.0**, which Lich's engine needs: on the `PATH` or where
//! Lich's Windows installer puts it. Without one this fails, saying so,
//! rather than passing over nothing.
//!
//! What the tests share -- the scripted character and its runner, the
//! world, the stand-in travel -- is `support` (`tests/support/runner.rs`).

#[path = "support/runner.rs"]
mod support;

use std::path::PathBuf;
use std::time::Duration;

use cena_agent::scripts::runner::find_ruby;
use cena_session::Origin;
use support::{
    DESCRIBED_ROOM, QUIET_ROOM, Running, World, in_the_quiet_glade, scripts_with, temp_dir,
};

#[tokio::test(flavor = "current_thread")]
async fn a_lich_script_runs_against_hydra() {
    assert!(
        find_ruby().is_some(),
        "no Ruby: the runner needs Ruby 4.0, on the PATH or under C:\\Ruby4Lich5 (Lich's installer)"
    );
    let dir = temp_dir("lich-script");
    let scripts = scripts_with(&dir, "hydratest.lic").unwrap();
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: &[("look", QUIET_ROOM)],
            ..World::default()
        },
    )
    .await
    .unwrap();

    assert!(running.typed("hydratest one \"two three\""));
    let heard = running.heard_until("hydratest has exited").await;
    let errors = running.errors();
    assert!(
        heard.finished,
        "the script did not finish.\ntold: {:#?}\nrunner's errors: {errors:#?}",
        heard.told
    );
    for expected in [
        "--- Lich: hydratest active.",
        "[hydratest: args: one|two three]",
        "[hydratest]>look",
        "[hydratest: heard: You see a quiet room.]",
        "a table",
        "  row one",
        "--- Lich: hydratest has exited.",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    assert_eq!(heard.sent, [("look".to_owned(), Origin::Script)]);
    assert_eq!(running.transcript.lines(), ["look"]);
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The reads** (`plan/46` §11 step 2): Lich's own `XMLData` readers,
/// `GameObj`, `Char` and the check family, answered from the local copy
/// that the game's description of the room filled.
#[tokio::test(flavor = "current_thread")]
async fn a_script_reads_its_character_as_lich_does() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("reads");
    let scripts = scripts_with(&dir, "readstest.lic").unwrap();
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: &[("look", DESCRIBED_ROOM)],
            ..World::default()
        },
    )
    .await
    .unwrap();
    let heard = running.run("readstest", "readstest").await.unwrap();
    let errors = running.errors();
    for expected in [
        "[readstest: room: [Quiet Glade] 7000 [\"n\", \"e\"] true]",
        "[readstest: npcs: kobold loot: rock pcs: [\"Kiyna\"]]",
        "[readstest: hands: sword nil]",
        "[readstest: char: Nisugi 50/100 standing=true stunned=false]",
        "[readstest: spell: Heroism circle=2 known=true active=true checkspell=true]",
        "[readstest: costs: mana=15.0 on another=1.0 minutes affordable=true]",
        "[readstest: others: false 215 nil]",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the scripted game says to the commands that fill the sheet, in
/// the game's words (the model's tests': `character_blocks.rs`,
/// `character_skills.rs`, `crates/cena-protocol/tests/fixtures/psm_list.xml`).
const SHEET: &[(&str, &[u8])] = &[
    (
        "info",
        b"Name: Nisugi Race: Half-Elf  Profession: Ranger\n\
Gender: Male    Age: 36    Expr: 43904921    Level: 100\n\
    Strength (STR):   110 (30)    ...  115 (32)    ...  120 (35)\n\
<prompt time=\"2\">&gt;</prompt>\n",
    ),
    (
        "skills",
        b" Nisugi (at level 100), your current skill bonuses and ranks (including all modifiers) are:\n\
  Skill Name                         | Current Current\n\
                                     |   Bonus   Ranks\n\
  Two Weapon Combat..................|     312     212\n\
  Elemental Lore - Air...............|     150      50\n\
\n\
Spell Lists\n\
  Minor Elemental....................|              75\n\
\n\
Training Points: 3673 Phy 0 Mnt\n\
<prompt time=\"3\">&gt;</prompt>\n",
    ),
    (
        "society",
        b"   You are a member in the Order of Voln at step 12.\n<prompt time=\"4\">&gt;</prompt>\n",
    ),
    (
        "cman",
        b"<a exist=\"-10966483\" noun=\"Nisugi\">Nisugi</a>, the following Combat Maneuvers are available:\n\
\n\
<output class=\"mono\"/>\n\
  Skill                Mnemonic        Ranks Type           Category        Subcategory\n\
  -------------------------------------------------------------------------------------\n\
<pushBold/>  Combat Focus         <d cmd='cman HELP focus'>focus</d>           5/5   <d cmd='cman LIST passive'>Passive</d>\n\
<popBold/>  Dirtkick             <d cmd='cman HELP dirtkick'>dirtkick</d>        0/5   <d cmd='cman LIST setup'>Setup</d>\n\
<pushBold/>  Hamstring            <d cmd='cman HELP hamstring'>hamstring</d>       4/5   <d cmd='cman LIST setup'>Setup</d>\n\
<popBold/>\n\
   Subcategory: all\n\
<output class=\"\"/>\n\
<prompt time=\"5\">&gt;</prompt>\n",
    ),
    (
        "exp",
        b"<dialogData id='expr'><label id='yourLvl' value='Level 100' top='0' left='0'/>\
<progressBar id='mindState' value='25' text='fresh and clear' top='45' left='3' field_exp='270' max_field_exp='1403' ascension_exp='24899176' lumnis='4' exp='43904921' until_next='79'/>\
</dialogData>\n<prompt time=\"6\">&gt;</prompt>\n",
    ),
    (
        "injuries",
        b"<dialogData id='injuries'><image id='head' name='Injury2'/><image id='neck' name='Scar1'/>\
<radio id='injrRad' value='0' text='Injuries'/><radio id='scarRad' value='0' text='Scars'/><radio id='bothRad' value='1' text='Both'/></dialogData>\n\
You glance over your injuries.\n<prompt time=\"7\">&gt;</prompt>\n",
    ),
];

/// **The character's sheet** (`plan/46` §11 step 7): Lich's own `Stats`,
/// `Skills`, `Spells`, `Society`, `Experience`, `CMan`, `Warcry`, `Wounds`
/// and `Scars`, unchanged, reading what the game said of the character
/// through an `Infomon` answered from the copy; nil before it said.
#[tokio::test(flavor = "current_thread")]
async fn a_script_reads_its_sheet_through_lichs_own_classes() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("sheet");
    let scripts = scripts_with(&dir, "sheettest.lic").unwrap();
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: SHEET,
            ..World::default()
        },
    )
    .await
    .unwrap();
    let heard = running.run("sheettest", "sheettest").await.unwrap();
    let errors = running.errors();
    for expected in [
        "[sheettest: before: refresh=true skill=0 society=nil]",
        "[sheettest: stats: Half-Elf Ranger 115 32 base=[110, 30] enhanced=[120, 35] level=100]",
        "[sheettest: skills: 212 150 50 minor elemental=75]",
        "[sheettest: society: Order of Voln 12]",
        "[sheettest: experience: 270/1403 until=79 lumnis=4]",
        "[sheettest: psms: focus=5 hamstring=false dirtkick=false holler=false]",
        "[sheettest: injuries: head=2 neck=1 mode=2]",
        "[sheettest: after: refresh=false resources=nil]",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The map and the stores** (`plan/46` §11 step 2): `Room.current` named
/// by Hydra, its `wayto` walked with Lich's own `move`, and `CharSettings`
/// kept in `lich.db3` from one runner to the next: the second runner is a
/// new process, so what it finds can only have come from the file.
#[tokio::test(flavor = "current_thread")]
async fn a_script_walks_the_map_and_keeps_its_settings() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("walk");
    let scripts = scripts_with(&dir, "walktest.lic").unwrap();
    let mut running = in_the_quiet_glade(&scripts, &dir).await.unwrap();
    let heard = running.run("walktest", "walktest").await.unwrap();
    let errors = running.errors();
    for expected in [
        "[walktest: from 228 [Quiet Glade] north to 229]",
        "[walktest: moved true, now 229 [North Glade]]",
        "[walktest: seen [229]]",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    assert_eq!(heard.sent, [("north".to_owned(), Origin::Script)]);
    running.end().await;

    let mut running = in_the_quiet_glade(&scripts, &dir).await.unwrap();
    let heard = running.run("walktest", "walktest").await.unwrap();
    assert!(
        heard
            .told
            .iter()
            .any(|line| line == "[walktest: seen [229, 229]]"),
        "the first runner's setting, from lich.db3: {:#?}
runner's errors: {:#?}",
        heard.told,
        running.errors()
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Hydra's built-ins, started as Lich's scripts** (`plan/46` §11 step 3):
/// `Script.run('go2', ...)` walks by Hydra's travel and waits, Lich's go2
/// settings left behind, and the room it arrived in is read after it;
/// `start_script('go2', ...)` is seen running by Lich's own `running?` and
/// `exists?`, and killing it stops travel's walk.
#[tokio::test(flavor = "current_thread")]
async fn a_script_runs_go2_as_hydras_travel() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("builtins");
    let scripts = scripts_with(&dir, "builtintest.lic").unwrap();
    let mut running = in_the_quiet_glade(&scripts, &dir).await.unwrap();
    let heard = running.run("builtintest", "builtintest").await.unwrap();
    let errors = running.errors();
    for expected in [
        "[builtintest: walked true: now 229]",
        "[builtintest: far: running=true exists=true]",
        "[builtintest: stopped: running=false]",
        "[builtintest: stopped at once]",
        "[builtintest: stopped while starting]",
    ] {
        assert!(
            heard.told.iter().any(|line| line == expected),
            "{expected:?} not told: {:#?}\nrunner's errors: {errors:#?}",
            heard.told
        );
    }
    // Every far walk a killed script started is stopped: the one it waited
    // on, the one killed at once, and the slow one killed before its run's
    // number came back. Over HTTP from the runner's own thread, which may still be
    // starting the second when the script has gone: the counts must hold
    // equal for 300 ms, within ten seconds for a busy machine.
    let mut counts = running.walks.counts();
    let mut steady = 0;
    for _ in 0..500 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let now = running.walks.counts();
        steady = if now == counts && now.0 >= 2 && now.0 == now.1 {
            steady + 1
        } else {
            0
        };
        counts = now;
        if steady == 15 {
            break;
        }
    }
    assert!(
        counts.0 >= 2 && counts.0 == counts.1,
        "killing the script stops travel's walk: {counts:?} (started, stopped)"
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// A room of two lines, the second a breeze.
const BREEZY_ROOM: &[u8] =
    b"You see a quiet room.\nA breeze blows.\n<prompt time=\"3\">&gt;</prompt>\n";

/// **Hooks** (`plan/46` §11 step 4): Lich's own `DownstreamHook` and
/// `UpstreamHook`, asked by Hydra. While the script runs, the breeze is
/// hidden from the player and still heard by the script, the room shown
/// changed (its markup not drawn), and then `tt` sent as `target`; killed,
/// its hooks go with it, and the room is shown as the game sent it.
#[tokio::test(flavor = "current_thread")]
async fn a_scripts_hooks_change_what_is_shown_and_typed() {
    assert!(find_ruby().is_some(), "no Ruby: the runner needs Ruby 4.0");
    let dir = temp_dir("hooks");
    let scripts = scripts_with(&dir, "hooktest.lic").unwrap();
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: &[
                (
                    "glance",
                    b"You glance about.\n<prompt time=\"2\">&gt;</prompt>\n",
                ),
                ("look", BREEZY_ROOM),
                ("look", BREEZY_ROOM),
            ],
            // The first text builds the session's classifiers, which a debug
            // build takes past the hooks' half second over.
            before: &["glance"],
            ..World::default()
        },
    )
    .await
    .unwrap();
    assert!(running.typed("hooktest"));
    let heard = running.heard_until("[hooktest: hooked]").await;
    assert!(
        heard.finished,
        "not hooked: {:#?}\nrunner's errors: {:#?}",
        heard.told,
        running.errors()
    );

    // The display hook alone, first.
    running.typed_line("look").await;
    let heard = running.heard_until("[hooktest: typing hooked]").await;
    assert!(
        heard
            .told
            .iter()
            .any(|told| told == "[hooktest: heard the breeze]"),
        "the script hears what its hook hides: {:#?}",
        heard.told
    );
    let mut shown = heard.shown;
    if shown.is_empty() {
        shown = running.shown(1).await;
    }
    assert_eq!(
        shown,
        ["You see a loud room."],
        "runner's errors: {:#?}",
        running.errors()
    );
    running.typed_line("tt").await;
    assert_eq!(running.transcript.lines(), ["glance", "look", "target"]);

    // Its hooks go with it, told to Hydra by Lich's cleanup, which runs
    // before Lich says it was killed.
    assert!(running.typed("k hooktest"));
    let killed = running
        .heard_until_either("hooktest was killed", "hooktest has exited")
        .await;
    assert!(killed.told.iter().any(|told| told.contains("hooktest")));
    running.typed_line("look").await;
    assert_eq!(
        running.shown(2).await,
        ["You see a quiet room.", "A breeze blows."]
    );
    assert_eq!(
        running.transcript.lines(),
        ["glance", "look", "target", "look"]
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The second real script end to end** (`plan/46` §11 step 2): Tillmen's
/// `wander`, unchanged, from `CENA_LICH_SCRIPTS`. It walks out of the Quiet
/// Glade by the map, finds the kobold, targets it, and stops.
#[tokio::test(flavor = "current_thread")]
#[ignore = "Tier 2: needs CENA_LICH_SCRIPTS, a folder holding wander.lic; run with --ignored"]
async fn wander_runs_unchanged() {
    let scripts = PathBuf::from(
        std::env::var_os("CENA_LICH_SCRIPTS").expect("CENA_LICH_SCRIPTS names no folder"),
    );
    assert!(scripts.join("wander.lic").is_file(), "no wander.lic");
    let dir = temp_dir("wander");
    let mut running = in_the_quiet_glade(&scripts, &dir).await.unwrap();
    let heard = running.run("wander", "wander").await.unwrap();
    let errors = running.errors();
    assert!(heard.finished, "{:#?}\n{errors:#?}", heard.told);
    let sent: Vec<&str> = heard.sent.iter().map(|(line, _)| line.as_str()).collect();
    assert_eq!(
        sent,
        ["north", "target random"],
        "{:#?}\n{errors:#?}",
        heard.told
    );
    assert_eq!(
        running.transcript.lines(),
        ["look", "north", "target random"]
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The first real script end to end** (`plan/46` §11 step 1): Tillmen's
/// `trollspeak`, from the player's own Lich scripts, unchanged. It is not
/// ours to commit, so this reads it from the folder `CENA_LICH_SCRIPTS`
/// names (the author's: `reference/lich_repo_mirror/lib`).
#[tokio::test(flavor = "current_thread")]
#[ignore = "Tier 2: needs CENA_LICH_SCRIPTS, a folder holding trollspeak.lic; run with --ignored"]
async fn trollspeak_runs_unchanged() {
    let scripts = PathBuf::from(
        std::env::var_os("CENA_LICH_SCRIPTS").expect("CENA_LICH_SCRIPTS names no folder"),
    );
    assert!(
        scripts.join("trollspeak.lic").is_file(),
        "no trollspeak.lic"
    );
    let dir = temp_dir("trollspeak");
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            answers: &[("look", QUIET_ROOM)],
            ..World::default()
        },
    )
    .await
    .unwrap();

    let heard = running
        .run("trollspeak say hello there, friend", "trollspeak")
        .await
        .unwrap();
    assert!(heard.finished, "{:#?}\n{:#?}", heard.told, running.errors());
    let [(said, Origin::Script)] = heard.sent.as_slice() else {
        panic!("{:?}", heard.sent);
    };
    assert!(
        said.starts_with("say ") && said != "say hello there, friend",
        "{said}"
    );
    assert!(
        heard
            .told
            .iter()
            .any(|line| *line == format!("[trollspeak]>{said}")),
        "{:#?}",
        heard.told
    );

    let heard = running
        .run("trollspeak echo hello", "trollspeak")
        .await
        .unwrap();
    assert!(heard.sent.is_empty(), "echo sends nothing");
    assert!(
        heard
            .told
            .iter()
            .any(|line| line.starts_with("[trollspeak: ")),
        "{:#?}",
        heard.told
    );

    let heard = running.run("trollspeak", "trollspeak").await.unwrap();
    assert!(
        heard
            .told
            .iter()
            .any(|line| line.starts_with("Usage: ;trollspeak")),
        "{:#?}",
        heard.told
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Lich's windows** (`plan/46` §10 question 12; the author: *"for now we
/// will load gtk"*): with the gtk3 gem the runner loads it as Lich does, and
/// a script's window is made and closed on Gtk's own thread through Lich's
/// `Gtk.queue`; without it `HAVE_GTK` is false, as for a Lich started
/// `--no-gtk`.
#[tokio::test(flavor = "current_thread")]
async fn a_scripts_window_opens_when_gtk_is_there() {
    let ruby = find_ruby().expect("no Ruby: the runner needs Ruby 4.0");
    let gtk = std::process::Command::new(&ruby)
        .args([
            "-e",
            "exit(Gem::Specification.find_all_by_name('gtk3').empty? ? 1 : 0)",
        ])
        .status()
        .is_ok_and(|status| status.success());
    let dir = temp_dir("windows");
    let scripts = scripts_with(&dir, "windowtest.lic").unwrap();
    let mut running = Running::start(
        &scripts,
        &dir,
        World {
            windows: true,
            ..World::default()
        },
    )
    .await
    .unwrap();
    let heard = running.run("windowtest", "windowtest").await.unwrap();
    let expected = if gtk {
        "[windowtest: a window opened and closed]"
    } else {
        "[windowtest: no windows]"
    };
    assert!(
        heard.told.iter().any(|line| line == expected),
        "{expected:?} not told: {:#?}\nrunner's errors: {:#?}",
        heard.told,
        running.errors()
    );
    running.end().await;
    let _ = std::fs::remove_dir_all(&dir);
}
