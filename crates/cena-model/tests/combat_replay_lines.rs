//! The 71 replay fixtures, read at the LINE level.
//!
//! `tests/fixtures/combat/*.txt` are Lich's `spec/fixtures/replay/` verbatim:
//! *"a slim, curated slice of that corpus -- the single most-common real-feed
//! shape per attack def, plus extra flare-bearing shapes"* (`replay_spec.rb`),
//! each blob a real captured feed chunk with an `# expect:` header:
//!
//! ```text
//! ##### attack 5110a6c49c3f7711
//! # expect: attacks=attack | res=as_ds | flares=ensorcell | outcomes=none | dmg=1 | statuses=
//! You swing a perfect mithril war-hammer at <pushBold/>a <a exist="37507821" ...
//! ```
//!
//! Lich's harness drives each blob through `parse_events` -- the state
//! machine -- and asserts the events reproduce the header. That test comes
//! with the FSM. **This one is the floor beneath it**: three of the header's
//! facts are properties of single lines, and a classifier that misses one of
//! them would make the FSM's job impossible before it starts:
//!
//! - `res=`: every roll kind named is classified from some line
//! - `flares=`: every flare named is classified from some line
//! - `outcomes=`: every outcome named is classified from some line
//!
//! `attacks=` is also checked, with the FSM-assigned names allowed as extras
//! the header may name that no line carries (see `FSM_NAMED`). `dmg=` and
//! `statuses=` are the FSM's: hits are damage lines attributed to attacks and
//! flares, and statuses include crit-derived and self statuses.
//!
//! The contract is Lich's: the header is a **floor** -- every expected fact
//! must be present; extras are tolerated.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cena_model::state::chunks::ChunkLine;
use cena_model::state::combat::bracket::{assault_start, sequence_start};
use cena_model::{AmbushPrefix, AttackLine, FlareLine, GameState, Outcome, Resolution};
use cena_protocol::Parser;

/// Event names the FSM assigns that no single line carries.
///
/// - `unknown`: the orphan sink for facts with no recognised initiation
/// - `cast`: the bare gesture wrapper (`via: :cast`) -- a `cast` line does
///   classify, but the header may also name it through `via`
/// - `volley`: a round with no initiation line of its own is still a round
const FSM_NAMED: &[&str] = &["unknown", "volley"];

struct Blob {
    file: String,
    header: String,
    expect: BTreeSet<(String, String)>,
    lines: Vec<String>,
}

/// Parse `# expect: k=v,v | k=v ...` into `(key, value)` pairs, dropping
/// `none` and empties, as `replay_spec.rb`'s `parse_expect` does.
fn parse_expect(line: &str) -> BTreeSet<(String, String)> {
    line.trim_start_matches("# expect: ")
        .split(" | ")
        .filter_map(|part| part.split_once('='))
        .flat_map(|(k, v)| {
            v.split(',')
                .filter(|x| !x.is_empty() && *x != "none")
                .map(move |x| (k.to_owned(), x.to_owned()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn load_blobs() -> Vec<Blob> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/combat");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.sort();
    let mut blobs = Vec::new();
    for path in paths {
        let file = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut header = None::<String>;
        let mut expect = None::<BTreeSet<(String, String)>>;
        let mut lines: Vec<String> = Vec::new();
        let mut flush =
            |header: &mut Option<String>, expect: &mut Option<_>, lines: &mut Vec<String>| {
                match (header.take(), expect.take()) {
                    (Some(h), Some(e)) if !lines.is_empty() => blobs.push(Blob {
                        file: file.clone(),
                        header: h,
                        expect: e,
                        lines: std::mem::take(lines),
                    }),
                    _ => {}
                }
                lines.clear();
            };
        for raw in text.lines() {
            if let Some(h) = raw.strip_prefix("##### ") {
                flush(&mut header, &mut expect, &mut lines);
                header = Some(h.to_owned());
            } else if raw.starts_with("# expect:") {
                expect = Some(parse_expect(raw));
            } else if raw.trim().is_empty() {
                flush(&mut header, &mut expect, &mut lines);
            } else if header.is_some() {
                lines.push(raw.to_owned());
            }
        }
        flush(&mut header, &mut expect, &mut lines);
    }
    blobs
}

/// A blob's lines, through the real parser, as the chunk would hold them.
fn chunk_lines(blob: &Blob) -> Vec<ChunkLine> {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    let wire = blob.lines.join("\n") + "\n";
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state.open_chunk().lines().to_vec()
}

/// What the classifiers found across a blob, in the header's vocabulary.
fn found_in(lines: &[ChunkLine]) -> BTreeSet<(String, String)> {
    let mut found = BTreeSet::new();
    for line in lines {
        if let Some(a) = AttackLine::classify(line) {
            found.insert(("attacks".to_owned(), a.name));
        }
        if AmbushPrefix::classify(line).is_some_and(|p| p.hidden) {
            found.insert(("attacks".to_owned(), "ambush".to_owned()));
        }
        if let Some(b) = assault_start(line) {
            found.insert(("attacks".to_owned(), b.name.as_str().to_owned()));
        }
        if let Some(b) = sequence_start(line) {
            found.insert(("attacks".to_owned(), b.name.as_str().to_owned()));
        }
        if let Some(r) = Resolution::classify(line) {
            found.insert(("res".to_owned(), r.kind.as_str().to_owned()));
        }
        if let Some(f) = FlareLine::classify(line) {
            found.insert(("flares".to_owned(), f.name));
        }
        if let Some(o) = Outcome::classify(line) {
            found.insert(("outcomes".to_owned(), o.kind.as_str().to_owned()));
        }
    }
    found
}

#[test]
fn the_fixture_set_is_the_curated_slice() {
    let blobs = load_blobs();
    assert!(
        blobs.len() >= 50,
        "replay_spec.rb asserts >= 50 blobs; got {}",
        blobs.len()
    );
    let files: BTreeSet<&str> = blobs.iter().map(|b| b.file.as_str()).collect();
    assert_eq!(files.len(), 71, "71 fixture files");
}

/// **Every line-level fact in every header is classified from some line.**
///
/// The floor contract, at the line level. A miss here is a classifier gap --
/// a pattern that did not survive the crossing -- and is reported per blob
/// with the fact it missed.
#[test]
fn every_line_level_fact_in_every_header_is_found() {
    let blobs = load_blobs();
    let mut report = Vec::new();
    let mut checked = 0usize;
    for blob in &blobs {
        let lines = chunk_lines(blob);
        let found = found_in(&lines);
        let missing: Vec<String> = blob
            .expect
            .iter()
            .filter(|(k, _)| matches!(k.as_str(), "attacks" | "res" | "flares" | "outcomes"))
            .filter(|(k, v)| !(k == "attacks" && FSM_NAMED.contains(&v.as_str())))
            .filter(|pair| !found.contains(pair))
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        checked += blob.expect.len();
        if !missing.is_empty() {
            report.push(format!(
                "  {} / {}: missing {}",
                blob.file,
                blob.header,
                missing.join(", ")
            ));
        }
    }
    assert!(
        checked > 200,
        "guard: {checked} facts checked; the loader has stopped reading"
    );
    assert!(
        report.is_empty(),
        "{} of {} blobs miss a line-level fact:\n{}",
        report.len(),
        blobs.len(),
        report.join("\n")
    );
}
