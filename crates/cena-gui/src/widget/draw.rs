//! How each widget draws itself: moved here from the play window's panes,
//! which each drew several of these at once (`plan/47` steps 4 and 8).

use cena_session::hands::Hand;
use cena_session::{Body, GameState, Notice, NoticeKind, RoomItem, Snapshot, Vital};
use egui::{Color32, Id, RichText};

use super::{Seen, Widget, character, status};
use crate::bar::{self, Amount, Bar, Says};
use crate::story::Shown;
use crate::text::{self, AMBER, CREATURE, OBJECT, PLAYER, WRONG};

/// Draw `widget` for `seen` into `ui`. Following another character, a
/// one-line widget puts its name before what it says, and the rest a line
/// with its name above.
#[allow(
    clippy::too_many_lines,
    reason = "the catalog's table: one arm per kind, each a call; split, it would hide which kind draws how"
)]
pub(super) fn draw(widget: Widget, ui: &mut egui::Ui, seen: &Seen<'_>, id: Id) {
    let state = seen.snapshot.map(|snapshot| &snapshot.state);
    let named = |label: &str| {
        seen.who
            .map_or_else(|| label.to_owned(), |who| format!("{who} {label}"))
    };
    let scrolled = |ui: &mut egui::Ui, add: &mut dyn FnMut(&mut egui::Ui)| {
        scrolled(ui, id, |ui| {
            if let Some(who) = seen.who {
                ui.weak(who);
            }
            add(ui);
        });
    };
    match widget {
        Widget::Story => story(ui, &seen.story.lines, id),
        Widget::Hydra => hydra(ui, &seen.story.said, id),
        Widget::Hunt => scrolled(ui, &mut |ui| hunt(ui, seen.hunt)),
        Widget::Health => vital(
            ui,
            &named("HP"),
            state.and_then(GameState::health),
            bar::HEALTH,
        ),
        Widget::Mana => vital(ui, &named("MP"), state.and_then(GameState::mana), bar::MANA),
        Widget::Stamina => vital(
            ui,
            &named("SP"),
            state.and_then(GameState::stamina),
            bar::STAMINA,
        ),
        Widget::Spirit => vital(
            ui,
            &named("Sp"),
            state.and_then(GameState::spirit),
            bar::SPIRIT,
        ),
        Widget::RightHand => held(ui, &named("Right"), state.map(|state| &state.right_hand)),
        Widget::LeftHand => held(ui, &named("Left"), state.map(|state| &state.left_hand)),
        Widget::Roundtime => clock(
            ui,
            &named("RT"),
            state.and_then(GameState::roundtime_remaining),
            AMBER,
        ),
        Widget::CastTime => clock(
            ui,
            &named("CT"),
            state.and_then(GameState::casttime_remaining),
            bar::MANA,
        ),
        Widget::RoomTitle => line(
            ui,
            RichText::new(named(
                state
                    .and_then(|state| state.room.title.as_deref())
                    .unwrap_or("Room unknown"),
            ))
            .color(AMBER)
            .strong(),
        ),
        Widget::RoomDescription => scrolled(ui, &mut |ui| {
            match state.and_then(cena_ui::room_description) {
                Some(runs) => ui.label(text::job(&runs, ui.style())),
                None => ui.weak("Description unknown"),
            };
        }),
        Widget::Creatures => scrolled(ui, &mut |ui| {
            let known = state.filter(|state| state.room.component("room objs").is_some());
            items(
                ui,
                "Creatures",
                known.map(|state| &state.room.creatures[..]),
                CREATURE,
            );
        }),
        Widget::Objects => scrolled(ui, &mut |ui| {
            let known = state.filter(|state| state.room.component("room objs").is_some());
            items(
                ui,
                "Also here",
                known.map(|state| &state.room.objects[..]),
                OBJECT,
            );
        }),
        Widget::Players => scrolled(ui, &mut |ui| players(ui, seen.snapshot)),
        Widget::Stance => character::stance(ui, state, &named),
        Widget::Encumbrance => character::encumbrance(ui, state, &named),
        Widget::EncumbranceDetail => line(
            ui,
            named(
                state
                    .and_then(|state| state.character.encumbrance_detail.as_deref())
                    .unwrap_or("Encumbrance unknown"),
            ),
        ),
        Widget::Mind => character::mind(ui, state, &named),
        Widget::NextLevel => character::next_level(ui, state, &named),
        Widget::Level => line(ui, named(&character::level(state))),
        Widget::TrainingPoints => line(ui, named(&character::training(state))),
        Widget::ExperienceTotals => scrolled(ui, &mut |ui| character::experience(ui, state)),
        Widget::Prepared => line(ui, named(&character::prepared(state))),
        Widget::Society => line(ui, named(&character::society(state))),
        Widget::Resources => scrolled(ui, &mut |ui| character::resources(ui, state)),
        Widget::Objectives => scrolled(ui, &mut |ui| character::objectives(ui, state)),
        Widget::Indicator(indicator) => {
            status::indicator(ui, indicator, state, &named(indicator.name()));
        }
        Widget::Effects(category) => {
            scrolled(ui, &mut |ui| status::effects(ui, category, state));
        }
        Widget::Exits => line(
            ui,
            named(&match state.and_then(|state| state.room.exits.as_ref()) {
                Some(exits) if exits.is_empty() => "Obvious exits: none".to_owned(),
                Some(exits) => format!("Obvious exits: {}", exits.join(", ")),
                None => "Exits unknown".to_owned(),
            }),
        ),
    }
}

/// One line of a one-line widget: never wrapped onto a second, which its
/// cell has no room for, but cut short with an ellipsis, the whole of it
/// shown when the pointer rests on it.
fn line(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.add(egui::Label::new(text).truncate());
}

/// A widget whose content may be taller than it is given: it scrolls rather
/// than growing what holds it, and asks for no height of its own, which is
/// the layout's to say.
fn scrolled(ui: &mut egui::Ui, id: Id, add: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::vertical()
        .min_scrolled_height(0.0)
        .id_salt(id.with("scroll"))
        .auto_shrink(false)
        .show(ui, add);
}

/// A vital as a bar as wide as it is given: its label, numbers and percent.
fn vital(ui: &mut egui::Ui, label: &str, vital: Option<Vital>, color: Color32) {
    let amount = vital.map(|vital| Amount {
        percent: vital.percent,
        current: vital.current,
        max: vital.max,
    });
    ui.add(
        Bar::new(label, amount)
            .fill(color)
            .size([ui.available_width(), 18.0])
            .says(Says {
                label: true,
                numbers: true,
                percent: true,
            }),
    );
}

/// What a hand holds, after which hand: `?` until the game has said.
fn held(ui: &mut egui::Ui, which: &str, hand: Option<&Hand>) {
    let holds = match hand {
        None | Some(Hand::Unknown) => "?",
        Some(Hand::Empty) => "empty",
        Some(Hand::Holding { name, .. }) => name,
    };
    line(ui, format!("{which}: {holds}"));
}

/// A clock counting down, in whole seconds, or that none runs.
fn clock(ui: &mut egui::Ui, label: &str, seconds: Option<u32>, color: Color32) {
    match seconds.filter(|seconds| *seconds > 0) {
        Some(seconds) => line(
            ui,
            RichText::new(format!("{label} {seconds}s")).color(color),
        ),
        None => line(ui, RichText::new(format!("{label} —")).weak()),
    }
}

/// What the hunt is doing: what runs, where it is in its cycle, what it did
/// last, the creature it fights, and -- the reason for the widget, since a
/// stuck hunt is a waiting one -- why it waits.
fn hunt(ui: &mut egui::Ui, hunt: Option<&cena_ui::HuntView>) {
    let Some(hunt) = hunt else {
        ui.weak("No hunt running.");
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.strong(&hunt.running);
        ui.label(&hunt.phase);
    });
    ui.label(&hunt.doing);
    if let Some(target) = &hunt.target {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Fighting:");
            ui.colored_label(CREATURE, target);
        });
    }
    if let Some(waiting) = &hunt.waiting {
        ui.colored_label(AMBER, format!("Waiting: {waiting}"));
    }
}

/// A labelled list of room items, each with its status when it has one;
/// `None` while the room's contents are not yet known.
fn items(ui: &mut egui::Ui, label: &str, items: Option<&[RoomItem]>, color: Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.weak(format!("{label}:"));
        match items {
            None => {
                ui.weak("unknown");
            }
            Some([]) => {
                ui.weak("none");
            }
            Some(items) => {
                for item in items {
                    let text = match &item.status {
                        Some(status) => format!("{} ({status})", item.text),
                        None => item.text.clone(),
                    };
                    ui.colored_label(color, text);
                }
            }
        }
    });
}

/// The room's players, each painted by the character's triggers as Despana
/// paints them ([`cena_ui::room_player`]).
fn players(ui: &mut egui::Ui, snapshot: Option<&Snapshot>) {
    ui.horizontal_wrapped(|ui| {
        ui.weak("Players:");
        let Some(snapshot) = snapshot.filter(|snapshot| snapshot.state.room.saw_players()) else {
            ui.weak("unknown");
            return;
        };
        let (state, room) = (&snapshot.state, &snapshot.state.room);
        if room.players.is_empty() {
            ui.weak("none");
        }
        for player in &room.players {
            match cena_ui::room_player(&player.text, &snapshot.triggers, state) {
                Some(runs) => ui.label(text::job(&runs, ui.style())),
                None => ui.colored_label(PLAYER, &player.text),
            };
        }
    });
}

/// Hydra's own messages, the newest at the bottom.
fn hydra(ui: &mut egui::Ui, said: &std::collections::VecDeque<Notice>, id: Id) {
    egui::ScrollArea::vertical()
        .min_scrolled_height(0.0)
        .id_salt(id.with("said"))
        .stick_to_bottom(true)
        .auto_shrink(false)
        .show(ui, |ui| {
            if said.is_empty() {
                ui.weak("Nothing yet.");
            }
            for notice in said {
                notice_lines(ui, notice);
            }
        });
}

/// One of Hydra's messages, coloured by what it is about.
fn notice_lines(ui: &mut egui::Ui, notice: &Notice) {
    let color = match notice.kind {
        NoticeKind::Error => WRONG,
        NoticeKind::Warn => AMBER,
        NoticeKind::Info | NoticeKind::Debug => ui.visuals().text_color(),
    };
    match &notice.body {
        Body::Lines(lines) => {
            for line in lines {
                ui.colored_label(color, line);
            }
        }
        Body::Mono(lines) => {
            for line in lines {
                ui.label(RichText::new(line).monospace().color(color));
            }
        }
    }
}

/// The story, newest at the bottom, where it stays unless the player
/// scrolls back.
fn story(ui: &mut egui::Ui, lines: &std::collections::VecDeque<Shown>, id: Id) {
    egui::ScrollArea::vertical()
        .min_scrolled_height(0.0)
        .id_salt(id.with("story"))
        .stick_to_bottom(true)
        .auto_shrink(false)
        .show(ui, |ui| {
            for shown in lines {
                match shown {
                    Shown::Game(runs) => {
                        ui.label(text::job(runs, ui.style()));
                    }
                    Shown::Typed(line) => {
                        ui.weak(format!("> {line}"));
                    }
                    Shown::Gap => {
                        ui.colored_label(WRONG, "Some lines were missed here.");
                    }
                }
            }
        });
}
