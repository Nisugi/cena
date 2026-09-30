//! `plan/47` step 10: the progress bar, to the author's spec -- both ways of
//! filling, any size, an overlay, and its text inside, on any side, or none,
//! saying any mix of label, numbers and percent. The last test renders every
//! kind at once and compares it with `tests/snapshots/bars.png`.

use cena_gui::bar::{Amount, Bar, Fills, HEALTH, MANA, Overlay, Place, SPIRIT, STAMINA, Says};
use egui::{Color32, Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

fn amount(percent: u32) -> Amount {
    Amount {
        percent,
        current: i32::try_from(percent * 4).ok(),
        max: Some(400),
    }
}

const ALL: Says = Says {
    label: true,
    numbers: true,
    percent: true,
    words: false,
};

/// Where each bar landed, in the order drawn.
#[derive(Default)]
struct Drawn {
    rects: Vec<Rect>,
    frame: Option<egui::TextureHandle>,
}

/// A frame to lay over a bar: a two-pixel gold border, clear inside.
fn frame(ctx: &egui::Context) -> egui::TextureHandle {
    let (width, height) = (24, 12);
    let mut image = egui::ColorImage::filled([width, height], Color32::TRANSPARENT);
    for y in 0..height {
        for x in 0..width {
            if x < 2 || y < 2 || x >= width - 2 || y >= height - 2 {
                image[(x, y)] = Color32::from_rgb(0xd4, 0xaf, 0x37);
            }
        }
    }
    ctx.load_texture("frame", image, egui::TextureOptions::NEAREST)
}

fn gallery(ui: &mut egui::Ui, drawn: &mut Drawn) {
    let texture = drawn.frame.get_or_insert_with(|| frame(ui.ctx())).clone();
    drawn.rects.clear();
    let mut add = |ui: &mut egui::Ui, bar: Bar<'_>| drawn.rects.push(ui.add(bar).rect);
    ui.horizontal(|ui| {
        add(
            ui,
            Bar::new("HP", Some(amount(87)))
                .fill(HEALTH)
                .says(ALL)
                .size([160.0, 20.0]),
        );
        add(
            ui,
            Bar::new("MP", Some(amount(40)))
                .fill(MANA)
                .text(Place::Right),
        );
        add(ui, Bar::new("SP", None).fill(STAMINA).text(Place::Left));
    });
    ui.horizontal(|ui| {
        add(
            ui,
            Bar::new("Sp", Some(amount(100)))
                .fill(SPIRIT)
                .text(Place::Above),
        );
        add(
            ui,
            Bar::new("Stance", Some(amount(30)))
                .text(Place::Below)
                .fills(Fills::Left),
        );
        add(
            ui,
            Bar::new("Hidden", Some(amount(60)))
                .fill(MANA)
                .text(Place::Hidden),
        );
        add(
            ui,
            Bar::new("Framed", Some(amount(55)))
                .fill(HEALTH)
                .size([120.0, 22.0])
                .overlay(Overlay::framed(
                    texture.id(),
                    Vec2::new(24.0, 12.0),
                    [2.0, 2.0, 2.0, 2.0],
                )),
        );
    });
    ui.horizontal(|ui| {
        add(
            ui,
            Bar::new("HP", Some(amount(75)))
                .fill(HEALTH)
                .fills(Fills::Up)
                .text(Place::Below),
        );
        add(
            ui,
            Bar::new("MP", Some(amount(25)))
                .fill(MANA)
                .fills(Fills::Down)
                .text(Place::Right),
        );
        add(
            ui,
            Bar::new("Tall", Some(amount(50)))
                .fill(STAMINA)
                .fills(Fills::Up)
                .size([30.0, 100.0])
                .says(Says {
                    label: false,
                    numbers: false,
                    percent: true,
                    words: false,
                }),
        );
    });
}

fn harness<'a>() -> Harness<'a, Drawn> {
    Harness::builder()
        .with_size((460.0, 280.0))
        .build_ui_state(gallery, Drawn::default())
}

/// Each bar is found by what it says, wherever the words are drawn, and a
/// bar with its text hidden still says it to a screen reader.
#[test]
fn every_bar_says_its_words() {
    let harness = harness();
    for words in [
        "HP 348/400 87%",
        "MP 40%",
        "SP ?",
        "Sp 100%",
        "Stance 30%",
        "Hidden 60%",
        "Framed 55%",
        "HP 75%",
        "MP 25%",
        "50%",
    ] {
        assert!(harness.query_by_label(words).is_some(), "{words}");
    }
}

/// A bar is the size it was given; text outside it adds to the space it
/// takes, text inside does not.
#[test]
fn a_bar_is_the_size_it_was_given() {
    let harness = harness();
    let rects = &harness.state().rects;
    assert_eq!(rects[0].size(), Vec2::new(160.0, 20.0), "inside");
    assert_eq!(rects[5].size(), Vec2::new(72.0, 18.0), "hidden text");
    assert_eq!(rects[9].size(), Vec2::new(30.0, 100.0), "upright");
    assert!(rects[1].width() > 72.0, "text to the right widens it");
    assert!(rects[3].height() > 18.0, "text above heightens it");
}

#[test]
fn the_bars_as_drawn() {
    let mut harness = Harness::builder()
        .with_size((460.0, 280.0))
        .wgpu()
        .build_ui_state(gallery, Drawn::default());
    harness.run();
    harness.snapshot("bars");
}

/// Fitted, a bar takes the room it is given, across or upright: the whole
/// room, so the space is its shape (the author, 2026-09-28: *"if I make a
/// progress bar horizontal fill and make it taller, the bar doesn't get
/// taller, it should"*); with its text outside, the text's room is left for
/// it (`plan/49` §2, a bar widget's options).
#[test]
fn a_fitted_bar_takes_its_room() {
    let room = Vec2::new(160.0, 100.0);
    let fitted = |fills: Fills, place: Place| {
        let mut harness = Harness::builder().with_size((300.0, 200.0)).build_ui_state(
            move |ui, drawn: &mut Option<Rect>| {
                ui.allocate_ui(room, |ui| {
                    ui.set_min_size(room);
                    let bar = Bar::new("HP", Some(amount(50)))
                        .fills(fills)
                        .text(place)
                        .says(ALL);
                    *drawn = Some(ui.add(bar.fitted(ui)).rect);
                });
            },
            None,
        );
        harness.run();
        harness.state().unwrap_or(Rect::NOTHING)
    };
    for fills in [Fills::Right, Fills::Up] {
        let whole = fitted(fills, Place::Inside);
        assert!((whole.size() - room).length() < 0.5, "{fills:?}: {whole:?}");
    }
    for place in [Place::Below, Place::Left] {
        let with_text = fitted(Fills::Up, place);
        assert!(
            with_text.width() <= room.x + 0.5 && with_text.height() <= room.y + 0.5,
            "{place:?}: {with_text:?} fits in {room:?}"
        );
        assert!(
            with_text.width() > room.x - 1.0 || with_text.height() > room.y - 1.0,
            "{place:?}: {with_text:?} fills the room"
        );
    }
}

/// Art for a round bar, made here rather than read from a file: a liquid
/// that darkens downward, with a band across it and bubbles down it, a dark
/// glass disc, and a gold rim clear inside.
fn round_art(ctx: &egui::Context) -> [egui::TextureHandle; 3] {
    let side = 48;
    let centre = 23.5_f32;
    let make = |name: &str, pixel: &dyn Fn(usize, usize, f32) -> Color32| {
        let mut image = egui::ColorImage::filled([side, side], Color32::TRANSPARENT);
        for y in 0..side {
            for x in 0..side {
                let (fx, fy) = (
                    f32::from(u16::try_from(x).unwrap_or(0)),
                    f32::from(u16::try_from(y).unwrap_or(0)),
                );
                let out = ((fx - centre).powi(2) + (fy - centre).powi(2)).sqrt();
                image[(x, y)] = pixel(x, y, out);
            }
        }
        ctx.load_texture(name, image, egui::TextureOptions::LINEAR)
    };
    let liquid = make("liquid", &|x, y, _| {
        if (20..24).contains(&y) || x % 12 < 2 {
            Color32::from_rgb(0xff, 0x90, 0x90)
        } else {
            let fall = u8::try_from(y * 3).unwrap_or(u8::MAX);
            Color32::from_rgb(0xe0_u8.saturating_sub(fall), 0x20, 0x30)
        }
    });
    let glass = make("glass", &|_, _, out| {
        if out < 24.0 {
            Color32::from_rgb(0x20, 0x24, 0x30)
        } else {
            Color32::TRANSPARENT
        }
    });
    let rim = make("rim", &|_, _, out| {
        if (21.5..24.0).contains(&out) {
            Color32::from_rgb(0xd4, 0xaf, 0x37)
        } else {
            Color32::TRANSPARENT
        }
    });
    [liquid, glass, rim]
}

/// Orbs and rings (the author, 2026-09-28: *"we also need circle progress
/// bars (health/mana orbs!)"*): plain, with art under, in and over the fill,
/// a thicker ring on glass, an orb in a wide space, round in its middle, and
/// a bar across with the same art, its liquid uncovered, not squeezed.
fn round_gallery(ui: &mut egui::Ui, art: &mut Option<[egui::TextureHandle; 3]>) {
    let [liquid, glass, rim] = art.get_or_insert_with(|| round_art(ui.ctx())).clone();
    let rim = Overlay::stretched(rim.id(), rim.size_vec2());
    let thick = cena_gui::bar::Look {
        fills: Fills::Ring,
        place: Place::Inside,
        says: Says::default(),
        color: [0x55, 0xb8, 0x6c],
        overlay: None,
        background: None,
        fill_image: None,
        ring: 50,
    };
    let round = |label, percent, fills| {
        Bar::new(label, Some(amount(percent)))
            .fills(fills)
            .size([80.0, 80.0])
    };
    ui.horizontal(|ui| {
        ui.add(round("HP", 30, Fills::Orb).fill(HEALTH));
        ui.add(round("MP", 75, Fills::Orb).fill(MANA).says(ALL));
        ui.add(
            round("HP", 60, Fills::Orb)
                .fill_image(liquid.id())
                .background(glass.id())
                .overlay(rim),
        );
    });
    ui.horizontal(|ui| {
        ui.add(round("Sp", 40, Fills::Ring).fill(SPIRIT));
        ui.add(
            round("SP", 70, Fills::Ring)
                .look(&thick)
                .background(glass.id())
                .overlay(rim),
        );
        ui.add(
            Bar::new("MP", Some(amount(50)))
                .fills(Fills::Orb)
                .fill(MANA)
                .size([140.0, 60.0])
                .overlay(rim),
        );
    });
    ui.add(
        Bar::new("HP", Some(amount(50)))
            .fill_image(liquid.id())
            .background(glass.id())
            .size([280.0, 20.0]),
    );
}

#[test]
fn the_orbs_and_rings_as_drawn() {
    let mut harness = Harness::builder()
        .with_size((300.0, 220.0))
        .wgpu()
        .build_ui_state(round_gallery, None);
    harness.run();
    harness.snapshot("orbs");
}
