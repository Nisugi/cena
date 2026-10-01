//! A box phased with 704 once it is in a bag (`box_phase`,
//! `eloot.lic:2975-2984`), when the profile phases boxes (`loot_phase`), the
//! spell is known and its cost can be paid: `prepare 704`, `cast at #box`,
//! cast again while armour hinders it, as Lich's casting step does. Not a box
//! of enruned or mithril, and not one put on a disk: eloot phases only what
//! its bags take (`single_drag`, `:4002`, after `single_drag_box` returned).
//! A phased box is unphased at the locksmith pool (`town/pool.rs`).

use std::collections::VecDeque;

use cena_session::GameState;

use super::{Planner, Step};
use crate::cast::{self, Casting};

/// Phase.
const PHASE: u16 = 704;
/// How many times a hindered cast is made again.
const PHASE_TRIES: u8 = 5;
/// Boxes never phased (`box_phase`, `:2977`).
const NOT_PHASED: [&str; 2] = ["enruned", "mithril"];

/// One box being phased.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Phasing {
    id: String,
    /// What is left to send of this cast.
    lines: VecDeque<String>,
    casts: u8,
    hindered: bool,
}

impl Planner {
    /// The box `id`, `name`, just dragged into `bag`, to be phased when the
    /// drag is confirmed: nothing when the profile does not phase boxes, the
    /// bag is a disk, or the box is one never phased.
    pub(super) fn may_phase(&mut self, id: &str, name: &str, disk: bool) {
        let named = |word: &str| name.to_ascii_lowercase().contains(word);
        self.into_box = (self.profile.phase_boxes && !disk && !NOT_PHASED.iter().any(|w| named(w)))
            .then(|| id.to_owned());
    }

    /// The drag of `item` was confirmed: phase it, if it was a box to phase.
    pub(super) fn stored(&mut self, item: &str) {
        if self.into_box.as_deref() == Some(item) {
            self.phasing = Some(Phasing {
                id: item.to_owned(),
                lines: VecDeque::new(),
                casts: 0,
                hindered: false,
            });
        }
        self.into_box = None;
    }

    /// The game said armour hindered the cast.
    pub(super) fn hindered(&mut self) {
        if let Some(phasing) = self.phasing.as_mut() {
            phasing.hindered = true;
        }
    }

    /// The phasing's next line; `None` when there is none to cast.
    pub(super) fn phase_step(&mut self, state: &GameState) -> Option<Step> {
        let phasing = self.phasing.as_mut()?;
        if let Some(line) = phasing.lines.pop_front() {
            return Some(Step::Cast(line));
        }
        let again = phasing.casts == 0 || (phasing.hindered && phasing.casts < PHASE_TRIES);
        let can = state.known_spells.knows(u32::from(PHASE)) == Some(true)
            && cast::ready(state, PHASE, 1, 0).is_ok();
        if !again || !can {
            self.phasing = None;
            return None;
        }
        phasing.casts += 1;
        phasing.hindered = false;
        phasing.lines = Casting {
            spell: PHASE,
            target: Some(format!("at #{}", phasing.id)),
            ..Casting::default()
        }
        .lines(state)
        .into();
        phasing.lines.pop_front().map(Step::Cast)
    }
}
