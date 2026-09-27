//! A styled line as egui lays it out: each run's colour, background and
//! font. The colours were decided in the session, by the character's
//! triggers (`plan/45` §0); this only reads them, never picks one.

use cena_ui::StyledRun;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, TextStyle};

/// `runs` as one job, in `style`'s fonts: a run's trigger colour, else the
/// strong colour for a bold run, else the text colour; monospace runs in the
/// monospace font.
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
                color: run.color.as_deref().and_then(hex).unwrap_or(plain),
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

    #[test]
    fn a_colour_that_does_not_parse_is_plain() {
        let style = egui::Style::default();
        let job = job(&[run("x", Some("orange"))], &style);
        assert_eq!(job.sections[0].format.color, style.visuals.text_color());
    }
}
