//! The hunt driver's selling round (`plan/31` Stage 4): the town planner
//! walked shop to shop.

use cena_session::{CommandId, Notice, NoticeKind};

use super::{BEAT, Driver, HuntEnd, SELL_STEPS};
use crate::loot::{Emptied, Learned};
use crate::town::{self, Round, Seller, Step as Errand, Town};
use crate::travel::{TravelNotes, walker_from};

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&Learned)> Driver<'_, F, W, L> {
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
        if let Round::Pool { drop: true, .. } = round {
            // `loot pool` keeps what is carried now, and banks the rest
            // after (`pool`, `eloot.lic:7626-7648`).
            self.send("wealth quiet", None).await?;
        }
        let town = Town::for_profile(&profile);
        let (bags, keep_closed) = (self.know_bags(&town).await?, town.keep_closed);
        let fwi = town.fwi && self.reaches_fwi(home);
        // A round frees the bags: none is known full from here. eloot clears
        // `sacks_full` before and after a sell and a pool trip alike
        // (`eloot.lic:7996`, `:8006`, `:8015`, `:8030`).
        self.memory.full.clear();
        let Some(mut seller) = Seller::for_round(town, &self.state, home, round) else {
            self.close_bags(keep_closed, &bags).await?;
            return Ok(false);
        };
        // Facts queued before the round are not the round's.
        let _ = self.state.take_loot();
        for _ in 0..SELL_STEPS {
            let here = self.locate();
            seller.worker_here(
                here.and_then(|room| self.map.room(room))
                    .and_then(cena_map::Room::pool_worker),
            );
            let step = {
                let now = self.state.game_time_now().unwrap_or(0);
                let walker = walker_from(&self.state, &self.notes, now);
                let (map, state) = (self.map, &self.state);
                let nearest = |tag: &str| {
                    here.and_then(|from| town::route::shop_room(map, &walker, from, tag, fwi))
                };
                seller.next(state, &nearest)
            };
            match &step {
                Errand::Walk(to) => {
                    self.walk(*to).await?;
                    continue;
                }
                Errand::EmptyBox(id) => {
                    let replies = match self.empty_box(&profile, id).await? {
                        Emptied::Out => Vec::new(),
                        Emptied::Locked => vec![town::Reply::BoxLocked],
                        Emptied::CoinsLeft => vec![town::Reply::CoinsLeft],
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
                _ => {}
            }
            let Some(line) = line_for(&step) else {
                break;
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
        self.memory.full.clear();
        self.share_full();
        self.close_bags(keep_closed, &bags).await?;
        Ok(true)
    }

    /// The bags the round sells from, by id, each one's contents listed
    /// first: one never looked in this session, or shut, is opened and
    /// looked in, as eloot opens every stow bag when it starts
    /// (`set_inventory`, `eloot.lic:2380-2383`).
    async fn know_bags(&mut self, town: &Town) -> Result<Vec<String>, HuntEnd> {
        let bags = town::goods::selling_bags(town, &self.state);
        for bag in &bags {
            if self.state.inventory.container(bag).is_none() {
                self.send(&format!("open #{bag}"), None).await?;
                self.send(&format!("look in #{bag}"), None).await?;
                self.hold(BEAT).await?;
            }
        }
        Ok(bags)
    }

    /// With `keep_closed`, the bags the round sold from closed again, but
    /// never one the ready list stores a weapon in (`close_sell_containers`,
    /// `eloot.lic:3737-3746`).
    async fn close_bags(&mut self, keep_closed: bool, bags: &[String]) -> Result<(), HuntEnd> {
        if !keep_closed {
            return Ok(());
        }
        for bag in bags {
            let ready = cena_session::containers::ReadySlot::ALL
                .iter()
                .filter_map(|slot| self.state.containers.ready(*slot))
                .any(|item| item.id == *bag);
            if !ready {
                self.send(&format!("close #{bag}"), None).await?;
            }
        }
        Ok(())
    }

    /// Whether a round that sells in Mist Harbor can get there from `home`
    /// (`town/route.rs`); said when it cannot, and the round sells here.
    fn reaches_fwi(&self, home: cena_map::RoomId) -> bool {
        let now = self.state.game_time_now().unwrap_or(0);
        let walker = walker_from(&self.state, &self.notes, now);
        let reaches = town::route::reaches_fwi(self.map, &walker, home);
        if !reaches {
            self.handle.say(Notice::line(
                NoticeKind::Warn,
                "Loot: the profile sells in Mist Harbor, but there is no way there from here: name the trinket on Travel's settings page. Selling here this round.",
            ));
        }
        reaches
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

    /// A thing on the floor, by id.
    pub(super) fn floor_item(&self, id: &str) -> Option<cena_session::RoomItem> {
        self.state
            .room
            .objects
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }
}

/// The line a step of the round sends; `None` for the end, and for the
/// steps the driver does itself (a walk, a box emptied).
fn line_for(step: &Errand) -> Option<String> {
    Some(match step {
        Errand::Done | Errand::Walk(_) | Errand::EmptyBox(_) => return None,
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
        Errand::LookAt(id) => format!("look at #{id}"),
        Errand::Pluck(id) => format!("pluck #{id}"),
    })
}
