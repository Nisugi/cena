//! Floating disks, read off the room's objects. Every line here is as the
//! game wrote it in Hydra's wire logs of 2026-09-23 to 2026-09-30, where no
//! disk is ever a possessive (`state/disk.rs`, the correction of 2026-10-01).

use cena_model::{Disk, GameState};
use cena_protocol::Parser;

/// A `room objs` component carrying links, as the wire sends it.
fn room_with(objects: &str) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    let wire = format!("<component id='room objs'>You also see {objects}</component>");
    for frame in parser.parse_line(&wire) {
        state.apply(&frame);
    }
    state
}

/// One `<a>` link, as `room objs` carries them.
fn link(id: &str, noun: &str, text: &str) -> String {
    format!("<a exist=\"{id}\" noun=\"{noun}\">{text}</a>")
}

#[test]
fn a_disk_is_found_with_its_owner() {
    let state = room_with(&format!("a {}", link("-12345", "disk", "Bagagwa disk")));
    let disks: Vec<Disk> = state.room.disks().collect();
    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].owner, "Bagagwa");
    assert_eq!(disks[0].noun, "disk");
    assert_eq!(disks[0].id, "-12345", "what a command targets");
}

#[test]
fn the_owner_is_the_name_before_the_noun_whatever_comes_first() {
    for (text, noun, owner) in [
        ("fiery red Vasstryke disk", "disk", "Vasstryke"),
        ("rusty iron Duffield disk", "disk", "Duffield"),
        ("lattice-woven wicker Lyracellan disk", "disk", "Lyracellan"),
        ("four-toned Desorceri coffret", "coffret", "Desorceri"),
        ("faenor Demandred disk", "disk", "Demandred"),
    ] {
        let state = room_with(&format!("the {}", link("-1", noun, text)));
        assert_eq!(
            state.room.disks().next().map(|d| d.owner),
            Some(owner.to_owned()),
            "{text}"
        );
    }
}

#[test]
fn every_one_of_the_eleven_nouns_carries_a_disk() {
    // `disk.rb:4`, ported whole. A twelfth noun added by the game is a visible
    // change here rather than a silent miss.
    for noun in cena_model::DISK_NOUNS {
        let state = room_with(&link("-1", noun, &format!("Ryeka {noun}")));
        assert_eq!(state.room.disks().count(), 1, "{noun} should carry a disk");
    }
}

#[test]
fn furniture_and_loot_are_not_somebodys_disk() {
    // BOTH halves are required. A chest or a sphere with no name before the
    // noun is scenery or loot, and treating it as a disk would have a
    // behavior take it for someone's.
    let state = room_with(&format!(
        "{} {} {} {}",
        link("-1", "chest", "iron-bound walnut chest"),
        link("-2", "chest", "waterlogged thanot chest"),
        link("-3", "sphere", "small shadowy black crystal sphere"),
        link("-4", "chest", "the chest"),
    ));
    assert_eq!(state.room.disks().count(), 0);
}

#[test]
fn a_name_before_a_noun_that_is_not_a_disk_is_not_a_disk() {
    let state = room_with(&link("-1", "backpack", "Ryeka backpack"));
    assert_eq!(state.room.disks().count(), 0);
}

#[test]
fn a_possessive_is_not_how_the_game_names_a_disk() {
    // Lich's `\b([A-Z][a-z]+) disk\b` does not match it either.
    let state = room_with(&link("-1", "disk", "Ryeka's disk"));
    assert_eq!(state.room.disks().count(), 0);
}

mod finding {
    use super::{link, room_with};

    #[test]
    fn a_disk_is_found_by_its_owners_exact_name() {
        let state = room_with(&link("-99", "disk", "Nisugi disk"));
        assert_eq!(
            state.room.disk_of("Nisugi").map(|d| d.id),
            Some("-99".to_owned())
        );
    }

    #[test]
    fn a_prefix_of_an_owners_name_finds_nothing() {
        // BUG FIX, not a port. Lich matches with `item.name.include?(name)`
        // (`disk.rb:12`), a SUBSTRING -- so a character called `Rye` looking
        // for their own disk would find `Ryeka disk` and try to loot a
        // stranger's property.
        let state = room_with(&link("-1", "disk", "Ryeka disk"));
        assert_eq!(state.room.disk_of("Rye"), None, "not a substring match");
        assert!(state.room.disk_of("Ryeka").is_some(), "guard: it is there");
    }

    #[test]
    fn the_right_disk_is_found_among_several() {
        let state = room_with(&format!(
            "{} {} {}",
            link("-1", "disk", "Ryeka disk"),
            link("-2", "chest", "Nisugi chest"),
            link("-3", "sphere", "fiery red Vasstryke sphere"),
        ));
        assert_eq!(state.room.disks().count(), 3);
        assert_eq!(
            state.room.disk_of("Nisugi").map(|d| d.noun),
            Some("chest".to_owned())
        );
    }
}
