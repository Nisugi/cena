//! A styled line as egui lays it out: each run's colour, background and
//! font. The colours were decided in the session, by the character's
//! triggers (`plan/45` §0); this only reads them, never picks one.

use cena_ui::{RunLink, StyledRun};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Pos2, TextStyle};

/// `runs` as one job, in `style`'s fonts: a run's trigger colour, else its
/// preset's ([`preset`]), else a link's ([`LINK`]) where it is not bold (a
/// creature keeps its own), else the strong colour for a bold run, else the
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
                    .or_else(|| (run.link.is_some() && !run.bold).then_some(LINK))
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

/// What a line with links in it was asked this frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Acted {
    /// A link clicked, and where.
    Clicked(RunLink, Pos2),
    /// A line to send without an echo: an object carried let go on another,
    /// `_drag #<item> #<onto>` (`carry.rs`).
    Quietly(String),
}

/// A line whose runs have links in it, `job` drawn as a label is: its words
/// selected by a drag, as any line's; a link under the pointer shows the
/// hand, and a click on one is the link's; with the drag key held an
/// object's link is carried from it instead (`carry.rs`), and one carried let
/// go on another object's link goes into it. The runs are the job's, in
/// order, their text its text.
///
/// It was not selectable, so a line with a link in it could not be selected,
/// and broke a selection across it (the author, 2026-09-28: *"links in the
/// lines break text selection"*). A drag selects; a click, which is no drag,
/// is still the link's.
pub(crate) fn linked(ui: &mut egui::Ui, job: LayoutJob, runs: &[StyledRun]) -> Option<Acted> {
    let carrying = crate::carry::held(ui);
    let sense = if carrying {
        egui::Sense::click_and_drag()
    } else {
        egui::Sense::click()
    };
    let (at, galley, response) = egui::Label::new(job)
        .sense(sense)
        .selectable(!carrying)
        .layout_in_ui(ui);
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, enabled, galley.text()));
    if ui.is_rect_visible(response.rect) {
        let color = ui.visuals().text_color();
        if carrying {
            ui.painter()
                .galley(at, std::sync::Arc::clone(&galley), color);
        } else {
            egui::text_selection::LabelSelectionState::label_text_selection(
                ui,
                &response,
                at,
                std::sync::Arc::clone(&galley),
                color,
                egui::Stroke::NONE,
            );
        }
    }
    let run_at = |pointer: Pos2| {
        // The nearest boundary between characters, and the character under
        // the pointer the one before it when the pointer is left of it: on
        // the right half of a link's last letter, the boundary is past it.
        let cursor = galley.cursor_from_pos(pointer - at);
        let mut char_at: usize = cursor.index.into();
        if char_at > 0 && (pointer - at).x < galley.pos_from_cursor(cursor).min.x {
            char_at -= 1;
        }
        let byte = galley
            .text()
            .char_indices()
            .nth(char_at)
            .map_or(galley.text().len(), |(byte, _)| byte);
        let mut start = 0;
        runs.iter().find(|run| {
            let span = start..start + run.text.len();
            start = span.end;
            span.contains(&byte)
        })
    };
    let link_at = |pointer: Pos2| run_at(pointer).and_then(|run| run.link.clone());
    if response.hover_pos().and_then(link_at).is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if carrying
        && response.drag_started()
        && let Some(origin) = ui.input(|input| input.pointer.press_origin())
        && let Some(run) = run_at(origin)
        && let Some(RunLink::Object { exist, .. }) = &run.link
    {
        response.dnd_set_drag_payload(crate::carry::Carried {
            exist: exist.clone(),
            name: run.text.clone(),
        });
    }
    if let Some(carried) = response.dnd_release_payload::<crate::carry::Carried>() {
        let onto = ui
            .input(|input| input.pointer.interact_pos())
            .and_then(link_at);
        if let Some(RunLink::Object { exist, .. }) = onto {
            // Into another object; onto itself, nothing.
            return (exist != carried.exist)
                .then(|| Acted::Quietly(format!("_drag #{} #{exist}", carried.exist)));
        }
        // Let go on the line's words: the window's own place takes it, the
        // story's floor.
        egui::DragAndDrop::set_payload(ui.ctx(), (*carried).clone());
    }
    if !response.clicked() {
        return None;
    }
    let pointer = response.interact_pointer_pos()?;
    link_at(pointer).map(|link| Acted::Clicked(link, pointer))
}

/// A link's colour: `VellumFE`'s, its `links` and `commands` presets' `Link`
/// (`defaults/globals/colors.toml`).
pub(crate) const LINK: Color32 = Color32::from_rgb(0x47, 0x7a, 0xb3);

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

    /// A link takes the link colour; a bold one (a creature) keeps the
    /// strong colour, and a trigger's paint still wins.
    #[test]
    fn a_link_takes_the_link_colour() {
        let style = egui::Style::default();
        let link = Some(cena_ui::RunLink::Command {
            command: "go north".to_owned(),
        });
        let linked = |bold: bool, color: Option<&str>| StyledRun {
            bold,
            link: link.clone(),
            ..run("north", color)
        };
        let job = job(
            &[
                linked(false, None),
                linked(true, None),
                linked(false, Some("#ff8000")),
            ],
            &style,
        );
        let colors: Vec<Color32> = job.sections.iter().map(|s| s.format.color).collect();
        assert_eq!(
            colors,
            [
                LINK,
                style.visuals.strong_text_color(),
                Color32::from_rgb(0xff, 0x80, 0x00)
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
