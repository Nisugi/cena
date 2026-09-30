//! Each widget draws what it shows, found as a screen reader finds it.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use super::*;
use crate::fixture::{snapshot, story};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// Every kind, each in a strip of its own height, over `snapshot`.
fn drawn<'a>(snapshot: Option<Snapshot>, hunt: Option<HuntView>) -> Harness<'a, ()> {
    let story = story();
    Harness::builder()
        .with_size((520.0, 2400.0))
        .build_ui(move |ui| {
            let seen = Seen {
                snapshot: snapshot.as_ref(),
                story: &story,
                hunt: hunt.as_ref(),
                who: None,
                open: &[],
                minimap: None,
                tags: false,
            };
            for widget in Widget::all() {
                let height = widget.size().y.min(120.0);
                ui.allocate_ui(egui::vec2(500.0, height), |ui| {
                    widget.draw(ui, &seen, Id::new(("widget-test", widget.name())));
                });
            }
        })
}

#[test]
fn every_kind_is_listed_once_under_a_name_of_its_own() {
    let all = Widget::all();
    let kinds: HashSet<Widget> = all.iter().cloned().collect();
    assert_eq!(kinds.len(), all.len());
    let names: HashSet<String> = all
        .iter()
        .map(|widget| widget.name().into_owned())
        .collect();
    assert_eq!(names.len(), all.len());
}

#[test]
fn each_widget_draws_what_it_shows() {
    let mut ashryn = snapshot();
    // The clocks count on the wall clock; a test does not.
    ashryn.state.roundtime_ends = None;
    let hunt = HuntView {
        running: "ojandhaart".to_owned(),
        phase: "resting (out of mana)".to_owned(),
        doing: "waiting 5s".to_owned(),
        target: None,
        waiting: Some("mana 30%, wants 50%".to_owned()),
    };
    let harness = drawn(Some(ashryn), Some(hunt));
    for label in [
        "You swing a steel broadsword at a kobold!",
        ">look",
        "HP 348/400 87%",
        "MP 48/120 40%",
        "SP ?",
        "Sp ?",
        "Right: empty",
        "Left: a steel broadsword",
        "RT —",
        "CT —",
        "Rawknuckle's, Watering Hole",
        "[Rawknuckle's, Watering Hole]",
        "Description unknown",
        "a kobold",
        "Also here: Maravel",
        "Maravel",
        "Obvious exits: north, out",
        "Hunt: resting until mana is 50%.",
        "ojandhaart",
        "Waiting: mana 30%, wants 50%",
        "Spell: none",
    ] {
        // Some twice: the Room widget says the room whole, beside its parts.
        assert!(
            harness.query_all_by_label(label).next().is_some(),
            "{label}"
        );
    }
}

/// Before the game has said anything, each widget says what it does not
/// know rather than drawing nothing.
#[test]
fn a_widget_says_what_it_does_not_know() {
    let harness = drawn(None, None);
    for (label, count) in [
        ("HP ?", 1),
        ("Right: ?", 1),
        ("Left: ?", 1),
        ("RT —", 1),
        ("Room unknown", 2),
        ("Description unknown", 1),
        ("unknown", 4),
        ("Exits unknown", 2),
        ("No hunt running.", 1),
        ("Spells unknown", 1),
        ("Reserve unknown", 1),
        ("Containers unknown", 1),
        ("World events unknown", 1),
        ("Pulse ?", 1),
    ] {
        assert_eq!(harness.query_all_by_label(label).count(), count, "{label}");
    }
}

/// A one-line widget stays one line in a narrow cell, however long what it
/// says: cut short, never wrapped onto a line its cell has not got.
#[test]
fn a_one_line_widget_stays_one_line() {
    let mut ashryn = snapshot();
    ashryn.state.room.title =
        Some("[Wehnimer's Landing, Town Square Central, beside the old fountain]".to_owned());
    ashryn.state.room.exits = Some(
        [
            "north",
            "northeast",
            "east",
            "southeast",
            "south",
            "southwest",
            "west",
            "out",
        ]
        .map(str::to_owned)
        .to_vec(),
    );
    ashryn.state.apply(&cena_session::Frame::LeftHand {
        item: "a gleaming vultite greatsword etched with twisting runes".to_owned(),
        link: None,
    });
    let heights = Arc::new(Mutex::new(Vec::new()));
    let seen_heights = Arc::clone(&heights);
    let story = story();
    let mut harness = Harness::builder()
        .with_size((400.0, 400.0))
        .build_ui(move |ui| {
            let seen = Seen {
                snapshot: Some(&ashryn),
                story: &story,
                hunt: None,
                who: None,
                open: &[],
                minimap: None,
                tags: false,
            };
            let mut heights = seen_heights
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            heights.clear();
            for widget in [Widget::RoomTitle, Widget::Exits, Widget::LeftHand] {
                let used = ui
                    .allocate_ui(egui::vec2(120.0, LINE), |ui| {
                        widget.draw(ui, &seen, Id::new(("one-line", widget.name())));
                    })
                    .response
                    .rect
                    .height();
                heights.push((widget, used));
            }
        });
    harness.run();
    let heights = heights
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(heights.len(), 3);
    for (widget, used) in heights.iter() {
        assert!(*used <= LINE, "{widget:?} took {used}");
    }
}

/// What the character is and has, each widget saying it as the game did.
#[test]
fn the_characters_widgets_say_what_it_is() {
    let mut ashryn = snapshot();
    let character = &mut ashryn.state.character;
    character.stance = Some("defensive (100%)".to_owned());
    character.stance_percent = Some(100);
    character.encumbrance = Some("Light".to_owned());
    character.encumbrance_percent = Some(20);
    character.encumbrance_detail = Some("You feel slightly weighed down.".to_owned());
    let experience = &mut character.experience;
    experience.mind_state = Some("clear as a bell".to_owned());
    experience.mind_percent = Some(0);
    experience.next_level = Some("11999265 experience".to_owned());
    experience.next_level_percent = Some(40);
    experience.level = Some("Level 100".to_owned());
    experience.physical_training = Some(3673);
    experience.mental_training = Some(0);
    experience.total_experience = Some(68_809_898);
    experience.field_experience = Some(12);
    experience.field_experience_max = Some(1403);
    experience.ascension_experience = Some(24_904_977);
    let standing = &mut character.standing;
    standing.society = Some(Some(cena_session::Society::CouncilOfLight));
    standing.society_rank = Some(20);
    standing.covert_arts_charges = Some(120);
    standing.suffused = Some(1_500);
    standing.shadow_essence = Some(3);
    ashryn.state.prepared = Some("Spirit Warding I".to_owned());
    let harness = drawn(Some(ashryn), None);
    for label in [
        "Stance: defensive 100%",
        "Encumbrance: Light 20%",
        "You feel slightly weighed down.",
        "Mind: clear as a bell 0%",
        "Next level: 11999265 experience 40%",
        "Level 100",
        "PTPs 3,673 · MTPs 0",
        "Total 68,809,898",
        "Field 12/1,403",
        "Ascension 24,904,977",
        "Spell: Spirit Warding I",
        "Council of Light, rank 20",
        "Covert Arts charges: 120/200",
        "Suffused: 1,500",
        "Shadow essence: 3/5",
        "No objectives",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
}

/// Before the game has told them, the character's widgets say so.
#[test]
fn the_characters_widgets_say_what_is_not_known() {
    let harness = drawn(None, None);
    for label in [
        "Stance ?",
        "Encumbrance ?",
        "Encumbrance unknown",
        "Mind ?",
        "Level ?",
        "PTPs ? · MTPs ?",
        "Experience unknown",
        "Spell: ?",
        "Society unknown",
        "Resources unknown",
        "Objectives unknown",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
}

/// Numbers are grouped by thousands, as the game writes them.
#[test]
fn numbers_are_grouped_as_the_game_writes_them() {
    for (n, said) in [
        (0, "0"),
        (999, "999"),
        (1_000, "1,000"),
        (68_809_898, "68,809,898"),
    ] {
        assert_eq!(character::grouped(n), said);
    }
}

/// Each indicator says whether it is on, off, or not yet told.
#[test]
fn an_indicator_says_whether_it_is_on() {
    let mut ashryn = snapshot();
    ashryn.state.status.set("stunned", true);
    ashryn.state.status.set("hidden", false);
    let harness = drawn(Some(ashryn), None);
    for label in [
        "Stunned: yes",
        "Hidden: no",
        "Poisoned: unknown",
        "Grouped: unknown",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
}

/// A list of effects: each by name, with its time left when it has one,
/// and none said so.
#[test]
fn a_list_of_effects_says_each_and_its_time() {
    let mut ashryn = snapshot();
    let now = ashryn.state.game_time_now().expect("a clock");
    let effect = |category: &str, text: &str, ends_at| cena_session::Effect {
        category: category.to_owned(),
        text: text.to_owned(),
        ends_at,
        percent: 80,
    };
    ashryn.state.effects.insert(
        "1".to_owned(),
        effect("Buffs", "Rapid Fire", Some(now + 119)),
    );
    ashryn.state.effects.insert(
        "2".to_owned(),
        effect("Active Spells", "Spirit Warding I", None),
    );
    let harness = drawn(Some(ashryn), None);
    assert!(
        harness.query_by_label("Spirit Warding I").is_some(),
        "indefinite"
    );
    assert!(
        harness.query_by_label_contains("Rapid Fire 1:5").is_some(),
        "about two minutes left"
    );
    assert_eq!(
        harness.query_all_by_label("None.").count(),
        2,
        "Debuffs and Cooldowns"
    );
}

/// Seconds left read as a clock.
#[test]
fn time_left_reads_as_a_clock() {
    for (seconds, said) in [(0, "0:00"), (59, "0:59"), (119, "1:59"), (3723, "1:02:03")] {
        assert_eq!(super::status::clock(seconds), said);
    }
}

/// Indicators and a list of effects as drawn: some indicators on, some
/// off, one never told; buffs, full and less so.
#[test]
fn indicators_and_effects_as_drawn() {
    let mut ashryn = snapshot();
    for (id, on) in [
        ("standing", true),
        ("stunned", true),
        ("hidden", false),
        ("poisoned", true),
        ("bleeding", false),
    ] {
        ashryn.state.status.set(id, on);
    }
    for (id, text, ends, percent) in [
        ("1", "Rapid Fire", None, 100),
        ("2", "Spirit Warding I", None, 60),
        // No time left shown: it counts on the wall clock, and an image must
        // not depend on how fast the machine rendering it is.
        ("3", "Mass Blur", None, 30),
    ] {
        ashryn.state.effects.insert(
            id.to_owned(),
            cena_session::Effect {
                category: "Buffs".to_owned(),
                text: text.to_owned(),
                ends_at: ends,
                percent,
            },
        );
    }
    let story = story();
    let mut harness = Harness::builder()
        .with_size((340.0, 220.0))
        .wgpu()
        .build_ui(move |ui| {
            let seen = Seen {
                snapshot: Some(&ashryn),
                story: &story,
                hunt: None,
                who: None,
                open: &[],
                minimap: None,
                tags: false,
            };
            ui.horizontal(|ui| {
                for indicator in [
                    Indicator::Standing,
                    Indicator::Stunned,
                    Indicator::Hidden,
                    Indicator::Poisoned,
                    Indicator::Dead,
                ] {
                    ui.allocate_ui(egui::vec2(60.0, LINE), |ui| {
                        Widget::Indicator(indicator).draw(ui, &seen, Id::new(indicator));
                    });
                }
            });
            ui.allocate_ui(egui::vec2(320.0, 100.0), |ui| {
                Widget::Effects(Category::Buffs).draw(ui, &seen, Id::new("buffs"));
            });
        });
    harness.run();
    harness.snapshot("status");
}

/// A compass over `snapshot`, for `who` when another character's, and the
/// lines its clicks asked to send.
fn compass<'a>(
    snapshot: Snapshot,
    who: Option<&'static str>,
) -> (Harness<'a, ()>, Arc<Mutex<Vec<String>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let heard = Arc::clone(&sent);
    let story = story();
    let harness = Harness::builder()
        .with_size((200.0, 150.0))
        .build_ui(move |ui| {
            let seen = Seen {
                snapshot: Some(&snapshot),
                story: &story,
                hunt: None,
                who,
                open: &[],
                minimap: None,
                tags: false,
            };
            if let Some(super::Clicked::Send(line)) =
                Widget::Compass.draw(ui, &seen, Id::new("compass"))
            {
                heard
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(line);
            }
        });
    (harness, sent)
}

/// The compass lights the room's ways out; a click on one goes that way,
/// and on one the room has not, nowhere.
#[test]
fn the_compass_lights_the_ways_out_and_goes() {
    let (mut harness, sent) = compass(snapshot(), None);
    harness.run();
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "south")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "north")
        .click();
    harness.run();
    assert_eq!(
        *sent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["north"]
    );
}

/// Another character's compass goes nowhere: a click would move this
/// window's character.
#[test]
fn another_characters_compass_goes_nowhere() {
    let (mut harness, sent) = compass(snapshot(), Some("Baelor"));
    harness.run();
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "north")
        .click();
    harness.run();
    assert!(
        sent.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

/// The combat list: the character and its friends, then its foes with
/// their statuses, or that there are none.
#[test]
fn the_combat_list_sorts_friends_from_foes() {
    use super::room::{Fighter, fighting};
    let fighter = |name: &str, friend, dead, statuses: &[&str], health| Fighter {
        name: name.to_owned(),
        friend,
        dead,
        statuses: statuses.iter().map(|status| (*status).to_owned()).collect(),
        health,
    };
    let fighters = vec![
        fighter("a spirit guide", true, false, &[], None),
        fighter(
            "a kobold",
            false,
            false,
            &["stunned", "off balance"],
            Some(40),
        ),
        fighter("a rat", false, true, &[], Some(0)),
    ];
    let harness = Harness::builder()
        .with_size((300.0, 300.0))
        .build_ui(move |ui| {
            fighting(ui, Some("defensive (100%)"), &fighters);
            fighting(ui, None, &[]);
        });
    for (label, count) in [
        ("FRIENDLY", 2),
        ("You", 2),
        ("defensive (100%)", 1),
        ("a spirit guide", 1),
        ("FOES", 2),
        ("a kobold", 1),
        ("stunned, off balance", 1),
        ("a rat", 1),
        ("dead", 1),
        ("No foes.", 1),
    ] {
        assert_eq!(harness.query_all_by_label(label).count(), count, "{label}");
    }
}

/// A compass and a combat list as drawn.
#[test]
fn the_compass_and_combat_as_drawn() {
    use super::room::{Fighter, fighting};
    let mut ashryn = snapshot();
    ashryn.state.room.exits = Some(["north", "east", "up", "out"].map(str::to_owned).to_vec());
    let story = story();
    let fighters = vec![Fighter {
        name: "a kobold".to_owned(),
        friend: false,
        dead: false,
        statuses: vec!["stunned".to_owned()],
        health: Some(40),
    }];
    let mut harness = Harness::builder()
        .with_size((420.0, 150.0))
        .wgpu()
        .build_ui(move |ui| {
            let seen = Seen {
                snapshot: Some(&ashryn),
                story: &story,
                hunt: None,
                who: None,
                open: &[],
                minimap: None,
                tags: false,
            };
            ui.horizontal(|ui| {
                ui.allocate_ui(egui::vec2(160.0, 120.0), |ui| {
                    let _ = Widget::Compass.draw(ui, &seen, Id::new("compass"));
                });
                ui.allocate_ui(egui::vec2(240.0, 140.0), |ui| {
                    ui.vertical(|ui| fighting(ui, Some("defensive (100%)"), &fighters));
                });
            });
        });
    harness.run();
    harness.snapshot("room");
}

/// A stream's widget shows its lines, or that none has come yet; its count
/// is how many it heard.
#[test]
fn a_streams_widget_shows_its_lines() {
    let mut story = story();
    story.hear(
        &cena_session::ObservedEvent {
            session: cena_session::SessionId::FIRST,
            generation: cena_session::Generation::FIRST,
            cursor: 5,
            event: cena_session::Event::Line(Arc::new(cena_session::Line::new(
                "thoughts",
                cena_session::ChunkLine::plain("[General] hello").runs,
            ))),
        },
        None,
    );
    let thoughts = Widget::Stream("thoughts".to_owned());
    let speech = Widget::Stream("speech".to_owned());
    let seen = Seen {
        snapshot: None,
        story: &story,
        hunt: None,
        who: None,
        open: &[],
        minimap: None,
        tags: false,
    };
    assert_eq!(thoughts.count(&seen), Some(1));
    assert_eq!(speech.count(&seen), Some(0));
    assert_eq!(thoughts.name(), "Thoughts");
    assert_eq!(Widget::Stream("mentor".to_owned()).name(), "Mentor");
    let harness = Harness::builder()
        .with_size((300.0, 200.0))
        .build_ui(move |ui| {
            let seen = Seen {
                snapshot: None,
                story: &story,
                hunt: None,
                who: None,
                open: &[],
                minimap: None,
                tags: false,
            };
            ui.allocate_ui(egui::vec2(280.0, 80.0), |ui| {
                let _ = thoughts.draw(ui, &seen, Id::new("thoughts"));
            });
            ui.allocate_ui(egui::vec2(280.0, 80.0), |ui| {
                let _ = speech.draw(ui, &seen, Id::new("speech"));
            });
        });
    assert!(harness.query_by_label("[General] hello").is_some());
    assert!(harness.query_by_label("Nothing yet.").is_some());
}

/// The spellbook lists each spell under its circle, as the game's Spells
/// window does; the reserve numbers what it holds from R1; a container
/// opens to what it holds.
#[test]
fn the_lists_say_what_the_character_has() {
    use super::lists::{containers_listed, reserved, spells_listed};
    let mut harness = Harness::builder().with_size((300.0, 500.0)).build_ui(|ui| {
        spells_listed(
            ui,
            &[
                (101, "Spirit Warding I", Some("Minor Spiritual")),
                (103, "Spirit Defense", Some("Minor Spiritual")),
                (401, "Elemental Defense I", Some("Minor Elemental")),
            ],
        );
        spells_listed(ui, &[]);
        reserved(ui, Some(&["a steel broadsword", "a buckler"]));
        reserved(ui, Some(&[]));
        reserved(ui, None);
        let _ = containers_listed(
            ui,
            &[
                (
                    "My Cloak".to_owned(),
                    "64863904".to_owned(),
                    vec![("a gold ring", "1"), ("a pink pearl", "2")],
                ),
                ("stow".to_owned(), "64863905".to_owned(), vec![]),
            ],
            true,
        );
        let _ = containers_listed(ui, &[], true);
    });
    harness.run();
    for (label, count) in [
        ("MINOR SPIRITUAL", 1),
        ("MINOR ELEMENTAL", 1),
        ("101 Spirit Warding I", 1),
        ("401 Elemental Defense I", 1),
        ("No spells listed", 1),
        ("R1: a steel broadsword", 1),
        ("R2: a buckler", 1),
        ("Nothing in reserve", 1),
        ("Reserve unknown", 1),
        ("My Cloak (2)", 1),
        ("stow (0)", 1),
        ("No container seen yet", 1),
        ("a gold ring", 0),
    ] {
        assert_eq!(harness.query_all_by_label(label).count(), count, "{label}");
    }
    harness.get_by_label("My Cloak (2)").click();
    harness.run();
    assert!(harness.query_by_label("a gold ring").is_some(), "opened");
}

/// The pulse bar asks before the first pulse or the clock, counts down to
/// the next, says a mana pulse, and is due once the least has passed.
#[test]
fn the_pulse_bar_says_when_the_next_comes() {
    use super::status::pulse_said;
    use cena_session::world::Pulse;
    let pulse = |mana| Pulse {
        at: Some(1_000),
        min: 46,
        max: 75,
        mana,
    };
    assert_eq!(
        pulse_said(None, Some(1_000), "Pulse"),
        ("Pulse ?".to_owned(), None)
    );
    assert_eq!(
        pulse_said(Some(&pulse(false)), None, "Pulse"),
        ("Pulse ?".to_owned(), None),
        "no clock"
    );
    assert_eq!(
        pulse_said(Some(&pulse(false)), Some(1_000), "Pulse"),
        ("Next pulse in 46-75s".to_owned(), Some(0))
    );
    assert_eq!(
        pulse_said(Some(&pulse(true)), Some(1_023), "Pulse"),
        ("Next mana pulse in 23-52s".to_owned(), Some(50))
    );
    assert_eq!(
        pulse_said(Some(&pulse(false)), Some(1_046), "Pulse"),
        ("Pulse due".to_owned(), Some(100))
    );
}

/// World events, each with where and how long it has left, or none.
#[test]
fn world_events_say_where_and_how_long() {
    use super::status::events_listed;
    let listed = |events: &'static [(Option<&str>, &str, Option<u32>)]| {
        Harness::builder()
            .with_size((400.0, 200.0))
            .build_ui(move |ui| events_listed(ui, events))
    };
    let some = listed(&[
        (Some("Wehnimer's Landing"), "An invasion begins!", Some(600)),
        (None, "The moons align.", None),
    ]);
    for label in [
        "Wehnimer's Landing: An invasion begins! (10:00 left)",
        "The moons align.",
    ] {
        assert!(some.query_by_label(label).is_some(), "{label}");
    }
    assert!(some.query_by_label("No world events.").is_none());
    assert!(listed(&[]).query_by_label("No world events.").is_some());
}

/// A bar's overlay is an image the player puts in the data folder: read
/// from its path and kept; a path with nothing there gives none, and the bar
/// is drawn bare.
#[test]
fn an_overlay_is_read_from_its_file() {
    let dir = std::env::temp_dir().join(format!("cena-overlay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("made");
    let png = dir.join("gloss.png");
    image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 255, 255, 128]))
        .save(&png)
        .expect("written");
    let path = png.display().to_string();
    let missing = dir.join("none.png").display().to_string();
    let mut harness = Harness::new_ui_state(
        move |ui, found: &mut (bool, bool)| {
            *found = (
                super::draw::overlay(ui, &path).is_some(),
                super::draw::overlay(ui, &missing).is_some(),
            );
        },
        (false, false),
    );
    harness.run();
    assert_eq!(*harness.state(), (true, false));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Field experience and the betrayer's Blood Points as bars (the author,
/// 2026-09-30): the mind bar's pair, and the Betrayer panel's label out of
/// 100.
#[test]
fn field_experience_and_blood_points_are_bars() {
    let mut ashryn = crate::fixture::snapshot();
    let state = &mut ashryn.state;
    state.character.experience.field_experience = Some(648);
    state.character.experience.field_experience_max = Some(1_403);
    state.apply(&cena_session::Frame::Label {
        id: "lblBPs".to_owned(),
        value: "Blood Points: 50".to_owned(),
        dialog: Some("BetrayerPanel".to_owned()),
        attrs: Vec::new(),
    });
    let harness = drawn(Some(ashryn), None);
    assert!(harness.query_by_label("Field 648/1403 46%").is_some());
    assert!(harness.query_by_label("Blood Points 50/100 50%").is_some());
}

/// The pulse as a clock: seconds to its earliest, then below zero until it
/// comes (the author, 2026-09-30: *"20 and be counting down then after 20
/// seconds it would be 0, -1, -2, -3, until the pulse"*).
#[test]
fn the_pulse_clock_counts_below_zero() {
    let pulse = cena_session::world::Pulse {
        at: Some(1_000),
        min: 46,
        max: 75,
        mana: true,
    };
    let clock = |now| super::status::pulse_clock(&pulse, now);
    assert_eq!(clock(1_026), Some(20));
    assert_eq!(clock(1_046), Some(0));
    assert_eq!(clock(1_049), Some(-3));
    let unclocked = cena_session::world::Pulse {
        at: None,
        ..pulse.clone()
    };
    assert_eq!(super::status::pulse_clock(&unclocked, 1_049), None);
}
