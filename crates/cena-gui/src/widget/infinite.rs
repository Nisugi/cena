//! The injury doll's **Infinite** style (`plan/55` §2f): `gs_studio`'s puppet,
//! posed and moving as the character is.
//!
//! `gs_studio` takes the game's words, not Hydra's types
//! (`gs_studio/crates/gs_field/src/doll_state.rs`, `DollState::from_game`):
//! each part's wound and scar by the injury window's id, and the names of
//! what the character is under. So this is an adapter and nothing more. The
//! feet it folds into the legs itself, and every character is `humanoid` for
//! now (the author: *"we will probably make puppets for all races, but just
//! humanoid for now"*). It wears the skin the player chose of those
//! `gs_studio` has for the form ([`skins`]), the lay figure with none chosen.
//!
//! Built only with the `doll-infinite` feature, on by default (*"optional at
//! build time, on by default"*). Each doll is its own view of `gs_studio`'s
//! GPU pass, named for its widget, so two characters' dolls in one frame do
//! not draw over each other. Where the puppet cannot be drawn the widget
//! falls back to the Doll.

use std::sync::Arc;

use cena_session::GameState;
use gs_field::doll_sky::doll_sky;
use gs_field::doll_state::DollState;
use gs_field::lighting::Sky;
use gs_field_egui::creature_puppet::PuppetCache;
use gs_field_egui::{doll_view, field_gpu};

/// The puppet every character is drawn as, for now.
const FORM: &str = "humanoid";

/// The skin a doll wears when the player chose none: muted, and bare of
/// anything a wound could be mistaken for.
const SKIN: &str = "lay_figure";

/// The skin setting that means the form's own texture, no skin over it.
pub(crate) const BARE: &str = "-";

/// The skins `gs_studio` has for the form, by name.
pub(crate) fn skins() -> Vec<String> {
    let set = gs_field::puppets::puppets();
    let mut skins: Vec<String> = set
        .skins()
        .filter(|(form, _)| *form == FORM)
        .map(|(_, skin)| skin.to_owned())
        .collect();
    skins.sort();
    skins
}

/// The puppet `gs_studio` draws for `chosen`: the form in that skin, in the
/// lay figure with none chosen, and the form's own texture where the player
/// asked for it or `gs_studio` has not the skin.
fn form(chosen: Option<&str>) -> String {
    let skin = chosen.unwrap_or(SKIN);
    let set = gs_field::puppets::puppets();
    if skin == BARE || set.skin(FORM, skin).is_none() {
        FORM.to_owned()
    } else {
        format!("{FORM}#{skin}")
    }
}

/// What the character is, as `gs_studio` reads it.
fn doll_state(state: &GameState) -> DollState {
    let injuries = &state.character.injuries;
    let wounds = injuries
        .iter()
        .filter(|(_, injury)| injury.wound > 0)
        .map(|(part, injury)| (part.as_str(), injury.wound));
    let scars = injuries
        .iter()
        .filter(|(_, injury)| injury.scar > 0)
        .map(|(part, injury)| (part.as_str(), injury.scar));
    let now = state.game_time_now();
    let statuses = state
        .status
        .iter()
        .filter(|(_, on)| *on)
        .map(|(name, _)| name)
        .chain(
            state
                .effects
                .in_category("Debuffs")
                .filter(|(id, _)| {
                    now.is_none_or(|now| state.effects.active_in("Debuffs", id, now) != Some(false))
                })
                .map(|(_, effect)| effect.text.as_str()),
        );
    DollState::from_game(wounds, scars, statuses)
}

/// Draw the puppet for `state` in what is left of `ui`, as the doll `id`
/// in the skin `chosen`; `false` when it could not be, and nothing was.
pub(super) fn infinite(
    ui: &mut egui::Ui,
    state: Option<&GameState>,
    id: egui::Id,
    chosen: Option<&str>,
) -> bool {
    let Some(state) = state else {
        return false;
    };
    let rect = ui.available_rect_before_wrap();
    let context = ui.ctx().clone();
    let form: Arc<str> = context.data_mut(|data| {
        data.get_temp_mut_or_insert_with(
            egui::Id::new(("infinite-form", chosen)),
            || -> Arc<str> { Arc::from(form(chosen)) },
        )
        .clone()
    });
    let doll = doll_state(state);
    let sky = doll_sky(None, &Sky::default(), true);
    let shared = PuppetCache::shared(&context);
    let mut cache = shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // The widget's own name, for its breathing and its turns, and its own
    // view of the pass.
    let name = format!("doll:{}", id.value());
    let drawn = doll_view::paint_state(
        ui,
        rect,
        &mut cache,
        &form,
        &doll,
        &sky,
        Some(&name),
        field_gpu::available(&context),
        id.value(),
    );
    if drawn {
        ui.allocate_rect(rect, egui::Sense::hover());
    }
    drawn
}

/// `gs_studio`'s GPU pass, set up once as the window opens: without it the
/// puppet is drawn unlit on grey. And `gs_studio`'s own files kept in
/// Hydra's data folder, not `VellumFE`'s.
pub(crate) fn open(creation: &eframe::CreationContext<'_>, data: &std::path::Path) {
    gs_field::data_dirs::set_base_dir(data.to_path_buf());
    if let Some(render) = &creation.wgpu_render_state {
        field_gpu::init(&creation.egui_ctx, render);
    }
}

#[cfg(test)]
mod tests {
    use super::{doll_state, form, skins};
    use cena_session::{Frame, GameState};

    /// The doll wears what the player chose of `gs_studio`'s skins, the
    /// form's own texture when asked or when the skin is not to be had.
    #[test]
    fn the_doll_wears_the_chosen_skin() {
        let skins = skins();
        let one = skins.first().expect("gs_studio has humanoid skins");
        assert_eq!(form(Some(one)), format!("humanoid#{one}"));
        assert_eq!(form(Some(super::BARE)), "humanoid");
        assert_eq!(form(Some("no such skin")), "humanoid");
    }

    /// The character's wounds, scars and what it is under, as `gs_studio`
    /// reads them: its foot on its leg, its stun and its debuff.
    #[test]
    fn the_character_is_told_to_gs_studio_in_its_words() {
        let mut state = GameState::default();
        let image = |part: &str, name: &str| Frame::InjuryImage {
            id: part.to_owned(),
            name: name.to_owned(),
            dialog: Some("injuries".to_owned()),
            attrs: Vec::new(),
        };
        state.apply(&image("leftFoot", "Injury2"));
        state.apply(&image("chest", "Scar1"));
        state.status.set("stunned", true);
        state.status.set("prone", false);
        let told = doll_state(&state);
        assert!(
            told.wounds
                .iter()
                .any(|(part, rank)| part == "leftLeg" && *rank == 2),
            "{told:?}"
        );
        assert!(
            told.scars
                .iter()
                .any(|(part, rank)| part == "chest" && *rank == 1),
            "{told:?}"
        );
        let status = |s| told.statuses.has(s);
        assert!(status(gs_field::doll_state::Status::Stunned), "{told:?}");
        assert!(
            !status(gs_field::doll_state::Status::Prone),
            "not on: {told:?}"
        );
    }
}
