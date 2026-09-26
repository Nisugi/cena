//! The hunt driver's errands that are not selling: healing with herbs
//! (`plan/36`), stocking at the herbalist, and the waggle run (`plan/37`).

use std::time::Duration;

use cena_session::{CommandId, Notice, NoticeKind};

use super::{BEAT, Driver, HEAL_STEPS, HuntEnd, STOCK_STEPS, WAGGLE_STEPS};
use crate::cast;
use crate::heal::stock::Step as StockStep;
use crate::heal::{self, Healed, Healer, Reply as HealReply, Step as HealStep, Stocked, Stocker};
use crate::travel::{TravelNotes, destination, walker_from};
use crate::waggle::{Step as WaggleStep, Waggled, Waggler};

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    /// Heal with the healer (`plan/36`): each step sent through the gate,
    /// each reply read from the transcript. Says how it ended.
    pub(super) async fn heal(&mut self) -> Result<(), HuntEnd> {
        let Some(profile) = self.machine.heal_profile().cloned() else {
            return Ok(());
        };
        let (spellcast, ranged) = self.machine.heal_mode();
        let mut healer = Healer::new(profile, spellcast, ranged);
        let mut ended = None;
        for _ in 0..HEAL_STEPS {
            let line = match healer.next(&self.state) {
                HealStep::Done(how) => {
                    ended = Some(how);
                    break;
                }
                HealStep::Look(target) => format!("look in {target}"),
                HealStep::Analyze(id) => format!("analyze #{id}"),
                HealStep::Point { kit, dose } => format!("point #{kit} at dose {dose}"),
                HealStep::Fetch(id) => format!("get #{id}"),
                HealStep::Eat(noun) => format!("eat my {noun}"),
                HealStep::Drink(noun) => format!("drink my {noun}"),
                HealStep::Stow { item, bag } => format!("_drag #{item} #{bag}"),
            };
            self.transcript.clear();
            self.send(&line, None).await?;
            self.hold(BEAT).await?;
            let replies: Vec<HealReply> =
                self.transcript.lines().filter_map(heal::classify).collect();
            healer.outcome(&replies);
        }
        let text = match ended {
            Some(Healed::Done { missing }) if missing.is_empty() => "Heal: done.".to_owned(),
            Some(Healed::Done { missing }) => format!(
                "Heal: done; no herb for {}.",
                missing
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Some(Healed::NoContainer) => {
                "Heal: the herb container is not in the inventory; set `container` in the heal profile.".to_owned()
            }
            Some(Healed::Refused) => "Heal: the herbs here have been eaten from enough.".to_owned(),
            None => "Heal: gave up after too many steps.".to_owned(),
        };
        self.handle.say(Notice::line(NoticeKind::Info, text));
        Ok(())
    }

    /// Stock the herb container (`plan/36` Stage 4): the nearest herbalist
    /// and bank by the map's tags, the town by the herbalist room's
    /// location, each step sent and each reply read. Says how it ended.
    pub(super) async fn stock(&mut self, fill: bool) -> Result<(), HuntEnd> {
        let Some(profile) = self.machine.heal_profile().cloned() else {
            return Ok(());
        };
        let say = |this: &Self, text: String| this.handle.say(Notice::line(NoticeKind::Info, text));
        let Some(sack) = heal::container_named(&self.state, &profile.container) else {
            say(
                self,
                "Heal: the herb container is not in the inventory; set `container` in the heal profile.".to_owned(),
            );
            return Ok(());
        };
        let Some(home) = self.locate() else {
            return Ok(());
        };
        let nearest = |this: &Self, tag: &str| {
            let now = this.state.game_time_now().unwrap_or(0);
            let walker = walker_from(&this.state, &this.notes, now);
            destination(
                this.map,
                &walker,
                home,
                tag,
                &std::collections::BTreeMap::new(),
            )
        };
        let shop = nearest(self, "herbalist");
        let bank = nearest(self, "bank");
        let location = shop
            .and_then(|room| self.map.room(room))
            .and_then(|room| room.location.clone())
            .unwrap_or_default();
        let mut stocker = Stocker::new(
            profile,
            &self.state,
            sack,
            (shop, bank, home),
            location,
            fill,
        );
        let mut ended = None;
        for _ in 0..STOCK_STEPS {
            let line = match stocker.next(&self.state) {
                StockStep::Done(how) => {
                    ended = Some(how);
                    break;
                }
                StockStep::Walk(to) => {
                    self.walk(to).await?;
                    continue;
                }
                StockStep::Fetch(id) => format!("get #{id}"),
                StockStep::Measure(id) => format!("measure #{id}"),
                StockStep::Stow { item, bag } => format!("_drag #{item} #{bag}"),
                StockStep::Menu => "order".to_owned(),
                StockStep::Order { count, number } => format!("order {count} {number}"),
                StockStep::Buy => "buy".to_owned(),
                StockStep::Open(id) => format!("open #{id}"),
                StockStep::Empty { from, into } => format!("empty #{from} in #{into}"),
                StockStep::Throw(id) => format!("throw #{id}"),
                StockStep::Withdraw(silver) => format!("withdraw {silver} silver"),
            };
            self.transcript.clear();
            self.send(&line, None).await?;
            self.hold(BEAT).await?;
            let replies: Vec<HealReply> =
                self.transcript.lines().filter_map(heal::classify).collect();
            stocker.outcome(&replies, &self.state);
        }
        let text = match ended {
            Some(Stocked::Done { bought: 0 }) => "Heal: stocked; nothing was short.".to_owned(),
            Some(Stocked::Done { bought }) => format!("Heal: stocked; {bought} bought."),
            Some(Stocked::NoHerbalist) => "Heal: the map has no herbalist to walk to.".to_owned(),
            Some(Stocked::NotOnMenu(herb)) => {
                format!("Heal: the herbalist's menu has no {herb}.")
            }
            Some(Stocked::NoSilver) => "Heal: the bank would not cover the herbs.".to_owned(),
            None => "Heal: stocking gave up after too many steps.".to_owned(),
        };
        say(self, text);
        Ok(())
    }

    /// Cast the waggle profile's spells on `targets` (`plan/37` Stage 5):
    /// each step sent, each reply read, the run's end said.
    pub(super) async fn waggle(&mut self, targets: &[String]) -> Result<(), HuntEnd> {
        let Some((profile, _)) = self.machine.waggle() else {
            return Ok(());
        };
        let mut waggler = Waggler::new(profile.clone(), targets);
        let mut ended = None;
        for _ in 0..WAGGLE_STEPS {
            let lines = match waggler.next(&self.state) {
                WaggleStep::Done(how) => {
                    ended = Some(how);
                    break;
                }
                WaggleStep::Wait(secs) => {
                    self.hold(Duration::from_secs(u64::from(secs))).await?;
                    continue;
                }
                WaggleStep::Ask(name) => vec![format!("spell active {name}")],
                WaggleStep::Cast(casting) => casting.lines(&self.state),
            };
            self.transcript.clear();
            for line in &lines {
                self.send(line, None).await?;
            }
            self.hold(BEAT).await?;
            let said: Vec<String> = self.transcript.lines().map(str::to_owned).collect();
            let answers: Vec<cast::Answer> =
                said.iter().filter_map(|l| cast::classify(l)).collect();
            waggler.outcome(&said, &answers, &self.state);
        }
        let text = match ended {
            Some(Waggled::Done(casts)) => format!("Waggle: done; {casts} cast."),
            Some(Waggled::OutOfMana) => "Waggle: out of mana; stopped.".to_owned(),
            None => "Waggle: gave up after too many steps.".to_owned(),
        };
        self.handle.say(Notice::line(NoticeKind::Info, text));
        Ok(())
    }
}
