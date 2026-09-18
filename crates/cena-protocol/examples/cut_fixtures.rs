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
];

/// Kept in step with `tests/fixtures_are_scrubbed.rs::PSEUDONYMS`; that test
/// is what proves the real names are gone.
const PSEUDONYMS: &[(&str, &str)] = &[("Inochi", "Alderin")];

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
        let text = std::fs::read_to_string(raw_dir.join(raw))?;
        let clean = scrubber.scrub(&text);
        std::fs::write(out.join(fixture), &clean)?;
        println!("{fixture}: {} bytes", clean.len());
    }
    Ok(())
}
