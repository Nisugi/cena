//! The editor: a draft over a built-in, saved as only what differs; a
//! built-in's name refused; the draft worn while editing.

use std::collections::BTreeSet;

use cena_ui::theme::{Scheme, Theme, Themes, Token};
use egui_kittest::Harness;

use super::{Asked, Editor};

#[test]
fn a_draft_over_a_built_in_saves_only_what_differs() {
    let themes = Themes::built_in();
    let mut editor = Editor::default();
    assert_eq!(editor.name, "My Despana");
    assert_eq!(editor.base.as_deref(), Some("Despana"));
    assert!(
        editor.recipe.pins.is_empty(),
        "the base's pins are the base's"
    );
    // Nothing changed: the file says nothing but its name and base.
    let draft = editor.draft(&themes);
    assert!(draft.recipe == cena_ui::theme::RecipeFile::default());
    assert!(draft.pins.is_empty() && draft.shape.is_empty() && draft.kind.is_empty());
    // A change: the seed and one pin, and the corners.
    editor.recipe.seed = [0xc9, 0x73, 0x3a];
    editor.recipe.scheme = Scheme::Split;
    editor.recipe.pins.insert(Token::Health, [0x11, 0x22, 0x33]);
    editor.shape.corner = 0;
    let draft = editor.draft(&themes);
    assert_eq!(draft.recipe.seed.as_deref(), Some("#c9733a"));
    assert_eq!(draft.recipe.scheme.as_deref(), Some("split"));
    assert_eq!(draft.recipe.background, None, "the base's, unsaid");
    assert_eq!(draft.shape.corner, Some(0));
    assert_eq!(draft.shape.stroke, None);
    let written = draft.to_toml();
    let again = Theme::parse(&written, "x").expect("reads");
    assert_eq!(again, draft);
    // Worn, it is Despana but for what changed.
    let outfit = editor.outfit(&themes).expect("worn");
    assert_eq!(outfit.palette.get(Token::Health), [0x11, 0x22, 0x33]);
    assert_eq!(
        outfit.palette.get(Token::Mana),
        Token::Mana.bare(),
        "Despana's pin"
    );
    assert_eq!(outfit.shape.corner, 0);
}

#[test]
fn a_file_theme_is_edited_as_it_is() {
    let mut themes = Themes::built_in();
    themes.add(
        Theme::parse(
            "base = \"Light\"\n[pins]\nhealth = \"#112233\"\n[shape]\ncorner = 9\n",
            "ember",
        )
        .expect("reads"),
    );
    let mut editor = Editor::default();
    editor.start_from(&themes, "ember");
    assert_eq!(editor.name, "ember");
    assert_eq!(editor.base.as_deref(), Some("Light"));
    assert_eq!(
        editor.recipe.pins.get(&Token::Health),
        Some(&[0x11, 0x22, 0x33])
    );
    assert_eq!(editor.shape.corner, 9);
    let draft = editor.draft(&themes);
    assert_eq!(draft.shape.corner, Some(9));
    assert_eq!(draft.pins.len(), 1);
}

#[test]
fn a_built_in_name_and_an_empty_one_are_refused_and_a_save_is_asked() {
    let themes = Themes::built_in();
    let mut editor = Editor {
        name: "Despana".to_owned(),
        ..Editor::default()
    };
    assert!(editor.savable(&themes).unwrap_err().contains("built in"));
    editor.name = "  ".to_owned();
    assert!(editor.savable(&themes).unwrap_err().contains("name"));
    editor.name = "Ember".to_owned();
    editor.base = Some("Ember".to_owned());
    assert!(editor.savable(&themes).unwrap_err().contains("own base"));
    editor.base = Some("Despana".to_owned());
    let theme = editor.savable(&themes).expect("saved");
    assert_eq!(theme.name, "Ember");
}

#[test]
fn worn_while_editing_the_draft_is_sent_once_per_change() {
    let themes = Themes::built_in();
    let fonts = BTreeSet::new();
    let mut editor = Editor {
        open: true,
        wearing: true,
        ..Editor::default()
    };
    let shown = |editor: &mut Editor, frames: usize| {
        let sent = std::cell::RefCell::new(Vec::new());
        let mut harness = Harness::builder()
            .with_size((1000.0, 700.0))
            .build_ui(|ui| sent.borrow_mut().extend(editor.show(ui, &themes, &fonts)));
        for _ in 0..frames {
            harness.run();
        }
        drop(harness);
        sent.into_inner()
    };
    let sent = shown(&mut editor, 2);
    let previews = sent
        .iter()
        .filter(|ask| matches!(ask, Asked::Preview(Some(_))))
        .count();
    assert_eq!(previews, 1, "sent once, not every frame: {sent:?}");
    // Over Despana every token is pinned, so a seed changes nothing; a pin does.
    editor.recipe.pins.insert(Token::Health, [0xc9, 0x73, 0x3a]);
    let sent = shown(&mut editor, 1);
    assert!(
        sent.iter()
            .any(|ask| matches!(ask, Asked::Preview(Some(_)))),
        "sent again for the change"
    );
    editor.wearing = false;
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .build_ui(|ui| {
            let _ = editor.show(ui, &themes, &fonts);
        });
    harness.run();
    harness.snapshot("theme_editor");
}
