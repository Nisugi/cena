//! The doll calibrator (`plan/55` §2e): where each part sits on a doll
//! picture, and how its dots look, saved **into the picture**
//! (`calibration.rs`).
//!
//! Opened from an Injuries widget's right-click menu once it has a picture.
//! The parts are listed, the calibrated ones marked; a click on the picture
//! puts the chosen part there and moves on to the next. The picture is drawn
//! by the widget's own renderer (`widget/doll.rs`, `drawn`) with the
//! calibration as it stands, every part showing a preview wound or scar, so
//! what is calibrated is what the widget will show. `VellumFE` drew its two
//! editors' canvases twice over, and its simple one put dots where the
//! widget drew art (`plan/55` §1b).

use std::collections::BTreeMap;
use std::path::Path;

use cena_session::Injury;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

use crate::calibration::{self, Calibration};
use crate::widget::doll::{self, PARTS};

/// The calibrator of one Injuries widget's picture, while it is open.
#[derive(Debug)]
pub(crate) struct Calibrator {
    /// The Injuries widget it was opened from.
    pub(crate) placed: u32,
    /// The picture's path.
    picture: String,
    calibration: Calibration,
    /// The part the next click places, by its place in [`PARTS`].
    chosen: usize,
    /// A click moves on to the next part.
    advance: bool,
    /// The preview shows scars, else wounds.
    scars: bool,
    /// The preview's rank.
    rank: u8,
    /// What saving last said.
    said: Option<String>,
    /// Where the picture was last drawn, on the screen.
    drawn_at: Option<Rect>,
}

impl Calibrator {
    /// Open on the picture at `picture`, its calibration as saved.
    pub(crate) fn open(placed: u32, picture: String) -> Self {
        let calibration = Calibration::of_picture(Path::new(&picture));
        Self {
            placed,
            picture,
            calibration,
            chosen: 0,
            advance: true,
            scars: false,
            rank: 2,
            said: None,
            drawn_at: None,
        }
    }

    /// Draw it in a window of its own, `id` its window's; `false` once it is
    /// closed.
    pub(crate) fn show(&mut self, context: &egui::Context, id: egui::Id) -> bool {
        let mut open = true;
        egui::Window::new("Calibrate doll")
            .id(id)
            .open(&mut open)
            .default_size([560.0, 520.0])
            .show(context, |ui| self.contents(ui));
        open
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                // A column of its own, so its separator and sliders do not
                // take the picture's room.
                ui.set_max_width(210.0);
                self.parts(ui);
                ui.separator();
                self.style(ui);
            });
            ui.add_space(8.0);
            ui.vertical(|ui| self.canvas(ui));
        });
    }

    /// The parts, the chosen one and the calibrated ones marked.
    fn parts(&mut self, ui: &mut egui::Ui) {
        ui.label("Click where each part is:");
        for (at, part) in PARTS.iter().enumerate() {
            let placed = if self.calibration.anchors.contains_key(part.id) {
                "\u{2022} "
            } else {
                "   "
            };
            if ui
                .selectable_label(self.chosen == at, format!("{placed}{}", part.name))
                .clicked()
            {
                self.chosen = at;
            }
        }
        ui.checkbox(&mut self.advance, "Then the next part");
        if ui
            .button("Use default")
            .on_hover_text("This part back where the doll puts it on any picture")
            .clicked()
        {
            self.calibration.anchors.remove(PARTS[self.chosen].id);
        }
        if ui.button("Reset all").clicked() {
            self.calibration.anchors.clear();
        }
    }

    /// The picture, drawn as the widget draws it, and where a click puts
    /// the chosen part.
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let Some(texture) = crate::pictures::picture(ui.ctx(), &self.picture) else {
            ui.label(format!("{} could not be read.", self.picture));
            return;
        };
        let preview = self.preview();
        let layers = crate::doll_art::layers(ui.ctx(), &self.picture);
        let (rect, response) = ui
            .allocate_ui(Vec2::new(300.0, 340.0), |ui| {
                doll::drawn(
                    ui,
                    Some(&preview),
                    Some(doll::Over {
                        texture: &texture,
                        calibration: &self.calibration,
                        layers: &layers,
                    }),
                    egui::Sense::click(),
                )
            })
            .inner;
        self.drawn_at = Some(rect);
        let part = &PARTS[self.chosen];
        let (x, y) = self.calibration.anchor(part);
        let marked = rect.min + Vec2::new(x * rect.width(), y * rect.height());
        ui.painter()
            .circle_stroke(marked, 12.0, Stroke::new(2.0, Color32::YELLOW));
        if response.clicked()
            && let Some(at) = response.interact_pointer_pos()
            && let Some(put) = fraction(rect, at)
        {
            self.calibration.anchors.insert(part.id, put);
            self.said = None;
            if self.advance {
                self.chosen = (self.chosen + 1) % PARTS.len();
            }
        }
    }

    /// Every part hurt at the preview's rank, wounds or scars.
    fn preview(&self) -> BTreeMap<String, Injury> {
        let injury = if self.scars {
            Injury {
                wound: 0,
                scar: self.rank,
            }
        } else {
            Injury {
                wound: self.rank,
                scar: 0,
            }
        };
        PARTS
            .iter()
            .map(|part| (part.id.to_owned(), injury))
            .collect()
    }

    /// The dots' size and opacity, the preview, and saving.
    fn style(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Slider::new(&mut self.calibration.diameter, 0.02..=0.2).text("dot size"));
        ui.add(egui::Slider::new(&mut self.calibration.opacity, 0.2..=1.0).text("opacity"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.scars, false, "Wounds");
            ui.radio_value(&mut self.scars, true, "Scars");
        });
        ui.add(egui::Slider::new(&mut self.rank, 1..=3).text("preview rank"));
        if ui
            .button("Save")
            .on_hover_text("Into the picture itself, so it travels with it")
            .clicked()
        {
            self.said = Some(
                match calibration::write(Path::new(&self.picture), &self.calibration) {
                    Ok(()) => "Saved into the picture.".to_owned(),
                    Err(why) => format!("Not saved: {why}"),
                },
            );
        }
        if let Some(said) = &self.said {
            ui.label(said.as_str());
        }
    }
}

impl super::Play {
    /// The calibrator, while it is open.
    pub(super) fn calibrator(&mut self, context: &egui::Context) {
        if let Some(open) = &mut self.calibrating
            && !open.show(
                context,
                egui::Id::new(("calibrate", self.session, open.placed)),
            )
        {
            self.calibrating = None;
        }
    }
}

/// Where `at` lies on the picture drawn in `rect`, as fractions of it, or
/// `None` off it.
pub(crate) fn fraction(rect: Rect, at: Pos2) -> Option<(f32, f32)> {
    if !rect.contains(at) || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    Some((
        (at.x - rect.min.x) / rect.width(),
        (at.y - rect.min.y) / rect.height(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{Calibrator, fraction};
    use egui::{Pos2, Rect};
    use egui_kittest::kittest::Queryable;

    /// A picture of the test's own, in a folder of its own.
    fn picture(name: &str) -> Option<std::path::PathBuf> {
        let dir =
            std::env::temp_dir().join(format!("cena-calibrator-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("doll.png");
        image::RgbaImage::from_pixel(100, 200, image::Rgba([40, 60, 90, 255]))
            .save(&path)
            .ok()?;
        Some(path)
    }

    /// The calibrator open on it: the chosen part placed where the picture
    /// is clicked, the next part chosen, and Save writing it into the
    /// picture, where the widget reads it.
    #[test]
    fn a_click_places_the_part_and_save_writes_it_into_the_picture() {
        let path = picture("click").unwrap();
        let open = Calibrator::open(7, path.to_string_lossy().into_owned());
        let mut harness = egui_kittest::Harness::builder()
            .with_size((640.0, 600.0))
            .build_ui_state(
                |ui, calibrator: &mut Calibrator| {
                    let _ = calibrator.show(ui.ctx(), egui::Id::new("calibrate"));
                },
                open,
            );
        harness.run();
        harness.snapshot("calibrator");

        let drawn = harness.state().drawn_at.expect("the picture drawn");
        // A click: pressed and let go where it was pressed.
        harness.hover_at(drawn.center());
        harness.step();
        harness.drag_at(drawn.center());
        harness.step();
        harness.drop_at(drawn.center());
        harness.run();
        let placed = harness.state().calibration.anchors.get("head").copied();
        let (x, y) = placed.expect("the head placed");
        assert!((x - 0.5).abs() < 0.02 && (y - 0.5).abs() < 0.02, "{x}, {y}");
        assert_eq!(harness.state().chosen, 1, "on to the left eye");

        harness.get_by_label("Save").click();
        harness.run();
        assert_eq!(
            harness.state().said.as_deref(),
            Some("Saved into the picture."),
            "saving said so"
        );
        let saved = crate::calibration::Calibration::of_picture(&path);
        assert_eq!(saved.anchors.get("head").copied(), placed, "in the picture");
        assert!(
            crate::calibration::embedded(&std::fs::read(&path).unwrap()).is_some(),
            "and in no file beside it"
        );
        assert!(!path.with_extension("toml").exists());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A click is a fraction of the drawn picture, whatever its size: the
    /// same place on a picture drawn small or large.
    #[test]
    fn a_click_is_a_fraction_of_the_picture_at_any_size() {
        let small = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(70.0, 140.0));
        let large = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(300.0, 600.0));
        assert_eq!(fraction(small, Pos2::new(25.0, 50.0)), Some((0.25, 0.25)));
        assert_eq!(fraction(large, Pos2::new(75.0, 150.0)), Some((0.25, 0.25)));
        assert_eq!(
            fraction(small, Pos2::new(5.0, 50.0)),
            None,
            "off the picture"
        );
    }
}
