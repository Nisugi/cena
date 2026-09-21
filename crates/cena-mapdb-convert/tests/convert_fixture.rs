//! The converter over a cut of the real upstream map.
//!
//! `fixtures/mapdb_cut.json` is 22 rooms cut from `map-1789942730.json` by
//! `research/mapdb-inventory/cut_fixture.py`, each chosen for what it
//! exercises; their exits are trimmed to one another so nothing dangles.

use cena_map::{Cost, Crossing, ExitKind, Room, RoomId, Uid};
use cena_mapdb_convert::run::{Conversion, convert};
use cena_mapdb_convert::shape::shape_id;
use cena_mapdb_convert::{output, run};

const CUT: &str = include_str!("fixtures/mapdb_cut.json");

/// Helpers return `Result`/`Option` and the tests unwrap: clippy's test
/// exemption covers `#[test]` functions, not the helpers beside them.
fn converted() -> Result<Conversion, serde_json::Error> {
    convert(CUT)
}

fn room(conversion: &Conversion, id: u32) -> Option<&Room> {
    conversion.rooms.iter().find(|room| room.id == RoomId(id))
}

#[test]
fn the_cut_converts_cleanly() {
    let conversion = converted().unwrap();
    assert_eq!(conversion.rooms.len(), 22);
    assert_eq!(conversion.report.dangling, 0);
    assert!(
        conversion.report.problems.is_empty(),
        "{:#?}",
        conversion.report.problems
    );
    assert!(
        conversion
            .rooms
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id),
        "rooms are ordered"
    );
}

/// The inn tables: upstream's most repeated hand-written script. Two table
/// names, one shape -- which is the whole premise of porting by shape.
#[test]
fn scripted_crossings_that_differ_only_in_a_parameter_share_a_shape() {
    let conversion = converted().unwrap();
    let atrium = room(&conversion, 0).unwrap();
    let shapes: Vec<_> = atrium
        .exits
        .iter()
        .filter_map(|exit| match &exit.crossing {
            Crossing::Unported(shape) => Some((exit.to, exit.kind, shape.clone())),
            Crossing::Command(_) => None,
        })
        .collect();
    assert_eq!(shapes.len(), 2);
    assert_eq!(
        shapes[0].2, shapes[1].2,
        "Cat's Paw and hammer are one shape"
    );
    assert!(
        shapes
            .iter()
            .all(|(_, kind, _)| *kind == ExitKind::Scripted)
    );
    // A scripted crossing with a plain numeric cost keeps that cost, and is
    // still not routable: the crossing is what is missing.
    let table = atrium
        .exits
        .iter()
        .find(|exit| exit.to == RoomId(1))
        .unwrap();
    assert!(matches!(table.cost, Some(Cost::Fixed(_))));
    assert!(!table.is_routable());
}

/// Urchins (`plan/21` §2b): a `;e true` crossing into a virtual hub, gated by a
/// scripted cost; and plain commands out of it, gated by another.
#[test]
fn the_urchin_hub_is_data_on_both_sides() {
    let conversion = converted().unwrap();
    let into_hub = room(&conversion, 7)
        .unwrap()
        .exits
        .iter()
        .find(|exit| exit.to == RoomId(30714))
        .expect("BriarStone Court enters the hub");
    assert_eq!(into_hub.crossing, Crossing::Unported(shape_id(";e true")));
    assert!(matches!(into_hub.cost, Some(Cost::Unported { .. })));

    let hub = room(&conversion, 30714).unwrap();
    assert!(hub.uid.is_empty(), "a virtual room has no game room number");
    for exit in &hub.exits {
        let Crossing::Command(command) = &exit.crossing else {
            panic!("the hub's exits are plain commands");
        };
        assert!(command.starts_with("urchin guide "), "{command}");
        assert_eq!(exit.kind, ExitKind::Other);
        assert!(
            !exit.is_routable(),
            "gated by a scripted cost until that cost is ported"
        );
    }
}

#[test]
fn identification_fields_survive() {
    let conversion = converted().unwrap();
    assert_eq!(
        room(&conversion, 4136).unwrap().uid.first(),
        Some(&Uid(-9054)),
        "negative uids are real"
    );
    assert!(
        room(&conversion, 18011).unwrap().uid.len() > 1,
        "an instanced room has several uids"
    );

    let unknowable = room(&conversion, 2640).unwrap();
    assert!(unknowable.location_unknowable && unknowable.location.is_none());
    let named = room(&conversion, 0).unwrap();
    assert_eq!(named.location.as_deref(), Some("the Moonglae Inn"));
    assert!(!named.location_unknowable);

    assert!(room(&conversion, 2927).unwrap().check_location);
    assert!(!room(&conversion, 682).unwrap().unique_loot.is_empty());
    assert!(room(&conversion, 87).unwrap().image.is_some());
}

#[test]
fn plain_exits_are_routable_and_typed() {
    let conversion = converted().unwrap();
    let exits = &room(&conversion, 87).unwrap().exits;
    assert!(exits.iter().all(cena_map::Exit::is_routable));
    assert!(exits.iter().any(|exit| exit.kind == ExitKind::Vertical));
    assert_eq!(conversion.report.exits, 30);
    assert_eq!(
        conversion.report.routable
            + conversion.report.unported_crossings
            + conversion
                .rooms
                .iter()
                .flat_map(|room| &room.exits)
                .filter(|exit| {
                    matches!(exit.crossing, Crossing::Command(_))
                        && !matches!(exit.cost, Some(Cost::Fixed(_)))
                })
                .count(),
        conversion.report.exits,
        "every exit is routable, an unported crossing, or a plain command without a usable cost",
    );
}

/// Forage sightings are most of upstream's tag volume and are not routing
/// data; the `meta:` prefix is structure, not content.
#[test]
fn tags_are_split() {
    let conversion = converted().unwrap();
    for room in &conversion.rooms {
        assert!(
            room.tags.iter().all(|tag| !tag.starts_with("meta:")),
            "{:?}",
            room.tags
        );
        assert!(
            room.meta
                .iter()
                .all(|meta| !meta.starts_with("forage-sensed")),
            "{:?}",
            room.meta
        );
    }
}

/// `output.rs`'s two promises: the same input writes the same bytes, and a
/// second run touches nothing.
#[test]
fn writing_is_deterministic_and_quiet() {
    let out = std::env::temp_dir().join(format!("cena-mapdb-convert-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);

    let first = output::write(&out, &converted().unwrap()).unwrap();
    assert_eq!((first.created, first.updated, first.unchanged), (22, 0, 0));
    let second = output::write(&out, &run::convert(CUT).unwrap()).unwrap();
    assert_eq!(
        (second.created, second.updated, second.unchanged),
        (0, 0, 22)
    );

    // What was written reads back as the same room.
    let conversion = converted().unwrap();
    for room in &conversion.rooms {
        let text = std::fs::read_to_string(output::room_path(&out, room.id)).unwrap();
        assert!(text.ends_with("}\n"));
        assert_eq!(&serde_json::from_str::<Room>(&text).unwrap(), room);
    }
    let index: Vec<RoomId> =
        serde_json::from_str(&std::fs::read_to_string(out.join("index.json")).unwrap()).unwrap();
    assert_eq!(index.len(), 22);
    assert!(out.join("report").join("unported_crossings.tsv").is_file());

    std::fs::remove_dir_all(&out).unwrap();
}

#[test]
fn a_file_that_is_not_a_map_is_an_error_not_an_empty_map() {
    assert!(convert("{}").is_err());
    assert!(convert("not json").is_err());
    assert_eq!(convert("[]").unwrap().rooms.len(), 0);
}
