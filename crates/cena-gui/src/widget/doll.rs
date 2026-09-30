//! The injury doll (`plan/55`): the Injuries widget.
//!
//! This is its first style, the **Doll**, over a body drawn in code (the
//! author: *"hydra should draw a body in code"*): each hurt part a dot at its
//! place, coloured by what shows there and numbered with its rank. Both
//! tracks show always, the wound first (*"wounds > scars > nothing"*), and a
//! foot is folded into its leg (*"rolled into legs everywhere"*).
//!
//! The body's shapes, the parts' places and the palette are `VellumFE`'s
//! (`reference/VellumFE/src/frontend/gui/app/widgets/injury.rs:253-370`,
//! `reference/VellumFE/src/config/skins.rs:616-631`,
//! `reference/VellumFE/src/config/widgets.rs:863-871`). Unlike `VellumFE`,
//! the dots take the palette's colour for their level, not one colour for
//! every wound, and a part's tooltip is built only while it is pointed at.

use std::collections::BTreeMap;

use cena_session::Injury;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use serde::{Deserialize, Serialize};

use crate::calibration::Calibration;

/// What a part shows: its wound, else its scar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shown {
    /// A wound of this rank.
    Wound(u8),
    /// A scar of this rank, with no wound over it.
    Scar(u8),
}

impl Shown {
    /// Its level in the palette: 1-3 a wound, 4-6 a scar.
    pub(crate) const fn level(self) -> usize {
        match self {
            Self::Wound(rank) => rank as usize,
            Self::Scar(rank) => rank as usize + 3,
        }
    }

    /// Its rank.
    const fn rank(self) -> u8 {
        match self {
            Self::Wound(rank) | Self::Scar(rank) => rank,
        }
    }
}

/// `VellumFE`'s palette: whole, three wounds brown to red, three scars light
/// to dark grey.
pub(crate) const PALETTE: [Color32; 7] = [
    Color32::from_rgb(0x33, 0x33, 0x33),
    Color32::from_rgb(0xaa, 0x55, 0x00),
    Color32::from_rgb(0xff, 0x88, 0x00),
    Color32::from_rgb(0xff, 0x00, 0x00),
    Color32::from_rgb(0x99, 0x99, 0x99),
    Color32::from_rgb(0x77, 0x77, 0x77),
    Color32::from_rgb(0x55, 0x55, 0x55),
];

/// The body drawn under the dots.
const BODY: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x50);

/// A part's shape on the body, in fractions of the doll's rect; sizes in
/// fractions of its height.
#[derive(Clone, Copy)]
enum Shape {
    Circle {
        c: (f32, f32),
        r: f32,
    },
    Block {
        min: (f32, f32),
        max: (f32, f32),
    },
    Line {
        a: (f32, f32),
        b: (f32, f32),
        w: f32,
    },
    Letter {
        c: (f32, f32),
        letter: &'static str,
    },
}

/// One of the doll's fourteen parts: the game's id, the feet folded into it,
/// what a player calls it, where its dot sits, and its shape on the body.
pub(crate) struct Part {
    /// The injury window's id.
    pub(crate) id: &'static str,
    /// A part folded into this one.
    folds: Option<&'static str>,
    /// What a player calls it.
    pub(crate) name: &'static str,
    /// Where its dot sits, as fractions of the picture.
    pub(crate) anchor: (f32, f32),
    shape: Shape,
}

/// The parts, in `VellumFE`'s order: head before the eyes, so the eyes draw
/// over it.
pub(crate) const PARTS: [Part; 14] = [
    part(
        "head",
        "head",
        (0.50, 0.09),
        Shape::Circle {
            c: (0.50, 0.105),
            r: 0.085,
        },
    ),
    part(
        "leftEye",
        "left eye",
        (0.44, 0.06),
        Shape::Circle {
            c: (0.465, 0.09),
            r: 0.018,
        },
    ),
    part(
        "rightEye",
        "right eye",
        (0.56, 0.06),
        Shape::Circle {
            c: (0.535, 0.09),
            r: 0.018,
        },
    ),
    part(
        "neck",
        "neck",
        (0.50, 0.20),
        Shape::Block {
            min: (0.465, 0.19),
            max: (0.535, 0.235),
        },
    ),
    part(
        "chest",
        "chest",
        (0.50, 0.30),
        Shape::Block {
            min: (0.38, 0.235),
            max: (0.62, 0.41),
        },
    ),
    part(
        "abdomen",
        "abdomen",
        (0.50, 0.45),
        Shape::Block {
            min: (0.395, 0.41),
            max: (0.605, 0.525),
        },
    ),
    part(
        "back",
        "back",
        (0.12, 0.92),
        Shape::Letter {
            c: (0.12, 0.93),
            letter: "B",
        },
    ),
    part(
        "leftArm",
        "left arm",
        (0.31, 0.36),
        Shape::Line {
            a: (0.365, 0.26),
            b: (0.265, 0.47),
            w: 0.045,
        },
    ),
    part(
        "rightArm",
        "right arm",
        (0.69, 0.36),
        Shape::Line {
            a: (0.635, 0.26),
            b: (0.735, 0.47),
            w: 0.045,
        },
    ),
    part(
        "leftHand",
        "left hand",
        (0.25, 0.53),
        Shape::Circle {
            c: (0.25, 0.515),
            r: 0.033,
        },
    ),
    part(
        "rightHand",
        "right hand",
        (0.75, 0.53),
        Shape::Circle {
            c: (0.75, 0.515),
            r: 0.033,
        },
    ),
    Part {
        folds: Some("leftFoot"),
        ..part(
            "leftLeg",
            "left leg",
            (0.42, 0.75),
            Shape::Line {
                a: (0.44, 0.53),
                b: (0.41, 0.90),
                w: 0.055,
            },
        )
    },
    Part {
        folds: Some("rightFoot"),
        ..part(
            "rightLeg",
            "right leg",
            (0.58, 0.75),
            Shape::Line {
                a: (0.56, 0.53),
                b: (0.59, 0.90),
                w: 0.055,
            },
        )
    },
    part(
        "nsys",
        "nervous system",
        (0.88, 0.92),
        Shape::Letter {
            c: (0.88, 0.93),
            letter: "N",
        },
    ),
];

const fn part(id: &'static str, name: &'static str, anchor: (f32, f32), shape: Shape) -> Part {
    Part {
        id,
        folds: None,
        name,
        anchor,
        shape,
    }
}

/// What `part` shows among `injuries`: a folded foot counting as its leg,
/// the worse of the two on each track.
pub(crate) fn shown(injuries: &BTreeMap<String, Injury>, part: &Part) -> Option<Shown> {
    let of = |id: Option<&str>| {
        id.and_then(|id| injuries.get(id))
            .copied()
            .unwrap_or_default()
    };
    let (own, folded) = (of(Some(part.id)), of(part.folds));
    let (wound, scar) = (own.wound.max(folded.wound), own.scar.max(folded.scar));
    if wound > 0 {
        Some(Shown::Wound(wound))
    } else if scar > 0 {
        Some(Shown::Scar(scar))
    } else {
        None
    }
}

/// The body's width to its height.
const ASPECT: f32 = 0.75;

/// What the player chose for one Injuries widget on its own page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DollLook {
    /// The picture the doll is drawn over, a PNG in the data folder's
    /// `dolls`; with none, the body drawn in code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) picture: Option<String>,
    /// Its style: the Doll (a picture's overlays make it the Doll plus), or
    /// Infinite, `gs_studio`'s puppet, where this build has it.
    #[serde(default, skip_serializing_if = "Style::is_doll")]
    pub(crate) style: Style,
    /// The skin the Infinite puppet wears, one of `gs_studio`'s for its
    /// form, or `-` for the form's own; with none, the lay figure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) skin: Option<String>,
}

/// An Injuries widget's style (`plan/55` §2a).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Style {
    /// Dots, over a picture or the body drawn in code; with a picture's
    /// overlays, the Doll plus.
    #[default]
    Doll,
    /// `gs_studio`'s puppet (`infinite.rs`); the Doll where it cannot be drawn,
    /// or this build has it not.
    Infinite,
    /// A line a part (`doll_text.rs`).
    Text,
}

impl Style {
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's skip_serializing_if hands a reference"
    )]
    fn is_doll(&self) -> bool {
        *self == Self::Doll
    }
}

/// How the dots are drawn: where each part's sits, how wide, how opaque.
struct Dots<'a> {
    calibration: Option<&'a Calibration>,
    /// A dot's radius, in points.
    radius: f32,
    opacity: f32,
}

impl Dots<'_> {
    fn anchor(&self, part: &Part) -> (f32, f32) {
        self.calibration
            .map_or(part.anchor, |calibration| calibration.anchor(part))
    }
}

/// Draw the doll for `injuries` in what is left of `ui`, over the picture
/// `look` names when it can be read, and over the body drawn in code when
/// not.
pub(super) fn doll(
    ui: &mut egui::Ui,
    injuries: Option<&BTreeMap<String, Injury>>,
    look: Option<&DollLook>,
) {
    let chosen = look.and_then(|look| look.picture.as_deref());
    let picture = chosen.and_then(|path| crate::pictures::picture(ui.ctx(), path));
    let calibration = chosen
        .filter(|_| picture.is_some())
        .map(|path| crate::pictures::calibration(ui.ctx(), path));
    let layers = chosen
        .filter(|_| picture.is_some())
        .map(|path| crate::doll_art::layers(ui.ctx(), path));
    let over = match (&picture, &calibration, &layers) {
        (Some(texture), Some(calibration), Some(layers)) => Some(Over {
            texture,
            calibration,
            layers,
        }),
        _ => None,
    };
    let _ = drawn(ui, injuries, over, egui::Sense::hover());
}

/// A doll picture as drawn: the picture, where its parts sit, and its
/// overlays (`doll_art.rs`).
#[derive(Clone, Copy)]
pub(crate) struct Over<'a> {
    /// The picture.
    pub(crate) texture: &'a egui::TextureHandle,
    /// Where its parts sit, and how its dots look.
    pub(crate) calibration: &'a Calibration,
    /// Its overlays, drawn in place of a part's dot.
    pub(crate) layers: &'a crate::doll_art::Layers,
}

/// Draw the doll for `injuries` in what is left of `ui`: over a picture,
/// where `over`'s calibration puts each part, or over the body drawn in
/// code. The rect it was drawn in, the picture's or the body's, and its
/// response as `sense` asks: the widget only points; the calibrator clicks
/// (`play/calibrator.rs`), and draws with the calibration it has not saved.
pub(crate) fn drawn(
    ui: &mut egui::Ui,
    injuries: Option<&BTreeMap<String, Injury>>,
    over: Option<Over<'_>>,
    sense: egui::Sense,
) -> (Rect, egui::Response) {
    // Before the game has said, the body is drawn whole. (An empty map
    // allocates nothing.)
    let whole = BTreeMap::new();
    let injuries = injuries.unwrap_or(&whole);
    let aspect = over.map_or(ASPECT, |over| {
        let [width, height] = over.texture.size();
        #[expect(
            clippy::cast_precision_loss,
            reason = "a picture's size in pixels, far inside f32's exact range"
        )]
        let aspect = width as f32 / height.max(1) as f32;
        aspect
    });
    let avail = ui.available_size();
    let mut height = avail.y.max(60.0);
    let mut width = height * aspect;
    if width > avail.x.max(40.0) {
        width = avail.x.max(40.0);
        height = width / aspect;
    }
    let (outer, response) = ui.allocate_exact_size(Vec2::new(avail.x.max(width), height), sense);
    let rect = Rect::from_center_size(outer.center(), Vec2::new(width, height));
    let painter = ui.painter().with_clip_rect(outer);
    let at = |(x, y): (f32, f32)| rect.min + Vec2::new(x * rect.width(), y * rect.height());
    let scale = rect.height();

    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    let dots = if let Some(Over {
        texture,
        calibration,
        ..
    }) = over
    {
        painter.image(texture.id(), rect, uv, Color32::WHITE);
        Dots {
            calibration: Some(calibration),
            radius: (calibration.diameter * scale * 0.5).max(4.0),
            opacity: calibration.opacity,
        }
    } else {
        body(&painter, &at, scale);
        Dots {
            calibration: None,
            radius: (scale * 0.035).max(5.0),
            opacity: 1.0,
        }
    };

    for part in &PARTS {
        let center = at(dots.anchor(part));
        let shows = shown(injuries, part);
        // The picture's own art for what the part shows, over all of the
        // picture (`doll_art.rs`); a dot where it has none.
        let level = shows.map_or(0, Shown::level);
        let art = over
            .and_then(|over| over.layers.get(&(part.id, level)))
            .and_then(|path| crate::pictures::picture(ui.ctx(), path));
        if let Some(art) = art {
            painter.image(art.id(), rect, uv, Color32::WHITE);
        } else if let Some(shows) = shows {
            dot(&painter, center, &dots, shows);
        }
        // Pointed at: a part's own shape on the body, its dot's
        // neighbourhood on a picture.
        let hover = match (&dots.calibration, part.shape) {
            (None, shape) => shape_rect(shape, &at, scale),
            (Some(_), _) => {
                Rect::from_center_size(center, Vec2::splat(dots.radius.max(scale * 0.04) * 2.0))
            }
        };
        ui.interact(
            hover,
            ui.id().with(("injury", part.id)),
            egui::Sense::hover(),
        )
        .on_hover_ui(|ui| {
            ui.label(format!("{}: {}", part.name, said(shows)));
        });
    }
    (rect, response)
}

/// The body drawn in code, every part in its colour.
fn body(painter: &egui::Painter, at: &impl Fn((f32, f32)) -> Pos2, scale: f32) {
    let letters = egui::FontId::proportional((scale * 0.09).clamp(10.0, 18.0));
    for part in &PARTS {
        match part.shape {
            Shape::Circle { c, r } => {
                painter.circle_filled(at(c), r * scale, BODY);
            }
            Shape::Block { min, max } => {
                painter.rect_filled(Rect::from_min_max(at(min), at(max)), scale * 0.02, BODY);
            }
            Shape::Line { a, b, w } => {
                painter.line_segment([at(a), at(b)], Stroke::new(w * scale, BODY));
            }
            Shape::Letter { c, letter } => {
                painter.text(
                    at(c),
                    egui::Align2::CENTER_CENTER,
                    letter,
                    letters.clone(),
                    BODY,
                );
            }
        }
    }
}

/// Where a part's shape on the body is, for pointing at it.
fn shape_rect(shape: Shape, at: &impl Fn((f32, f32)) -> Pos2, scale: f32) -> Rect {
    match shape {
        Shape::Circle { c, r } => Rect::from_center_size(at(c), Vec2::splat(r * scale * 2.0)),
        Shape::Block { min, max } => Rect::from_min_max(at(min), at(max)),
        Shape::Line { a, b, w } => Rect::from_two_pos(at(a), at(b)).expand(w * scale * 0.5),
        Shape::Letter { c, .. } => Rect::from_center_size(at(c), Vec2::splat(scale * 0.11)),
    }
}

/// A part's dot: its level's colour, its rank in it.
fn dot(painter: &egui::Painter, center: Pos2, dots: &Dots<'_>, shows: Shown) {
    let fill = PALETTE[shows.level()].gamma_multiply(dots.opacity);
    let edge = Color32::BLACK.gamma_multiply(dots.opacity);
    painter.circle(center, dots.radius, fill, Stroke::new(1.0, edge));
    // The rank, when there is room to read it (`VellumFE`'s threshold).
    if dots.radius >= 5.5 {
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            rank_text(shows.rank()),
            egui::FontId::proportional(dots.radius * 1.3),
            Color32::WHITE.gamma_multiply(dots.opacity),
        );
    }
}

/// A rank as its numeral, with no allocation.
const fn rank_text(rank: u8) -> &'static str {
    match rank {
        1 => "1",
        2 => "2",
        _ => "3",
    }
}

/// What a tooltip says of a part.
const fn said(shows: Option<Shown>) -> &'static str {
    match shows {
        None => "whole",
        Some(Shown::Wound(1)) => "minor wound",
        Some(Shown::Wound(2)) => "wound",
        Some(Shown::Wound(_)) => "severe wound",
        Some(Shown::Scar(1)) => "minor scar",
        Some(Shown::Scar(2)) => "scar",
        Some(Shown::Scar(_)) => "severe scar",
    }
}

#[cfg(test)]
mod tests {
    use super::{PARTS, Shown, shown};
    use cena_session::Injury;
    use std::collections::BTreeMap;

    fn injuries(parts: &[(&str, u8, u8)]) -> BTreeMap<String, Injury> {
        parts
            .iter()
            .map(|(id, wound, scar)| {
                (
                    (*id).to_owned(),
                    Injury {
                        wound: *wound,
                        scar: *scar,
                    },
                )
            })
            .collect()
    }

    fn part(id: &str) -> &'static super::Part {
        PARTS.iter().find(|p| p.id == id).unwrap_or(&PARTS[0])
    }

    #[test]
    fn a_wound_shows_over_a_scar_and_a_foot_folds_into_its_leg() {
        let hurt = injuries(&[
            ("head", 1, 3),
            ("chest", 0, 2),
            ("leftFoot", 2, 0),
            ("leftLeg", 1, 1),
        ]);
        assert_eq!(shown(&hurt, part("head")), Some(Shown::Wound(1)));
        assert_eq!(shown(&hurt, part("chest")), Some(Shown::Scar(2)));
        assert_eq!(
            shown(&hurt, part("leftLeg")),
            Some(Shown::Wound(2)),
            "the worse, the foot's"
        );
        assert_eq!(shown(&hurt, part("rightLeg")), None);
    }

    fn drawn(hurt: BTreeMap<String, Injury>) -> egui_kittest::Harness<'static, ()> {
        egui_kittest::Harness::builder()
            .with_size((180.0, 240.0))
            .build_ui(move |ui| super::doll(ui, Some(&hurt), None))
    }

    /// The body alone, before anything hurts.
    #[test]
    fn a_whole_body() {
        let mut harness = drawn(BTreeMap::new());
        harness.run();
        harness.snapshot("injuries_whole");
    }

    /// Each rank of wound and scar once, the back and the nerves in their
    /// corners, and a foot's wound on its leg.
    #[test]
    fn a_dot_for_each_hurt_part_coloured_and_numbered() {
        let mut harness = drawn(injuries(&[
            ("head", 1, 0),
            ("chest", 2, 1),
            ("rightArm", 3, 0),
            ("leftHand", 0, 1),
            ("abdomen", 0, 2),
            ("leftEye", 0, 3),
            ("back", 2, 0),
            ("nsys", 0, 2),
            ("rightFoot", 1, 0),
        ]));
        harness.run();
        harness.snapshot("injuries_hurt");
    }

    /// A picture of the test's own, calibrated beside it as `VellumFE`
    /// keeps its working copy: its dots where the calibration says, sized
    /// and faded as it says.
    #[test]
    fn a_dot_for_each_hurt_part_on_a_picture_where_it_is_calibrated() {
        let dir = std::env::temp_dir().join(format!("cena-doll-picture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let path = dir.join("tall.png");
        let mut art = image::RgbaImage::from_pixel(60, 120, image::Rgba([40, 60, 90, 255]));
        for y in 10..110 {
            for x in 25..35 {
                art.put_pixel(x, y, image::Rgba([200, 190, 170, 255]));
            }
        }
        art.save(&path).expect("saved");
        std::fs::write(
            dir.join("tall.toml"),
            "kind = \"doll\"
[anchors]
head = [0.5, 0.1]
chest = [0.5, 0.35]
             leftleg = [0.5, 0.8]
[dots]
opacity = 0.8
diameter = 0.1
",
        )
        .expect("calibrated");
        let look = super::DollLook {
            picture: Some(path.to_string_lossy().into_owned()),
            style: super::Style::Doll,
            skin: None,
        };
        let hurt = injuries(&[("head", 2, 0), ("chest", 0, 3), ("leftFoot", 1, 0)]);
        let mut harness = egui_kittest::Harness::builder()
            .with_size((180.0, 240.0))
            .build_ui(move |ui| super::doll(ui, Some(&hurt), Some(&look)));
        harness.run();
        harness.snapshot("injuries_picture");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Doll plus: a picture with the chest's art for a middling wound
    /// and for a scar. The wound's art is drawn in place of the chest's
    /// dot; the head, with no art, keeps its dot; the scar's art, not
    /// shown, is never read.
    #[test]
    fn a_parts_art_is_drawn_in_place_of_its_dot_and_art_not_shown_never_read() {
        let dir = std::env::temp_dir().join(format!("cena-doll-plus-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let base = dir.join("plus.png");
        image::RgbaImage::from_pixel(60, 120, image::Rgba([40, 60, 90, 255]))
            .save(&base)
            .expect("saved");
        let mut art = image::RgbaImage::new(60, 120);
        for y in 30..55 {
            for x in 18..42 {
                art.put_pixel(x, y, image::Rgba([40, 200, 60, 255]));
            }
        }
        art.save(dir.join("plus_chest_injury2.png")).expect("saved");
        let scar = dir.join("plus_chest_scar1.png");
        art.save(&scar).expect("saved");
        let look = super::DollLook {
            picture: Some(base.to_string_lossy().into_owned()),
            style: super::Style::Doll,
            skin: None,
        };
        let hurt = injuries(&[("chest", 2, 1), ("head", 1, 0)]);
        let mut harness = egui_kittest::Harness::builder()
            .with_size((180.0, 240.0))
            .build_ui(move |ui| super::doll(ui, Some(&hurt), Some(&look)));
        harness.run();
        harness.snapshot("injuries_plus");
        assert!(!crate::pictures::was_read(
            &harness.ctx,
            &scar.to_string_lossy()
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_doll_has_fourteen_parts_every_one_the_models() {
        let ids: Vec<&str> = PARTS.iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), 14);
        for id in ids {
            assert!(cena_session::body::ALL_PARTS.contains(&id), "{id}");
        }
    }
}
