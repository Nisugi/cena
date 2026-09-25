//! The loot profile and its import from eloot's settings (`plan/31` §6),
//! against Nisugi's own `eloot.yaml` (copied to `tests/fixtures/` on
//! 2026-09-24) and against what it must hold rather than lose.

use cena_behavior::loot::{LootProfile, import, path};

fn nisugi() -> std::io::Result<String> {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/eloot.yaml"
    ))
}

#[test]
fn nisugis_settings_come_across_key_by_key() {
    let brought = import(&nisugi().unwrap()).unwrap();
    let p = &brought.profile;
    assert_eq!(p.take.len(), 18, "eighteen categories wanted: {:?}", p.take);
    for kind in ["box", "gem", "skin", "lm trap", "uncommon", "valuable"] {
        assert!(p.takes(kind), "{kind} is wanted");
    }
    for kind in ["cursed", "herb", "junk", "weapon"] {
        assert!(!p.takes(kind), "{kind} is not wanted");
    }
    assert_eq!(p.leave, ["black ora", "urglaes"]);
    assert!(p.defensive, "loot_defensive");
    assert!(p.disk, "use_disk");
    assert!(p.sigil_on_fail, "sigil_determination_on_fail");
    assert!(!p.phase_boxes, "loot_phase");
    assert!(p.overflow.is_empty(), "no overflow containers");
    assert_eq!(p.crumbly.len(), 18, "the crumbly names learned so far");
    assert!(p.crumbly.iter().any(|name| name == "ornate ruic lyre"));
    assert!(p.unlootable.is_empty() && p.autoclose.is_empty());
}

#[test]
fn the_town_keys_are_carried_verbatim_for_the_rest_phase() {
    let brought = import(&nisugi().unwrap()).unwrap();
    let town = &brought.profile.town;
    assert_eq!(
        town.get("sell_appraise_gemshop")
            .and_then(toml::Value::as_integer),
        Some(14_999)
    );
    assert_eq!(
        town.get("sell_container")
            .and_then(toml::Value::as_array)
            .map(Vec::len),
        Some(15),
        "a block list stays a list"
    );
    assert_eq!(
        town.get("charm_name").and_then(toml::Value::as_str),
        Some("fossil charm")
    );
    assert_eq!(
        town.get("alpha_rate").and_then(toml::Value::as_float),
        Some(2.5)
    );
    assert!(
        town.keys().all(|key| !key.starts_with("skin_")),
        "skinning keys are dropped, not carried as town"
    );
}

#[test]
fn nothing_in_nisugis_file_is_dropped_unsaid() {
    let brought = import(&nisugi().unwrap()).unwrap();
    // Skinning is off, loot_keep and critter_exclude are empty, blood bands
    // are off: the file holds nothing the importer has to warn about.
    assert!(brought.notes.is_empty(), "{:?}", brought.notes);
}

#[test]
fn what_would_change_behavior_is_named_when_dropped() {
    let yaml = "---\n:loot_types:\n- gem\n:skin_enable: true\n:skin_weapon: knife\n\
                :loot_keep:\n- blue crystal\n:use_bloodbands: true\n:mystery_key: 7\n";
    let brought = import(yaml).unwrap();
    let notes = brought.notes.join("\n");
    assert!(
        notes.contains("loot_keep") && notes.contains("blue crystal"),
        "{notes}"
    );
    assert!(notes.contains("use_bloodbands"), "{notes}");
    assert!(notes.contains("mystery_key"), "{notes}");
    // Skinning is built: its keys are carried, not noted.
    assert!(!notes.contains("skin"), "{notes}");
    assert!(brought.profile.skin.enable);
    assert_eq!(brought.profile.skin.weapon, "knife");
}

#[test]
fn the_skinning_keys_come_across_and_read_back() {
    let yaml = "---\n:skin_enable: true\n:skin_kneel: true\n:skin_604: false\n:skin_resolve: true\n\
                :skin_bounty_only: false\n:skin_sheath: sheath\n:skin_weapon: dagger\n\
                :skin_sheath_blunt: \n:skin_weapon_blunt: cudgel\n:skin_exclude:\n- giant rat\n\
                :unskinnable:\n- stone golem\n";
    let brought = import(yaml).unwrap();
    let skin = &brought.profile.skin;
    assert!(skin.enable && skin.kneel && skin.resolve && !skin.spell_604);
    assert_eq!(
        (skin.weapon.as_str(), skin.sheath.as_str()),
        ("dagger", "sheath")
    );
    assert_eq!(
        (skin.weapon_blunt.as_str(), skin.sheath_blunt.as_str()),
        ("cudgel", "")
    );
    assert_eq!(skin.exclude, ["giant rat"]);
    assert_eq!(skin.unskinnable, ["stone golem"]);
    let text = brought.profile.to_toml().unwrap();
    assert!(text.contains("[skin]"), "{text}");
    assert_eq!(LootProfile::parse(&text).unwrap(), brought.profile);
    // Skinning off is left out of the file.
    assert!(!LootProfile::default().to_toml().unwrap().contains("skin"));
}

#[test]
fn the_rendered_file_reads_back_as_the_same_profile() {
    let brought = import(&nisugi().unwrap()).unwrap();
    let text = brought.render().unwrap();
    assert!(text.starts_with("# Hydra loot profile"));
    let again = LootProfile::parse(&text).unwrap();
    assert_eq!(again, brought.profile);
}

#[test]
fn a_take_word_the_game_does_not_type_is_a_problem() {
    let profile = LootProfile::parse("take = [\"gem\", \"shinies\"]\n").unwrap();
    let problems = profile.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("shinies"));
    assert!(
        LootProfile::parse("take = [\"gem\"]\nlaser = true\n").is_err(),
        "an unknown key is refused"
    );
}

#[test]
fn the_file_is_per_character_under_the_hunt_directory() {
    let dir = std::path::Path::new("data");
    let file = path(dir, "GSIV", "Nisugi").unwrap();
    assert!(
        file.ends_with(std::path::Path::new("hunt/loot/gsiv_nisugi.toml")),
        "{file:?}"
    );
}
