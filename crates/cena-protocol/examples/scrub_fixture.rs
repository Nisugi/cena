//! Re-cut a fixture through the scrubber: `cargo run -p cena-protocol
//! --example scrub_fixture -- <path>...`
//!
//! `tests/fixtures_are_scrubbed.rs` says *"re-cut it through
//! `cena_protocol::scrub::Scrubber`, do not edit it by hand"* and names
//! `examples/cut_fixtures.rs` as the tool -- which did not exist (MEASURED
//! 2026-09-21: the only reference to that path was the comment). This is that
//! tool, kept minimal: the pseudonym table is the test's, because the test is
//! what proves the names are absent.

use cena_protocol::scrub::Scrubber;

/// **Kept in step with `tests/fixtures_are_scrubbed.rs`'s `PSEUDONYMS`.** The
/// test is the enforcement; this is the application.
const PSEUDONYMS: &[(&str, &str)] = &[
    ("Inochi", "Alderin"),
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
    ("Nisugi", "Ashryn"),
    ("Nerten", "Baelor"),
];

fn main() -> std::io::Result<()> {
    let mut scrubber = Scrubber::new();
    for (real, pseudonym) in PSEUDONYMS {
        scrubber.pseudonymise(real, pseudonym);
    }
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path)?;
        let scrubbed = scrubber.scrub(&text);
        if scrubbed == text {
            println!("unchanged: {path}");
        } else {
            std::fs::write(&path, &scrubbed)?;
            println!("scrubbed:  {path}");
        }
    }
    Ok(())
}
