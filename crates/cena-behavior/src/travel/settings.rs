//! Travel's settings, the ones its code reads, as the table the settings
//! menu draws (`plan/50` §7 step 4; §6 item 2, the author: *"checked"*).
//! Each is named here once, and the code reads it by this name, so the
//! table and the code cannot name different keys.
//!
//! The map names more (`Map::setting_names`): `ice_mode`, the sack a house
//! key is kept in. The map, not this code, says what those mean, so the
//! menu keeps them beside these as words.
//!
//! Every value is kept as text in the travel file's settings, as go2 keeps
//! them; a switch is on when it is `true`, or `yes` or `on` as go2's own
//! command line takes them (`go2.lic:1263`), and [`on`] is the one reading
//! of it.

use std::collections::HashMap;

use crate::settings::{Key, KeyKind};

/// Whether the switch `name` is on in `settings`.
///
/// **The one reading.** Five places read a switch and two ways: four took
/// `true` alone and the day pass took `yes` as well, so `get_silvers = yes`
/// bought a Chronomage pass and no ferry's fare (the review of 2026-09-29).
#[must_use]
pub fn on<S: std::hash::BuildHasher>(settings: &HashMap<String, String, S>, name: &str) -> bool {
    settings.get(name).is_some_and(|is| {
        ["true", "yes", "on"]
            .iter()
            .any(|word| is.trim().eq_ignore_ascii_case(word))
    })
}

/// Ask the urchin guides' status before a trip, so a route may use them.
pub const USE_URCHINS: &str = "use_urchins";
/// Walk to a bank for the silver a route charges, when short.
pub const GET_SILVERS: &str = "get_silvers";
/// Count the way back in the silver fetched.
pub const GET_RETURN_TRIP_SILVERS: &str = "get_return_trip_silvers";
/// Use Chronomage day passes.
pub const USE_DAY_PASS: &str = "use_day_pass";
/// The container day passes are kept in.
pub const DAY_PASS_SACK: &str = "day_pass_sack";
/// Buy a day pass when none that serves is held: `yes`, or the routes.
pub const BUY_DAY_PASS: &str = "buy_day_pass";
/// Cross to and from the Hinterwilds by gigas fragments.
pub const USE_GIGAS_HWTRAVEL: &str = "use_gigas_hwtravel";
/// The fewest gigas fragments kept back from that crossing.
pub const GIGAS_MIN_NUMBER: &str = "gigas_min_number";
/// The trinket that takes the walker to Mist Harbor.
pub const FWI_TRINKET: &str = "fwi_trinket";

/// Every setting travel's code reads, in the menu's order.
pub const TABLE: &[Key] = &[
    Key {
        name: USE_URCHINS,
        label: "Use the urchin guides",
        help: "Ask the urchins' status before a trip, so a route may use them.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: GET_SILVERS,
        label: "Fetch silver from the bank",
        help: "Short of what a route charges, walk to a bank first. Off, go anyway.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: GET_RETURN_TRIP_SILVERS,
        label: "Fetch silver for the way back",
        help: "Count the return trip in the silver fetched.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: USE_DAY_PASS,
        label: "Use day passes",
        help: "Travel between towns on a Chronomage day pass from the pass sack.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: DAY_PASS_SACK,
        label: "Day pass sack",
        help: "The container day passes are kept in.",
        kind: KeyKind::Text,
    },
    Key {
        name: BUY_DAY_PASS,
        label: "Buy day passes",
        help: "`yes` to buy a pass when none held serves; or the routes to buy for, `wl,imt`.",
        kind: KeyKind::Text,
    },
    Key {
        name: USE_GIGAS_HWTRAVEL,
        label: "Use gigas fragments to the Hinterwilds",
        help: "Cross to and from the Hinterwilds by gigas fragments.",
        kind: KeyKind::Toggle,
    },
    Key {
        name: GIGAS_MIN_NUMBER,
        label: "Gigas fragments kept back",
        help: "Take the fragment crossing only with at least this many; 4 when unset.",
        kind: KeyKind::Whole { min: 0, max: 1000 },
    },
    Key {
        name: FWI_TRINKET,
        label: "Mist Harbor trinket",
        help: "The trinket that takes the walker to Mist Harbor, by its name.",
        kind: KeyKind::Text,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Each key is in the table once, and the table is every name above.
    #[test]
    fn the_table_is_every_setting_the_code_reads() {
        let names = crate::settings::names(TABLE);
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "{names:?}");
        assert_eq!(
            unique,
            [
                BUY_DAY_PASS,
                DAY_PASS_SACK,
                FWI_TRINKET,
                GET_RETURN_TRIP_SILVERS,
                GET_SILVERS,
                GIGAS_MIN_NUMBER,
                USE_DAY_PASS,
                USE_GIGAS_HWTRAVEL,
                USE_URCHINS,
            ]
        );
    }
}
