//! The importer's rest keys: thresholds, commands, the fog home and its
//! waypoints, and `wounded_eval` read as typed conditions. Split from
//! `import.rs` at its cap; an `impl` of the same `Job`.

use super::{Job, flag, health_at_most, list, number, unparenthesised};

impl Job {
    pub(super) fn rest(&mut self) {
        self.profile.rest.fried = number(&self.take("fried"));
        self.profile.rest.overkill = number(&self.take("overkill")).unwrap_or(0);
        self.profile.rest.encumbered = number(&self.take("encumbered"));
        self.profile.rest.mana_below = number(&self.take("oom"));
        self.profile.rest.until.experience = number(&self.take("rest_till_exp"));
        self.profile.rest.until.mana = number(&self.take("rest_till_mana"));
        self.profile.rest.until.spirit = number(&self.take("rest_till_spirit"));
        self.profile.rest.until.stamina = number(&self.take("rest_till_percentstamina"));
        self.profile.rest.commands = self.commands("resting_commands");
        self.fog();
        self.profile.rest.wracking = flag(&self.take("use_wracking"));
        self.profile.rest.lte_boost = number(&self.take("lte_boost")).unwrap_or(0);
        self.profile.rest.wracking_spirit = number(&self.take("wracking_spirit")).unwrap_or(0);
        self.rest_when();
    }

    /// `fog_return`, bigshot's six choices as the lines each sends
    /// (`bigshot.lic:7687-7722`), `fog_optional`, `fog_rift`, and
    /// `return_waypoint_ids`.
    pub(super) fn fog(&mut self) {
        let custom = self.commands("custom_fog");
        let choice = self.take("fog_return");
        let choice = choice.trim();
        let pick = |id: &str, spell: &str| choice == id || choice.contains(spell);
        let lines: Vec<&str> = if pick("1", "130") {
            vec!["incant 130"]
        } else if pick("2", "Symbol of Return") {
            vec!["symbol of return"]
        } else if pick("3", "1020") {
            vec!["incant 1020"]
        } else if pick("4", "Sigil of Escape") {
            vec!["sigil of escape"]
        } else if pick("5", "930") {
            vec!["incant 930", "go portal"]
        } else {
            Vec::new()
        };
        self.profile.rest.fog = if pick("6", "Custom") {
            custom
        } else {
            lines.into_iter().map(str::to_owned).collect()
        };
        self.profile.rest.fog_optional = flag(&self.take("fog_optional"));
        self.profile.rest.fog_rift = flag(&self.take("fog_rift"));
        for entry in list(&self.take("rallypoint_room_ids")) {
            match number(&entry) {
                Some(id) => self.profile.rooms.rally.push(id),
                None => self.note(format!(
                    "rallypoint_room_ids: `{entry}` is not a room number; Hydra's rooms are the map's numbers"
                )),
            }
        }
        for entry in list(&self.take("return_waypoint_ids")) {
            match number(&entry) {
                Some(id) => self.profile.rest.waypoints.push(id),
                None => self.note(format!(
                    "return_waypoint_ids: `{entry}` is not a room number; Hydra's rooms are the map's numbers"
                )),
            }
        }
    }

    /// `wounded_eval`: a Ruby expression of terms joined by `||`. Each term
    /// that is one of the known shapes becomes a threshold; each that is
    /// not is named, and Hydra will not rest on it.
    pub(super) fn rest_when(&mut self) {
        let when = &mut self.profile.rest.when;
        when.creeping_dread = number(&self.source.take("creeping_dread")).filter(|n| *n > 0);
        let when = &mut self.profile.rest.when;
        when.crushing_dread = number(&self.source.take("crushing_dread")).filter(|n| *n > 0);
        self.profile.rest.when.wot_poison = flag(&self.source.take("wot_poison"));
        self.profile.rest.when.confused = flag(&self.source.take("confusion"));
        let text = self.take("wounded_eval");
        for term in text.split("||").map(str::trim).filter(|t| !t.is_empty()) {
            let bare = unparenthesised(term);
            if bare.contains("&&") {
                self.note(format!(
                    "rest.when: `{bare}` joins conditions with &&, which a profile cannot say; Hydra will not rest on it"
                ));
                continue;
            }
            match bare {
                "bleeding?" | "checkbleeding" => self.profile.rest.when.bleeding = true,
                "!Injured.able_to_use_ranged?" => self.profile.rest.when.cannot_use_ranged = true,
                "!Injured.able_to_cast?" => self.profile.rest.when.cannot_cast = true,
                _ => match health_at_most(bare) {
                    Some(percent) => self.profile.rest.when.health_at_most = Some(percent),
                    None => self.note(format!(
                        "rest.when: `{bare}` is not a shape Hydra reads; Hydra will not rest on it"
                    )),
                },
            }
        }
    }
}
