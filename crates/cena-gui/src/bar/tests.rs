//! The bar's own tests: its words, its fill, its frame, its text.

use egui::Rect;

use super::overlay::nine_slice;
use super::shape::{Paint, filled, frame, paint, segment, share};
use super::*;

fn amount(percent: u32) -> Amount {
    Amount {
        percent,
        current: i32::try_from(percent * 4).ok(),
        max: Some(400),
    }
}

#[test]
fn the_words_are_any_combination_in_one_order() {
    let says = |label, numbers, percent| Says {
        label,
        numbers,
        percent,
    };
    let bar = |s| Bar::new("HP", Some(amount(87))).says(s).words();
    assert_eq!(bar(says(true, true, true)), "HP 348/400 87%");
    assert_eq!(bar(says(true, false, false)), "HP");
    assert_eq!(bar(says(false, true, false)), "348/400");
    assert_eq!(bar(says(false, false, true)), "87%");
    assert_eq!(bar(says(false, false, false)), "");
}

#[test]
fn unknown_is_a_question_and_missing_numbers_are_left_out() {
    let all = Says {
        label: true,
        numbers: true,
        percent: true,
    };
    assert_eq!(Bar::new("HP", None).says(all).words(), "HP ?");
    let percent_only = Some(Amount {
        percent: 50,
        current: None,
        max: None,
    });
    assert_eq!(Bar::new("MP", percent_only).says(all).words(), "MP 50%");
}

#[test]
fn each_direction_fills_from_its_own_edge() {
    let bar = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(100.0, 40.0));
    let quarter = share(25);
    assert_eq!(
        filled(bar, Fills::Right, quarter),
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(25.0, 40.0))
    );
    assert_eq!(
        filled(bar, Fills::Left, quarter),
        Rect::from_min_max(egui::pos2(75.0, 0.0), egui::pos2(100.0, 40.0))
    );
    assert_eq!(
        filled(bar, Fills::Up, quarter),
        Rect::from_min_max(egui::pos2(0.0, 30.0), egui::pos2(100.0, 40.0))
    );
    assert_eq!(
        filled(bar, Fills::Down, quarter),
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 10.0))
    );
    assert_eq!(
        filled(bar, Fills::Right, share(250)),
        bar,
        "past 100 is full"
    );
}

/// A round shape sits in the square in the middle of its space, so it stays
/// round; a bar takes the whole of it.
#[test]
fn a_round_shape_keeps_to_its_square() {
    let wide = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(120.0, 60.0));
    let square = Rect::from_min_size(egui::pos2(30.0, 0.0), Vec2::splat(60.0));
    assert_eq!(frame(Fills::Orb, wide), square);
    assert_eq!(frame(Fills::Ring, wide), square);
    assert_eq!(frame(Fills::Up, wide), wide);
}

/// Whether the fill covers the middle, where text inside sits and takes its
/// colour from: past half for an orb or a bar, never for a ring's hole.
#[test]
fn the_fill_covers_the_middle_past_half() {
    let covers = |fills: Fills, percent: u32| {
        let mut covered = None;
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            let area = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::splat(60.0));
            let colours = Paint {
                trough: Color32::BLACK,
                fill: Color32::WHITE,
                background: None,
                fill_image: None,
                ring: 0.25,
            };
            covered = Some(paint(ui.painter(), fills, area, share(percent), &colours));
        });
        harness.run();
        drop(harness);
        covered
    };
    for fills in [Fills::Orb, Fills::Right, Fills::Up] {
        assert_eq!(covers(fills, 60), Some(true), "{fills:?}");
        assert_eq!(covers(fills, 40), Some(false), "{fills:?}");
    }
    assert_eq!(covers(Fills::Ring, 100), Some(false), "a ring's hole");
}

/// An orb fills as a flask does: its fill is the part of the circle under
/// a level that far up it, from edge to edge, never above the level.
#[test]
fn an_orb_fills_to_its_level() {
    let centre = egui::pos2(50.0, 50.0);
    let near = |a: egui::Pos2, b: egui::Pos2| (a - b).length() < 0.01;
    let half = segment(centre, 40.0, 0.5);
    assert!(near(half[0], egui::pos2(90.0, 50.0)), "{:?}", half[0]);
    assert!(near(half[half.len() - 1], egui::pos2(10.0, 50.0)));
    assert!(half.iter().all(|at| at.y >= 50.0 - 0.01), "the lower half");
    let quarter = segment(centre, 40.0, 0.25);
    assert!(
        quarter.iter().all(|at| at.y >= 70.0 - 0.01),
        "under a level a quarter of the way up"
    );
    assert!(
        quarter.iter().any(|at| near(*at, egui::pos2(50.0, 90.0))),
        "to the bottom"
    );
    let full = segment(centre, 40.0, 1.0);
    assert!(near(full[0], egui::pos2(50.0, 10.0)) && near(full[full.len() - 1], full[0]));
    assert!(
        full.iter()
            .all(|at| ((*at - centre).length() - 40.0).abs() < 0.01)
    );
}

/// A frame's corners keep their size and its middle is left clear,
/// however long the bar.
#[test]
fn a_frame_keeps_its_corners_and_leaves_the_middle_clear() {
    let bar = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(200.0, 20.0));
    let patches = nine_slice(Vec2::new(24.0, 12.0), [2.0, 2.0, 2.0, 2.0], bar);
    assert_eq!(patches.len(), 8, "no centre");
    let (corner, uv) = patches[0];
    assert_eq!(corner.size(), Vec2::new(2.0, 2.0));
    assert_eq!(uv.max, egui::pos2(2.0 / 24.0, 2.0 / 12.0));
    assert!(
        patches.iter().all(|(dest, _)| !dest.contains(bar.center())),
        "the fill shows through"
    );
    let squeezed = nine_slice(
        Vec2::new(24.0, 12.0),
        [8.0, 2.0, 8.0, 2.0],
        Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(200.0, 8.0)),
    );
    let (top_left, _) = squeezed[0];
    assert!(
        (top_left.height() - 4.0).abs() < f32::EPSILON,
        "shrunk to fit"
    );
}

#[test]
fn text_reads_on_light_and_dark() {
    assert_eq!(readable_on(Color32::WHITE), Color32::BLACK);
    assert_eq!(readable_on(Color32::from_rgb(20, 20, 20)), Color32::WHITE);
    assert_eq!(readable_on(SPIRIT), Color32::BLACK);
    assert_eq!(readable_on(HEALTH), Color32::WHITE);
}
