//! The hunt driver's selling round (`plan/31` Stage 4): the town planner
//! walked shop to shop.

use cena_session::{CommandId, Notice, NoticeKind};

use super::{BEAT, Driver, HuntEnd, SELL_STEPS};
use crate::town::{self, Round, Seller, Step as Errand, Town};
use crate::travel::{TravelNotes, destination, walker_from};

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    /// Sell with the town planner (`plan/31` Stage 4): each shop the nearest
    /// room tagged for it, walked with travel's driver; each step sent through
    /// the gate; each reply read as the ledger's facts from this driver's own
    /// fold of the stream, plus the few replies that are not facts. Ends
    /// back at the resting room, or wherever the round gave up.
    pub(super) async fn sell(&mut self) -> Result<(), HuntEnd> {
        self.sell_round(Round::All).await.map(|_| ())
    }

    /// [`Self::sell`], for all of the round or one stop of it; `false` when
    /// there was nothing for it to do.
    pub(super) async fn sell_round(&mut self, round: Round) -> Result<bool, HuntEnd> {
        let Some(profile) = self.machine.loot_profile().cloned() else {
            return Ok(false);
        };
        let Some(home) = self.locate() else {
            return Ok(false);
        };
        self.know_stow_list().await?;
        let town = Town::for_profile(&profile);
        let Some(mut seller) = Seller::for_round(town, &self.state, home, round) else {
            return Ok(false);
        };
        // Facts queued before the round are not the round's.
        let _ = self.state.take_loot();
        for _ in 0..SELL_STEPS {
            let here = self.locate();
            let step = {
                let now = self.state.game_time_now().unwrap_or(0);
                let walker = walker_from(&self.state, &self.notes, now);
                let (map, state) = (self.map, &self.state);
                let nearest = |tag: &str| {
                    here.and_then(|from| {
                        destination(map, &walker, from, tag, &std::collections::BTreeMap::new())
                    })
                };
                seller.next(state, &nearest)
            };
            let line = match &step {
                Errand::Done => break,
                Errand::Walk(to) => {
                    self.walk(*to).await?;
                    continue;
                }
                Errand::Fetch(id) => format!("get #{id}"),
                Errand::Sell(id) | Errand::SellSack(id) => format!("sell #{id}"),
                Errand::Appraise(id) => format!("appraise #{id}"),
                Errand::Analyze(id) => format!("analyze #{id}"),
                Errand::Wear(id) => format!("wear #{id}"),
                Errand::ReadNote(id) | Errand::ReadScroll(id) => format!("read #{id}"),
                Errand::Stow { item, bag } => format!("_drag #{item} #{bag}"),
                Errand::Deposit(id) => format!("deposit #{id}"),
                Errand::Give { item, to } => format!("give #{item} to #{to}"),
                Errand::Unbundle => "bundle remove".to_owned(),
                Errand::DepositAll => "deposit all".to_owned(),
                Errand::Withdraw(silver) => format!("withdraw {silver} silver"),
                Errand::Swap => "swap".to_owned(),
                Errand::Tip {
                    to,
                    amount,
                    percent,
                    confirm,
                } => format!(
                    "give #{to} {amount}{}{}",
                    if *percent { " PERCENT" } else { "" },
                    if *confirm { " confirm" } else { "" }
                ),
                Errand::AskReturn(to) => format!("ask #{to} for return"),
                Errand::Trash(id) => format!("trash #{id}"),
                Errand::Drop(id) => format!("drop #{id}"),
                Errand::EmptyBox(id) => {
                    let locked = self.empty_box(&profile, id).await?;
                    let replies = if locked {
                        vec![town::Reply::BoxLocked]
                    } else {
                        Vec::new()
                    };
                    let facts: Vec<cena_session::LootFact> = self
                        .state
                        .take_loot()
                        .into_iter()
                        .flat_map(|chunk| chunk.facts)
                        .collect();
                    seller.outcome(&facts, &replies, &self.state);
                    continue;
                }
            };
            self.transcript.clear();
            self.send(&line, None).await?;
            // The shopkeeper answers a beat after the verb lands.
            self.hold(BEAT).await?;
            let facts: Vec<cena_session::LootFact> = self
                .state
                .take_loot()
                .into_iter()
                .flat_map(|chunk| chunk.facts)
                .collect();
            let replies: Vec<town::Reply> =
                self.transcript.lines().filter_map(town::classify).collect();
            seller.outcome(&facts, &replies, &self.state);
        }
        // What it came to (`plan/61` step 2), said and kept for `loot last`.
        let came_to = seller.breakdown();
        if !came_to.is_empty() {
            self.reports.keep_round(came_to.clone());
            self.handle.say(Notice::table(NoticeKind::Info, came_to));
        }
        Ok(true)
    }

    /// Ask for the stow list when it has not been read: the round reads the
    /// bags by it.
    async fn know_stow_list(&mut self) -> Result<(), HuntEnd> {
        if !self.state.containers.stow_checked() {
            self.send("stow list", None).await?;
            self.hold(BEAT).await?;
        }
        Ok(())
    }

    /// The name of a thing on the floor, by id.
    pub(super) fn floor_name(&self, id: &str) -> Option<String> {
        self.state
            .room
            .objects
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.text.clone())
    }
}
