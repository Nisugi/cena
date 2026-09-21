//! Floating disks, read off the room's objects.

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
fn a_possessive_disk_is_found_with_its_owner() {
    let state = room_with(&link("-12345", "disk", "Ryeka's disk"));
    let disks: Vec<Disk> = state.room.disks().collect();
    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].owner, "Ryeka");
    assert_eq!(disks[0].noun, "disk");
    assert_eq!(disks[0].id, "-12345", "what a command targets");
}

#[test]
fn every_one_of_the_eleven_nouns_carries_a_disk() {
    // `disk.rb:4`, ported whole. A twelfth noun added by the game is a visible
    // change here rather than a silent miss.
    for noun in cena_model::DISK_NOUNS {
        let state = room_with(&link("-1", noun, &format!("Ryeka's {noun}")));
        assert_eq!(state.room.disks().count(), 1, "{noun} should carry a disk");
    }
}

#[test]
fn furniture_is_not_somebodys_disk() {
    // BOTH halves are required. A plain chest in a room is scenery, and
    // treating it as a disk would have a behavior try to loot it.
    let state = room_with(&format!(
        "{} {}",
        link("-1", "chest", "a wooden chest"),
        link("-2", "chest", "the chest"),
    ));
    assert_eq!(state.room.disks().count(), 0);
}

#[test]
fn a_possessive_that_is_not_a_disk_noun_is_not_a_disk() {
    let state = room_with(&link("-1", "backpack", "Ryeka's backpack"));
    assert_eq!(state.room.disks().count(), 0);
}

#[test]
fn the_owner_is_read_from_the_noun_backwards() {
    // Not by splitting on the first apostrophe. That is a real distinction for
    // a real name: `Ryeka's disk` splits the same either way, but a possessive
    // is defined by what precedes the NOUN, and reading forwards would break
    // on any text the game puts in front.
    let state = room_with(&link("-1", "disk", "the tarnished Ryeka's disk"));
    let disks: Vec<Disk> = state.room.disks().collect();
    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].owner, "the tarnished Ryeka");
}

#[test]
fn the_owners_name_is_read_whatever_its_shape() {
    // NOT a demonstration that Lich is wrong. Its `[A-Z][a-z]+` (`disk.rb:7`)
    // encodes what the game permits, and an earlier version of this test used
    // invented names -- `McTavish`, `D'iel` -- that GemStone would not issue,
    // to make a simplification look like a bug fix (author, 2026-09-20).
    //
    // What this pins is that nothing here re-derives the name's shape: the
    // noun comes off the typed link, so the owner is whatever precedes `'s`.
    // If the game ever widens what a name may be, this needs no change.
    let state = room_with(&link("-1", "disk", "Ryeka's disk"));
    assert_eq!(
        state.room.disks().next().map(|d| d.owner),
        Some("Ryeka".to_owned())
    );
}

mod finding {
    use super::{link, room_with};

    #[test]
    fn a_disk_is_found_by_its_owners_exact_name() {
        let state = room_with(&link("-99", "disk", "Nisugi's disk"));
        assert_eq!(
            state.room.disk_of("Nisugi").map(|d| d.id),
            Some("-99".to_owned())
        );
    }

    #[test]
    fn a_prefix_of_an_owners_name_finds_nothing() {
        // BUG FIX, not a port. Lich matches with `item.name.include?(name)`
        // (`disk.rb:12`), a SUBSTRING -- so a character called `Rye` looking
        // for their own disk would find `Ryeka's` and try to loot a
        // stranger's property.
        let state = room_with(&link("-1", "disk", "Ryeka's disk"));
        assert_eq!(state.room.disk_of("Rye"), None, "not a substring match");
        assert!(state.room.disk_of("Ryeka").is_some(), "guard: it is there");
    }

    #[test]
    fn the_right_disk_is_found_among_several() {
        let state = room_with(&format!(
            "{} {} {}",
            link("-1", "disk", "Ryeka's disk"),
            link("-2", "chest", "Nisugi's chest"),
            link("-3", "sphere", "D'iel's sphere"),
        ));
        assert_eq!(state.room.disks().count(), 3);
        assert_eq!(
            state.room.disk_of("Nisugi").map(|d| d.noun),
            Some("chest".to_owned())
        );
    }
}
