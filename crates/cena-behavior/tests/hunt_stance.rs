//! The stance a step is sent in: the hunting stance before each step bigshot
//! takes it for (`bigshot.lic:4051-4053`), and the offensive stance for a
//! spell Lich's table marks as wanting it (`spell.rb:762-787`).

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs};

/// A bold creature link.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn kobold() -> Run {
    let mut run = Run {
        text: "kobold".to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "42".to_owned(),
                noun: "kobold".to_owned(),
            },
            text: "kobold".to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    run
}

/// Room 10, kobold #42 targeted, the stance bar at `percent`.
fn fighting(percent: u32) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some("10".to_owned());
    state.status.set("standing", true);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![kobold()],
        },
    });
    state.apply(&Frame::CreatureStatus {
        id: "42".into(),
        attrs: vec![
            ("exist".to_owned(), "42".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
    state.targeting.read("#42", None);
    state.character.stance_percent = Some(percent);
    state
}

/// The first `n` lines a routine of `step` sends, hunting in `hunting`.
fn lines(step: &str, hunting: &str, state: &GameState, n: usize) -> Vec<String> {
    let profile = match Profile::parse(&format!(
        "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\n[stance]\nhunting = \"{hunting}\"\n[routines]\na = [\"{step}\"]\n"
    )) {
        Ok(profile) => profile,
        // A profile that does not read fails the assertion that follows.
        Err(e) => return vec![format!("profile: {e}")],
    };
    let mut hunt = Hunt::new(profile, 1);
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    (0..n)
        .map(|_| match hunt.tick(state, here, state.game_time_now()) {
            Said::Send { line, .. } => line,
            Said::Wait(s) => format!("wait {s}"),
            _ => "nothing".to_owned(),
        })
        .collect()
}

#[test]
fn the_hunting_stance_goes_before_a_step_but_not_before_bigshots_exceptions() {
    let defensive = fighting(100);
    assert_eq!(
        lines("kick", "offensive", &defensive, 1),
        ["stance offensive"]
    );
    for (step, first) in [
        ("611", "prepare 611"),
        ("wait 3", "wait 1"),
        ("sleep 2", "wait 1"),
        ("hide", "hide"),
    ] {
        assert_eq!(
            lines(step, "offensive", &defensive, 1),
            [first],
            "`{step}` is sent in the stance it finds"
        );
    }
    assert_eq!(
        lines("kick", "offensive", &fighting(0), 1),
        ["kick"],
        "already offensive"
    );
}

#[test]
fn a_stance_spell_is_cast_offensive_and_the_stance_put_back() {
    // Minor Water (903) wants the offensive stance; a bare number is Lich's
    // own `cast`, which leaves the safest stance after.
    assert_eq!(
        lines("903", "defensive", &fighting(100), 4),
        [
            "stance offensive",
            "prepare 903",
            "cast #42",
            "stance guarded"
        ]
    );
    // `incant 903` goes back to the stance it found (bigshot's
    // `after_stance`).
    assert_eq!(
        lines("incant 903", "defensive", &fighting(100), 4),
        [
            "stance offensive",
            "prepare 903",
            "cast #42",
            "stance defensive"
        ]
    );
    // Offensive already, and hunting offensive: no dance at all.
    assert_eq!(
        lines("incant 903", "offensive", &fighting(0), 2),
        ["prepare 903", "cast #42"]
    );
    // A spell the table does not mark is cast in the stance it finds.
    assert_eq!(
        lines("611", "defensive", &fighting(100), 2),
        ["prepare 611", "cast #42"]
    );
}

#[test]
fn celerity_and_902_are_cast_with_no_target() {
    assert_eq!(lines("902", "defensive", &fighting(100), 1), ["incant 902"]);
    assert_eq!(lines("506", "defensive", &fighting(100), 1), ["incant 506"]);
}
