//! What a PSM costs, and Lich's `available?` over it
//! (`state/character/psm/cost.rs`), with the PSM tables folded from the wire
//! (`state/character/psm.rs`, `read_tables`).
//!
//! The tables are the committed real `cman list`, `feat list` and `armor
//! list` (`crates/cena-protocol/tests/fixtures/psm_*.xml`). The stamina bar is
//! the committed form (`character_info.xml`) with its numbers changed. The
//! `Cooldowns` dialog carrying `Volley` is copied whole from
//! `crates/cena-behavior/tests/fixtures/arch_kill.xml:155`. Every other dialog
//! entry is SYNTHETIC, in that element's shape: no committed fixture lists a
//! PSM other than Volley on cooldown, `Overexerted`, `Glorious Momentum` or
//! `Ardor of the Scourge`.

use cena_model::state::character::psm::read_tables;
use cena_model::state::character::snapshot::Group;
use cena_model::{
    GameState, Gauge, PsmAvailability, PsmCategory, Warcry, every_cost, psm_cost, warcry_cost,
};
use cena_protocol::Parser;

const CMAN_LIST: &str = include_str!("../../cena-protocol/tests/fixtures/psm_list.xml");
const FEAT_LIST: &str = include_str!("../../cena-protocol/tests/fixtures/psm_feat.xml");
const ARMOR_LIST: &str = include_str!("../../cena-protocol/tests/fixtures/psm_armor.xml");
const TABLE: &str = include_str!("../data/psm_costs.tsv");

/// `arch_kill.xml:155`'s Cooldowns dialog, verbatim.
const VOLLEY_COOLING: &str = "<dialogData id='Cooldowns' clear='t'></dialogData><dialogData id='Cooldowns'><progressBar id='4685' value='92' text=\"Aspect of the Panther\" left='22%' top='0' width='76%' height='15' time='00:03:41'/><label id='l4685' value='0:03 ' top='0' left='0' justify='2' anchor_right=''/><progressBar id='37594784' value='76' text=\"Rapid Fire Recovery\" left='22%' top='16' width='76%' height='15' time='00:01:32'/><label id='l37594784' value='92s ' top='16' left='0' justify='2' anchor_right=''/><progressBar id='86705824' value='100' text=\"Multi-Strike\" left='22%' top='32' width='76%' height='15' time='00:00:45'/><label id='l86705824' value='45s ' top='32' left='0' justify='2' anchor_right=''/><progressBar id='119818926' value='0' text=\"Volley\" left='22%' top='48' width='76%' height='15' time='00:00:10'/><label id='l119818926' value='10s ' top='48' left='0' justify='2' anchor_right=''/></dialogData>\n";

fn feed(state: &mut GameState, wire: &str) {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
}

/// A clock, so effects can be live.
fn clocked() -> GameState {
    let mut state = GameState::default();
    feed(&mut state, "<prompt time=\"1000\">&gt;</prompt>\n");
    state
}

/// The committed stamina bar's form, with these points.
fn stamina(state: &mut GameState, current: u32) {
    feed(
        state,
        &format!(
            "<dialogData id='minivitals'><progressBar id='stamina' value='50' text='stamina {current}/200' left='50%' customText='t' top='0%' width='25%' height='100%'/></dialogData>\n"
        ),
    );
}

/// One dialog stated whole, listing these names for ten seconds.
fn dialog(state: &mut GameState, id: &str, names: &[&str]) {
    use std::fmt::Write as _;
    let bars = names
        .iter()
        .enumerate()
        .fold(String::new(), |mut out, (n, name)| {
            // Writing to a `String` cannot fail.
            let _ = write!(
                out,
                "<progressBar id='{}' value='50' text=\"{name}\" left='22%' top='0' width='76%' height='15' time='00:00:10'/>",
                7000 + n
            );
            out
        });
    feed(
        state,
        &format!(
            "<dialogData id='{id}' clear='t'></dialogData><dialogData id='{id}'>{bars}</dialogData>\n"
        ),
    );
}

/// A state that knows every category's table, holds `points` stamina, and
/// has stated Cooldowns, Debuffs and Buffs empty.
fn ready(points: u32) -> GameState {
    let mut state = clocked();
    feed(&mut state, CMAN_LIST);
    for (category, row) in [
        (
            "Feats",
            "  Excoriate            excoriate       3/5   Attack",
        ),
        (
            "Armor Specializations",
            "  Armor Blessing       blessing        1/5   Buff",
        ),
        (
            "Shield Specializations",
            "  Shield Trample       trample         2/5   Area of Effect",
        ),
    ] {
        feed(
            &mut state,
            &format!(
                "Ashryn, the following {category} are available:\n<pushBold/>{row}\n<popBold/>   Subcategory: all\n<prompt time=\"1000\">&gt;</prompt>\n"
            ),
        );
    }
    feed(
        &mut state,
        "Ashryn, the following Weapon Techniques are available:\n<pushBold/>  Volley               volley          2/5   Area of Effect\n<popBold/><pushBold/>  Barrage              barrage         1/5   Assault\n<popBold/>   Subcategory: all\n<prompt time=\"1000\">&gt;</prompt>\n",
    );
    stamina(&mut state, points);
    dialog(&mut state, "Cooldowns", &[]);
    dialog(&mut state, "Debuffs", &[]);
    dialog(&mut state, "Buffs", &[]);
    state
}

// --- the table -------------------------------------------------------------

#[test]
fn every_row_of_lichs_six_tables_is_cut_and_cites_its_line() {
    let stated: usize = TABLE
        .lines()
        .find_map(|l| l.strip_prefix("# rows\t"))
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let rows: Vec<_> = every_cost().collect();
    assert_eq!(stated, 187, "the header's count (`extract_psm_costs.rb`)");
    assert_eq!(rows.len(), stated, "every data line parses");

    for (category, file, count) in [
        ("armor", "armor", 11),
        ("cman", "cman", 80),
        ("feat", "feat", 33),
        ("shield", "shield", 33),
        ("weapon", "weapon", 24),
        ("warcry", "warcry", 6),
    ] {
        let mine: Vec<_> = rows.iter().filter(|(c, _, _)| *c == category).collect();
        assert_eq!(
            mine.len(),
            count,
            "{category}: psm.rs counts the same hash literals"
        );
        // Lich's game directory sits between the two; Rule 3.4's scan keeps
        // its name out of shared code, and the check does not need it.
        let tail = format!("{file}.rb:");
        assert!(
            mine.iter()
                .all(|(_, _, cost)| cost.source.starts_with("lich-5/lib/")
                    && cost
                        .source
                        .rsplit_once("/psms/")
                        .and_then(|(_, at)| at.strip_prefix(&tail))
                        .is_some_and(|n| n.parse::<u32>().is_ok_and(|n| n > 0))),
            "{category}: every row cites a line of {file}.rb"
        );
    }
}

#[test]
fn the_rows_read_what_lich_wrote() {
    let coup = psm_cost(PsmCategory::CombatManeuver, "coupdegrace");
    assert_eq!(
        coup.map(|c| (c.gauge, c.amount, c.long_name)),
        Some((Gauge::Stamina, 20, "coup_de_grace"))
    );
    assert_eq!(
        coup.and_then(|c| c.source.rsplit_once("/psms/"))
            .map(|(_, at)| at),
        Some("cman.rb:106")
    );

    let excoriate = psm_cost(PsmCategory::Feat, "excoriate");
    assert_eq!(
        excoriate.map(|c| (c.gauge, c.amount)),
        Some((Gauge::Mana, 10)),
        "feat.rb:131, the one mana row"
    );

    let volley = psm_cost(PsmCategory::Weapon, "volley");
    assert_eq!(
        volley.map(|c| (c.kind, c.amount)),
        Some(("area_of_effect", 20))
    );

    assert_eq!(psm_cost(PsmCategory::Weapon, "nosuchthing"), None);
    assert_eq!(
        psm_cost(PsmCategory::Feat, "coupdegrace"),
        None,
        "the category is part of the key"
    );
}

#[test]
fn only_burst_and_surge_cost_more_while_cooling() {
    let doubled: Vec<&str> = every_cost()
        .filter(|(_, _, cost)| cost.while_cooling.is_some())
        .map(|(_, mnemonic, _)| mnemonic)
        .collect();
    assert_eq!(doubled, ["burst", "surge"], "cman.rb:62, :576");
    let burst = psm_cost(PsmCategory::CombatManeuver, "burst");
    assert_eq!(
        burst.map(|c| (c.amount, c.while_cooling)),
        Some((30, Some(60)))
    );
}

#[test]
fn the_two_feats_that_share_wps_cost_the_same() {
    let wps: Vec<_> = every_cost()
        .filter(|(c, m, _)| *c == "feat" && *m == "wps")
        .map(|(_, _, cost)| (cost.long_name, cost.gauge, cost.amount, cost.kind))
        .collect();
    assert_eq!(
        wps,
        [
            ("weighting", Gauge::Stamina, 0, "passive"),
            ("padding", Gauge::Stamina, 0, "passive"),
        ]
    );
}

#[test]
fn all_six_warcries_have_lichs_cost() {
    let costs: Vec<Option<u16>> = Warcry::ALL
        .iter()
        .map(|cry| warcry_cost(*cry).map(|c| c.amount))
        .collect();
    // warcry.rb:27-64, in Warcry::ALL's order: bellow, yowlp, growl, shout, cry, holler.
    assert_eq!(
        costs,
        [Some(20), Some(20), Some(14), Some(20), Some(20), Some(20)]
    );
}

// --- the tables, folded from the wire ---------------------------------------

#[test]
fn a_real_cman_list_is_folded_into_the_model() {
    let mut state = clocked();
    assert!(!state.character.psms.has_table(PsmCategory::CombatManeuver));
    feed(&mut state, CMAN_LIST);
    let psms = &state.character.psms;
    assert!(psms.has_table(PsmCategory::CombatManeuver));
    assert_eq!(
        psms.len(PsmCategory::CombatManeuver),
        27,
        "the capture's 27 rows (`tests/psm_list.rs:102-115`)"
    );
    assert_eq!(
        psms.get(PsmCategory::CombatManeuver, "disarm")
            .map(|r| (r.ranks, r.known_by_bold)),
        Some((5, true))
    );
    assert_eq!(
        psms.get(PsmCategory::CombatManeuver, "trip")
            .map(|r| (r.ranks, r.known_by_bold)),
        Some((0, false))
    );
    assert!(
        psms.mnemonics(PsmCategory::CombatManeuver)
            .all(|(_, r)| !r.disagrees()),
        "bold and the fraction agree on every row, as psm_list.rs measured"
    );
    assert!(
        !psms.has_table(PsmCategory::Feat),
        "one table says nothing of another"
    );
}

#[test]
fn the_reader_stops_at_the_footer_and_reads_each_table_in_a_chunk() {
    let mut state = GameState::default();
    feed(&mut state, FEAT_LIST);
    // `psm_armor.xml`'s own prompt is split by Lich's echo of the next
    // command (`<prompt tim<!-- CLIENT -->shield list all all...`), so it
    // closes nothing; a whole prompt is appended.
    feed(&mut state, ARMOR_LIST);
    feed(&mut state, "<prompt time=\"1\">&gt;</prompt>\n");
    assert!(state.character.psms.has_table(PsmCategory::Feat));
    assert!(state.character.psms.has_table(PsmCategory::Armor));

    // Two tables before one prompt, and a row-shaped line after the footer.
    let mut two = GameState::default();
    feed(
        &mut two,
        "Ashryn, the following Feats are available:\n  Absorb Magic         absorbmagic     1/1   Buff\n   Subcategory: all\n  Not A Row            notarow         3/5   Buff\nAshryn, the following Armor Specializations are available:\n  Armor Blessing       blessing        2/5   Buff\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    let psms = &two.character.psms;
    assert_eq!(
        psms.len(PsmCategory::Feat),
        1,
        "the footer closed the feat table"
    );
    assert_eq!(psms.get(PsmCategory::Feat, "notarow"), None);
    assert_eq!(
        psms.get(PsmCategory::Armor, "blessing").map(|r| r.ranks),
        Some(2)
    );
}

#[test]
fn the_group_is_taught_only_when_all_five_tables_are_held() {
    let mut state = GameState::default();
    feed(&mut state, CMAN_LIST);
    feed(&mut state, FEAT_LIST);
    // The armor fixture's own prompt is split (see above).
    feed(&mut state, ARMOR_LIST);
    feed(&mut state, "<prompt time=\"1\">&gt;</prompt>\n");
    assert!(
        !state.character.take_taught().contains(&Group::Psms),
        "three of five"
    );

    let tables = |category: &str, row: &str| {
        format!(
            "Ashryn, the following {category} are available:\n{row}\n   Subcategory: all\n<prompt time=\"1\">&gt;</prompt>\n"
        )
    };
    feed(
        &mut state,
        &tables(
            "Shield Specializations",
            "  Shield Bash          bash            0/5   Setup",
        ),
    );
    assert!(
        !state.character.take_taught().contains(&Group::Psms),
        "four of five"
    );
    feed(
        &mut state,
        &tables(
            "Weapon Techniques",
            "  Volley               volley          0/5   Area of Effect",
        ),
    );
    assert!(state.character.take_taught().contains(&Group::Psms));

    // Every later prompt is not a new lesson: a chunk with no table teaches
    // nothing, or each one would push the store's save out again.
    feed(
        &mut state,
        "You look around.\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    assert!(!state.character.take_taught().contains(&Group::Psms));
}

#[test]
fn bold_padding_alone_does_not_mark_a_row_known() {
    // SYNTHETIC: the last row's bold span closing after this row's padding,
    // the case `tests/psm_list.rs:35` trims for. No committed table has it.
    let mut state = GameState::default();
    feed(
        &mut state,
        "Ashryn, the following Feats are available:\n<pushBold/>  Absorb Magic         absorbmagic     1/1   Buff\n  <popBold/>Kroll's Might        krolls          0/5   Buff\n   Subcategory: all\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    let psms = &state.character.psms;
    assert_eq!(
        psms.get(PsmCategory::Feat, "absorbmagic")
            .map(|r| r.known_by_bold),
        Some(true)
    );
    assert_eq!(
        psms.get(PsmCategory::Feat, "krolls")
            .map(|r| (r.ranks, r.known_by_bold)),
        Some((0, false))
    );
}

#[test]
fn read_tables_ignores_a_chunk_with_no_header() {
    let mut state = GameState::default();
    feed(
        &mut state,
        "  Trip                 trip            3/5   Setup\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    assert!(
        state.character.psms.is_empty(),
        "a row with no header is not a table"
    );
    // And the reader itself, on an empty chunk.
    assert!(read_tables(&cena_model::Chunk::default()).is_empty());
}

// --- availability ------------------------------------------------------------

#[test]
fn nothing_stated_is_not_known_rather_than_no() {
    let state = GameState::default();
    let coup = state.psm_availability(PsmCategory::CombatManeuver, "coupdegrace");
    assert_eq!(
        (coup.known, coup.affordable, coup.cooling, coup.overexerted),
        (None, None, None, None)
    );
    assert_eq!(coup.available(), None);
    assert!(
        coup.cost.is_some(),
        "the cost is data, known before anything is stated"
    );
}

#[test]
fn known_affordable_and_nothing_cooling_is_available() {
    let state = ready(100);
    let disarm = state.psm_availability(PsmCategory::CombatManeuver, "disarm");
    assert_eq!(
        (
            disarm.known,
            disarm.affordable,
            disarm.cooling,
            disarm.overexerted
        ),
        (Some(true), Some(true), Some(false), Some(false))
    );
    assert_eq!(disarm.available(), Some(true));

    let trip = state.psm_availability(PsmCategory::CombatManeuver, "trip");
    assert_eq!(trip.known, Some(false), "0/5: listed, untrained");
    assert_eq!(trip.available(), Some(false));

    let absent = state.psm_availability(PsmCategory::CombatManeuver, "nosuchthing");
    assert_eq!(
        absent.known,
        Some(false),
        "the table was read and does not list it"
    );
    assert_eq!(
        (absent.cost, absent.affordable, absent.cooling),
        (None, None, None)
    );
    assert_eq!(absent.available(), Some(false));
}

#[test]
fn affordable_is_strictly_more_points_than_the_cost() {
    // Disarm costs 7 (cman.rb:149). psms.rb:118: `cost_amount < stamina`.
    assert_eq!(
        ready(7)
            .psm_availability(PsmCategory::CombatManeuver, "disarm")
            .affordable,
        Some(false)
    );
    assert_eq!(
        ready(8)
            .psm_availability(PsmCategory::CombatManeuver, "disarm")
            .affordable,
        Some(true)
    );
    // Excoriate spends mana, and no mana bar has been stated.
    assert_eq!(
        ready(200)
            .psm_availability(PsmCategory::Feat, "excoriate")
            .affordable,
        None
    );
}

#[test]
fn a_bar_that_states_no_points_answers_not_known() {
    let mut state = ready(100);
    feed(
        &mut state,
        "<dialogData id='minivitals'><progressBar id='stamina' value='50' text='stamina' left='50%' customText='t' top='0%' width='25%' height='100%'/></dialogData>\n",
    );
    assert_eq!(
        state
            .psm_availability(PsmCategory::CombatManeuver, "disarm")
            .affordable,
        None
    );
}

#[test]
fn a_psm_in_the_cooldowns_dialog_is_not_available() {
    let mut state = ready(100);
    // The committed dialog: Volley is cooling, and nothing named Disarm Weapon.
    feed(&mut state, VOLLEY_COOLING);
    let volley = state.psm_availability(PsmCategory::Weapon, "volley");
    assert_eq!(
        volley.cooling,
        Some(true),
        "`Volley` meets the long name `volley`"
    );
    assert!(!volley.cooldown_waived);
    assert_eq!(volley.available(), Some(false));
    assert_eq!(
        state
            .psm_availability(PsmCategory::CombatManeuver, "disarm")
            .cooling,
        Some(false)
    );

    // The long name folds to the dialog's words: `disarm_weapon` is `Disarm Weapon`.
    dialog(&mut state, "Cooldowns", &["Disarm Weapon"]);
    let disarm = state.psm_availability(PsmCategory::CombatManeuver, "disarm");
    assert_eq!(
        (disarm.cooling, disarm.available()),
        (Some(true), Some(false))
    );
}

#[test]
fn overexerted_bars_every_psm() {
    let mut state = ready(100);
    dialog(&mut state, "Debuffs", &["Overexerted"]);
    let disarm = state.psm_availability(PsmCategory::CombatManeuver, "disarm");
    assert_eq!(
        (disarm.overexerted, disarm.available()),
        (Some(true), Some(false))
    );
}

#[test]
fn burst_costs_sixty_while_its_own_cooldown_is_up() {
    // Cooldowns stated, burst not in it: 30.
    let state = ready(45);
    let burst = state.psm_availability(PsmCategory::CombatManeuver, "burst");
    assert_eq!(burst.affordable, Some(true));

    let mut cooling = ready(45);
    dialog(&mut cooling, "Cooldowns", &["Burst of Swiftness"]);
    assert_eq!(
        cooling
            .psm_availability(PsmCategory::CombatManeuver, "burst")
            .affordable,
        Some(false)
    );

    // Cooldowns never stated: 45 covers 30 and not 60, so it cannot say.
    let mut unknown = clocked();
    feed(&mut unknown, CMAN_LIST);
    stamina(&mut unknown, 45);
    assert_eq!(
        unknown
            .psm_availability(PsmCategory::CombatManeuver, "burst")
            .affordable,
        None
    );
    // 61 covers both, so it can.
    stamina(&mut unknown, 61);
    assert_eq!(
        unknown
            .psm_availability(PsmCategory::CombatManeuver, "burst")
            .affordable,
        Some(true)
    );
    // 30 covers neither.
    stamina(&mut unknown, 30);
    assert_eq!(
        unknown
            .psm_availability(PsmCategory::CombatManeuver, "burst")
            .affordable,
        Some(false)
    );
}

#[test]
fn glorious_momentum_waives_an_area_technique_cost_and_cooldown() {
    let mut state = ready(5);
    feed(&mut state, VOLLEY_COOLING);
    assert_eq!(
        state
            .psm_availability(PsmCategory::Weapon, "volley")
            .affordable,
        Some(false)
    );

    dialog(&mut state, "Buffs", &["Glorious Momentum"]);
    let volley = state.psm_availability(PsmCategory::Weapon, "volley");
    assert_eq!(
        (volley.affordable, volley.cooling, volley.cooldown_waived),
        (Some(true), Some(true), true)
    );
    assert_eq!(volley.available(), Some(true), "weapon.rb:236, :257-258");

    // A combat maneuver of the same type is not waived (cman.rb has no such rule).
    let bullrush = state.psm_availability(PsmCategory::CombatManeuver, "bullrush");
    assert_eq!(
        (bullrush.affordable, bullrush.cooldown_waived),
        (Some(false), false)
    );
    // A shield area technique's cost is (shield.rb:307); its cooldown is not.
    let trample = state.psm_availability(PsmCategory::Shield, "trample");
    assert_eq!(
        (trample.affordable, trample.cooldown_waived),
        (Some(true), false)
    );
}

#[test]
fn ardor_of_the_scourge_waives_an_assaults_cooldown_only() {
    let mut state = ready(100);
    dialog(&mut state, "Cooldowns", &["Barrage"]);
    assert_eq!(
        state
            .psm_availability(PsmCategory::Weapon, "barrage")
            .available(),
        Some(false)
    );
    dialog(&mut state, "Buffs", &["Ardor of the Scourge"]);
    let barrage = state.psm_availability(PsmCategory::Weapon, "barrage");
    assert!(barrage.cooldown_waived, "weapon.rb:259-260");
    assert_eq!(barrage.available(), Some(true));
    assert!(
        !state
            .psm_availability(PsmCategory::Weapon, "volley")
            .cooldown_waived,
        "volley is not an assault"
    );
}

#[test]
fn available_is_false_as_soon_as_one_test_fails() {
    let unknown_but_failing = PsmAvailability {
        cost: None,
        known: None,
        affordable: None,
        cooling: None,
        cooldown_waived: false,
        overexerted: Some(true),
    };
    assert_eq!(unknown_but_failing.available(), Some(false));
    let all_pass_but_one_unknown = PsmAvailability {
        overexerted: None,
        known: Some(true),
        affordable: Some(true),
        cooling: Some(false),
        ..unknown_but_failing
    };
    assert_eq!(all_pass_but_one_unknown.available(), None);
}

// --- effects by name ---------------------------------------------------------

#[test]
fn an_effect_by_name_is_three_valued_and_folded_like_lichs() {
    let mut state = clocked();
    let now = 1000;
    assert_eq!(
        state.effects.active_named("Cooldowns", "volley", now),
        None,
        "never stated"
    );
    feed(&mut state, VOLLEY_COOLING);
    let effects = &state.effects;
    assert_eq!(effects.active_named("Cooldowns", "volley", now), Some(true));
    assert_eq!(effects.active_named("Cooldowns", "VOLLEY", now), Some(true));
    assert_eq!(
        effects.active_named("Cooldowns", "multi_strike", now),
        Some(true),
        "`-` and `_` both fold to a space"
    );
    assert_eq!(
        effects.active_named("Cooldowns", "rapid_fire_recovery", now),
        Some(true)
    );
    assert_eq!(
        effects.active_named("Cooldowns", "volley", now + 11),
        Some(false),
        "expired: ten seconds were left"
    );
    assert_eq!(
        effects.active_named("Cooldowns", "burst_of_swiftness", now),
        Some(false),
        "stated, and absent"
    );
    assert_eq!(
        effects.active_named("Buffs", "volley", now),
        None,
        "another dialog, never stated"
    );

    let mut apostrophe = clocked();
    dialog(&mut apostrophe, "Cooldowns", &["Predator's Eye"]);
    assert_eq!(
        apostrophe
            .effects
            .active_named("Cooldowns", "predators_eye", now),
        Some(true)
    );
}
