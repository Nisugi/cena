//! What to treat next: eherbs' `next_herb_type` (`eherbs.lic:3200-3310`).
//!
//! The order is eherbs', and it is the whole of the healing's judgment:
//! blood first when under half health, then poison and disease, then the
//! wounds by area -- the major ones before any minor one -- a severed limb,
//! a missing eye, the major scars, the minor scars unless the profile skips
//! them, and blood again when seven or more short. The areas are Lich's five
//! (`head`, `neck`, `torso`, `limbs`, `nerves`), each folded into a herb's
//! four (`neck` is `head`, `torso` is `organ`).
//!
//! `--spellcast` and `--ranged` treat only what stops a cast or a shot: the
//! arms and hands, and for a caster the head, the eyes and the nerves; scars
//! only when major. eherbs' severed-limb test in that mode reads the right
//! hand twice and never the left arm (`:3276`); ported as written.

use std::collections::BTreeSet;

use cena_session::GameState;
use cena_session::body::{Body, Track};
use cena_session::herbs::{Area, HerbKind, Hurt, Severity};

/// What eherbs can be told to leave alone for the rest of a run: an area,
/// or blood, poison, disease. A kind with no herb joins it (`track_missing`).
pub type Skipped = BTreeSet<&'static str>;

/// How the next kind is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "eherbs' switches for one run, each read in one place"
)]
pub struct Mode {
    /// Leave minor scars (`skip_scars`).
    pub skip_scars: bool,
    /// Only what stops a cast (`--spellcast`).
    pub spellcast: bool,
    /// Only what stops a shot (`--ranged`).
    pub ranged: bool,
    /// Only blood (`blood`, or `blood_toggle`).
    pub blood_only: bool,
}

/// The five areas in eherbs' order, with the herb area each folds into.
const AREAS: [(&str, Area); 5] = [
    ("head", Area::Head),
    ("neck", Area::Head),
    ("torso", Area::Organ),
    ("limbs", Area::Limb),
    ("nerves", Area::Nerve),
];

/// The next kind to treat, or `None` when nothing is left that this mode
/// treats.
#[must_use]
pub fn next_kind(state: &GameState, mode: Mode, skipped: &Skipped) -> Option<HerbKind> {
    let body = Body::new(&state.character.injuries);
    let percent = state.health().map(|h| h.percent);
    let short = state
        .health()
        .and_then(|h| h.current.zip(h.max))
        .is_some_and(|(now, max)| now + 7 < max);
    if mode.blood_only {
        return (short && !skipped.contains("blood")).then_some(HerbKind::Blood);
    }
    if percent.is_some_and(|p| p < 50) && !skipped.contains("blood") {
        return Some(HerbKind::Blood);
    }
    let status = state.status.known();
    if status.poisoned() == Some(true) && !skipped.contains("poison") {
        return Some(HerbKind::Poison);
    }
    if status.diseased() == Some(true) && !skipped.contains("disease") {
        return Some(HerbKind::Disease);
    }
    let kind = if mode.spellcast || mode.ranged {
        narrow(body, mode, skipped)
    } else {
        whole(body, mode, skipped)
    };
    if kind.is_some() {
        return kind;
    }
    (short && !skipped.contains("blood")).then_some(HerbKind::Blood)
}

/// Lich's composite for one of the five areas.
fn area_rank(body: Body<'_>, area: &str, track: Track) -> u8 {
    match area {
        "head" => body.rank("head", track),
        "neck" => body.rank("neck", track),
        "torso" => body.torso(track),
        "limbs" => body.limbs(track),
        _ => body.rank("nsys", track),
    }
}

const fn injury(severity: Severity, area: Area, hurt: Hurt) -> HerbKind {
    HerbKind::Injury {
        severity,
        area,
        hurt,
    }
}

/// The first area, in order, whose rank passes `test`.
fn first_area(
    body: Body<'_>,
    skipped: &Skipped,
    track: Track,
    test: impl Fn(u8) -> bool,
) -> Option<Area> {
    AREAS
        .iter()
        .filter(|(name, _)| !skipped.contains(name))
        .find(|(name, _)| test(area_rank(body, name, track)))
        .map(|(_, area)| *area)
}

/// eherbs' ordinary order (`:3205-3240`).
fn whole(body: Body<'_>, mode: Mode, skipped: &Skipped) -> Option<HerbKind> {
    if let Some(area) = first_area(body, skipped, Track::Wound, |r| r > 1) {
        return Some(injury(Severity::Major, area, Hurt::Wound));
    }
    if let Some(area) = first_area(body, skipped, Track::Wound, |r| r == 1) {
        return Some(injury(Severity::Minor, area, Hurt::Wound));
    }
    if body.limbs(Track::Scar) == 3 && !skipped.contains("limbs") {
        return Some(HerbKind::SeveredLimb);
    }
    let eye = body.rank("rightEye", Track::Scar) == 3 || body.rank("leftEye", Track::Scar) == 3;
    if eye && !skipped.contains("torso") {
        return Some(HerbKind::MissingEye);
    }
    if let Some(area) = first_area(body, skipped, Track::Scar, |r| r > 1) {
        return Some(injury(Severity::Major, area, Hurt::Scar));
    }
    if !mode.skip_scars
        && let Some(area) = first_area(body, skipped, Track::Scar, |r| r == 1)
    {
        return Some(injury(Severity::Minor, area, Hurt::Scar));
    }
    None
}

/// The parts that stop a cast or a shot, by eherbs' four areas in its
/// narrow order: `limbs`, `head`, `nerves`, `torso` (`:3242-3300`). `None`
/// for an area this mode does not look at.
fn narrow_parts(area: &str, mode: Mode) -> Option<&'static [&'static str]> {
    match area {
        "limbs" => Some(&["leftArm", "leftHand", "rightArm", "rightHand"]),
        "head" if mode.spellcast => Some(&["head"]),
        "torso" if mode.spellcast => Some(&["leftEye", "rightEye"]),
        "nerves" if mode.spellcast => Some(&["nsys"]),
        _ => None,
    }
}

/// eherbs' `--spellcast` / `--ranged` order.
fn narrow(body: Body<'_>, mode: Mode, skipped: &Skipped) -> Option<HerbKind> {
    const ORDER: [(&str, Area); 4] = [
        ("limbs", Area::Limb),
        ("head", Area::Head),
        ("nerves", Area::Nerve),
        ("torso", Area::Organ),
    ];
    let areas = || {
        ORDER
            .iter()
            .filter(|(name, _)| !skipped.contains(name))
            .filter_map(|(name, area)| narrow_parts(name, mode).map(|parts| (*area, parts)))
    };
    for (area, parts) in areas() {
        match body.worst(parts, Track::Wound) {
            0 => {}
            1 => return Some(injury(Severity::Minor, area, Hurt::Wound)),
            _ => return Some(injury(Severity::Major, area, Hurt::Wound)),
        }
    }
    // As written: the right hand twice, the left arm never.
    let severed = body.worst(
        &["rightHand", "rightArm", "leftHand", "rightHand"],
        Track::Scar,
    );
    if severed == 3 && !skipped.contains("limbs") {
        return Some(HerbKind::SeveredLimb);
    }
    if mode.spellcast
        && body.worst(&["rightEye", "leftEye"], Track::Scar) == 3
        && !skipped.contains("torso")
    {
        return Some(HerbKind::MissingEye);
    }
    for (area, parts) in areas() {
        if body.worst(parts, Track::Scar) > 1 {
            return Some(injury(Severity::Major, area, Hurt::Scar));
        }
    }
    None
}

/// The word eherbs skips when a kind has no herb (`track_missing`,
/// `:1897-1913`): the area for an injury, else the kind itself.
#[must_use]
pub fn skip_word(kind: HerbKind) -> &'static [&'static str] {
    match kind {
        HerbKind::Injury { area, .. } => match area {
            Area::Head => &["head", "neck"],
            Area::Organ => &["torso"],
            Area::Limb => &["limbs"],
            Area::Nerve => &["nerves"],
        },
        HerbKind::MissingEye => &["torso"],
        HerbKind::SeveredLimb => &["limbs"],
        HerbKind::Blood => &["blood"],
        HerbKind::Poison => &["poison"],
        HerbKind::Disease => &["disease"],
        HerbKind::Lifekeep => &["lifekeep"],
        HerbKind::RaiseDead => &["raisedead"],
    }
}
