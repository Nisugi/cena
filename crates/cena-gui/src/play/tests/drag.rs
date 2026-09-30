//! Carrying an object from a link, as a player does it (the author,
//! 2026-09-28): the drag key held, pressed on the link, moved, let go on a
//! place that takes it -- the story's floor, a hand, another object, a
//! container -- each saying `_drag` without an echo; let go anywhere else,
//! nothing.

use std::sync::Arc;

use super::links::{heard, rat};
use super::*;
use crate::widget::{Character, Widget};
use cena_session::LinkKind;
use egui::{Modifiers, Pos2};

fn button(at: Pos2, pressed: bool, keys: Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: keys,
    }
}

/// Pressed at `from` with `keys` held, moved in steps well past egui's
/// click distance to `to`: the carrying under way, not yet let go.
fn carry_to(harness: &mut Harness<'_, Scene>, keys: Modifiers, from: Pos2, to: Pos2) {
    harness.input_mut().modifiers = keys;
    harness.hover_at(from);
    harness.step();
    harness.event(button(from, true, keys));
    harness.step();
    for step in 1..=5u8 {
        harness.hover_at(from + (to - from) * (f32::from(step) / 5.0));
        harness.step();
    }
}

/// [`carry_to`], then let go at `to`, the pointer staying.
fn carry(harness: &mut Harness<'_, Scene>, keys: Modifiers, from: Pos2, to: Pos2) {
    carry_to(harness, keys, from, to);
    harness.event(button(to, false, keys));
    harness.step();
    harness.input_mut().modifiers = Modifiers::NONE;
    harness.run();
}

/// A window with a grey rat in its story, and where the rat is.
fn with_a_rat<'a>() -> (Harness<'a, Scene>, Pos2) {
    let mut harness = harness();
    heard(harness.state_mut(), &[("a grey rat", Some(rat()))]);
    harness.run();
    let at = harness.get_by_label("a grey rat").rect().center();
    (harness, at)
}

/// `widget` added in a window of its own over the Hunt and Room windows,
/// in the right-hand column, clear of the story: a window over the story is under
/// it once a press on the rat raises the story. Its id.
fn beside_the_story(
    harness: &mut Harness<'_, Scene>,
    widget: Widget,
    follows: Option<&str>,
) -> u32 {
    let hunt = kept(harness, "Hunt")
        .zip(kept(harness, "Room"))
        .map_or(egui::Rect::NOTHING, |(hunt, room)| hunt.union(room));
    let Some(layout) = &mut harness.state_mut().play.layout else {
        return 0;
    };
    let placed = layout.add_widget(widget, follows.map(str::to_owned));
    let window = layout
        .holders
        .iter()
        .find(|holder| matches!(&holder.holds, crate::layout::Holds::One(one) if one.id == placed))
        .map(|holder| holder.id)
        .unwrap_or_default();
    layout.set(window, hunt);
    harness.run();
    placed
}

fn quietly(line: &str) -> Vec<Asked> {
    vec![Asked::Quietly(line.to_owned())]
}

/// Let go on the story's blank space, the rat is dropped, and while it is
/// carried the pointer says what.
#[test]
fn let_go_on_the_storys_floor_it_is_dropped() {
    let (mut harness, rat) = with_a_rat();
    let floor = rat + egui::vec2(0.0, 200.0);
    carry_to(&mut harness, Modifiers::ALT, rat, floor);
    assert!(
        harness.query_by_label("Dragging: a grey rat").is_some(),
        "the pointer says what is carried"
    );
    harness.event(button(floor, false, Modifiers::ALT));
    harness.step();
    harness.run();
    assert_eq!(harness.state().asked, quietly("_drag #456 drop"));
}

/// Let go on a hand widget, the rat goes into that hand.
#[test]
fn let_go_on_a_hand_it_goes_into_it() {
    let (mut harness, rat) = with_a_rat();
    let hand = harness.get_by_label("Right: empty").rect().center();
    carry(&mut harness, Modifiers::ALT, rat, hand);
    assert_eq!(harness.state().asked, quietly("_drag #456 right"));
}

/// Let go on the plain words of a line with a link in it, the rat is
/// dropped: those words are the story's, as its blank space is.
#[test]
fn let_go_on_a_linked_lines_words_it_is_dropped() {
    let mut harness = harness();
    heard(
        harness.state_mut(),
        &[
            ("You see a lot of words and then ", None),
            ("a grey rat", Some(rat())),
        ],
    );
    harness.run();
    let line = harness
        .get_by_label("You see a lot of words and then a grey rat")
        .rect();
    let rat = egui::pos2(line.right() - 20.0, line.center().y);
    let words = egui::pos2(line.left() + 20.0, line.center().y);
    carry(&mut harness, Modifiers::ALT, rat, words);
    assert_eq!(harness.state().asked, quietly("_drag #456 drop"));
}

/// Let go on another object's link, the rat goes into it; on its own
/// link, nothing.
#[test]
fn let_go_on_another_object_it_goes_into_it() {
    let (mut harness, rat) = with_a_rat();
    let sack = LinkKind::Exist {
        id: "789".to_owned(),
        noun: "sack".to_owned(),
    };
    heard(harness.state_mut(), &[("a patched sack", Some(sack))]);
    harness.run();
    carry(
        &mut harness,
        Modifiers::ALT,
        rat,
        rat + egui::vec2(12.0, 0.0),
    );
    assert!(harness.state().asked.is_empty(), "onto itself");
    let into = harness.get_by_label("a patched sack").rect().center();
    carry(&mut harness, Modifiers::ALT, rat, into);
    assert_eq!(harness.state().asked, quietly("_drag #456 #789"));
}

/// Let go on a container in the Containers widget, the rat goes into it,
/// by the container object's id.
#[test]
fn let_go_on_a_container_it_goes_into_it() {
    let (mut harness, rat) = with_a_rat();
    harness
        .state_mut()
        .snapshot
        .state
        .apply(&cena_session::Frame::Container {
            id: "stow".to_owned(),
            title: Some("My Cloak".to_owned()),
            target: Some("#64863904".to_owned()),
            attrs: Vec::new(),
        });
    beside_the_story(&mut harness, Widget::Containers, None);
    let cloak = harness.get_by_label_contains("My Cloak").rect().center();
    carry(&mut harness, Modifiers::ALT, rat, cloak);
    assert_eq!(harness.state().asked, quietly("_drag #456 #64863904"));
}

/// Let go where nothing takes it, nothing is sent; without the drag key a
/// drag carries nothing (it moves the window, as it always has); the key is
/// the one chosen. Each in a window of its own, the rat where it was.
#[test]
fn only_the_drag_key_carries_and_only_a_place_takes() {
    let hand = |harness: &Harness<'_, Scene>| harness.get_by_label("Right: empty").rect().center();
    let (mut harness, rat) = with_a_rat();
    let bar = harness.get_by_label_contains("HP ").rect().center();
    carry(&mut harness, Modifiers::ALT, rat, bar);
    assert!(harness.state().asked.is_empty(), "a bar takes nothing");

    let (mut harness, rat) = with_a_rat();
    let right = hand(&harness);
    carry(&mut harness, Modifiers::NONE, rat, right);
    assert!(harness.state().asked.is_empty(), "no key, no carrying");

    let (mut harness, rat) = with_a_rat();
    let right = hand(&harness);
    crate::carry::set_key(&harness.ctx, Modifiers::CTRL);
    carry(&mut harness, Modifiers::ALT, rat, right);
    assert!(harness.state().asked.is_empty(), "not the key chosen");

    let (mut harness, rat) = with_a_rat();
    let right = hand(&harness);
    crate::carry::set_key(&harness.ctx, Modifiers::CTRL);
    carry(&mut harness, Modifiers::CTRL, rat, right);
    assert_eq!(harness.state().asked, quietly("_drag #456 right"));
}

/// Another character's hand, in the Advanced place, takes nothing: the
/// line would go to this window's character.
#[test]
fn another_characters_hand_takes_nothing() {
    let (mut harness, rat) = with_a_rat();
    harness.state_mut().others = vec![Character {
        name: "Baelor".to_owned(),
        snapshot: Some(Arc::new(crate::fixture::snapshot())),
        hunt: None,
    }];
    let theirs = beside_the_story(&mut harness, Widget::RightHand, Some("Baelor"));
    let hand = harness.get_by_label("Baelor Right: empty").rect().center();
    carry(&mut harness, Modifiers::ALT, rat, hand);
    assert!(harness.state().asked.is_empty(), "{theirs}");
}

/// The broadsword in the left hand, with its id.
fn armed(scene: &mut Scene) {
    scene.snapshot.state.left_hand = cena_session::hands::Hand::Holding {
        id: Some("321".to_owned()),
        noun: Some("broadsword".to_owned()),
        name: "a steel broadsword".to_owned(),
    };
}

/// From a hand (the author: *"Yes drag from hands"*): to the other hand,
/// to the story's floor; let go on its own hand, nothing.
#[test]
fn from_a_hand_it_goes_to_the_other_or_the_floor() {
    let from_the_left = |to: &dyn Fn(&Harness<'_, Scene>, Pos2) -> Pos2| {
        let mut harness = harness();
        armed(harness.state_mut());
        harness.run();
        let left = harness
            .get_by_label("Left: a steel broadsword")
            .rect()
            .center();
        let at = to(&harness, left);
        carry(&mut harness, Modifiers::ALT, left, at);
        harness.state().asked.clone()
    };
    let right = |harness: &Harness<'_, Scene>, _: Pos2| {
        harness.get_by_label("Right: empty").rect().center()
    };
    assert_eq!(from_the_left(&right), quietly("_drag #321 right"));
    let floor = |harness: &Harness<'_, Scene>, _: Pos2| {
        let story = harness
            .get_by_label("You swing a steel broadsword at a kobold!")
            .rect();
        story.center() + egui::vec2(0.0, 200.0)
    };
    assert_eq!(from_the_left(&floor), quietly("_drag #321 drop"));
    let itself = |_: &Harness<'_, Scene>, left: Pos2| left + egui::vec2(12.0, 0.0);
    assert!(from_the_left(&itself).is_empty(), "onto its own hand");
    let mut harness = harness();
    armed(harness.state_mut());
    harness.run();
    let left = harness
        .get_by_label("Left: a steel broadsword")
        .rect()
        .center();
    let right = harness.get_by_label("Right: empty").rect().center();
    carry(&mut harness, Modifiers::NONE, left, right);
    assert!(harness.state().asked.is_empty(), "no key, no carrying");
}

/// A container's line as the game sends it: one linked item.
fn contains(scene: &mut Scene, container: &str, (id, words, noun): (&str, &str, &str)) {
    let mut line = cena_session::ChunkLine::plain(words).runs;
    if let Some(run) = line.runs.first_mut() {
        run.link = Some(cena_session::Link {
            kind: LinkKind::Exist {
                id: id.to_owned(),
                noun: noun.to_owned(),
            },
            text: words.to_owned(),
            coord: None,
        });
    }
    scene
        .snapshot
        .state
        .apply(&cena_session::Frame::ContainerItem {
            container_id: container.to_owned(),
            content: line,
        });
}

/// From a container's contents (*"drag from containers"*): to another
/// container, to a hand, onto another item.
#[test]
fn from_a_container_it_goes_elsewhere() {
    let from = |what: &str, to: &str| {
        let mut harness = harness();
        let scene = harness.state_mut();
        scene.snapshot.state.right_hand = cena_session::hands::Hand::Holding {
            id: Some("200".to_owned()),
            noun: Some("sack".to_owned()),
            name: "a patched sack".to_owned(),
        };
        for (id, title, target) in [
            ("stow", "My Cloak", "#100"),
            ("200", "a patched sack", "#200"),
        ] {
            scene.snapshot.state.apply(&cena_session::Frame::Container {
                id: id.to_owned(),
                title: Some(title.to_owned()),
                target: Some(target.to_owned()),
                attrs: Vec::new(),
            });
        }
        contains(scene, "stow", ("11", "a gold ring", "ring"));
        contains(scene, "200", ("22", "a pink pearl", "pearl"));
        beside_the_story(&mut harness, Widget::Containers, None);
        for title in ["My Cloak (1)", "a patched sack (1)"] {
            harness.get_by_label(title).click();
            harness.run();
        }
        let start = harness.get_by_label(what).rect().center();
        let at = harness.get_by_label(to).rect().center();
        carry(&mut harness, Modifiers::ALT, start, at);
        harness.state().asked.clone()
    };
    let ring = "a gold ring";
    assert_eq!(from(ring, "a patched sack (1)"), quietly("_drag #11 #200"));
    assert_eq!(from(ring, "a pink pearl"), quietly("_drag #11 #22"));
    assert_eq!(
        from(ring, "Left: a steel broadsword"),
        quietly("_drag #11 left")
    );
    assert!(
        from("Right: a patched sack", "a patched sack (1)").is_empty(),
        "a container into itself"
    );
}

/// Another character's hand, in the Advanced place, is carried from by
/// nothing: its item's id is that character's.
#[test]
fn another_characters_hand_is_no_source() {
    let mut harness = harness();
    let mut theirs = crate::fixture::snapshot();
    theirs.state.left_hand = cena_session::hands::Hand::Holding {
        id: Some("654".to_owned()),
        noun: Some("mace".to_owned()),
        name: "a spiked mace".to_owned(),
    };
    harness.state_mut().others = vec![Character {
        name: "Baelor".to_owned(),
        snapshot: Some(Arc::new(theirs)),
        hunt: None,
    }];
    beside_the_story(&mut harness, Widget::LeftHand, Some("Baelor"));
    let mace = harness
        .get_by_label("Baelor Left: a spiked mace")
        .rect()
        .center();
    let right = harness.get_by_label("Right: empty").rect().center();
    carry(&mut harness, Modifiers::ALT, mace, right);
    assert!(harness.state().asked.is_empty());
}
