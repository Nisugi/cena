//! Cut the committed M1 fixtures from raw corpus excerpts.
//!
//! Run as `cargo run -p cena-protocol --example cut_fixtures -- <src-dir>`,
//! where `<src-dir>` holds the raw excerpts named below. It exists so the
//! fixtures are **reproducible output of the scrubber**, not the product of a
//! hand-editing session nobody can repeat -- which is what
//! `tests/fixtures_are_scrubbed.rs::every_fixture_is_already_a_scrub_fixed_point`
//! asserts about them.
//!
//! The raw excerpts are NOT committed: they are unscrubbed wire logs. The
//! byte ranges they were cut from are recorded in `tests/fixtures/README.md`.

use cena_protocol::scrub::Scrubber;
use std::path::Path;

/// `raw-name` -> `fixture-name`.
const CUTS: &[(&str, &str)] = &[
    ("room.src", "room.xml"),
    ("prompt.src", "prompt.xml"),
    ("vitals.src", "vitals.xml"),
    ("vs.src", "vitals_secondary.xml"),
    // M2's golden: every family the MODEL learned to hold. See
    // `crates/cena-model/tests/golden_model.rs` for why the model needs its own.
    ("m2model.src", "m2_model.xml"),
    // ---- M2's corpus, cut 2026-09-19 from `dev/lich-5/logs`. --------------
    // The six shapes M2 renders. Provenance in `tests/FIXTURES.md`.
    ("login.src", "login_burst.xml"),
    ("room2.src", "room_populated.xml"),
    ("combat.src", "combat_exchange.xml"),
    ("creatures.src", "creature_status.xml"),
    ("effects.src", "effect_dialogs.xml"),
    ("inventory.src", "inventory_container.xml"),
    // M3: the `info` command's blob -- ten stat lines, two of them enhancive.
    ("info.src", "character_info.xml"),
    // M3 step 5: `skills full` -- all 46 skills (zeros included), then each
    // spell circle under its own `Spell Lists` header. Bold marks the
    // enhancive cells, per-number rather than per-line.
    ("skills.src", "character_skills.xml"),
];

/// Kept in step with `tests/fixtures_are_scrubbed.rs::PSEUDONYMS`; that test
/// is what proves the real names are gone.
const PSEUDONYMS: &[(&str, &str)] = &[
    ("Inochi", "Alderin"),
    // The eleven players standing in a public arena in `room_populated.xml`.
    // Every one appeared in `<component id='room players'>` in
    // `GSIV-Nisugi/2026/09/xml/2026-09-01_15-13-56.xml:240`.
    //
    // They are pseudonymised, not dropped, because the line's VALUE is its
    // shape: eleven links, several behind titles ("Arena Icon", "Captain of
    // the Falcon", "Legendary Lady"), which is the form a room roster takes
    // and which no hand-written snippet would get right. The `exist=` ids are
    // KEPT, so id-to-name correlation is still under test.
    ("Fulmen", "Aldric"),
    ("Khadzim", "Bresnik"),
    ("Vortalis", "Cerdwyn"),
    ("Gidion", "Dorlan"),
    ("Komoki", "Eshvarr"),
    ("Eliaku", "Faldrin"),
    ("Jabalia", "Gwenlyn"),
    ("Attalynx", "Halvorn"),
    ("Jinxt", "Ithriel"),
    ("Richland", "Jorvath"),
    ("Berean", "Kelmond"),
    // **The author's own character**, in `character_info.xml`'s `Name:` line at
    // both the `noun=` and text sites. `info` output is about the logged-in
    // character, so its name is unavoidable in this fixture -- which is exactly
    // why it is pseudonymised rather than the fixture avoided.
    ("Nisugi", "Ashryn"),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_dir = std::env::args()
        .nth(1)
        .ok_or("usage: cut_fixtures <src-dir>")?;
    let raw_dir = Path::new(&raw_dir);
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    std::fs::create_dir_all(&out)?;

    let mut scrubber = Scrubber::new();
    for (real, pseudonym) in PSEUDONYMS {
        scrubber.pseudonymise(real, pseudonym);
    }

    for (raw, fixture) in CUTS {
        // **Skip a source that is not here, rather than failing the run.** The
        // raw excerpts are not committed, so a caller regenerating ONE fixture
        // has only that one source -- and making them re-cut all four to touch
        // a fifth is what stops a tool like this being used.
        let src = raw_dir.join(raw);
        if !src.exists() {
            println!("{fixture}: skipped, no {raw}");
            continue;
        }
        let text = std::fs::read_to_string(src)?;
        let clean = scrubber.scrub(&text);
        std::fs::write(out.join(fixture), &clean)?;
        println!("{fixture}: {} bytes", clean.len());
    }
    Ok(())
}
