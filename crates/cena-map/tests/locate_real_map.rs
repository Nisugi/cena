//! The ladder over every room of a real map. Needs a built map, so it is
//! `#[ignore]`d and runs only when asked for, with `CENA_MAP` naming one:
//!
//! ```powershell
//! $env:CENA_MAP = "E:\Cena\reference\mapdb\converted\hydra.map"
//! cargo test --release -p cena-map --test locate_real_map -- --ignored --nocapture
//! ```
//!
//! **Ignored rather than silently passing.** It used to return early when
//! `CENA_MAP` was unset and report `1 passed` -- a green line for a check that
//! never ran -- and `decode(..).ok()?` turned a map that *failed to load* into
//! the same skip, so a format regression read as success on exactly the run
//! meant to catch it. Now an ordinary run says `ignored`, and a named map that
//! cannot be read or decoded fails.
//!
//! The property is the module's promise, **unambiguous or nothing**: shown a
//! room's own recorded text, `locate` names that room or declines -- it never
//! names another.

use cena_map::{Located, Map, Origin, Room, Sighting, binary};

fn sighting(room: &Room, with_number: bool) -> Sighting<'_> {
    Sighting {
        uid: if with_number {
            room.uid.first().copied()
        } else {
            None
        },
        title: room.title.first().map(String::as_str),
        description: room.description.first().map(String::as_str),
        paths: room.paths.first().map(String::as_str),
        location: None,
    }
}

#[derive(Default)]
struct Tally {
    found: usize,
    ambiguous: usize,
    unknown: usize,
    wrong: Vec<(u32, u32)>,
}

impl Tally {
    fn see(&mut self, room: &Room, located: &Located) {
        match located {
            Located::Here { room: named, .. } if *named == room.id => self.found += 1,
            Located::Here { room: named, .. } => self.wrong.push((room.id.0, named.0)),
            Located::Ambiguous(_) => self.ambiguous += 1,
            Located::Unknown => self.unknown += 1,
        }
    }

    fn line(&self, what: &str) -> String {
        format!(
            "{what:<34} found {:>6}  ambiguous {:>5}  unknown {:>4}  WRONG {}",
            self.found,
            self.ambiguous,
            self.unknown,
            self.wrong.len()
        )
    }
}

/// The map `CENA_MAP` names; `None` only when it names nothing. A map that is
/// named and cannot be read or decoded is a failure, never a skip.
// `cfg(test)` so clippy.toml's allow-expect-in-tests covers a helper too.
#[cfg(test)]
fn load() -> Option<Map> {
    let path = std::env::var_os("CENA_MAP")?;
    let bytes = std::fs::read(&path).expect("CENA_MAP names a file that cannot be read");
    Some(binary::decode(&bytes).expect("CENA_MAP names a map that does not decode"))
}

#[test]
#[ignore = "Tier 2: needs a built map in CENA_MAP; run with --ignored"]
fn a_rooms_own_text_never_names_another_room() {
    let Some(map) = load() else {
        eprintln!("CENA_MAP is not set; skipped");
        return;
    };
    let started = std::time::Instant::now();

    let mut numbered = Tally::default();
    let mut by_text = Tally::default();
    let mut arriving = Tally::default();
    for room in map.rooms() {
        if room.uid.is_empty() {
            by_text.see(room, &map.locate(&sighting(room, false), Origin::Nowhere));
        } else {
            numbered.see(room, &map.locate(&sighting(room, true), Origin::Nowhere));
        }
    }
    // Walking in: every exit into a room the map has no number for.
    for from in map.rooms() {
        for exit in &from.exits {
            let Some(to) = map.room(exit.to).filter(|to| to.uid.is_empty()) else {
                continue;
            };
            arriving.see(to, &map.locate(&sighting(to, false), Origin::Left(from.id)));
        }
    }

    println!("{}", numbered.line("rooms with a number"));
    println!("{}", by_text.line("rooms without, from nowhere"));
    println!("{}", arriving.line("rooms without, walked into"));
    println!("{:?} for the whole pass", started.elapsed());

    for tally in [&numbered, &by_text, &arriving] {
        assert!(
            tally.wrong.is_empty(),
            "named another room: {:?}",
            &tally.wrong[..tally.wrong.len().min(10)]
        );
    }
}
