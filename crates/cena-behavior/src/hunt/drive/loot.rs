//! The hunt driver's looting: the loot planner run through the gate
//! (`plan/31` Stage 2), boxes emptied with it.

use cena_session::{CommandId, Notice, NoticeKind};

use super::{BEAT, Driver, HuntEnd, LOOT_STEPS};
use cena_session::containers::StowSlot;

use crate::loot::{Errand, Left, LootProfile, Outcome as LootOutcome, Planner, Step, classify};
use crate::town::{self, Round, Town};
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
        self.keep_unskinnable();
        Ok(())
    }

    /// Hand what this run learned unskinnable to whoever writes the profile,
    /// once each.
    fn keep_unskinnable(&mut self) {
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
    }

    /// One part of looting or selling by itself (`plan/61` step 1), as
    /// eloot's commands run them, and how it went, said.
    pub(super) async fn loot_errand(&mut self, errand: Errand) -> Result<(), HuntEnd> {
        let Some(profile) = self.machine.loot_profile().cloned() else {
            return Ok(());
        };
        let corpses: Vec<i64> = self
            .state
            .creatures()
            .in_room()
            .filter(|creature| creature.corpse())
            .map(|creature| creature.id)
            .collect();
        let text = match errand {
            Errand::Room | Errand::Skin => {
                let memory = std::mem::take(&mut self.memory);
                let planner = if errand == Errand::Skin {
                    Planner::for_skinning(profile, memory, &corpses)
                } else {
                    Planner::new(profile, memory, &corpses)
                };
                let planner = self.run_loot(planner, false).await?;
                self.keep_unskinnable();
                match planner.ended() {
                    Some(Left::Nothing) => "done.",
                    Some(Left::BagsFull) => "stopped: every bag that would take it is full.",
                    Some(Left::BoxInHand) => "stopped: a box is in hand that no bag will take.",
                    None => "gave up after too many steps.",
                }
            }
            Errand::Box => self.box_errand(&profile).await?,
            Errand::Sell | Errand::Pool { .. } | Errand::Deposit => {
                let (round, done, nothing) = match errand {
                    Errand::Pool { drop, collect } => (
                        Round::Pool { drop, collect },
                        "the pool is done.",
                        "no boxes to take to the pool.",
                    ),
                    Errand::Deposit => (Round::Bank, "deposited.", "the bank was not reached."),
                    _ => (Round::All, "the selling round is done.", "nothing to sell."),
                };
                if self.sell_round(round).await? {
                    done
                } else {
                    nothing
                }
            }
        };
        self.handle
            .say(Notice::line(NoticeKind::Info, format!("Loot: {text}")));
        Ok(())
    }

    /// `loot box`: the open box in hand emptied, then kept when it is one
    /// the profile sells empty, else thrown out (`box_loot` and
    /// `save_trash_box`, `eloot.lic:5086`, `:7773`).
    async fn box_errand(&mut self, profile: &LootProfile) -> Result<&'static str, HuntEnd> {
        let Some(id) = town::box_in_hand(&self.state) else {
            return Ok("there is no box in hand.");
        };
        if self.empty_box(profile, &id).await? {
            return Ok("the box is locked.");
        }
        let held =
            |this: &Self| this.state.right_hand.holds(&id) || this.state.left_hand.holds(&id);
        if town::keeps_box(&Town::for_profile(profile), &self.state, &id) {
            let bag = self.state.containers.stow(StowSlot::Default);
            if let Some(bag) = bag.map(|bag| bag.id.clone()) {
                self.send(&format!("_drag #{id} #{bag}"), None).await?;
            }
            return Ok("the box is emptied and kept.");
        }
        // Thrown out, asked twice when the game wants to be sure, dropped
        // where there is nothing to throw it in.
        for line in [
            format!("trash #{id}"),
            format!("trash #{id}"),
            format!("drop #{id}"),
        ] {
            if !held(self) {
                break;
            }
            self.send(&line, None).await?;
            self.hold(BEAT).await?;
        }
        Ok("the box is emptied.")
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
