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
    const fn level(self) -> usize {
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
const PALETTE: [Color32; 7] = [
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

/// The doll's width to its height.
const ASPECT: f32 = 0.75;

/// Draw the doll for `injuries` in what is left of `ui`.
pub(super) fn doll(ui: &mut egui::Ui, injuries: Option<&BTreeMap<String, Injury>>) {
    // Before the game has said, the body is drawn whole. (An empty map
    // allocates nothing.)
    let whole = BTreeMap::new();
    let injuries = injuries.unwrap_or(&whole);
    let avail = ui.available_size();
    let mut height = avail.y.max(60.0);
    let mut width = height * ASPECT;
    if width > avail.x.max(40.0) {
        width = avail.x.max(40.0);
        height = width / ASPECT;
    }
    let (outer, _) =
        ui.allocate_exact_size(Vec2::new(avail.x.max(width), height), egui::Sense::hover());
    let rect = Rect::from_center_size(outer.center(), Vec2::new(width, height));
    let painter = ui.painter().with_clip_rect(outer);
    let at = |(x, y): (f32, f32)| rect.min + Vec2::new(x * rect.width(), y * rect.height());
    let scale = rect.height();
    let letters = egui::FontId::proportional((scale * 0.09).clamp(10.0, 18.0));

    for part in &PARTS {
        let hover = match part.shape {
            Shape::Circle { c, r } => {
                painter.circle_filled(at(c), r * scale, BODY);
                Rect::from_center_size(at(c), Vec2::splat(r * scale * 2.0))
            }
            Shape::Block { min, max } => {
                let block = Rect::from_min_max(at(min), at(max));
                painter.rect_filled(block, scale * 0.02, BODY);
                block
            }
            Shape::Line { a, b, w } => {
                painter.line_segment([at(a), at(b)], Stroke::new(w * scale, BODY));
                Rect::from_two_pos(at(a), at(b)).expand(w * scale * 0.5)
            }
            Shape::Letter { c, letter } => {
                painter.text(
                    at(c),
                    egui::Align2::CENTER_CENTER,
                    letter,
                    letters.clone(),
                    BODY,
                );
                Rect::from_center_size(at(c), Vec2::splat(scale * 0.11))
            }
        };
        let shows = shown(injuries, part);
        if let Some(shows) = shows {
            dot(&painter, at(part.anchor), scale, shows);
        }
        ui.interact(
            hover,
            ui.id().with(("injury", part.id)),
            egui::Sense::hover(),
        )
        .on_hover_ui(|ui| {
            ui.label(format!("{}: {}", part.name, said(shows)));
        });
    }
}

/// A part's dot: its level's colour, its rank in it.
fn dot(painter: &egui::Painter, center: Pos2, scale: f32, shows: Shown) {
    let fill = PALETTE[shows.level()];
    let radius = (scale * 0.035).max(5.0);
    painter.circle(center, radius, fill, Stroke::new(1.0, Color32::BLACK));
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        rank_text(shows.rank()),
        egui::FontId::proportional(radius * 1.3),
        Color32::WHITE,
    );
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
            .build_ui(move |ui| super::doll(ui, Some(&hurt)))
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

    #[test]
    fn the_doll_has_fourteen_parts_every_one_the_models() {
        let ids: Vec<&str> = PARTS.iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), 14);
        for id in ids {
            assert!(cena_session::body::ALL_PARTS.contains(&id), "{id}");
        }
    }
}
