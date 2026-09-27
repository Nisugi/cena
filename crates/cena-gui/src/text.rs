//! A styled line as egui lays it out: each run's colour, background and
//! font. The colours were decided in the session, by the character's
//! triggers (`plan/45` §0); this only reads them, never picks one.

use cena_ui::StyledRun;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, TextStyle};

/// `runs` as one job, in `style`'s fonts: a run's trigger colour, else its
/// preset's ([`preset`]), else the strong colour for a bold run, else the
/// text colour; monospace runs in the monospace font.
pub(crate) fn job(runs: &[StyledRun], style: &egui::Style) -> LayoutJob {
    let body = TextStyle::Body.resolve(style);
    let mono = TextStyle::Monospace.resolve(style);
    let mut job = LayoutJob::default();
    for run in runs {
        let plain = if run.bold {
            style.visuals.strong_text_color()
        } else {
            style.visuals.text_color()
        };
        job.append(
            &run.text,
            0.0,
            TextFormat {
                font_id: if run.monospace {
                    mono.clone()
                } else {
                    body.clone()
                },
                color: run
                    .color
                    .as_deref()
                    .and_then(hex)
                    .or_else(|| run.preset.as_deref().and_then(preset))
                    .unwrap_or(plain),
                background: run
                    .background
                    .as_deref()
                    .and_then(hex)
                    .unwrap_or(Color32::TRANSPARENT),
                ..TextFormat::default()
            },
        );
    }
    job
}

/// The colour of a game preset, as Despana draws it
/// (`cena-web/assets/app.js`'s `PRESETS`, `style.css`); `None` for one it
/// does not colour.
pub(crate) fn preset(name: &str) -> Option<Color32> {
    match name {
        "roomName" => Some(AMBER),
        "monsterbold" => Some(CREATURE),
        "speech" => Some(Color32::from_rgb(0xf0, 0xee, 0xe8)),
        "whisper" | "thought" => Some(Color32::from_rgb(0xa9, 0xbb, 0xf5)),
        _ => None,
    }
}

/// Despana's amber: a room's name, a warning.
pub(crate) const AMBER: Color32 = Color32::from_rgb(0xd7, 0xad, 0x63);
/// A creature's colour, Despana's room window's and `monsterbold`'s.
pub(crate) const CREATURE: Color32 = Color32::from_rgb(0xbd, 0x8c, 0xff);
/// A player's colour in the room window, Despana's.
pub(crate) const PLAYER: Color32 = Color32::from_rgb(0x8c, 0xa8, 0xff);
/// An object's colour in the room window, Despana's.
pub(crate) const OBJECT: Color32 = Color32::from_rgb(0xb7, 0xbd, 0xc3);
/// Despana's colour for something lost or wrong.
pub(crate) const WRONG: Color32 = Color32::from_rgb(0xf0, 0x96, 0x8c);

/// A `#rrggbb` colour, as the session writes a trigger's paint
/// (`cena_model::trigger::Color`'s `Display`).
fn hex(text: &str) -> Option<Color32> {
    Color32::from_hex(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, color: Option<&str>) -> StyledRun {
        StyledRun {
            text: text.to_owned(),
            color: color.map(str::to_owned),
            ..StyledRun::default()
        }
    }

    #[test]
    fn a_trigger_colour_is_kept_and_the_rest_are_plain() {
        let style = egui::Style::default();
        let job = job(
            &[run("Kiyna", Some("#ff8000")), run(" says, hi", None)],
            &style,
        );
        assert_eq!(job.text, "Kiyna says, hi");
        let colors: Vec<Color32> = job.sections.iter().map(|s| s.format.color).collect();
        assert_eq!(
            colors,
            [
                Color32::from_rgb(0xff, 0x80, 0x00),
                style.visuals.text_color()
            ]
        );
    }

    /// A game preset takes Despana's colour; a trigger's paint still wins.
    #[test]
    fn a_preset_is_coloured_and_paint_wins() {
        let style = egui::Style::default();
        let spoken = |color: Option<&str>| StyledRun {
            text: "hi".to_owned(),
            preset: Some("speech".to_owned()),
            color: color.map(str::to_owned),
            ..StyledRun::default()
        };
        let job = job(&[spoken(None), spoken(Some("#ff0000"))], &style);
        assert_eq!(
            job.sections[0].format.color,
            Color32::from_rgb(0xf0, 0xee, 0xe8)
        );
        assert_eq!(job.sections[1].format.color, Color32::from_rgb(0xff, 0, 0));
    }

    #[test]
    fn a_colour_that_does_not_parse_is_plain() {
        let style = egui::Style::default();
        let job = job(&[run("x", Some("orange"))], &style);
        assert_eq!(job.sections[0].format.color, style.visuals.text_color());
    }
}
