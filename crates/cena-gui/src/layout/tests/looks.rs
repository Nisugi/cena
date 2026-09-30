//! A bar's look kept with the layout.

use crate::layout::Layout;
use egui::Vec2;

/// A bar's look is kept with the layout and read back as it was.
#[test]
fn a_bars_look_is_kept_with_the_layout() {
    use crate::bar::{Fills, Look, Place, Says};
    let dir = std::env::temp_dir().join(format!("cena-layout-look-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let look = Look {
        fills: Fills::Up,
        place: Place::Below,
        says: Says {
            label: true,
            numbers: false,
            percent: true,
            words: false,
        },
        color: Some([0x10, 0x20, 0x30]),
        overlay: Some("C:/overlays/gloss.png".to_owned()),
        background: Some("C:/overlays/glass.png".to_owned()),
        fill_image: None,
        ring: 40,
        clock: false,
    };
    layout.looks.insert(7, look.clone());
    let lines = crate::widget::Lines {
        stamps: crate::widget::Stamps::End,
        seconds: true,
        hours: crate::story::Hours::TwentyFour,
        ..crate::widget::Lines::default()
    };
    layout.lines.insert(8, lines);
    layout.save(&dir, None, "Ashryn").expect("saved");
    let read = Layout::load(&dir, None, "Ashryn").expect("read back");
    assert_eq!(read.looks.get(&7), Some(&look));
    assert_eq!(read.lines.get(&8), Some(&lines), "a story's times too");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A look saved before orbs, rings and their images reads as it was, a
/// ring's thickness its default: a look that failed to read would lose the
/// whole layout to a fitted one.
#[test]
fn a_look_saved_before_orbs_reads_as_it_was() {
    use crate::bar::{Fills, Look};
    let saved = r#"{"fills":"up","place":"inside","says":{"label":true,"numbers":false,"percent":true},"color":[71,132,217]}"#;
    let look: Look = serde_json::from_str(saved).expect("read");
    assert_eq!(look.fills, Fills::Up);
    assert_eq!(
        (look.ring, look.background.clone(), look.fill_image.clone()),
        (Look::RING, None, None)
    );
    // Its colour is mana's as it was before themes, saved whether or not
    // the player chose it: on a mana bar it is read as none, so a theme can
    // colour the bar; on a health bar it is a colour the player chose.
    assert_eq!(look.clone().themed(crate::theme::T::Mana).color, None);
    assert_eq!(
        look.themed(crate::theme::T::Health).color,
        Some([71, 132, 217])
    );
}
