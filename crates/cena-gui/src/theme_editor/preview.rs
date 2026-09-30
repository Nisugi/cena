//! The editor's preview: the whole palette as it comes out, each token a
//! swatch by its group, and a sample of story text in it, in the draft's
//! fonts and on its canvas.

use std::collections::BTreeSet;

use cena_ui::StyledRun;
use cena_ui::theme::{Group, Outfit, Token};

/// The story's sample: a room's name, a creature, speech, a link, and
/// Hydra's own line.
fn sample() -> Vec<Vec<StyledRun>> {
    let run = |text: &str| StyledRun {
        text: text.to_owned(),
        ..StyledRun::default()
    };
    let preset = |text: &str, preset: &str| StyledRun {
        preset: Some(preset.to_owned()),
        ..run(text)
    };
    vec![
        vec![preset("[Rawknuckle's, Watering Hole]", "roomName")],
        vec![
            run("You also see "),
            StyledRun {
                link: Some(cena_ui::RunLink::Object {
                    exist: "1".to_owned(),
                    noun: "sword".to_owned(),
                    coord: None,
                }),
                ..run("a steel broadsword")
            },
            run(" and "),
            StyledRun {
                bold: true,
                ..preset("a kobold", "monsterbold")
            },
            run("."),
        ],
        vec![preset("Maravel says, \"Watch the kobold.\"", "speech")],
        vec![preset(
            "You hear the faint thoughts of Dicate echo in your mind.",
            "thought",
        )],
        vec![StyledRun {
            monospace: true,
            ..run("  HP 348/400   MP 48/120")
        }],
    ]
}

/// Draw the preview into `ui`.
pub(super) fn show(ui: &mut egui::Ui, outfit: &Outfit, fonts: &BTreeSet<String>) {
    let style = crate::theme::style(outfit, fonts);
    let of = |token| crate::theme::rgb(outfit.palette.get(token));
    egui::Frame::new()
        .fill(of(Token::Canvas))
        .stroke(egui::Stroke::new(1.0, of(Token::LineStrong)))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            for runs in sample() {
                let job = crate::text::job(&runs, &style, &outfit.palette);
                ui.label(job);
            }
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (token, label) in [
                    (Token::Health, "HP"),
                    (Token::Mana, "MP"),
                    (Token::Stamina, "SP"),
                    (Token::Spirit, "Sp"),
                ] {
                    ui.add(
                        crate::bar::Bar::new(
                            label,
                            Some(crate::bar::Amount {
                                percent: 70,
                                current: None,
                                max: None,
                            }),
                        )
                        .fill(of(token))
                        .size([72.0, 16.0]),
                    );
                }
            });
        });
    ui.add_space(6.0);
    for group in [
        Group::Surfaces,
        Group::Text,
        Group::Vitals,
        Group::Status,
        Group::Injuries,
        Group::Map,
        Group::Marks,
        Group::Chrome,
    ] {
        ui.horizontal_wrapped(|ui| {
            for token in Token::ALL.into_iter().filter(|t| t.group() == group) {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(22.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, of(token));
                response.on_hover_text(format!(
                    "{}: {}",
                    token.name(),
                    cena_ui::theme::hex(outfit.palette.get(token))
                ));
            }
        });
    }
}
