//! bigshot's switches for trouble and death (`hunt/death.rs`,
//! `bigshot.lic:6755-6790`).

use cena_behavior::hunt::engine::Phase;
use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_behavior::waggle::WaggleProfile;
use cena_map::RoomId;
use cena_session::{Amount, Frame, GameState, ProgressBar, Runs};

/// A bar: `id` at `percent`.
fn bar(state: &mut GameState, id: &str, percent: u32) {
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: id.to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent,
        text: format!("{id} {percent}/100"),
        amount: Some(Amount {
            current: i32::try_from(percent).unwrap_or(0),
            max: 100,
        }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

/// Second `second` in room 10 on `instance`, at `health` percent, dead or not.
fn at(second: u32, instance: &str, health: u32, dead: bool) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state.room.id = Some("10".to_owned());
    state.character.instance = Some(instance.to_owned());
    state.status.set("standing", !dead);
    state.status.set("dead", dead);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    bar(&mut state, "health", health);
    bar(&mut state, "spirit", 100);
    state
}

/// A hunt with these `[react]` switches.
fn hunt(react: &str) -> Result<Hunt, String> {
    let profile = Profile::parse(&format!(
        "[rooms]\nhunting = 10\nresting = 20\n[react]\n{react}\n"
    ))?;
    Ok(Hunt::new(profile, 1))
}

fn tick(hunt: &mut Hunt, state: &GameState) -> Said {
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    hunt.tick(state, here, state.game_time_now())
}

fn line(said: &Said) -> Option<&str> {
    match said {
        Said::Send { line, .. } => Some(line),
        _ => None,
    }
}

#[test]
fn the_dead_mans_switch_quits_on_shattered_only() {
    let mut on = hunt("dead_man_switch = true").unwrap();
    assert_eq!(
        tick(&mut on, &at(1_000, "Shattered", 39, false)),
        Said::Done(Ending::Trouble),
        "below 40 percent"
    );
    let mut dead = hunt("dead_man_switch = true").unwrap();
    assert_eq!(
        tick(&mut dead, &at(1_000, "Shattered", 100, true)),
        Said::Done(Ending::Trouble)
    );
    let mut fine = hunt("dead_man_switch = true").unwrap();
    assert_ne!(
        tick(&mut fine, &at(1_000, "Shattered", 40, false)),
        Said::Done(Ending::Trouble),
        "40 is not below 40"
    );
    let mut prime = hunt("dead_man_switch = true").unwrap();
    assert_ne!(
        tick(&mut prime, &at(1_000, "Prime", 10, false)),
        Said::Done(Ending::Trouble),
        "bigshot's switch is for GSF alone"
    );
    let mut off = hunt("").unwrap();
    assert_ne!(
        tick(&mut off, &at(1_000, "Shattered", 10, false)),
        Said::Done(Ending::Trouble)
    );
}

#[test]
fn without_the_depart_switch_death_ends_the_hunt() {
    let mut h = hunt("").unwrap();
    assert_eq!(
        tick(&mut h, &at(1_000, "Prime", 0, true)),
        Said::Done(Ending::Dead)
    );
}

#[test]
fn the_depart_switch_departs_waggles_waits_and_goes_back() {
    let mut h = hunt("depart_switch = true")
        .unwrap()
        .with_waggle(WaggleProfile {
            cast_list: vec![401],
            ..WaggleProfile::default()
        });
    let dead = at(1_000, "Prime", 0, true);
    let lines: Vec<Option<String>> = (0..4)
        .map(|_| line(&tick(&mut h, &dead)).map(str::to_owned))
        .collect();
    assert_eq!(
        lines,
        [
            Some("depart".to_owned()),
            Some("depart".to_owned()),
            Some("depart confirm".to_owned()),
            Some("depart confirm".to_owned()),
        ]
    );
    // Departed, alive again in town.
    let alive = at(1_010, "Prime", 100, false);
    assert_eq!(tick(&mut h, &alive), Said::Wait(1));
    assert_eq!(
        tick(&mut h, &alive),
        Said::Waggle(Vec::new()),
        "ewaggle on itself"
    );
    assert_eq!(tick(&mut h, &alive), Said::Wait(5));
    let minute = at(1_071, "Prime", 100, false);
    assert_eq!(
        line(&tick(&mut h, &minute)),
        Some("info"),
        "info once a minute"
    );
    assert_eq!(tick(&mut h, &minute), Said::Wait(5));
    // Fifteen minutes on, spirit not full: still waiting.
    let mut weak = at(1_011 + 900, "Prime", 100, false);
    bar(&mut weak, "spirit", 90);
    assert_eq!(tick(&mut h, &weak), Said::Wait(5));
    let whole = at(1_011 + 900, "Prime", 100, false);
    let back = tick(&mut h, &whole);
    // Back to the hunt: the walk back ends at once, room 10 being the
    // hunting room, and the prepare commands are next.
    assert_eq!(h.phase(), Phase::Preparing, "{back:?}");
}
