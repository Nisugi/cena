//! An object's menu as a frontend shows it: the game's `<menu>` of bare
//! coordinates, each labelled and given the command it sends by the model's
//! dictionary ([`cena_model::MenuCommands`]), grouped by category in the
//! game's order. The model resolves once for every frontend
//! (`crates/cena-model/src/state/menu.rs`, *Why this lives in the model*);
//! this is what a frontend draws of it (the author, 2026-09-28: *"links ...
//! clickable with their menus popping up"*).

use cena_model::{GameState, Menu, MenuCommands, category_path};

/// One entry of an object's menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    /// What it says: `attack`, `ask about amplify`; for a coordinate the
    /// dictionary does not know, the coordinate, so the entry is seen.
    pub label: String,
    /// What choosing it sends; `None` when it cannot be sent: an unknown
    /// coordinate, or one still wanting a second word (`%`).
    pub command: Option<String>,
}

/// A category of an object's menu, and its entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuGroup {
    /// Its path below the top: empty for a top-level category, whose
    /// entries a menu shows in place; `["focused multistrike"]` for one a
    /// level down (`cena_model::category_path`).
    pub under: Vec<String>,
    /// Its entries, in the game's order.
    pub entries: Vec<MenuEntry>,
}

/// `menu` for the object clicked, its id and noun, as `state` has taught the
/// dictionary. The dictionary's `_dialog` entries are left out: each opens a
/// dialog of the Wrayth client's own, which no frontend here has, as
/// `VellumFE` leaves them out (`core/app_core/state/menus.rs`).
#[must_use]
pub fn object_menu(
    menu: &Menu,
    clicked: (&str, &str),
    state: Option<&GameState>,
) -> Vec<MenuGroup> {
    let learned = state.map(|state| &state.learned_commands);
    MenuCommands::get()
        .resolve_grouped(menu, clicked, None, learned)
        .into_iter()
        .map(|(category, items)| MenuGroup {
            under: category.as_deref().map_or_else(Vec::new, |category| {
                category_path(category)
                    .into_iter()
                    .skip(1)
                    .map(str::to_owned)
                    .collect()
            }),
            entries: items
                .into_iter()
                .filter(|item| {
                    !item
                        .command
                        .as_deref()
                        .is_some_and(|command| command.starts_with("_dialog"))
                })
                .map(|item| {
                    let wanting = item.needs_secondary;
                    MenuEntry {
                        label: item.label.unwrap_or_else(|| format!("({})", item.coord)),
                        command: item.command.filter(|_| !wanting),
                    }
                })
                .collect(),
        })
        .filter(|group| !group.entries.is_empty())
        .collect()
}

/// The command an object link's own `coord=` names, for the object `clicked`,
/// its id and noun, as `state` has taught the dictionary: a click sends it
/// rather than asking for a menu, as `VellumFE`'s `resolve_link_dispatch`
/// does. `None` for a coordinate the dictionary lacks, a dialog, or one
/// wanting a second word.
///
/// Resolved as a menu's entry is, the server's row first: this read the
/// shipped table alone, so a coordinate the game had changed sent its old
/// command from a link and its new one from the menu (the crate review of
/// 2026-09-28, R14).
#[must_use]
pub fn link_command(
    coord: &str,
    clicked: (&str, &str),
    state: Option<&GameState>,
) -> Option<String> {
    let item = cena_model::MenuItem {
        coord: Some(coord.to_owned()),
        ..cena_model::MenuItem::default()
    };
    let learned = state.map(|state| &state.learned_commands);
    let resolved = MenuCommands::get().resolve_item(&item, clicked, None, learned);
    let wanting = resolved.needs_secondary;
    resolved
        .command
        .filter(|command| !wanting && !command.starts_with("_dialog"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A group's entries as a test compares them: each label and command.
    type Entries = Vec<(String, Option<String>)>;

    /// A direct link and the menu agree on what a coordinate sends, the
    /// server's row first: one the game changed, and one only it knows.
    #[test]
    fn a_link_and_the_menu_send_what_the_game_taught() {
        let mut state = GameState::default();
        let row = |coord: &str, command: &str| cena_model::MenuCommand {
            coord: coord.to_owned(),
            label: command.replace('#', "@"),
            command: command.to_owned(),
            category: "1".to_owned(),
        };
        state
            .learned_commands
            .absorb(&[row("2524,1543", "inspect #"), row("2524,9999", "pet #")]);
        let clicked = ("123", "kobold");
        for (coord, sent) in [("2524,1543", "inspect #123"), ("2524,9999", "pet #123")] {
            let menu = object_menu(&menu(&[(coord, None)]), clicked, Some(&state));
            assert_eq!(menu[0].entries[0].command.as_deref(), Some(sent));
            assert_eq!(
                link_command(coord, clicked, Some(&state)).as_deref(),
                Some(sent)
            );
        }
        assert_eq!(
            link_command("2524,1543", clicked, None).as_deref(),
            Some("attack #123"),
            "the shipped table, untaught"
        );
        assert_eq!(link_command("2524,1553", clicked, None), None, "a dialog");
        assert_eq!(
            link_command("2524,2184", clicked, None),
            None,
            "wanting a word"
        );
    }

    /// A menu of the coordinates given, as the game sends one, `id` 1.
    fn menu(items: &[(&str, Option<&str>)]) -> Menu {
        let mut menu = Menu {
            id: "1".to_owned(),
            ..Menu::default()
        };
        for (coord, noun) in items {
            menu.items.push(cena_model::MenuItem::default());
            if let Some(item) = menu.items.last_mut() {
                item.coord = Some((*coord).to_owned());
                item.noun = noun.map(str::to_owned);
            }
        }
        menu
    }

    /// Each entry labelled and given its command for the object clicked,
    /// grouped under its category's path; a dialog left out; one still
    /// wanting its second word, and an unknown coordinate, shown unsendable.
    #[test]
    fn an_objects_menu_is_labelled_and_grouped() {
        let groups = object_menu(
            &menu(&[
                ("2524,1543", None),
                ("2524,1906", Some("amplify")),
                ("2524,1553", None),
                ("2524,2184", None),
                ("9999,9999", None),
            ]),
            ("123", "kobold"),
            None,
        );
        let said: Vec<(Vec<String>, Entries)> = groups
            .into_iter()
            .map(|group| {
                let entries = group
                    .entries
                    .into_iter()
                    .map(|entry| (entry.label, entry.command))
                    .collect();
                (group.under, entries)
            })
            .collect();
        let entry =
            |label: &str, command: Option<&str>| (label.to_owned(), command.map(str::to_owned));
        assert_eq!(
            said,
            [
                (vec![], vec![entry("attack", Some("attack #123"))]),
                (
                    vec!["questions".to_owned()],
                    vec![entry("ask about amplify", Some("ask #123 about amplify"))]
                ),
                (vec![], vec![entry("transfer %", None)]),
                (vec![], vec![entry("(9999,9999)", None)]),
            ]
        );
    }
}
