//! The importer's group keys: bigshot's "MA Grouping" tab, `quiet_followers`,
//! `troubadours_rally` and `disable_commands` (`bigshot.lic:3470-3551`), into
//! the profile's `[group]` (`plan/39` Stage 5). An `impl` of the same `Job`.

use super::{Job, expanded, flag, list};

impl Job {
    pub(super) fn group(&mut self) {
        let ma_looter = self.take("ma_looter").trim().to_owned();
        let never_loot = list(&self.take("never_loot"));
        let random_loot = flag(&self.take("random_loot"));
        let final_loot = flag(&self.take("final_loot"));
        let independent_travel = flag(&self.take("independent_travel"));
        let independent_return = flag(&self.take("independent_return"));
        let troubadours_rally = flag(&self.take("troubadours_rally"));
        let quiet = self.take("quiet_followers");
        let disabled = expanded(&self.take("disable_commands"));
        let disable_commands = disabled.iter().map(|entry| self.step(entry)).collect();
        if flag(&self.take("group_deader")) {
            self.note(
                "group_deader: a dead member no longer pauses the leader; every hunt ends and \
                 the leader or the first member able carries it out (plan/39 §8, question 10)"
                    .to_owned(),
            );
        }
        let table = &mut self.profile.group;
        table.ma_looter = Some(ma_looter).filter(|text| !text.is_empty());
        table.never_loot = never_loot;
        table.random_loot = random_loot;
        table.final_loot = final_loot;
        table.independent_travel = independent_travel;
        table.independent_return = independent_return;
        table.troubadours_rally = troubadours_rally;
        table.disable_commands = disable_commands;
        // bigshot's default is on (`bigshot.lic:3551`): only a stated value
        // turns it off.
        if !quiet.trim().is_empty() {
            table.quiet_followers = flag(&quiet);
        }
    }
}
