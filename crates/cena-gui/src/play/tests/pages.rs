//! A widget's own page, which its right-click opens (`plan/50` §7 step 8,
//! as the author corrected it): a bar's look and a Room widget's parts,
//! each kept with the layout by the widget's id. Moved out of `arrange.rs`
//! as it neared its cap.

use super::*;
use crate::layout::Holds;
use crate::widget::Widget;

/// The first Health bar in the play window, and its page's id.
fn health_page(harness: &Harness<'_, Scene>) -> (u32, String) {
    let health = layout(harness)
        .holders
        .iter()
        .find_map(|holder| match &holder.holds {
            Holds::Custom(custom) => custom
                .cells
                .iter()
                .flat_map(|cell| cell.tabs.iter())
                .find(|placed| placed.widget == Widget::Health)
                .map(|placed| placed.id),
            Holds::One(_) => None,
        })
        .unwrap_or_default();
    (health, format!("widget:{health}"))
}

/// A bar's page takes a ring's thickness, no thinner than a ring may be,
/// and its three images by their paths: each kept with the layout, and put
/// back to the kind's own.
#[test]
fn a_bars_page_takes_a_ring_and_its_images() {
    let mut harness = harness();
    harness.run();
    let (health, page) = health_page(&harness);
    let glass = "C:/overlays/glass.png";
    let play = &mut harness.state_mut().play;
    play.widget_change(&page, "ring", Some("60"))
        .expect("changed");
    assert!(
        play.widget_change(&page, "ring", Some("3")).is_err(),
        "thinner than a ring may be"
    );
    for key in ["fill_image", "background", "overlay"] {
        play.widget_change(&page, key, Some(glass))
            .expect("changed");
    }
    let look = layout(&harness).looks.get(&health).cloned().expect("kept");
    assert_eq!(look.ring, 60);
    assert_eq!(
        [look.fill_image, look.background, look.overlay]
            .map(|image| image.as_deref() == Some(glass)),
        [true; 3]
    );
    let play = &mut harness.state_mut().play;
    for key in ["ring", "fill_image", "background", "overlay"] {
        play.widget_change(&page, key, None).expect("put back");
    }
    assert!(
        !layout(&harness).looks.contains_key(&health),
        "all its own again"
    );
}

/// A bar widget's own page, which its right-click opens, says how it draws;
/// each change is kept with the layout by the widget's id, and drawn at
/// once; back at the kind's own, nothing is kept; the widget's look goes
/// with it when it is removed.
#[test]
fn a_bar_is_drawn_as_its_page_says() {
    use crate::bar::{Fills, Place};
    use cena_ui::settings::Value;
    let mut harness = harness();
    harness.run();
    let (health, page) = health_page(&harness);
    let pages = harness
        .state()
        .play
        .widget_pages(&crate::play::Pictures::default());
    let own = pages
        .iter()
        .find(|found| found.id == page)
        .expect("its page");
    assert_eq!(own.title, "Health (Vitals)");
    let keys: Vec<&str> = own.rows.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "fills",
            "ring",
            "text",
            "label",
            "numbers",
            "percent",
            "color",
            "fill_image",
            "background",
            "overlay"
        ]
    );
    assert!(own.rows.iter().all(|row| !row.here), "the kind's own");

    let across = harness.get_by_label_contains("HP ").rect().height();
    let play = &mut harness.state_mut().play;
    play.widget_change(&page, "text", Some("below"))
        .expect("changed");
    harness.run();
    let below = harness.get_by_label_contains("HP ").rect().height();
    assert!(
        below > across + 4.0,
        "drawn at once: {below} against {across}"
    );

    let play = &mut harness.state_mut().play;
    play.widget_change(&page, "fills", Some("up"))
        .expect("changed");
    play.widget_change(&page, "color", Some("#102030"))
        .expect("changed");
    play.widget_change(&page, "numbers", Some("off"))
        .expect("changed");
    assert!(
        play.widget_change(&page, "fills", Some("sideways"))
            .is_err()
    );
    assert!(play.widget_change(&page, "volume", Some("on")).is_err());
    let look = layout(&harness).looks.get(&health).cloned().expect("kept");
    assert_eq!((look.fills, look.place), (Fills::Up, Place::Below));
    assert_eq!(look.color, [0x10, 0x20, 0x30]);
    assert!(!look.says.numbers);
    let pages = harness
        .state()
        .play
        .widget_pages(&crate::play::Pictures::default());
    let own = pages
        .iter()
        .find(|found| found.id == page)
        .expect("its page");
    let color = own
        .rows
        .iter()
        .find(|row| row.key == "color")
        .expect("a colour row");
    assert_eq!(
        (&color.value, color.here),
        (&Value::Text("#102030".to_owned()), true)
    );

    let play = &mut harness.state_mut().play;
    for key in ["fills", "text", "color", "numbers"] {
        play.widget_change(&page, key, None).expect("put back");
    }
    assert!(
        !layout(&harness).looks.contains_key(&health),
        "all its own again"
    );

    harness
        .state_mut()
        .play
        .widget_change(&page, "fills", Some("down"))
        .expect("changed");
    harness.get_by_label_contains("HP ").click_secondary();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();
    assert!(
        !layout(&harness).looks.contains_key(&health),
        "its look goes with it"
    );
}

/// A change on a bar's own page is saved with the layout at once, and a
/// window opened again draws the bar so.
#[test]
fn a_bars_page_is_saved_with_the_layout() {
    let dir = std::env::temp_dir().join(format!("cena-bar-page-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut play = Play::new(0, "Ashryn", Some("Prime"), Some(dir.clone()));
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .build_ui_state(
            |ui, scene: &mut Scene| scene.draw(ui),
            Scene {
                play: std::mem::replace(&mut play, Play::new(0, "Ashryn", None, None)),
                ..Scene::new()
            },
        );
    harness.run();
    let page = harness
        .state()
        .play
        .widget_pages(&crate::play::Pictures::default())
        .into_iter()
        .find(|page| page.title.starts_with("Health"))
        .map(|page| page.id)
        .expect("a health bar's page");
    harness
        .state_mut()
        .play
        .widget_change(&page, "fills", Some("up"))
        .expect("changed");
    let reopened = Play::new(0, "Ashryn", Some("Prime"), Some(dir.clone()));
    let fills = reopened
        .widget_pages(&crate::play::Pictures::default())
        .into_iter()
        .find(|found| found.id == page)
        .and_then(|found| found.rows.into_iter().find(|row| row.key == "fills"))
        .map(|row| row.value);
    assert_eq!(fills, Some(cena_ui::settings::Value::Text("up".to_owned())));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A play window laid out as a player's first is, with the Room widget,
/// and that widget's page.
fn with_the_room_widget<'a>() -> (Harness<'a, Scene>, u32, String) {
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .build_ui_state(
            |ui, scene: &mut Scene| scene.draw(ui),
            Scene {
                play: Play::new(0, "Ashryn", None, None),
                ..Scene::new()
            },
        );
    harness.run();
    let room = layout(&harness)
        .holders
        .iter()
        .find_map(|holder| match &holder.holds {
            Holds::One(placed) if placed.widget == Widget::Room => Some(placed.id),
            Holds::One(_) | Holds::Custom(_) => None,
        })
        .unwrap_or_default();
    (harness, room, format!("widget:{room}"))
}

/// The Room widget's right-click opens its own page (the author,
/// 2026-09-28: *"it should take you to pick which streams show in the room
/// window"*): each part, all on, and the creatures not apart.
#[test]
fn the_rooms_right_click_opens_its_page() {
    use cena_ui::settings::Value;
    let (mut harness, _, page) = with_the_room_widget();
    harness
        .get_by_label("Obvious exits: north, out")
        .click_secondary();
    harness.run();
    harness.get_by_label("Settings...").click();
    harness.run();
    assert!(
        matches!(&harness.state().asked[..], [Asked::Settings(Some(opened))] if *opened == page),
        "{:?}",
        harness.state().asked
    );
    let pages = harness
        .state()
        .play
        .widget_pages(&crate::play::Pictures::default());
    let own = pages
        .iter()
        .find(|found| found.id == page)
        .expect("its page");
    assert_eq!(own.title, "Room");
    let rows: Vec<(&str, &Value)> = own
        .rows
        .iter()
        .map(|row| (row.key.as_str(), &row.value))
        .collect();
    assert_eq!(
        rows,
        [
            ("title", &Value::On(true)),
            ("description", &Value::On(true)),
            ("objects", &Value::On(true)),
            ("creatures", &Value::On(true)),
            ("players", &Value::On(true)),
            ("exits", &Value::On(true)),
            ("apart", &Value::On(false)),
        ]
    );
    assert!(own.rows.iter().all(|row| !row.here), "as it comes");
}

/// A change on the Room widget's page is kept by the widget's id and drawn
/// at once; every part back as it was keeps nothing; its parts go with it
/// when it is removed.
#[test]
fn the_room_shows_the_parts_its_page_picks() {
    let (mut harness, room, page) = with_the_room_widget();
    assert!(harness.query_by_label("Also here:").is_some());
    assert!(harness.query_by_label("Creatures:").is_none());
    let play = &mut harness.state_mut().play;
    play.widget_change(&page, "players", Some("off"))
        .expect("changed");
    play.widget_change(&page, "apart", Some("on"))
        .expect("changed");
    assert!(play.widget_change(&page, "apart", Some("maybe")).is_err());
    assert!(play.widget_change(&page, "weather", Some("on")).is_err());
    harness.run();
    assert!(
        harness.query_by_label("Also here:").is_none(),
        "drawn at once"
    );
    assert!(harness.query_by_label("Creatures:").is_some(), "apart");
    let kept = layout(&harness).rooms.get(&room).copied().expect("kept");
    assert!(!kept.players && kept.apart && kept.exits);
    let pages = harness
        .state()
        .play
        .widget_pages(&crate::play::Pictures::default());
    let here: Vec<&str> = pages
        .iter()
        .find(|found| found.id == page)
        .expect("its page")
        .rows
        .iter()
        .filter(|row| row.here)
        .map(|row| row.key.as_str())
        .collect();
    assert_eq!(here, ["players", "apart"]);

    let play = &mut harness.state_mut().play;
    play.widget_change(&page, "players", Some("on"))
        .expect("put back");
    play.widget_change(&page, "apart", None).expect("put back");
    assert!(
        !layout(&harness).rooms.contains_key(&room),
        "as it comes again"
    );
    harness
        .state_mut()
        .play
        .widget_change(&page, "description", Some("off"))
        .expect("changed");
    harness.run();
    assert!(layout(&harness).rooms.contains_key(&room), "kept");
    harness
        .get_by_label("Obvious exits: north, out")
        .click_secondary();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();
    assert!(
        !layout(&harness).rooms.contains_key(&room),
        "its parts go with it"
    );
}

/// The id of the first `widget` in the window's layout.
fn placed(harness: &Harness<'_, Scene>, widget: &Widget) -> u32 {
    layout(harness)
        .holders
        .iter()
        .find_map(|holder| match &holder.holds {
            Holds::One(one) if one.widget == *widget => Some(one.id),
            Holds::One(_) => None,
            Holds::Custom(custom) => custom
                .cells
                .iter()
                .flat_map(|cell| cell.tabs.iter())
                .find(|one| one.widget == *widget)
                .map(|one| one.id),
        })
        .unwrap_or_default()
}

/// The keys of page `page`, in order.
fn keys(harness: &Harness<'_, Scene>, page: &str) -> Vec<String> {
    harness
        .state()
        .play
        .widget_pages(&crate::play::Pictures::default())
        .into_iter()
        .find(|found| found.id == page)
        .map(|found| found.rows.into_iter().map(|row| row.key).collect())
        .unwrap_or_default()
}

/// The story's own page, which its right-click opens, says how it draws its
/// lines (the author, 2026-09-28: *"timestamp should also offer the
/// granularity, XX:XX, XX:XX:XX, XX:XX:XX AM/PM, 12/24 hour"*): a change is
/// drawn at once, a value it does not take is refused, and all back as it
/// was keeps nothing. A stream's page is the same less the story's own two.
#[test]
fn the_story_draws_its_lines_as_its_page_says() {
    let mut harness = harness();
    harness.run();
    let page = format!("widget:{}", placed(&harness, &Widget::Story));
    harness
        .get_by_label("You swing a steel broadsword at a kobold!")
        .click_secondary();
    harness.run();
    harness.get_by_label("Settings...").click();
    harness.run();
    assert!(
        matches!(&harness.state().asked[..], [Asked::Settings(Some(opened))] if *opened == page),
        "{:?}",
        harness.state().asked
    );
    assert_eq!(
        keys(&harness, &page),
        ["stamps", "seconds", "hours", "wrap", "prompts", "echo"]
    );

    let play = &mut harness.state_mut().play;
    play.widget_change(&page, "stamps", Some("start"))
        .expect("changed");
    play.widget_change(&page, "hours", Some("24"))
        .expect("changed");
    play.widget_change(&page, "echo", Some("off"))
        .expect("changed");
    assert!(play.widget_change(&page, "stamps", Some("middle")).is_err());
    assert!(play.widget_change(&page, "hours", Some("13")).is_err());
    harness.run();
    assert!(
        harness
            .query_by_label_contains("] You swing a steel broadsword at a kobold!")
            .is_some(),
        "its time before it"
    );
    assert!(
        harness.query_by_label_contains("M] You swing").is_none(),
        "on a 24-hour clock"
    );
    assert!(harness.query_by_label(">look").is_none(), "nothing typed");

    let play = &mut harness.state_mut().play;
    for key in ["stamps", "hours", "echo"] {
        play.widget_change(&page, key, None).expect("put back");
    }
    assert!(layout(&harness).lines.is_empty(), "as it always was");

    let Some(laid) = &mut harness.state_mut().play.layout else {
        panic!("laid out");
    };
    let thoughts = laid.add_widget(Widget::Stream("thoughts".to_owned()), None);
    let stream = format!("widget:{thoughts}");
    harness.run();
    assert_eq!(
        keys(&harness, &stream),
        ["stamps", "seconds", "hours", "wrap"]
    );
    let play = &mut harness.state_mut().play;
    assert!(
        play.widget_change(&stream, "prompts", Some("off")).is_err(),
        "a stream has no prompts"
    );
    play.widget_change(&stream, "wrap", Some("off"))
        .expect("changed");
    assert!(layout(&harness).lines.contains_key(&thoughts), "kept");
    let window = layout(&harness)
        .holders
        .iter()
        .find(|holder| matches!(&holder.holds, Holds::One(one) if one.id == thoughts))
        .map(|holder| holder.id)
        .unwrap_or_default();
    if let Some(layout) = &mut harness.state_mut().play.layout {
        layout.remove_widget(window, thoughts);
    }
    assert!(layout(&harness).lines.is_empty(), "gone with it");
}

/// The Injuries widget's page offers the dolls folder's pictures and keeps
/// the one chosen with the layout; None puts the body back.
#[test]
fn the_injury_dolls_page_takes_a_picture() {
    let mut harness = harness();
    let doll = harness
        .state_mut()
        .play
        .layout
        .as_mut()
        .map(|layout| layout.add_widget(Widget::Injuries, None))
        .unwrap_or_default();
    harness.run();
    let pictures = crate::play::Pictures {
        dolls: vec!["C:/dolls/ranger.png".into()],
        ..crate::play::Pictures::default()
    };
    let pages = harness.state().play.widget_pages(&pictures);
    let page = pages
        .iter()
        .find(|page| page.id == format!("widget:{doll}"))
        .expect("a page of its own");
    assert!(page.rows.iter().any(|row| row.key == "picture"));

    let play = &mut harness.state_mut().play;
    play.widget_change(&page.id, "picture", Some("C:/dolls/ranger.png"))
        .expect("changed");
    let kept = layout(&harness).dolls.get(&doll).cloned();
    assert_eq!(
        kept.and_then(|look| look.picture).as_deref(),
        Some("C:/dolls/ranger.png")
    );
    let play = &mut harness.state_mut().play;
    play.widget_change(&format!("widget:{doll}"), "picture", None)
        .expect("put back");
    assert!(!layout(&harness).dolls.contains_key(&doll));

    // Infinite, where this build has it; an unknown style refused.
    let play = &mut harness.state_mut().play;
    let page = format!("widget:{doll}");
    assert!(play.widget_change(&page, "style", Some("paper")).is_err());
    play.widget_change(&page, "style", Some("infinite"))
        .expect("changed");
    let kept = layout(&harness).dolls.get(&doll).map(|look| look.style);
    assert_eq!(kept, Some(crate::widget::doll::Style::Infinite));

    // Its skin, one of gs_studio's; an unknown one refused.
    #[cfg(feature = "doll-infinite")]
    skin_kept(&mut harness, &page, doll);
}

/// The Injuries page `page` keeps the skin chosen for the doll `doll`.
#[cfg(feature = "doll-infinite")]
fn skin_kept(harness: &mut Harness<'_, Scene>, page: &str, doll: u32) {
    let play = &mut harness.state_mut().play;
    assert!(play.widget_change(page, "skin", Some("paper")).is_err());
    play.widget_change(page, "skin", Some("sheruvian_monk"))
        .expect("changed");
    let kept = layout(harness).dolls.get(&doll).cloned();
    assert_eq!(
        kept.and_then(|look| look.skin).as_deref(),
        Some("sheruvian_monk")
    );
}
