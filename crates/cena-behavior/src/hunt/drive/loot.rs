//! The hunt driver's looting: the loot planner run through the gate
//! (`plan/31` Stage 2), boxes emptied with it.

use cena_session::{CommandId, Notice, NoticeKind};

use super::{BEAT, Driver, HuntEnd, LOOT_STEPS};
use crate::loot::{Left, LootProfile, Outcome as LootOutcome, Planner, Step, classify};
use crate::town::Town;
use crate::travel::TravelNotes;

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    /// Loot with the planner (`plan/31` Stage 2): each step sent through the
    /// gate, each reply read for what eloot would act on, until the planner
    /// says it is done. What it learned is kept for the next room, and a
    /// reason to rest is handed to the machine.
    pub(super) async fn loot(&mut self, corpses: &[i64]) -> Result<(), HuntEnd> {
        let Some(profile) = self.machine.loot_profile().cloned() else {
            return Ok(());
        };
        let memory = std::mem::take(&mut self.memory);
        let planner = Planner::new(profile, memory, corpses);
        self.run_loot(planner, true).await?;
        let fresh: Vec<String> = self
            .memory
            .unskinnable
            .difference(&self.saved_unskinnable)
            .cloned()
            .collect();
        if !fresh.is_empty() {
            self.saved_unskinnable.extend(fresh.iter().cloned());
            (self.learned)(&fresh);
        }
        Ok(())
    }

    /// Empty the box in hand with the loot planner (`box_loot`), for the
    /// selling round. `true` when the box would not open.
    pub(super) async fn empty_box(
        &mut self,
        profile: &LootProfile,
        id: &str,
    ) -> Result<bool, HuntEnd> {
        let town = Town::for_profile(profile);
        let memory = std::mem::take(&mut self.memory);
        let charm = (!town.charm.is_empty()).then(|| town.charm.clone());
        let planner = Planner::for_box(profile.clone(), memory, id, charm);
        let planner = self.run_loot(planner, false).await?;
        Ok(planner.box_locked())
    }

    /// Run a loot planner to its end: each step sent, each reply fed back.
    /// `hunting` says whether the hunt's machine hears how it ended.
    pub(super) async fn run_loot(
        &mut self,
        mut planner: Planner,
        hunting: bool,
    ) -> Result<Planner, HuntEnd> {
        for _ in 0..LOOT_STEPS {
            let step = planner.next(&self.state);
            let (line, touched) = match &step {
                Step::Done(left) => {
                    if !hunting {
                        break;
                    }
                    self.machine.loot_ended(*left);
                    if *left != Left::Nothing {
                        self.handle.say(Notice::line(
                            NoticeKind::Info,
                            format!(
                                "Hunt: looting stopped: {}.",
                                self.machine.rest_reason_text()
                            ),
                        ));
                    }
                    break;
                }
                Step::Stance(line) | Step::Cast(line) => (line.clone(), None),
                Step::Ask(what) => ((*what).to_owned(), None),
                Step::Search(id) => (format!("loot #{id}"), None),
                Step::LootRoom => ("loot room".to_owned(), None),
                Step::LootItem(id) => (format!("loot #{id}"), self.floor_name(id)),
                Step::Open(bag) => (format!("open #{bag}"), self.floor_name(bag)),
                Step::LookIn(bag) => (format!("look in #{bag}"), None),
                Step::Drag { item, bag } => {
                    (format!("_drag #{item} #{bag}"), self.floor_name(item))
                }
                Step::Wield(id) => (format!("get #{id}"), None),
                Step::Kneel => ("kneel".to_owned(), None),
                Step::Stand => ("stand".to_owned(), None),
                Step::Skin { corpse, hand } => (format!("skin #{corpse} {hand}"), None),
                Step::StowGem(id) => (format!("stow gem #{id}"), None),
                Step::Coins(id) => (format!("get coins from #{id}"), None),
                Step::Describe(what) => (format!("describe {what}"), None),
                Step::Charm { charm, box_ } => (format!("point {charm} at #{box_}"), None),
            };
            self.transcript.clear();
            self.send(&line, None).await?;
            let outcomes: Vec<LootOutcome> = self.transcript.lines().filter_map(classify).collect();
            for outcome in &outcomes {
                planner.outcome_in(outcome, &self.state);
                if let Some(name) = &touched {
                    planner.learn(outcome, name);
                }
            }
            if outcomes.is_empty() {
                // The floor is restated a moment after the verb lands.
                self.hold(BEAT).await?;
                // A stow the text did not confirm is confirmed by the bag's
                // contents, as eloot confirms it (`eloot.lic:4102-4108`).
                if let Step::Drag { item, bag } = &step
                    && self
                        .state
                        .inventory
                        .container(bag)
                        .is_some_and(|held| held.items.iter().any(|thing| thing.id == *item))
                {
                    planner.outcome(&LootOutcome::Stored);
                }
            }
        }
        self.memory = planner.memory().clone();
        Ok(planner)
    }
}
