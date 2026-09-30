//! How each widget draws itself: moved here from the play window's panes,
//! which each drew several of these at once (`plan/47` steps 4 and 8).

use cena_session::hands::Hand;
use cena_session::{Body, GameState, Notice, NoticeKind, RoomItem, Snapshot, Vital};
use cena_ui::StyledRun;
use egui::{Color32, Id, RichText};

use super::{Seen, Widget, character, lists, room, status};
use crate::bar::{self, Amount, Bar};
use crate::text::{self, AMBER, CREATURE, OBJECT, WRONG};

/// Draw `widget` for `seen` into `ui`. Following another character, a
/// one-line widget puts its name before what it says, and the rest a line
/// with its name above. A line to send, when the player asked for one.
#[allow(
    clippy::too_many_lines,
    reason = "the catalog's table: one arm per kind, each a call; split, it would hide which kind draws how"
)]
pub(super) fn draw(
    widget: &Widget,
    ui: &mut egui::Ui,
    seen: &Seen<'_>,
    id: Id,
    chosen: &super::Chosen,
) -> Option<super::Clicked> {
    let state = seen.snapshot.map(|snapshot| &snapshot.state);
    let look = chosen.look.clone().or_else(|| widget.bar_look());
    let lines = chosen.lines.unwrap_or_default();
    let look = look.as_ref();
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
        // Never for another character's compass: a click would move this
        // window's character.
        Widget::Compass => {
            return room::compass(ui, state, seen.who.is_none()).map(super::Clicked::Send);
        }
        Widget::Combat => scrolled(ui, &mut |ui| room::combat(ui, state)),
        Widget::Injuries => {
            #[cfg(feature = "doll-infinite")]
            if chosen
                .doll
                .as_ref()
                .is_some_and(|look| look.style == super::doll::Style::Infinite)
                && super::infinite::infinite(
                    ui,
                    state,
                    id,
                    chosen.doll.as_ref().and_then(|look| look.skin.as_deref()),
                )
            {
                return None;
            }
            let injuries = state.map(|state| &state.character.injuries);
            if chosen
                .doll
                .as_ref()
                .is_some_and(|look| look.style == super::doll::Style::Text)
            {
                scrolled(ui, &mut |ui| super::doll_text::text(ui, injuries));
            } else {
                super::doll::doll(ui, injuries, chosen.doll.as_ref());
            }
        }
        // Another character's minimap only shows: a click would walk this
        // window's character.
        Widget::Minimap => {
            return super::minimap::minimap(
                ui,
                seen.minimap,
                id,
                seen.who.is_none(),
                chosen.minimap.unwrap_or_default(),
            );
        }
        // Another character's room only shows: its ids are not this
        // window's character's to send.
        Widget::Room => {
            let mut clicked = None;
            let own = seen.who.is_none();
            scrolled(ui, &mut |ui| {
                clicked =
                    super::described::room(ui, seen.snapshot, chosen.room.unwrap_or_default(), own);
            });
            return clicked;
        }
        Widget::Spellbook => scrolled(ui, &mut |ui| lists::spellbook(ui, state)),
        Widget::Reserve => scrolled(ui, &mut |ui| lists::reserve(ui, state)),
        Widget::Containers => {
            let mut put = None;
            let own = seen.who.is_none();
            scrolled(ui, &mut |ui| put = lists::containers(ui, state, own));
            return put.map(super::Clicked::Quietly);
        }
        Widget::Pulse => status::pulse(ui, state, &named("Pulse"), look),
        Widget::Dialog(dialog_id) => {
            let mut sent = None;
            let own = seen.who.is_none();
            scrolled(ui, &mut |ui| {
                sent = super::dialog::dialog(ui, state, dialog_id, own);
            });
            return sent;
        }
        Widget::WorldEvents => scrolled(ui, &mut |ui| status::world_events(ui, state)),
        Widget::Story => {
            return super::lines::story(ui, seen.story, seen.open, (id, lines));
        }
        Widget::Stream(stream_id) => {
            return super::lines::stream(ui, seen, stream_id, (id, lines));
        }
        Widget::Hydra => hydra(ui, &seen.story.said, id),
        Widget::Hunt => scrolled(ui, &mut |ui| hunt(ui, seen.hunt)),
        Widget::GameState => super::state::game_state(ui, state, seen.who, id),
        Widget::Health => vital(
            ui,
            &named("HP"),
            state.and_then(GameState::health),
            bar::HEALTH,
            look,
        ),
        Widget::Mana => vital(
            ui,
            &named("MP"),
            state.and_then(GameState::mana),
            bar::MANA,
            look,
        ),
        Widget::Stamina => vital(
            ui,
            &named("SP"),
            state.and_then(GameState::stamina),
            bar::STAMINA,
            look,
        ),
        Widget::Spirit => vital(
            ui,
            &named("Sp"),
            state.and_then(GameState::spirit),
            bar::SPIRIT,
            look,
        ),
        Widget::FieldExperience => vital(
            ui,
            &named("Field"),
            state.and_then(field_experience),
            bar::MIND,
            look,
        ),
        Widget::BloodPoints => vital(
            ui,
            &named("Blood Points"),
            state.and_then(blood_points),
            bar::BLOOD,
            look,
        ),
        Widget::RightHand => {
            let hand = state.map(|state| &state.right_hand);
            let clicked = held(ui, &named("Right"), hand, seen.who.is_none());
            return clicked.or_else(|| place(ui, id, "right", hand));
        }
        Widget::LeftHand => {
            let hand = state.map(|state| &state.left_hand);
            let clicked = held(ui, &named("Left"), hand, seen.who.is_none());
            return clicked.or_else(|| place(ui, id, "left", hand));
        }
        Widget::Roundtime => clock(
            ui,
            &named("RT"),
            state.and_then(GameState::roundtime_remaining),
            AMBER,
        ),
        Widget::Stun => stun(ui, &named("Stun"), state),
        // Where the character aims, as the game last said (the author,
        // 2026-09-30); the aim timer it was drawn from has never been seen
        // in the author's logs.
        Widget::Aim => line(
            ui,
            match state.and_then(|state| state.aiming.as_deref()) {
                Some(at) => RichText::new(named(&format!("Aim: {at}"))).color(AMBER),
                None => RichText::new(named("Aim —")).weak(),
            },
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
        Widget::Creatures | Widget::Npcs | Widget::Objects | Widget::Players => {
            let items = room_list(widget, seen.snapshot);
            let (listing, own) = (chosen.list.unwrap_or_default(), seen.who.is_none());
            let mut clicked = None;
            scrolled(ui, &mut |ui| {
                clicked = super::listed::list(ui, items.clone(), listing, own);
            });
            return clicked;
        }
        Widget::Stance => character::stance(ui, state, &named, look),
        Widget::Encumbrance => character::encumbrance(ui, state, &named, look),
        Widget::EncumbranceDetail => line(
            ui,
            named(
                state
                    .and_then(|state| state.character.encumbrance_detail.as_deref())
                    .unwrap_or("Encumbrance unknown"),
            ),
        ),
        Widget::Mind => character::mind(ui, state, &named, look),
        Widget::NextLevel => character::next_level(ui, state, &named, look),
        Widget::Level => line(ui, named(&character::level(state))),
        Widget::TrainingPoints => line(ui, named(&character::training(state))),
        Widget::ExperienceTotals => scrolled(ui, &mut |ui| character::experience(ui, state)),
        Widget::Prepared => return spell_hand(ui, &named("Spell"), state, seen.who.is_none()),
        Widget::Society => line(ui, named(&character::society(state))),
        Widget::Resources => scrolled(ui, &mut |ui| character::resources(ui, state)),
        Widget::Objectives => scrolled(ui, &mut |ui| character::objectives(ui, state)),
        Widget::Indicator(indicator) => {
            status::indicator(ui, *indicator, state, &named(indicator.name()));
        }
        Widget::Effects(category) => {
            scrolled(ui, &mut |ui| status::effects(ui, *category, state));
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
    None
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
/// Scrolled as a key asked, when it is the window in use (`plan/52` step 4).
fn scrolled(ui: &mut egui::Ui, id: Id, add: impl FnOnce(&mut egui::Ui)) {
    let scroll = super::split::asked(ui, id);
    egui::ScrollArea::vertical()
        .min_scrolled_height(0.0)
        .id_salt(id.with("scroll"))
        .auto_shrink(false)
        .show(ui, |ui| super::split::keyed(ui, scroll, add));
}

/// A vital as a bar fitted to what it is given, drawn as `look` says.
fn vital(
    ui: &mut egui::Ui,
    label: &str,
    vital: Option<Vital>,
    color: Color32,
    look: Option<&bar::Look>,
) {
    let amount = vital.map(|vital| Amount {
        percent: vital.percent,
        current: vital.current,
        max: vital.max,
    });
    let drawn = as_looks(ui, Bar::new(label, amount).fill(color), look);
    ui.add(drawn.fitted(ui));
}

/// `current` of `max` as a vital is: its percent, and the pair.
fn out_of(current: u64, max: u64) -> Option<Vital> {
    let percent = current.saturating_mul(100).checked_div(max)?;
    Some(Vital {
        percent: u32::try_from(percent.min(100)).unwrap_or(100),
        current: i32::try_from(current).ok(),
        max: i32::try_from(max).ok(),
    })
}

/// Field experience against its most, as the mind bar's `field_exp` and
/// `max_field_exp` say.
fn field_experience(state: &GameState) -> Option<Vital> {
    let experience = &state.character.experience;
    out_of(
        experience.field_experience?.into(),
        experience.field_experience_max?.into(),
    )
}

/// The Betrayer panel's `Blood Points: 50`, out of 100 (the author,
/// 2026-09-30): a label in the game's `BetrayerPanel` dialog, measured in
/// the author's logs at 0 to 100.
fn blood_points(state: &GameState) -> Option<Vital> {
    let panel = state.dialogs.get("BetrayerPanel")?;
    let points = panel.parts.iter().find_map(|(id, part)| match part {
        cena_session::dialogs::Part::Label(text) if id == "lblBPs" => text
            .strip_prefix("Blood Points:")?
            .trim()
            .parse::<u64>()
            .ok(),
        _ => None,
    })?;
    out_of(points, 100)
}

/// `bar` as `look` says, its images among it (`plan/49` §2).
pub(super) fn as_looks<'a>(ui: &egui::Ui, bar: Bar<'a>, look: Option<&bar::Look>) -> Bar<'a> {
    let Some(look) = look else {
        return bar;
    };
    let mut drawn = bar.look(look);
    if let Some(overlay) = look.overlay.as_deref().and_then(|path| overlay(ui, path)) {
        drawn = drawn.overlay(overlay);
    }
    if let Some(image) = look.background.as_deref().and_then(|path| image(ui, path)) {
        drawn = drawn.background(image.id());
    }
    if let Some(image) = look.fill_image.as_deref().and_then(|path| image(ui, path)) {
        drawn = drawn.fill_image(image.id());
    }
    drawn
}

/// The image at `path`, as an overlay stretched over a bar.
pub(super) fn overlay(ui: &egui::Ui, path: &str) -> Option<bar::Overlay> {
    image(ui, path).map(|texture| bar::Overlay::stretched(texture.id(), texture.size_vec2()))
}

/// The image at `path` (`pictures.rs`), `None` when it cannot be read.
fn image(ui: &egui::Ui, path: &str) -> Option<egui::TextureHandle> {
    crate::pictures::picture(ui.ctx(), path)
}

/// What a hand holds, after which hand: `?` until the game has said.
/// An object carried and let go on a hand, put `onto` it (`carry.rs`);
/// the hand's own item, let go on it, nothing. Another character's widget
/// asks nothing of this window: the play window draws it and sends nothing
/// (`play/draw.rs`).
fn place(ui: &mut egui::Ui, id: Id, onto: &str, hand: Option<&Hand>) -> Option<super::Clicked> {
    let holding = hand.and_then(Hand::id);
    crate::carry::target(ui, id, onto, holding).map(super::Clicked::Quietly)
}

/// What a hand holds, after which hand. On the window's `own` character's
/// it is a link, as the game sends it (the author, 2026-09-29: *"hands,
/// left, right, and spell are links"*): a click asks for the item's menu,
/// as a click on it in the story does, and with the drag key held the item
/// is carried from it (`carry.rs`). The link clicked, if it was.
fn held(ui: &mut egui::Ui, which: &str, hand: Option<&Hand>, own: bool) -> Option<super::Clicked> {
    let holds = match hand {
        None | Some(Hand::Unknown) => "?",
        Some(Hand::Empty) => "empty",
        Some(Hand::Holding { name, .. }) => name,
    };
    let label = egui::Label::new(format!("{which}: {holds}"))
        .truncate()
        .selectable(false);
    match hand {
        Some(Hand::Holding {
            id: Some(id),
            noun,
            name,
        }) if own => {
            let carrying = crate::carry::held(ui);
            let sense = if carrying {
                crate::carry::sense(ui)
            } else {
                egui::Sense::click()
            };
            let response = ui.add(label.sense(sense));
            crate::carry::source(
                &response,
                crate::carry::Carried {
                    exist: id.clone(),
                    name: name.clone(),
                },
            );
            linked(&response, id, noun.as_deref().unwrap_or_default())
        }
        _ => {
            ui.add(label);
            None
        }
    }
}

/// The spell prepared, after `which`: on the window's `own` character's a
/// link to it, `#spell`, as the game sends it, whose menu a click asks for.
/// Never carried: a spell is not put anywhere. The link clicked, if it was.
fn spell_hand(
    ui: &mut egui::Ui,
    which: &str,
    state: Option<&GameState>,
    own: bool,
) -> Option<super::Clicked> {
    let spell = match state.map(|state| state.prepared.as_deref()) {
        None => "?",
        Some(None) => "none",
        Some(Some(spell)) if spell.eq_ignore_ascii_case("none") => "none",
        Some(Some(spell)) => spell,
    };
    let label = egui::Label::new(format!("{which}: {spell}"))
        .truncate()
        .selectable(false);
    match state.and_then(|state| state.prepared_id.as_deref()) {
        Some(id) if own => {
            let response = ui.add(label.sense(egui::Sense::click()));
            linked(&response, id, "")
        }
        _ => {
            ui.add(label);
            None
        }
    }
}

/// A hand's words as a link to the object `id` (`noun`): the hand shown on
/// hover, a click its link, where the pointer was.
fn linked(response: &egui::Response, id: &str, noun: &str) -> Option<super::Clicked> {
    let response = response
        .clone()
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.clicked().then(|| {
        let at = response
            .interact_pointer_pos()
            .unwrap_or(response.rect.center());
        super::Clicked::Link(
            cena_ui::RunLink::Object {
                exist: id.to_owned(),
                noun: noun.to_owned(),
                coord: None,
            },
            at,
        )
    })
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

/// The stun left, as a clock: its seconds while the rounds the game gave
/// run, and `stunned` while the indicator stays lit past them or the game
/// gave none (`cena_model`'s `state/stun.rs`).
fn stun(ui: &mut egui::Ui, label: &str, state: Option<&GameState>) {
    let left = state.and_then(GameState::stun_remaining).unwrap_or(0);
    if left > 0 {
        line(ui, RichText::new(format!("{label} {left}s")).color(WRONG));
    } else if state.is_some_and(|state| state.status.get("stunned")) {
        line(ui, RichText::new(format!("{label}: stunned")).color(WRONG));
    } else {
        line(ui, RichText::new(format!("{label} —")).weak());
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
/// The names a room list shows, each its runs: its creatures or objects
/// once the game has said what is here, or its players, each painted by the
/// character's triggers as Despana paints them ([`cena_ui::room_player`]).
/// `None` until the game has said.
fn room_list(widget: &Widget, snapshot: Option<&Snapshot>) -> Option<Vec<Vec<StyledRun>>> {
    let snapshot = snapshot?;
    let room = &snapshot.state.room;
    let things = |items: &[RoomItem], color| {
        room.component("room objs").is_some().then(|| {
            items
                .iter()
                .map(|item| super::described::item_runs(item, color))
                .collect()
        })
    };
    // Hostile when the game first said, whatever it says now (the author,
    // 2026-09-30: a sympathied creature still is).
    let hostile = |item: &&RoomItem| {
        item.id
            .parse()
            .ok()
            .and_then(|id| snapshot.state.creatures().get(id))
            .and_then(cena_session::CreatureInstance::hostile_when_first_seen)
            == Some(true)
    };
    let (targets, npcs): (Vec<RoomItem>, Vec<RoomItem>) = room
        .creatures
        .iter()
        .cloned()
        .partition(|item| hostile(&item));
    match widget {
        Widget::Creatures => things(&targets, CREATURE),
        Widget::Npcs => things(&npcs, CREATURE),
        Widget::Objects => things(&room.objects, OBJECT),
        _ => room.saw_players().then(|| {
            room.players
                .iter()
                .map(|player| super::described::player_runs(snapshot, player))
                .collect()
        }),
    }
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
                notice_lines(ui, notice, ui.visuals().text_color());
            }
        });
}

/// One of Hydra's messages, coloured by what it is about: `info` for
/// what is neither wrong nor off.
pub(super) fn notice_lines(ui: &mut egui::Ui, notice: &Notice, info: Color32) {
    let color = match notice.kind {
        NoticeKind::Error => WRONG,
        NoticeKind::Warn => AMBER,
        NoticeKind::Info | NoticeKind::Debug => info,
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
