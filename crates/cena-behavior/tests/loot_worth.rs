//! Is this thing on the floor worth taking: eloot's two filters as one
//! answer with its reason (`plan/31` §2b), by table.

use cena_behavior::loot::{LootProfile, Verdict, stow_slot, verdict};
use cena_session::RoomItem;
use cena_session::containers::StowSlot;

fn item(id: &str, noun: &str, text: &str) -> RoomItem {
    RoomItem {
        id: id.to_owned(),
        noun: noun.to_owned(),
        text: text.to_owned(),
        before: None,
        after: None,
        status: None,
    }
}

fn nisugi() -> Result<LootProfile, String> {
    let yaml = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/eloot.yaml"
    ))
    .map_err(|e| e.to_string())?;
    Ok(cena_behavior::loot::import(&yaml)?.profile)
}

fn left(v: &Verdict) -> Option<&'static str> {
    match v {
        Verdict::Leave(why) => Some(why),
        Verdict::Take(_) => None,
    }
}

#[test]
fn what_is_not_loot_is_left_with_the_reason() {
    let p = nisugi().unwrap();
    let cases = [
        (
            item("-483342", "smithy", "thatched timber smithy"),
            "part of the room",
        ),
        (
            item("242512467", "briar", "violently lashing emerald briar"),
            "not loot",
        ),
        (item("5", "mist", "sourceless mist"), "not loot"),
        (item("6", "disk", "Ashryn disk"), "someone's disk"),
        (
            item("7", "lyre", "ornate ruic lyre"),
            "crumbles when stowed",
        ),
        (item("8", "ring", "black ora ring"), "excluded by name"),
        (
            item("9", "greatsword", "huge steel greatsword"),
            "a weapon or armor",
        ),
    ];
    for (thing, why) in cases {
        let v = verdict(&thing, &p);
        assert_eq!(left(&v), Some(why), "{}: {v:?}", thing.text);
    }
}

#[test]
fn what_is_wanted_is_taken_and_the_rest_by_kind_is_left() {
    let p = nisugi().unwrap();
    for (thing, expect_take) in [
        (item("1", "emerald", "uncut emerald"), true),
        (item("2", "coffer", "enruned steel coffer"), true),
        (item("3", "acantha", "acantha leaf"), false),
        (item("4", "vial", "murky viscous liquid"), true),
    ] {
        let v = verdict(&thing, &p);
        assert_eq!(left(&v).is_none(), expect_take, "{}: {v:?}", thing.text);
    }
    assert_eq!(
        left(&verdict(&item("3", "acantha", "acantha leaf"), &p)),
        Some("not a wanted kind"),
        "herb is a category Nisugi does not take"
    );
}

#[test]
fn a_thing_of_no_known_kind_is_taken_as_eloot_takes_it() {
    let p = nisugi().unwrap();
    let v = verdict(&item("10", "whatsit", "peculiar glowing whatsit"), &p);
    assert!(left(&v).is_none(), "{v:?}");
}

#[test]
fn excluded_names_match_whole_words_only() {
    let p = LootProfile {
        leave: vec!["ora".to_owned()],
        ..LootProfile::default()
    };
    assert_eq!(
        left(&verdict(&item("1", "ring", "black ora ring"), &p)),
        Some("excluded by name")
    );
    assert!(
        left(&verdict(&item("2", "orb", "coral orb"), &p)) != Some("excluded by name"),
        "`ora` inside `coral` is not the word"
    );
}

/// `loot_keep`: taken whatever its kind, past the weapon rule and a name
/// learned crumbly (`eloot.lic:5647`); `leave` still wins, and so does a
/// curse not wanted (`:5588-5590`).
#[test]
fn a_thing_kept_by_name_is_taken_past_every_rule_but_leave_and_a_curse() {
    let p = LootProfile {
        keep: vec![
            "greatsword".to_owned(),
            "lyre".to_owned(),
            "dagger".to_owned(),
        ],
        leave: vec!["black".to_owned()],
        crumbly: vec!["ornate ruic lyre".to_owned()],
        ..LootProfile::default()
    };
    for thing in [
        item("1", "greatsword", "huge steel greatsword"),
        item("2", "lyre", "ornate ruic lyre"),
    ] {
        let v = verdict(&thing, &p);
        assert!(left(&v).is_none(), "{}: {v:?}", thing.text);
    }
    assert_eq!(
        left(&verdict(&item("3", "dagger", "black steel dagger"), &p)),
        Some("excluded by name")
    );
}

/// The `unlootable` list is read only when the profile remembers them
/// (`eloot.lic:5655`).
#[test]
fn the_unlootable_list_is_read_only_when_remembered() {
    let mut p = LootProfile {
        unlootable: vec!["peculiar glowing whatsit".to_owned()],
        ..LootProfile::default()
    };
    let whatsit = item("10", "whatsit", "peculiar glowing whatsit");
    assert!(left(&verdict(&whatsit, &p)).is_none());
    p.remember_unlootable = true;
    assert_eq!(
        left(&verdict(&whatsit, &p)),
        Some("could not be held before")
    );
}

#[test]
fn the_stow_slot_is_the_first_kind_that_names_one() {
    let p = nisugi().unwrap();
    let Verdict::Take(types) = verdict(&item("1", "emerald", "uncut emerald"), &p) else {
        panic!("an emerald is taken");
    };
    assert_eq!(stow_slot(&types), StowSlot::Gem);
    let Verdict::Take(types) = verdict(&item("2", "coffer", "enruned steel coffer"), &p) else {
        panic!("a coffer is taken");
    };
    assert_eq!(stow_slot(&types), StowSlot::Box);
    let Verdict::Take(types) = verdict(&item("3", "whatsit", "peculiar glowing whatsit"), &p)
    else {
        panic!("a whatsit is taken");
    };
    assert_eq!(stow_slot(&types), StowSlot::Default);
}
