//! The injury doll's **Text** style (the author, 2026-09-29: *"injury doll
//! needs a text version, added to the dropdown"*): each part that shows
//! anything on a line of its own, in the Doll's colour and its tooltip's
//! words for it (`minor wound`, `severe scar`), a wound over
//! a scar and a foot as its leg, as the Doll shows them (`plan/55` §4); and
//! a line saying so when none does.

use std::collections::BTreeMap;

use cena_session::Injury;

use super::doll::{PARTS, level_color, said, shown};

/// `injuries` as lines, part by part in the Doll's order.
pub(super) fn text(ui: &mut egui::Ui, injuries: Option<&BTreeMap<String, Injury>>) {
    let Some(injuries) = injuries else {
        ui.weak("Injuries unknown");
        return;
    };
    let mut any = false;
    for part in &PARTS {
        let Some(shows) = shown(injuries, part) else {
            continue;
        };
        any = true;
        ui.colored_label(
            level_color(ui.ctx(), shows.level()),
            format!("{}: {}", capital(part.name), said(Some(shows))),
        );
    }
    if !any {
        ui.label("No injuries");
    }
}

/// `name` with its first letter a capital: `left arm` as `Left arm`.
fn capital(name: &str) -> String {
    let mut letters = name.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cena_session::Injury;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    /// A line a part: a wound over its scar, a foot as its leg, a scar
    /// alone; a whole part nothing; and with nothing to show, a line
    /// saying so.
    #[test]
    fn each_part_that_shows_is_a_line() {
        let hurt: BTreeMap<String, Injury> = [
            ("leftArm", 2, 1),
            ("rightFoot", 1, 0),
            ("chest", 0, 3),
            ("head", 0, 0),
        ]
        .into_iter()
        .map(|(part, wound, scar)| (part.to_owned(), Injury { wound, scar }))
        .collect();
        let drawn = |injuries: BTreeMap<String, Injury>| {
            let mut harness = Harness::builder()
                .with_size((240.0, 240.0))
                .build_ui(move |ui| super::text(ui, Some(&injuries)));
            harness.run();
            harness
        };
        let harness = drawn(hurt);
        for said in [
            "Left arm: wound",
            "Right leg: minor wound",
            "Chest: severe scar",
        ] {
            assert!(harness.query_by_label(said).is_some(), "{said}");
        }
        assert!(harness.query_by_label_contains("Head").is_none(), "whole");
        assert!(harness.query_by_label("No injuries").is_none());
        let whole = drawn(BTreeMap::new());
        assert!(whole.query_by_label("No injuries").is_some());
    }
}
