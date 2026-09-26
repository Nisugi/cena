//! Lich's `available?` before a maneuver, technique, shield move or feat
//! (`psms.rb:142-145`, the model's `psm_availability`), bigshot's Shield
//! Bash maneuver before the shield move, and the spells `cmd_spell` will
//! not cast at a creature they already hold.

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{
    Amount, Effect, Frame, GameState, Link, LinkKind, ProgressBar, PsmCategory, PsmLine, PsmRanks,
    Run, Runs, TextFrame,
};

fn link(noun: &str) -> Link {
    Link {
        kind: LinkKind::Exist {
            id: "42".to_owned(),
            noun: noun.to_owned(),
        },
        text: noun.to_owned(),
        coord: None,
    }
}

/// Kobold #42 targeted in room 10 at second 1000, with `stamina` points.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn fighting(stamina: i32) -> GameState {
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
    let mut run = Run {
        text: "kobold".to_owned(),
        style: Default::default(),
        link: Some(link("kobold")),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: vec![run] },
    });
    state.apply(&Frame::CreatureStatus {
        id: "42".into(),
        attrs: vec![
            ("exist".to_owned(), "42".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
    state.targeting.read("#42", None);
    // The Cooldowns and Debuffs dialogs stated, empty, as the login sends
    // them: `available?` is unknown until they are.
    state.effects.clear_category("Cooldowns");
    state.effects.clear_category("Debuffs");
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "stamina".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 100,
        text: format!("stamina {stamina}/100"),
        amount: Some(Amount {
            current: stamina,
            max: 100,
        }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
    state
}

/// `state` with `category`'s table read, holding `trained` at 1 rank.
fn trained(mut state: GameState, category: PsmCategory, trained: &[&str]) -> GameState {
    let rows: Vec<PsmLine> = trained
        .iter()
        .map(|mnemonic| PsmLine {
            mnemonic: (*mnemonic).to_owned(),
            display_name: (*mnemonic).to_owned(),
            ranks: PsmRanks {
                ranks: 1,
                max: 5,
                known_by_bold: false,
            },
            kind: String::new(),
            category: String::new(),
        })
        .collect();
    state.character.psms.replace_category(category, &rows);
    state
}

fn first(step: &str, state: &GameState) -> String {
    let profile = format!(
        "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\n[routines]\na = [\"{step}\"]\n"
    );
    let Ok(profile) = Profile::parse(&profile) else {
        return "profile".to_owned();
    };
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    match Hunt::new(profile, 1).tick(state, here, state.game_time_now()) {
        Said::Send { line, .. } => line,
        Said::Wait(n) => format!("wait {n}"),
        _ => "nothing".to_owned(),
    }
}

#[test]
fn a_maneuver_goes_only_when_trained_and_affordable_and_unknown_lets_it_go() {
    // Sweep, not coup de grace: a trained coup meets bigshot's own
    // last-moment gate first, and an unhurt kobold does not qualify.
    use PsmCategory::CombatManeuver as Cman;
    assert_eq!(
        first("sweep", &fighting(100)),
        "cman sweep #42",
        "the table never read"
    );
    assert_eq!(
        first("sweep", &trained(fighting(100), Cman, &["trip"])),
        "wait 1",
        "not trained"
    );
    let sweep = |stamina| trained(fighting(stamina), Cman, &["sweep"]);
    assert_eq!(first("sweep", &sweep(100)), "cman sweep #42");
    let cost = cena_session::psm_cost(Cman, "sweep").map_or(0, |c| i32::from(c.amount));
    assert!(cost > 0, "Lich's table costs sweep");
    assert_eq!(
        first("sweep", &sweep(cost)),
        "wait 1",
        "not more than the cost"
    );
    let mut spent = sweep(100);
    spent.effects.clear_category("Debuffs");
    spent.effects.insert(
        "o".to_owned(),
        Effect {
            category: "Debuffs".to_owned(),
            text: "Overexerted".to_owned(),
            ends_at: Some(1_060),
            percent: 50,
        },
    );
    assert_eq!(first("sweep", &spent), "wait 1", "overexerted");
}

#[test]
fn techniques_feats_and_burst_are_gated_the_same_way() {
    let weapon = trained(fighting(100), PsmCategory::Weapon, &["volley"]);
    assert_eq!(first("volley", &weapon), "weapon volley #42");
    let other = trained(fighting(100), PsmCategory::Weapon, &["barrage"]);
    assert_eq!(first("volley", &other), "wait 1");
    let feat = trained(fighting(100), PsmCategory::Feat, &["chastise"]);
    assert_eq!(first("chastise", &feat), "feat chastise #42");
    let no_feat = trained(fighting(100), PsmCategory::Feat, &[]);
    assert_eq!(first("chastise", &no_feat), "wait 1");
    let no_burst = trained(fighting(100), PsmCategory::CombatManeuver, &["sweep"]);
    assert_eq!(first("burst", &no_burst), "wait 1", "not trained");
}

#[test]
fn shield_bash_is_the_maneuver_when_it_is_available_else_the_shield_move() {
    let both = trained(
        trained(fighting(100), PsmCategory::CombatManeuver, &["sbash"]),
        PsmCategory::Shield,
        &["bash"],
    );
    assert_eq!(first("shield bash", &both), "cman sbash #42");
    let shield_only = trained(
        trained(fighting(100), PsmCategory::CombatManeuver, &[]),
        PsmCategory::Shield,
        &["bash"],
    );
    assert_eq!(first("shield bash", &shield_only), "shield bash #42");
    let neither = trained(
        trained(fighting(100), PsmCategory::CombatManeuver, &[]),
        PsmCategory::Shield,
        &[],
    );
    assert_eq!(first("shield bash", &neither), "wait 1");
    assert_eq!(
        first("shield bash", &fighting(100)),
        "shield bash #42",
        "unread tables let the shield move go"
    );
    assert_eq!(
        first("shield up", &fighting(100)),
        "shield up",
        "not a move"
    );
}

#[test]
#[expect(
    clippy::default_trait_access,
    reason = "the text's style type is not re-exported for behaviors"
)]
fn corrupt_essence_is_not_cast_at_a_creature_the_haze_holds() {
    let mut hazed = fighting(100);
    assert_eq!(first("703", &hazed), "prepare 703");
    // The line bigshot reads (`bigshot.lic:2767`), as the parser leaves it.
    for (at, (text, bold)) in [
        ("The ", false),
        ("kobold", true),
        (" is suddenly surrounded by a blood red haze.", false),
    ]
    .into_iter()
    .enumerate()
    {
        let mut frame = TextFrame {
            content: text.to_owned(),
            stream: String::new(),
            style: Default::default(),
            link: bold.then(|| link("kobold")),
            inner_link: None,
            ends_line: at == 2,
        };
        if bold {
            frame.style.bold_depth = 1;
        }
        hazed.apply(&Frame::Text(frame));
    }
    hazed.apply(&Frame::Prompt {
        time: "1001".into(),
        text: ">".into(),
    });
    assert_eq!(first("703", &hazed), "wait 1");
}
