//! Asking what a trip must know before it is priced, and fetching the silver
//! it will be asked for (`plan/24` stage 5). The reading is pure
//! (`super::super::preflight`); this sends, walks and stops.
//!
//! # The silver detour (`go2.lic:2217-2299`)
//!
//! Add up what the route's exits charge -- and the way back, if the profile
//! says `get_return_trip_silvers` -- and compare it with `wealth`. Short, and
//! allowed the bank (`get_silvers`): walk to the nearest bank the silver in
//! hand can reach, price the walk again from there, withdraw the difference,
//! and look again. Short and not allowed, upstream warns, waits ten seconds
//! and goes anyway; this warns and goes, since the ferryman will say no soon
//! enough and the trip plans round him.
//!
//! **Upstream `exit`s where this halts**: too poor to reach a bank, or the
//! bank has not enough. Both are for the player to settle.

use cena_map::{RoomId, Target, Uid};
use cena_session::{CommandId, Notice, NoticeKind};

use super::super::preflight::{silver_for, urchin_access, withdraw_command};
use super::super::routines::day_pass;
use super::{Cx, Driver, Ended};

/// The Long Snow's encampment and Cairnfang Manor's attic: the two ends of
/// the long way to the Hinterwilds (`go2.lic:2192`).
const HINTERWILDS_WAYS: [u32; 2] = [29860, 22154];

fn is_on(cx: &Cx<'_>, setting: &str) -> bool {
    cx.notes
        .settings
        .get(setting)
        .is_some_and(|is| is == "true")
}

impl<N: FnMut() -> CommandId> Driver<'_, N> {
    /// Before the first plan. `goal` is where the trip is going.
    pub(super) async fn preflight(&mut self, cx: &mut Cx<'_>, goal: RoomId) -> Result<(), Ended> {
        if is_on(cx, "use_urchins") {
            let answer = self.put(cx.trip, "urchin status").await?;
            if let Some(access) = urchin_access(&answer) {
                self.found.insert("urchin_access".to_owned(), access);
            }
        }
        self.day_passes(cx).await?;
        // Silver is priced from a room; an unplaced walker is the walk's to
        // wait for, and the ferryman's to refuse.
        let Some(here) = self.locate(cx.map) else {
            return Ok(());
        };
        self.by_gigas(cx, here, goal).await?;
        let Some(here) = self.locate(cx.map) else {
            return Ok(());
        };
        let needed = self.silver_needed(cx, here, goal);
        if needed == 0 {
            return Ok(());
        }
        let have = self.silver(cx).await?;
        if have >= needed {
            return Ok(());
        }
        if !is_on(cx, "get_silvers") {
            self.handle.say(Notice::line(
                NoticeKind::Warn,
                format!(
                    "Travel: this route asks for {needed} silver and you carry {have}. \
                     `get_silvers` is off, so I am going anyway."
                ),
            ));
            return Ok(());
        }
        self.by_way_of_the_bank(cx, here, goal, have).await
    }

    /// Which day passes the walker holds (`day_pass_cost_head.rb`): upstream
    /// finds out inside the cost script, opening the sack and reading each
    /// pass while Dijkstra runs. Here it is asked once, of a profile that
    /// uses passes and names the sack they are kept in, and each pass that is
    /// still good becomes the flag the map asks for: `day_pass:imt,wl`.
    async fn day_passes(&mut self, cx: &mut Cx<'_>) -> Result<(), Ended> {
        /// Upstream reads every pass in the sack; nobody keeps this many.
        const MAX_PASSES: usize = 20;
        let sack = cx.notes.settings.get("day_pass_sack");
        let Some(sack) = sack.filter(|sack| !sack.is_empty() && is_on(cx, "use_day_pass")) else {
            return Ok(());
        };
        let sack = day_pass::sack_in(&self.state, sack)
            .map_or_else(|| format!("my {sack}"), |id| format!("#{id}"));
        let mut inside = self.put(cx.trip, &format!("look in {sack}")).await?;
        let shut = inside.iter().any(|line| line.text().contains("closed"));
        if shut {
            self.put(cx.trip, &format!("open {sack}")).await?;
            inside = self.put(cx.trip, &format!("look in {sack}")).await?;
        }
        let now = self.state.game_time().map(i64::from);
        for id in day_pass::passes_in(&inside).into_iter().take(MAX_PASSES) {
            let looked = self.put(cx.trip, &format!("look #{id}")).await?;
            let Some(pass) = day_pass::read_pass(&looked) else {
                continue;
            };
            let flag = pass.towns.as_ref().and_then(|(one, other)| {
                pass.serves(one, other, now)
                    .then(|| day_pass::flag_for(one, other))?
            });
            if let Some(flag) = flag {
                self.found.insert(flag, true);
            }
        }
        if shut {
            self.put(cx.trip, &format!("close {sack}")).await?;
        }
        Ok(())
    }

    /// To or from the Hinterwilds by gigas fragments (`go2.lic:2191-2200`,
    /// `:337-371`): when the route passes the Long Snow's encampment or the
    /// manor's attic, the profile says `use_gigas_hwtravel`, and `wealth
    /// gigas` counts at least `gigas_min_number` (four, unless said), walk to
    /// the teleporter instead -- Ta'Illistim's if the route passes Seethe
    /// Naedal, the Abbey's otherwise -- and `go sliver`; or, leaving, to
    /// Sparkfinger's workroom and `order 3`. Which side was entered by is
    /// remembered (`hinterwilds_location`) while the walker is there. The
    /// trip then plans from wherever that landed.
    async fn by_gigas(&mut self, cx: &mut Cx<'_>, here: RoomId, goal: RoomId) -> Result<(), Ended> {
        const WORKROOM: i64 = 7_503_253;
        if !is_on(cx, "use_gigas_hwtravel") {
            return Ok(());
        }
        let walker = self.walker(cx.notes);
        let Some(path) = cx.trip.path_from(cx.map, &walker, here, goal) else {
            return Ok(());
        };
        if !path.iter().any(|room| HINTERWILDS_WAYS.contains(&room.0)) {
            return Ok(());
        }
        let wild = |room: RoomId| {
            cx.map.room(room).and_then(|room| room.location.as_deref()) == Some("the Hinterwilds")
        };
        let (leaving, arriving) = (wild(here), wild(goal));
        if leaving == arriving {
            return Ok(());
        }
        self.put(cx.trip, "wealth gigas").await?;
        let fragments = self
            .state
            .character
            .currency
            .gigas_artifact_fragments
            .unwrap_or(0);
        let least = cx.notes.settings.get("gigas_min_number");
        if fragments < least.and_then(|least| least.parse().ok()).unwrap_or(4) {
            return Ok(());
        }
        let elven = path.iter().any(|room| {
            cx.map.room(*room).is_some_and(|room| {
                room.title
                    .first()
                    .is_some_and(|title| title.contains("Seethe Naedal"))
            })
        });
        let (uid, side, commands): (i64, _, &[&str]) = match (arriving, elven) {
            (true, true) => (13_205_202, Some("EN"), &["go sliver", "go sliver"]),
            (true, false) => (4_132_054, Some("IM"), &["go sliver", "go sliver"]),
            (false, _) => (WORKROOM, None, &["order 3", "order confirm"]),
        };
        let Some(teleporter) = cx.map.ids_for_uid(Uid(uid)).first().copied() else {
            return Ok(());
        };
        if !self.walk_to(cx, teleporter).await? {
            return Ok(());
        }
        for command in commands {
            self.put(cx.trip, command).await?;
        }
        // Upstream's own rule: remembered only while standing in the workroom.
        let landed = self.locate(cx.map);
        let there = landed.is_some_and(|room| cx.map.ids_for_uid(Uid(WORKROOM)).contains(&room));
        match side.filter(|_| there) {
            Some(side) => cx
                .notes
                .memories
                .insert("hinterwilds_location".to_owned(), side.to_owned()),
            None => cx.notes.memories.remove("hinterwilds_location"),
        };
        (cx.wrote)(cx.notes);
        Ok(())
    }

    /// What the route there -- and back, if the profile asks -- charges.
    fn silver_needed(&self, cx: &Cx<'_>, from: RoomId, goal: RoomId) -> u64 {
        let walker = self.walker(cx.notes);
        let leg = |from, to| {
            cx.trip
                .path_from(cx.map, &walker, from, to)
                .map_or(0, |path| silver_for(cx.map, &path))
        };
        let back = if is_on(cx, "get_return_trip_silvers") {
            leg(goal, from)
        } else {
            0
        };
        leg(from, goal) + back
    }

    /// `wealth`, as the model read it. Nothing said is nothing carried.
    async fn silver(&mut self, cx: &mut Cx<'_>) -> Result<u64, Ended> {
        self.put(cx.trip, "wealth quiet").await?;
        Ok(self.state.character.currency.silver.unwrap_or(0))
    }

    async fn by_way_of_the_bank(
        &mut self,
        cx: &mut Cx<'_>,
        here: RoomId,
        goal: RoomId,
        have: u64,
    ) -> Result<(), Ended> {
        let walker = self.walker(cx.notes);
        let mut banks: Vec<(f64, RoomId)> = cx
            .map
            .rooms()
            .iter()
            .filter(|room| room.tags.iter().any(|tag| tag == "bank"))
            .filter_map(|room| {
                let path = cx.trip.path_from(cx.map, &walker, here, room.id)?;
                (silver_for(cx.map, &path) <= have).then_some(())?;
                let routes = cx
                    .map
                    .routes(here, Target::Room(room.id), cx.trip.pricing(&walker));
                Some((routes.seconds_to(room.id)?, room.id))
            })
            .collect();
        banks.sort_by(|(a, _), (b, _)| a.total_cmp(b));
        let Some((_, bank)) = banks.first().copied() else {
            return self.halt("you are too poor to reach a bank.");
        };
        if !self.walk_to(cx, bank).await? {
            return self.halt("I could not get to the bank.");
        }
        let needed = self.silver_needed(cx, bank, goal);
        let have = self.silver(cx).await?;
        if needed > have {
            let walker = self.walker(cx.notes);
            let unseen = ["hidden", "invisible"]
                .iter()
                .any(|flag| walker.flags.get(*flag) == Some(&true));
            if unseen {
                self.put(cx.trip, "unhide").await?;
            }
            let command = withdraw_command(self.state.room.title.as_deref(), needed - have);
            self.put(cx.trip, &command).await?;
            if self.silver(cx).await? < needed {
                return self.halt("there is not enough silver in this bank for the trip.");
            }
        }
        Ok(())
    }

    fn halt(&mut self, why: &str) -> Result<(), Ended> {
        self.halted = Some(why.to_owned());
        Err(Ended::Halted)
    }
}
